mod app;
mod headless;

use app::BlockGameApp;
use blockgame_core::{
    block::{BlockId, BlockRegistry},
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Z},
};
use winit::event_loop::EventLoop;

fn create_sample_chunk() -> Chunk {
    let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

    // 1. Bedrock floor at y = 0
    for z in 0..CHUNK_SIZE_Z {
        for x in 0..CHUNK_SIZE_X {
            chunk.set(x, 0, z, BlockId::BEDROCK);
        }
    }

    // 2. Terrain with stone, dirt, and rolling grass surface
    for z in 0..CHUNK_SIZE_Z {
        for x in 0..CHUNK_SIZE_X {
            // Gentle hill height between 7 and 9
            let h = 8 + (((x as f32 * 0.4).sin() + (z as f32 * 0.4).cos()) * 1.5) as usize;
            let surface_y = h.clamp(6, 10);

            for y in 1..surface_y - 2 {
                if (x + y * 2 + z * 3) % 11 == 0 {
                    chunk.set(x, y, z, BlockId::COBBLESTONE);
                } else {
                    chunk.set(x, y, z, BlockId::STONE);
                }
            }

            for y in (surface_y - 2)..surface_y {
                chunk.set(x, y, z, BlockId::DIRT);
            }

            chunk.set(x, surface_y, z, BlockId::GRASS);
        }
    }

    // 3. Add an Oak Tree
    let tree_x = 8;
    let tree_z = 8;
    let mut base_y = 10;
    for y in (6..14).rev() {
        if let Some(id) = chunk.get(tree_x, y, tree_z) {
            if id == BlockId::GRASS {
                base_y = y + 1;
                break;
            }
        }
    }

    // Tree Trunk (4 blocks high)
    for y in base_y..(base_y + 4) {
        chunk.set(tree_x, y, tree_z, BlockId::WOOD_LOG);
    }

    // Leaves Crown
    let crown_base = base_y + 2;
    for ly in crown_base..=(base_y + 5) {
        let radius: i32 = if ly >= base_y + 4 { 1 } else { 2 };
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let lx = tree_x as i32 + dx;
                let lz = tree_z as i32 + dz;
                if lx >= 0 && lx < 16 && lz >= 0 && lz < 16 && ly < 16 {
                    if chunk.get(lx as usize, ly, lz as usize) == Some(BlockId::AIR) {
                        // Omit outer corners on wider leaf layer
                        if dx.abs() == 2 && dz.abs() == 2 && (dx + dz) % 2 == 0 {
                            continue;
                        }
                        chunk.set(lx as usize, ly, lz as usize, BlockId::LEAVES);
                    }
                }
            }
        }
    }

    // 4. Wooden planks and glass showcase
    chunk.set(3, 9, 3, BlockId::PLANKS);
    chunk.set(3, 9, 4, BlockId::PLANKS);
    chunk.set(4, 9, 3, BlockId::PLANKS);
    chunk.set(4, 9, 4, BlockId::PLANKS);

    chunk.set(3, 10, 3, BlockId::GLASS);
    chunk.set(3, 11, 3, BlockId::GLASS);

    chunk
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let registry = BlockRegistry::new();
    let chunk = create_sample_chunk();

    let args: Vec<String> = std::env::args().collect();

    // Check if offscreen render mode is requested
    if let Some(idx) = args.iter().position(|a| a == "--render-to-image") {
        let output_path = args.get(idx + 1).map(|s| s.as_str()).unwrap_or("chunk_render.png");
        log::info!("Starting headless chunk render to {}", output_path);
        pollster::block_on(headless::render_chunk_to_image(
            &chunk,
            &registry,
            1280,
            720,
            output_path,
        ))?;
        return Ok(());
    }

    if args.iter().any(|a| a == "--headless") {
        let output_path = "chunk_render.png";
        log::info!("Starting headless chunk render to {}", output_path);
        pollster::block_on(headless::render_chunk_to_image(
            &chunk,
            &registry,
            1280,
            720,
            output_path,
        ))?;
        return Ok(());
    }

    log::info!("Launching BlockGame interactive client window...");
    let event_loop = EventLoop::new()?;
    let mut app = BlockGameApp::new(chunk, registry);
    event_loop.run_app(&mut app)?;

    Ok(())
}
