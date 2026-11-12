use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use blockgame_core::{
    net::{MultiplayerServer, DEFAULT_SERVER_PORT},
    save::{parse_or_hash_seed, sanitize_folder_name},
};
use blockgame_gen::TerrainGenerator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

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

    let server = MultiplayerServer::start(
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
        "Dedicated server active on {}. Press Ctrl+C to stop.",
        server.local_addr()
    );

    loop {
        thread::sleep(Duration::from_secs(5));
    }
}
