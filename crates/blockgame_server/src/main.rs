mod config;

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
    net::MultiplayerServer,
    save::{parse_or_hash_seed, sanitize_folder_name},
};
use blockgame_gen::TerrainGenerator;

use config::{ServerConfig, DEFAULT_CONFIG_FILE};

fn print_help() {
    println!(
        r#"BlockGame Dedicated Multiplayer Server

USAGE:
    blockgame_server [OPTIONS]

OPTIONS:
    -c, --config <FILE>          Path to server.properties config file (default: server.properties)
    -p, --port <PORT>            Server port to listen on (default: 25565)
    -w, --world <NAME>           World name and save folder (default: 'Multiplayer World')
    -s, --seed <SEED>            World generation seed (default: 1337)
    -b, --bind <HOST>            Bind address host (default: 0.0.0.0)
        --discoverable           Register this server on the public directory list
        --no-discoverable        Disable registration (overriding config setting)
        --server-name <NAME>     Public server name (default: world name)
        --motd <TEXT>            Public server MOTD, supports & color codes
        --directory-url <URL>    Central directory URL (default: {})
        --public-host <HOST>     Public IP/domain override (default: auto)
        --max-players <N>        Reported max player slots (default: 20)
    -h, --help                   Print help information

CONFIGURATION FILE:
    By default, the server reads and automatically creates 'server.properties'.
    Any command-line flags provided will override values set in the config file.

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

    // 1. Determine config file path and load/create it
    let config_path = arg_value(&args, "--config", Some("-c"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_FILE));

    let mut config = ServerConfig::load_or_create(&config_path)?;

    // 2. Command-line argument overrides
    if let Some(p) = arg_value(&args, "--port", Some("-p")).and_then(|s| s.parse::<u16>().ok()) {
        config.port = p;
    }
    if let Some(w) = arg_value(&args, "--world", Some("-w")) {
        config.world_name = w;
    }
    if let Some(s) = arg_value(&args, "--seed", Some("-s")) {
        config.seed = parse_or_hash_seed(&s);
    }
    if let Some(b) = arg_value(&args, "--bind", Some("-b")) {
        config.bind_host = b;
    }
    if args.iter().any(|a| a == "--discoverable") {
        config.discoverable = true;
    } else if args.iter().any(|a| a == "--no-discoverable") {
        config.discoverable = false;
    }
    if let Some(n) = arg_value(&args, "--server-name", None) {
        config.server_name = n;
    }
    if let Some(m) = arg_value(&args, "--motd", None) {
        config.motd = m;
    }
    if let Some(u) = arg_value(&args, "--directory-url", None) {
        config.directory_url = u;
    }
    if let Some(h) = arg_value(&args, "--public-host", None) {
        config.public_host = h;
    }
    if let Some(mp) = arg_value(&args, "--max-players", None).and_then(|s| s.parse::<usize>().ok()) {
        config.max_players = mp.max(1);
    }

    let bind_addr = format!("{}:{}", config.bind_host, config.port);
    let server_root = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_dir = server_root.join("config");
    let saves_root = PathBuf::from("saves");
    let folder_name = sanitize_folder_name(&config.world_name);
    let effective_seed = blockgame_core::save::load_world_metadata(&saves_root.join(&folder_name))
        .map(|m| m.seed)
        .unwrap_or(config.seed);

    let generator = Arc::new(TerrainGenerator::new(effective_seed));
    let gen_clone = generator.clone();

    log::info!(
        "Starting BlockGame Dedicated Multiplayer Server on {} (World: '{}', Seed: {})...",
        bind_addr,
        config.world_name,
        effective_seed
    );

    let mut server = MultiplayerServer::start_with_config_dir(
        &bind_addr,
        saves_root,
        config_dir,
        folder_name,
        config.world_name.clone(),
        effective_seed,
        None,
        move |cpos| {
            let mut chunk = gen_clone.generate_chunk(cpos);
            gen_clone.populate_chunk_features(&mut chunk);
            chunk
        },
    )?;
    server.set_broadcast_achievements(config.broadcast_achievements);

    let actual_port = server.local_addr().port();
    log::info!(
        "Dedicated server active on {}. Type 'help' for console commands, or 'stop' to shut down.",
        server.local_addr()
    );

    let running = Arc::new(AtomicBool::new(true));
    let player_count_shared = Arc::new(AtomicUsize::new(server.online_player_count()));

    let discovery_session = if config.discoverable {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let srv_id = format!("srv-{}-{}-{}", pid, actual_port, nanos & 0xFFFF_FFFF);
        let token = format!("tok-{:x}-{:x}", nanos, pid);

        log::info!(
            "Discoverable mode enabled: registering '{}' with directory {}",
            config.server_name,
            config.directory_url
        );

        let hb_running = running.clone();
        let hb_players = player_count_shared.clone();
        let hb_dir_url = config.directory_url.clone();
        let hb_id = srv_id.clone();
        let hb_token = token.clone();
        let hb_name = config.server_name.clone();
        let hb_motd = config.motd.clone();
        let hb_host = config.public_host.clone();
        let hb_max_players = config.max_players;

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
                            max_players: hb_max_players,
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

        Some((config.directory_url, srv_id, token))
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
