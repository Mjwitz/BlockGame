mod app;
mod headless;
mod world_manager;

use app::BlockGameApp;
use blockgame_core::block::BlockRegistry;
use std::sync::Arc;
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let registry = Arc::new(BlockRegistry::new());
    let seed: u32 = 1337;
    let render_distance: i32 = 4;

    let args: Vec<String> = std::env::args().collect();

    // Check if offscreen render mode is requested
    if let Some(idx) = args.iter().position(|a| a == "--render-to-image") {
        let output_path = args
            .get(idx + 1)
            .map(|s| s.as_str())
            .unwrap_or("landscape_render.png");
        log::info!(
            "Starting headless world landscape render to {}",
            output_path
        );
        pollster::block_on(headless::render_world_to_image(
            seed,
            render_distance,
            1280,
            720,
            output_path,
        ))?;
        return Ok(());
    }

    if args.iter().any(|a| a == "--headless") {
        let output_path = "landscape_render.png";
        log::info!(
            "Starting headless world landscape render to {}",
            output_path
        );
        pollster::block_on(headless::render_world_to_image(
            seed,
            render_distance,
            1280,
            720,
            output_path,
        ))?;
        return Ok(());
    }

    log::info!(
        "Launching BlockGame client (Seed: {}, Render Distance: {})...",
        seed,
        render_distance
    );
    let event_loop = EventLoop::new()?;
    let mut app = BlockGameApp::new(seed, render_distance, registry);
    event_loop.run_app(&mut app)?;

    Ok(())
}
