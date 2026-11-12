use std::io::{self, BufRead};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use blockgame_core::{
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
    -p, --port <PORT>        Server port to listen on (default: 25565)
    -w, --world <NAME>       World name and save folder (default: 'Multiplayer World')
    -s, --seed <SEED>        World generation seed (default: 1337)
    -b, --bind <HOST>        Bind address host (default: 0.0.0.0)
    -h, --help               Print help information

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
"#
    );
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
            .unwrap_or(DEFAULT_SERVER_PORT)
    } else {
        DEFAULT_SERVER_PORT
    };

    let world_name = if let Some(idx) = args.iter().position(|a| a == "--world" || a == "-w") {
        args.get(idx + 1)
            .cloned()
            .unwrap_or_else(|| "Multiplayer World".to_string())
    } else {
        "Multiplayer World".to_string()
    };

    let seed: u32 = if let Some(idx) = args.iter().position(|a| a == "--seed" || a == "-s") {
        args.get(idx + 1)
            .map(|s| parse_or_hash_seed(s))
            .unwrap_or(1337)
    } else {
        1337
    };

    let bind_host = if let Some(idx) = args.iter().position(|a| a == "--bind" || a == "-b") {
        args.get(idx + 1)
            .cloned()
            .unwrap_or_else(|| "0.0.0.0".to_string())
    } else {
        "0.0.0.0".to_string()
    };

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

    log::info!(
        "Dedicated server active on {}. Type 'help' for console commands, or 'stop' to shut down.",
        server.local_addr()
    );

    let running = Arc::new(AtomicBool::new(true));
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

    log::info!("Shutting down dedicated server...");
    server.shutdown();
    log::info!("Server shut down cleanly. Goodbye!");

    Ok(())
}
