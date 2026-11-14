//! BlockGame Central Server Directory
//!
//! Lightweight HTTP/1.1 service that tracks opt-in discoverable BlockGame servers.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use blockgame_core::directory::{
    encode_servers_response, json_escape, DirectoryServerEntry, HeartbeatRequest,
    UnregisterRequest, DEFAULT_DIRECTORY_PORT, DIRECTORY_STALE_TIMEOUT_SECS,
};

#[derive(Debug, Clone)]
struct RegisteredServer {
    entry: DirectoryServerEntry,
    token: String,
    last_heartbeat: Instant,
}

#[derive(Debug, Default)]
struct DirectoryState {
    servers: HashMap<String, RegisteredServer>,
}

impl DirectoryState {
    fn prune_stale(&mut self, now: Instant) -> usize {
        let timeout = Duration::from_secs(DIRECTORY_STALE_TIMEOUT_SECS);
        let before = self.servers.len();
        self.servers
            .retain(|_, srv| now.duration_since(srv.last_heartbeat) <= timeout);
        before - self.servers.len()
    }

    fn upsert_heartbeat(
        &mut self,
        req: HeartbeatRequest,
        peer_ip: IpAddr,
        now: Instant,
    ) -> Result<(), (u16, String)> {
        if let Some(existing) = self.servers.get(&req.id) {
            if existing.token != req.token {
                return Err((403, "Invalid session token for server id".to_string()));
            }
        }

        let resolved_host = if req.host.trim().is_empty() || req.host.eq_ignore_ascii_case("auto") {
            peer_ip.to_string()
        } else {
            req.host.trim().to_string()
        };

        let entry = DirectoryServerEntry {
            id: req.id.clone(),
            name: if req.name.trim().is_empty() {
                "BlockGame Server".to_string()
            } else {
                req.name.chars().take(64).collect()
            },
            motd: req.motd.chars().take(128).collect(),
            host: resolved_host,
            port: if req.port == 0 { 25565 } else { req.port },
            players: req.players,
            max_players: req.max_players.max(1),
            version: req.version,
            voice_enabled: req.voice_enabled,
        };

        self.servers.insert(
            req.id,
            RegisteredServer {
                entry,
                token: req.token,
                last_heartbeat: now,
            },
        );
        Ok(())
    }

    fn unregister(&mut self, req: UnregisterRequest) -> Result<bool, (u16, String)> {
        if let Some(existing) = self.servers.get(&req.id) {
            if existing.token != req.token {
                return Err((403, "Invalid session token for server id".to_string()));
            }
            self.servers.remove(&req.id);
            return Ok(true);
        }
        Ok(false)
    }

    fn active_servers(&mut self, now: Instant) -> Vec<DirectoryServerEntry> {
        self.prune_stale(now);
        let mut list: Vec<DirectoryServerEntry> =
            self.servers.values().map(|s| s.entry.clone()).collect();
        list.sort_by(|a, b| b.players.cmp(&a.players).then_with(|| a.name.cmp(&b.name)));
        list
    }
}

fn print_help() {
    println!(
        r#"BlockGame Central Directory Server

USAGE:
    blockgame_directory [OPTIONS]

OPTIONS:
    -p, --port <PORT>    HTTP port to listen on (default: {})
    -b, --bind <HOST>    Bind address host (default: 0.0.0.0)
    -h, --help           Print help information
"#,
        DEFAULT_DIRECTORY_PORT
    );
}

