//! Shared protocol types, JSON codecs, and HTTP/1.1 client helpers for the
//! BlockGame Central Server Directory (`blockgame_directory`).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const DEFAULT_DIRECTORY_URL: &str = "http://15.204.81.97:25569";
pub const DEFAULT_DIRECTORY_PORT: u16 = 25569;
pub const DIRECTORY_HEARTBEAT_INTERVAL_SECS: u64 = 15;
pub const DIRECTORY_STALE_TIMEOUT_SECS: u64 = 45;

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

/// Public server listing entry returned by `GET /api/servers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryServerEntry {
    pub id: String,
    pub name: String,
    pub motd: String,
    pub host: String,
    pub port: u16,
    pub players: usize,
    pub max_players: usize,
    pub version: String,
    pub voice_enabled: bool,
}

/// Heartbeat payload sent by a discoverable game server to `POST /api/servers/heartbeat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatRequest {
    pub id: String,
    pub token: String,
    pub name: String,
    pub motd: String,
    pub host: String,
    pub port: u16,
    pub players: usize,
    pub max_players: usize,
    pub version: String,
    pub voice_enabled: bool,
}

/// Unregister payload sent on clean server shutdown to `POST /api/servers/unregister`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnregisterRequest {
    pub id: String,
    pub token: String,
}

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl DirectoryServerEntry {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"name\":{},\"motd\":{},\"host\":{},\"port\":{},\"players\":{},\"max_players\":{},\"version\":{},\"voice_enabled\":{}}}",
            json_escape(&self.id),
            json_escape(&self.name),
            json_escape(&self.motd),
            json_escape(&self.host),
            self.port,
            self.players,
            self.max_players,
            json_escape(&self.version),
            self.voice_enabled
        )
    }

    pub fn from_json_object(obj: &JsonValue) -> Result<Self, String> {
        Ok(Self {
            id: obj.get_str("id").unwrap_or_default().to_string(),
            name: obj
                .get_str("name")
                .unwrap_or("BlockGame Server")
                .to_string(),
            motd: obj.get_str("motd").unwrap_or_default().to_string(),
            host: obj
                .get_str("host")
                .ok_or_else(|| "Missing 'host'".to_string())?
                .to_string(),
            port: obj.get_u64("port").unwrap_or(25565) as u16,
            players: obj.get_u64("players").unwrap_or(0) as usize,
            max_players: obj.get_u64("max_players").unwrap_or(20) as usize,
            version: obj.get_str("version").unwrap_or("0.1.0").to_string(),
            voice_enabled: obj.get_bool("voice_enabled").unwrap_or(true),
        })
    }
}

impl HeartbeatRequest {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"token\":{},\"name\":{},\"motd\":{},\"host\":{},\"port\":{},\"players\":{},\"max_players\":{},\"version\":{},\"voice_enabled\":{}}}",
            json_escape(&self.id),
            json_escape(&self.token),
            json_escape(&self.name),
            json_escape(&self.motd),
            json_escape(&self.host),
            self.port,
            self.players,
            self.max_players,
            json_escape(&self.version),
            self.voice_enabled
        )
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let val = parse_json(json)?;
        let id = val
            .get_str("id")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing 'id'".to_string())?
            .to_string();
        let token = val
            .get_str("token")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing 'token'".to_string())?
            .to_string();
        Ok(Self {
            id,
            token,
            name: val
                .get_str("name")
                .unwrap_or("BlockGame Server")
                .to_string(),
            motd: val.get_str("motd").unwrap_or_default().to_string(),
            host: val.get_str("host").unwrap_or("auto").to_string(),
            port: val.get_u64("port").unwrap_or(25565) as u16,
            players: val.get_u64("players").unwrap_or(0) as usize,
            max_players: val.get_u64("max_players").unwrap_or(20) as usize,
            version: val.get_str("version").unwrap_or("0.1.0").to_string(),
            voice_enabled: val.get_bool("voice_enabled").unwrap_or(true),
        })
    }
}

impl UnregisterRequest {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"token\":{}}}",
            json_escape(&self.id),
            json_escape(&self.token)
        )
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let val = parse_json(json)?;
        let id = val
            .get_str("id")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing 'id'".to_string())?
            .to_string();
        let token = val
            .get_str("token")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Missing 'token'".to_string())?
            .to_string();
        Ok(Self { id, token })
    }
}

pub fn encode_servers_response(servers: &[DirectoryServerEntry]) -> String {
    let mut out = String::from("{\"servers\":[");
    for (i, s) in servers.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&s.to_json());
    }
    out.push_str("]}");
    out
}

pub fn decode_servers_response(json: &str) -> Result<Vec<DirectoryServerEntry>, String> {
    let val = parse_json(json)?;
    let arr = match val.get("servers") {
        Some(JsonValue::Array(items)) => items,
        _ => return Err("Expected 'servers' array in response".to_string()),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(DirectoryServerEntry::from_json_object(item)?);
    }
    Ok(out)
}

