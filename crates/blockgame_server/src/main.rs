use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use blockgame_core::{
    directory::{
        send_heartbeat, send_unregister, HeartbeatRequest, UnregisterRequest,
        DEFAULT_DIRECTORY_URL, DIRECTORY_HEARTBEAT_INTERVAL_SECS,
    },
    net::{MultiplayerServer, DEFAULT_SERVER_PORT},
    save::{parse_or_hash_seed, sanitize_folder_name},
};
use blockgame_gen::TerrainGenerator;

fn print_help() {
    println!(
        r#"BlockGame Dedicated Multiplayer Server

USAGE:
    blockgame_server [OPTIONS]

OPTIONS:
    -p, --port <PORT>            Server port to listen on (default: 25565)
    -w, --world <NAME>           World name and save folder (default: 'Multiplayer World')
    -s, --seed <SEED>            World generation seed (default: 1337)
    -b, --bind <HOST>            Bind address host (default: 0.0.0.0)
        --discoverable           Register this server on the public directory list
        --server-name <NAME>     Public server name (default: world name)
        --motd <TEXT>            Public server MOTD, supports & color codes
        --directory-url <URL>    Central directory URL (default: {})
        --public-host <HOST>     Public IP/domain override (default: auto)
        --max-players <N>        Reported max player slots (default: 20)
    -h, --help                   Print help information

SERVER CONSOLE COMMANDS:
    kick <player> [reason]              Kick a player from the server
    ban <player> [reason]               Ban a player from joining
    unban <player>                      Unban a player
    tp <player> <target>                Teleport a player to another player
    tp <player> <x> <y> <z>             Teleport a player to coordinates
    give <player> <item> [amount]       Give items to a player
    perm <user|group|list> ...          Manage permissions and groups
    list                                List online players
    say <message>                       Broadcast message to chat
    help                                Show command help
    stop                                Save and cleanly stop the server
"#,
        DEFAULT_DIRECTORY_URL
    );
}