fn send_http_response(stream: &mut TcpStream, status: u16, status_text: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

fn read_http_request(stream: &mut TcpStream) -> Result<(String, String, HashMap<String, String>, String), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut buf = Vec::with_capacity(2048);
    let mut temp = [0u8; 1024];
    let header_end;
    loop {
        let n = stream.read(&mut temp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("EOF before HTTP headers".to_string());
        }
        buf.extend_from_slice(&temp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if buf.len() > 32 * 1024 {
            return Err("HTTP headers too large".to_string());
        }
    }

    let head_str = String::from_utf8_lossy(&buf[..header_end - 4]);
    let mut lines = head_str.lines();
    let request_line = lines.next().ok_or_else(|| "Empty request".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let raw_target = parts.next().unwrap_or("/");
    let path = raw_target.split('?').next().unwrap_or("/").to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    let content_len = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(64 * 1024);

    let mut body_bytes = buf[header_end..].to_vec();
    while body_bytes.len() < content_len {
        let n = stream.read(&mut temp).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&temp[..n]);
    }
    body_bytes.truncate(content_len);
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    Ok((method, path, headers, body))
}

fn handle_client(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    state: Arc<Mutex<DirectoryState>>,
) {
    let (method, path, headers, body) = match read_http_request(&mut stream) {
        Ok(req) => req,
        Err(e) => {
            log::debug!("Bad request from {}: {}", peer_addr, e);
            return;
        }
    };

    let client_ip: IpAddr = headers
        .get("x-forwarded-for")
        .and_then(|v| v.split(',').next())
        .and_then(|s| s.trim().parse::<IpAddr>().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
        })
        .unwrap_or_else(|| peer_addr.ip());

    match (method.as_str(), path.as_str()) {
        ("GET", "/api/servers") => {
            let list = state.lock().unwrap().active_servers(Instant::now());
            let json = encode_servers_response(&list);
            send_http_response(&mut stream, 200, "OK", &json);
        }
        ("POST", "/api/servers/heartbeat") => match HeartbeatRequest::from_json(&body) {
            Ok(req) => {
                let mut guard = state.lock().unwrap();
                let is_new = !guard.servers.contains_key(&req.id);
                let name = req.name.clone();
                let port = req.port;
                match guard.upsert_heartbeat(req, client_ip, Instant::now()) {
                    Ok(()) => {
                        if is_new {
                            log::info!(
                                "Registered server '{}' ({}:{})",
                                name,
                                client_ip,
                                port
                            );
                        }
                        send_http_response(&mut stream, 200, "OK", "{\"ok\":true}");
                    }
                    Err((code, msg)) => {
                        let err_body = format!("{{\"error\":{}}}", json_escape(&msg));
                        send_http_response(&mut stream, code, "Forbidden", &err_body);
                    }
                }
            }
            Err(e) => {
                let err_body = format!("{{\"error\":{}}}", json_escape(&e));
                send_http_response(&mut stream, 400, "Bad Request", &err_body);
            }
        },
        ("POST", "/api/servers/unregister") => match UnregisterRequest::from_json(&body) {
            Ok(req) => {
                let id = req.id.clone();
                match state.lock().unwrap().unregister(req) {
                    Ok(removed) => {
                        if removed {
                            log::info!("Unregistered server '{}'", id);
                        }
                        send_http_response(&mut stream, 200, "OK", "{\"ok\":true}");
                    }
                    Err((code, msg)) => {
                        let err_body = format!("{{\"error\":{}}}", json_escape(&msg));
                        send_http_response(&mut stream, code, "Forbidden", &err_body);
                    }
                }
            }
            Err(e) => {
                let err_body = format!("{{\"error\":{}}}", json_escape(&e));
                send_http_response(&mut stream, 400, "Bad Request", &err_body);
            }
        },
        ("GET", "/" | "/health") => {
            let count = state.lock().unwrap().active_servers(Instant::now()).len();
            let body = format!("{{\"status\":\"ok\",\"servers\":{}}}", count);
            send_http_response(&mut stream, 200, "OK", &body);
        }
        _ => {
            send_http_response(&mut stream, 404, "Not Found", "{\"error\":\"Not found\"}");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    let port: u16 = if let Some(idx) = args.iter().position(|a| a == "--port" || a == "-p") {
        args.get(idx + 1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(DEFAULT_DIRECTORY_PORT)
    } else {
        DEFAULT_DIRECTORY_PORT
    };

    let bind_host = if let Some(idx) = args.iter().position(|a| a == "--bind" || a == "-b") {
        args.get(idx + 1)
            .cloned()
            .unwrap_or_else(|| "0.0.0.0".to_string())
    } else {
        "0.0.0.0".to_string()
    };

    let bind_addr = format!("{}:{}", bind_host, port);
    let listener = TcpListener::bind(&bind_addr)?;
    log::info!("BlockGame Central Directory listening on http://{}", bind_addr);

    let state = Arc::new(Mutex::new(DirectoryState::default()));
    let reaper_state = state.clone();
    thread::Builder::new()
        .name("directory-reaper".to_string())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            let pruned = reaper_state.lock().unwrap().prune_stale(Instant::now());
            if pruned > 0 {
                log::info!("Pruned {} stale server(s)", pruned);
            }
        })?;

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let peer = stream
                    .peer_addr()
                    .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
                let st = state.clone();
                thread::spawn(move || handle_client(stream, peer, st));
            }
            Err(e) => {
                log::warn!("Accept error: {}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn heartbeat_auto_host_token_check_and_pruning() {
        let mut st = DirectoryState::default();
        let t0 = Instant::now();
        let ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42));

        let hb = HeartbeatRequest {
            id: "srv-a".to_string(),
            token: "secret-1".to_string(),
            name: "Alpha".to_string(),
            motd: "&bWelcome".to_string(),
            host: "auto".to_string(),
            port: 25565,
            players: 3,
            max_players: 20,
            version: "0.1.0".to_string(),
            voice_enabled: true,
        };
        st.upsert_heartbeat(hb.clone(), ip, t0).unwrap();

        let list = st.active_servers(t0);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].host, "203.0.113.42");

        // Wrong token rejected
        let mut hijack = hb.clone();
        hijack.token = "wrong".to_string();
        assert!(st.upsert_heartbeat(hijack, ip, t0).is_err());

        // Pruned after timeout
        let t_expired = t0 + Duration::from_secs(DIRECTORY_STALE_TIMEOUT_SECS + 1);
        assert!(st.active_servers(t_expired).is_empty());
    }
}