/// Parses `http://host[:port][/base]` into `(host, port, base_path)`.
pub fn parse_http_url(url: &str) -> Result<(String, u16, String), String> {
    let trimmed = url.trim().trim_end_matches('/');
    let rest = if let Some(r) = trimmed.strip_prefix("http://") {
        r
    } else if trimmed.starts_with("https://") {
        return Err("HTTPS is not supported directly by built-in HTTP client; use http:// or an HTTP proxy".to_string());
    } else {
        trimmed
    };

    let (authority, base_path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], rest[idx..].to_string()),
        None => (rest, String::new()),
    };

    if authority.is_empty() {
        return Err("Empty host in directory URL".to_string());
    }

    let (host, port) = if let Some(bracket_end) = authority.strip_prefix('[') {
        // IPv6 `[::1]:25569`
        let end = bracket_end
            .find(']')
            .ok_or_else(|| "Invalid IPv6 host in URL".to_string())?;
        let h = &bracket_end[..end];
        let after = &bracket_end[end + 1..];
        let p = if let Some(port_str) = after.strip_prefix(':') {
            port_str
                .parse::<u16>()
                .map_err(|_| "Invalid port in URL".to_string())?
        } else {
            80
        };
        (h.to_string(), p)
    } else if let Some((h, port_str)) = authority.rsplit_once(':') {
        let p = port_str
            .parse::<u16>()
            .map_err(|_| "Invalid port in URL".to_string())?;
        (h.to_string(), p)
    } else {
        (authority.to_string(), 80)
    };

    Ok((host, port, base_path))
}

fn http_request(
    directory_url: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let (host, port, base_path) = parse_http_url(directory_url)?;
    let full_path = format!("{}{}", base_path, path);

    let addrs: Vec<SocketAddr> = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS/address error for {}:{}: {}", host, port, e))?
        .collect();
    let addr = addrs
        .first()
        .copied()
        .ok_or_else(|| format!("Could not resolve {}:{}", host, port))?;

    let mut stream = TcpStream::connect_timeout(&addr, HTTP_TIMEOUT)
        .map_err(|e| format!("Connection to {}:{} failed: {}", host, port, e))?;
    let _ = stream.set_read_timeout(Some(HTTP_TIMEOUT));
    let _ = stream.set_write_timeout(Some(HTTP_TIMEOUT));

    let payload = body.unwrap_or("");
    let req = if body.is_some() {
        format!(
            "{} {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            method,
            full_path,
            host,
            port,
            payload.len(),
            payload
        )
    } else {
        format!(
            "{} {} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
            method, full_path, host, port
        )
    };

    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("Failed writing HTTP request: {}", e))?;
    let _ = stream.flush();

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("Failed reading HTTP response: {}", e))?;
    let text = String::from_utf8_lossy(&raw);

    let (head, resp_body) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .ok_or_else(|| "Malformed HTTP response".to_string())?;

    let status_line = head.lines().next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| format!("Invalid HTTP status line: {}", status_line))?;

    Ok((status_code, resp_body.to_string()))
}

pub fn send_heartbeat(directory_url: &str, req: &HeartbeatRequest) -> Result<(), String> {
    let body = req.to_json();
    let (status, resp) = http_request(directory_url, "POST", "/api/servers/heartbeat", Some(&body))?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("Directory returned HTTP {}: {}", status, resp.trim()))
    }
}

pub fn send_unregister(directory_url: &str, req: &UnregisterRequest) -> Result<(), String> {
    let body = req.to_json();
    let (status, resp) =
        http_request(directory_url, "POST", "/api/servers/unregister", Some(&body))?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("Directory returned HTTP {}: {}", status, resp.trim()))
    }
}

pub fn fetch_servers(directory_url: &str) -> Result<Vec<DirectoryServerEntry>, String> {
    let (status, resp) = http_request(directory_url, "GET", "/api/servers", None)?;
    if !(200..300).contains(&status) {
        return Err(format!("Directory returned HTTP {}: {}", status, resp.trim()));
    }
    decode_servers_response(&resp)
}

// -----------------------------------------------------------------------------
// Minimal, strict pure-Rust JSON parser (zero external crate dependencies)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(JsonValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        match self.get(key) {
            Some(JsonValue::Number(n)) if *n >= 0.0 => Some(*n as u64),
            _ => None,
        }
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(JsonValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }
}

pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let val = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err("Trailing characters after JSON value".to_string());
    }
    Ok(val)
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let b = self.bytes.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b't') => {
                if self.bytes[self.pos..].starts_with(b"true") {
                    self.pos += 4;
                    Ok(JsonValue::Bool(true))
                } else {
                    Err("Invalid literal".to_string())
                }
            }
            Some(b'f') => {
                if self.bytes[self.pos..].starts_with(b"false") {
                    self.pos += 5;
                    Ok(JsonValue::Bool(false))
                } else {
                    Err("Invalid literal".to_string())
                }
            }
            Some(b'n') => {
                if self.bytes[self.pos..].starts_with(b"null") {
                    self.pos += 4;
                    Ok(JsonValue::Null)
                } else {
                    Err("Invalid literal".to_string())
                }
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(c) => Err(format!("Unexpected character '{}' at {}", c as char, self.pos)),
            None => Err("Unexpected end of JSON".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.next(); // '{'
        self.skip_ws();
        let mut entries = Vec::new();
        if self.peek() == Some(b'}') {
            self.next();
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err("Expected string key in JSON object".to_string());
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.next() != Some(b':') {
                return Err("Expected ':' in JSON object".to_string());
            }
            let val = self.parse_value()?;
            entries.push((key, val));
            self.skip_ws();
            match self.next() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err("Expected ',' or '}' in JSON object".to_string()),
            }
        }
        Ok(JsonValue::Object(entries))
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.next(); // '['
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.next();
            return Ok(JsonValue::Array(items));
        }
        loop {
            let val = self.parse_value()?;
            items.push(val);
            self.skip_ws();
            match self.next() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err("Expected ',' or ']' in JSON array".to_string()),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        if self.next() != Some(b'"') {
            return Err("Expected '\"'".to_string());
        }
        let mut out = String::new();
        while let Some(b) = self.next() {
            match b {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = self.next().ok_or_else(|| "Unterminated escape".to_string())?;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\x08'),
                        b'f' => out.push('\x0c'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .ok_or_else(|| "Invalid \\u escape".to_string())?;
                            let hex_str = std::str::from_utf8(hex)
                                .map_err(|_| "Invalid UTF-8 in \\u".to_string())?;
                            let code = u32::from_str_radix(hex_str, 16)
                                .map_err(|_| "Invalid hex in \\u".to_string())?;
                            self.pos += 4;
                            if let Some(ch) = char::from_u32(code) {
                                out.push(ch);
                            }
                        }
                        _ => return Err("Invalid escape sequence".to_string()),
                    }
                }
                _ => {
                    // Preserve multi-byte UTF-8 sequences cleanly
                    if b < 0x80 {
                        out.push(b as char);
                    } else {
                        let start = self.pos - 1;
                        let rest = std::str::from_utf8(&self.bytes[start..])
                            .or_else(|e| {
                                let valid = e.valid_up_to();
                                if valid == 0 {
                                    Err("Invalid UTF-8 in JSON string".to_string())
                                } else {
                                    Ok(std::str::from_utf8(&self.bytes[start..start + valid]).unwrap())
                                }
                            })?;
                        let ch = rest.chars().next().ok_or_else(|| "Invalid UTF-8".to_string())?;
                        out.push(ch);
                        self.pos = start + ch.len_utf8();
                    }
                }
            }
        }
        Err("Unterminated JSON string".to_string())
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "Invalid number".to_string())?;
        let n = s
            .parse::<f64>()
            .map_err(|_| format!("Invalid JSON number: {}", s))?;
        Ok(JsonValue::Number(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_and_servers_json_roundtrip() {
        let hb = HeartbeatRequest {
            id: "srv-1".to_string(),
            token: "tok-abc".to_string(),
            name: "BlockGame \"Official\" &bTest".to_string(),
            motd: "&aWelcome to &bBlockGame!\nHave fun!".to_string(),
            host: "15.204.81.97".to_string(),
            port: 25565,
            players: 4,
            max_players: 20,
            version: "0.1.0".to_string(),
            voice_enabled: true,
        };
        let encoded_hb = hb.to_json();
        let decoded_hb = HeartbeatRequest::from_json(&encoded_hb).expect("decode hb");
        assert_eq!(decoded_hb, hb);

        let entry = DirectoryServerEntry {
            id: hb.id.clone(),
            name: hb.name.clone(),
            motd: hb.motd.clone(),
            host: hb.host.clone(),
            port: hb.port,
            players: hb.players,
            max_players: hb.max_players,
            version: hb.version.clone(),
            voice_enabled: hb.voice_enabled,
        };
        let resp_json = encode_servers_response(&[entry.clone()]);
        let decoded_list = decode_servers_response(&resp_json).expect("decode list");
        assert_eq!(decoded_list, vec![entry]);
    }

    #[test]
    fn parse_http_url_handles_ip_port_and_path() {
        let (h, p, base) = parse_http_url("http://15.204.81.97:25569").unwrap();
        assert_eq!(h, "15.204.81.97");
        assert_eq!(p, 25569);
        assert_eq!(base, "");

        let (h2, p2, base2) = parse_http_url("http://example.com/dir/").unwrap();
        assert_eq!(h2, "example.com");
        assert_eq!(p2, 80);
        assert_eq!(base2, "/dir");
    }
}