fn arg_value(args: &[String], long: &str, short: Option<&str>) -> Option<String> {
    args.iter()
        .position(|a| a == long || short.is_some_and(|s| a == s))
        .and_then(|idx| args.get(idx + 1).cloned())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    let port: u16 = arg_value(&args, "--port", Some("-p"))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_SERVER_PORT);

    let world_name = arg_value(&args, "--world", Some("-w"))
        .unwrap_or_else(|| "Multiplayer World".to_string());

    let seed: u32 = arg_value(&args, "--seed", Some("-s"))
        .map(|s| parse_or_hash_seed(&s))
        .unwrap_or(1337);

    let bind_host = arg_value(&args, "--bind", Some("-b"))
        .unwrap_or_else(|| "0.0.0.0".to_string());

    let discoverable = args.iter().any(|a| a == "--discoverable");
    let server_name = arg_value(&args, "--server-name", None).unwrap_or_else(|| world_name.clone());
    let motd = arg_value(&args, "--motd", None)
        .unwrap_or_else(|| "&aA BlockGame Dedicated Server".to_string());
    let directory_url = arg_value(&args, "--directory-url", None)
        .unwrap_or_else(|| DEFAULT_DIRECTORY_URL.to_string());
    let public_host = arg_value(&args, "--public-host", None)
        .unwrap_or_else(|| "auto".to_string());
    let max_players: usize = arg_value(&args, "--max-players", None)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20)
        .max(1);

    let bind_addr = format!("{}:{}", bind_host, port);
    let saves_root = PathBuf::from("saves");
    let folder_name = sanitize_folder_name(&world_name);

    let generator = Arc::new(TerrainGenerator::new(seed));
    let gen_clone = generator.clone();

    log::info!(
        "Starting BlockGame Dedicated Multiplayer Server on {} (World: '{}', Seed: {})...",
        bind_addr,
        world_name,
        seed
    );

    let mut server = MultiplayerServer::start(
        &bind_addr,
        saves_root,
        folder_name,
        world_name,
        seed,
        None,
        move |cpos| {
            let mut chunk = gen_clone.generate_chunk(cpos);
            gen_clone.populate_chunk_features(&mut chunk);
            chunk
        },
    )?;

    let actual_port = server.local_addr().port();
    log::info!(
        "Dedicated server active on {}. Type 'help' for console commands, or 'stop' to shut down.",
        server.local_addr()
    );

    let running = Arc::new(AtomicBool::new(true));
    let player_count_shared = Arc::new(AtomicUsize::new(server.online_player_count()));

    let discovery_session = if discoverable {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let srv_id = format!("srv-{}-{}-{}", pid, actual_port, nanos & 0xFFFF_FFFF);
        let token = format!("tok-{:x}-{:x}", nanos, pid);

        log::info!(
            "Discoverable mode enabled: registering '{}' with directory {}",
            server_name,
            directory_url
        );

        let hb_running = running.clone();
        let hb_players = player_count_shared.clone();
        let hb_dir_url = directory_url.clone();
        let hb_id = srv_id.clone();
        let hb_token = token.clone();
        let hb_name = server_name.clone();
        let hb_motd = motd.clone();
        let hb_host = public_host.clone();

        thread::Builder::new()
            .name("directory-heartbeat".to_string())
            .spawn(move || {
                let mut registered_once = false;
                let mut last_sent = Instant::now() - Duration::from_secs(DIRECTORY_HEARTBEAT_INTERVAL_SECS);
                while hb_running.load(Ordering::Relaxed) {
                    if last_sent.elapsed() >= Duration::from_secs(DIRECTORY_HEARTBEAT_INTERVAL_SECS) {
                        let req = HeartbeatRequest {
                            id: hb_id.clone(),
                            token: hb_token.clone(),
                            name: hb_name.clone(),
                            motd: hb_motd.clone(),
                            host: hb_host.clone(),
                            port: actual_port,
                            players: hb_players.load(Ordering::Relaxed),
                            max_players,
                            version: env!("CARGO_PKG_VERSION").to_string(),
                            voice_enabled: true,
                        };
                        match send_heartbeat(&hb_dir_url, &req) {
                            Ok(()) => {
                                if !registered_once {
                                    log::info!("Successfully registered with central directory.");
                                    registered_once = true;
                                }
                            }
                            Err(e) => {
                                log::warn!("Directory heartbeat failed: {}", e);
                            }
                        }
                        last_sent = Instant::now();
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            })?;

        Some((directory_url, srv_id, token))
    } else {
        None
    };

    let running_stdin = running.clone();
    let (input_tx, input_rx) = std::sync::mpsc::channel::<String>();
    thread::Builder::new()
        .name("server-console-stdin".to_string())
        .spawn(move || {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let mut line = String::new();
            while running_stdin.load(Ordering::Relaxed) {
                line.clear();
                match handle.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let _ = input_tx.send(trimmed.to_string());
                        }
                    }
                    Err(_) => break,
                }
            }
        })?;

    while running.load(Ordering::Relaxed) {
        player_count_shared.store(server.online_player_count(), Ordering::Relaxed);
        while let Ok(cmd) = input_rx.try_recv() {
            server.execute_command(&cmd);
            let clean = cmd.trim().strip_prefix('/').unwrap_or(cmd.trim());
            if clean.eq_ignore_ascii_case("stop") {
                running.store(false, Ordering::SeqCst);
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    if let Some((dir_url, id, token)) = discovery_session {
        log::info!("Unregistering from central directory...");
        if let Err(e) = send_unregister(&dir_url, &UnregisterRequest { id, token }) {
            log::warn!("Failed to unregister from directory: {}", e);
        }
    }

    log::info!("Shutting down dedicated server...");
    server.shutdown();
    log::info!("Server shut down cleanly. Goodbye!");

    Ok(())
}
