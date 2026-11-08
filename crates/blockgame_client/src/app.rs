use std::sync::Arc;
use std::time::Instant;

use blockgame_core::{
    block::{BlockId, BlockRegistry},
    physics::PlayerPhysics,
    raycast::{raycast_voxel, RaycastHit},
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, DayNightCycle, VoxelPipeline, WireframePipeline,
};
use glam::Vec3;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::world_manager::WorldManager;

pub struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    pipeline: VoxelPipeline,
    wireframe_pipeline: WireframePipeline,
    world_manager: WorldManager,
    player: PlayerPhysics,
    camera: Camera,
    camera_uniform: CameraUniform,
    pub day_night: DayNightCycle,
    targeted_block: Option<RaycastHit>,
    wireframe_buffer: Option<wgpu::Buffer>,
    active_block: BlockId,
    // Movement inputs
    moving_forward: bool,
    moving_backward: bool,
    moving_left: bool,
    moving_right: bool,
    jumping: bool,
    sprinting: bool,
    mouse_captured: bool,
    last_frame_time: Instant,
}

pub struct BlockGameApp {
    seed: u32,
    render_distance: i32,
    registry: Arc<BlockRegistry>,
    state: Option<AppState>,
}

impl BlockGameApp {
    pub fn new(seed: u32, render_distance: i32, registry: Arc<BlockRegistry>) -> Self {
        Self {
            seed,
            render_distance,
            registry,
            state: None,
        }
    }
}

impl ApplicationHandler for BlockGameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = WindowAttributes::default()
            .with_title("BlockGame - Milestone 4 (Atmosphere, Day/Night Cycle & Dual-Channel Lighting)")
            .with_inner_size(PhysicalSize::new(1280, 720));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find suitable graphics adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("BlockGame Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("Failed to create wgpu device");

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let (_depth_tex, depth_view) = create_depth_texture(&device, width, height);

        let atlas = TextureAtlas::generate_default();
        let pipeline = VoxelPipeline::new(&device, &queue, surface_format, &atlas);
        let wireframe_pipeline = WireframePipeline::new(
            &device,
            surface_format,
            &pipeline.camera_bind_group_layout,
        );

        let mut world_manager =
            WorldManager::new(self.seed, self.render_distance, self.registry.clone());
        world_manager.init_spawn_area(&device, &pipeline);

        // Spawn player atop the surface
        let player = PlayerPhysics::new(Vec3::new(0.0, 32.0, 0.0));

        let camera = Camera::new(
            player.eye_position(),
            (-90.0_f32).to_radians(),
            (-10.0_f32).to_radians(),
        );

        let day_night = DayNightCycle::new();
        let mut camera_uniform = CameraUniform::default();
        camera.update_uniform_with_atmosphere(&mut camera_uniform, width as f32 / height as f32, &day_night);
        pipeline.update_camera(&queue, &camera_uniform);

        self.state = Some(AppState {
            window,
            surface,
            device,
            queue,
            surface_config,
            depth_view,
            pipeline,
            wireframe_pipeline,
            world_manager,
            player,
            camera,
            camera_uniform,
            day_night,
            targeted_block: None,
            wireframe_buffer: None,
            active_block: BlockId::WOOD_LOG,
            moving_forward: false,
            moving_backward: false,
            moving_left: false,
            moving_right: false,
            jumping: false,
            sprinting: false,
            mouse_captured: false,
            last_frame_time: Instant::now(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state.surface.configure(&state.device, &state.surface_config);
                    let (_depth_tex, depth_view) =
                        create_depth_texture(&state.device, new_size.width, new_size.height);
                    state.depth_view = depth_view;
                }
            }
            WindowEvent::MouseInput {
                button,
                state: ElementState::Pressed,
                ..
            } => {
                if !state.mouse_captured {
                    state.mouse_captured = true;
                    let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                    state.window.set_cursor_visible(false);
                    return;
                }

                match button {
                    MouseButton::Left => {
                        // Break targeted block
                        if let Some(hit) = state.targeted_block {
                            log::info!("Breaking block at {:?}", hit.hit_voxel);
                            state.world_manager.break_block(
                                hit.hit_voxel,
                                &state.device,
                                &state.pipeline,
                            );
                        }
                    }
                    MouseButton::Right => {
                        // Place active block on the targeted face
                        if let Some(hit) = state.targeted_block {
                            log::info!(
                                "Placing {:?} at {:?}",
                                state.active_block,
                                hit.place_voxel
                            );
                            state.world_manager.place_block(
                                hit.place_voxel,
                                state.active_block,
                                &state.player.aabb(),
                                &state.device,
                                &state.pipeline,
                            );
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let pressed = key_state == ElementState::Pressed;
                match key {
                    KeyCode::KeyW => state.moving_forward = pressed,
                    KeyCode::KeyS => state.moving_backward = pressed,
                    KeyCode::KeyA => state.moving_left = pressed,
                    KeyCode::KeyD => state.moving_right = pressed,
                    KeyCode::Space => state.jumping = pressed,
                    KeyCode::ControlLeft => state.sprinting = pressed,
                    KeyCode::KeyF if pressed => {
                        state.player.toggle_flight();
                        log::info!("Flying mode: {}", state.player.flying);
                    }
                    KeyCode::Escape if pressed => {
                        state.mouse_captured = false;
                        let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    }
                    KeyCode::KeyT if pressed => {
                        // Advance time of day by 3 hours (3000 ticks)
                        state.day_night.time = (state.day_night.time + 3000.0).rem_euclid(24000.0);
                        log::info!("Time of day advanced to: {:.1} ticks", state.day_night.time);
                    }
                    // Hotbar block selection
                    KeyCode::Digit1 if pressed => state.active_block = BlockId::STONE,
                    KeyCode::Digit2 if pressed => state.active_block = BlockId::DIRT,
                    KeyCode::Digit3 if pressed => state.active_block = BlockId::GRASS,
                    KeyCode::Digit4 if pressed => state.active_block = BlockId::COBBLESTONE,
                    KeyCode::Digit5 if pressed => state.active_block = BlockId::WOOD_LOG,
                    KeyCode::Digit6 if pressed => state.active_block = BlockId::LEAVES,
                    KeyCode::Digit7 if pressed => state.active_block = BlockId::SAND,
                    KeyCode::Digit8 if pressed => state.active_block = BlockId::PLANKS,
                    KeyCode::Digit9 if pressed => state.active_block = BlockId::GLASS,
                    KeyCode::Digit0 if pressed => state.active_block = BlockId::TORCH,
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame_time).as_secs_f32().min(0.1);
                state.last_frame_time = now;

                // Update dynamic Day/Night celestial cycle
                state.day_night.update(dt);

                // 1. Calculate player wish direction relative to camera yaw
                let forward = Vec3::new(state.camera.yaw.cos(), 0.0, state.camera.yaw.sin()).normalize();
                let right = Vec3::new(-state.camera.yaw.sin(), 0.0, state.camera.yaw.cos()).normalize();

                let mut wish_dir = Vec3::ZERO;
                if state.moving_forward { wish_dir += forward; }
                if state.moving_backward { wish_dir -= forward; }
                if state.moving_right { wish_dir += right; }
                if state.moving_left { wish_dir -= right; }
                if wish_dir.length_squared() > 0.0 {
                    wish_dir = wish_dir.normalize();
                }

                // If flying, vertical movement with Space / Shift
                if state.player.flying {
                    if state.jumping { wish_dir.y += 1.0; }
                }

                // 2. Step player physics against voxel world
                {
                    let world_guard = state.world_manager.world().lock().unwrap();
                    state.player.update(
                        dt,
                        wish_dir,
                        state.sprinting,
                        state.jumping,
                        &world_guard,
                        &self.registry,
                    );
                }

                // 3. Update Camera position and atmosphere uniform
                state.camera.position = state.player.eye_position();
                let aspect = state.surface_config.width as f32 / state.surface_config.height as f32;
                state.camera.update_uniform_with_atmosphere(&mut state.camera_uniform, aspect, &state.day_night);
                state.pipeline.update_camera(&state.queue, &state.camera_uniform);

                // 4. Perform Fast Voxel Traversal (DDA) raycast from crosshair
                {
                    let world_guard = state.world_manager.world().lock().unwrap();
                    let hit = raycast_voxel(
                        state.camera.position,
                        state.camera.forward(),
                        5.0, // 5 blocks reach
                        &world_guard,
                        &self.registry,
                    );

                    if let Some(h) = hit {
                        // Recreate wireframe buffer if target voxel changed
                        if state.targeted_block.map(|t| t.hit_voxel) != Some(h.hit_voxel) {
                            state.wireframe_buffer = Some(
                                WireframePipeline::create_box_buffer(&state.device, h.hit_voxel),
                            );
                        }
                        state.targeted_block = Some(h);
                    } else {
                        state.targeted_block = None;
                        state.wireframe_buffer = None;
                    }
                }

                // 5. Update chunk streaming around player
                state.world_manager.update(
                    state.player.position,
                    &state.device,
                    &state.pipeline,
                );

                // 6. Render frame
                let output = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost) => {
                        state.surface.configure(&state.device, &state.surface_config);
                        return;
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                        return;
                    }
                    Err(e) => {
                        log::warn!("Surface error: {:?}", e);
                        return;
                    }
                };

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Frame Command Encoder"),
                    });

                let sky = state.day_night.sky_color();

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Main Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: sky[0] as f64,
                                    g: sky[1] as f64,
                                    b: sky[2] as f64,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(
                            wgpu::RenderPassDepthStencilAttachment {
                                view: &state.depth_view,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.0),
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            },
                        ),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    // 1. Render voxel terrain meshes
                    render_pass.set_pipeline(&state.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    state.world_manager.render(&mut render_pass);

                    // 2. Render targeted block selection wireframe
                    if let Some(wireframe_buf) = &state.wireframe_buffer {
                        render_pass.set_pipeline(&state.wireframe_pipeline.render_pipeline);
                        render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, wireframe_buf.slice(..));
                        render_pass.draw(0..24, 0..1);
                    }
                }

                state.queue.submit(Some(encoder.finish()));
                output.present();

                state.window.request_redraw();
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(state) = &mut self.state {
            if state.mouse_captured {
                if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
                    let sensitivity = 0.003;
                    state.camera.yaw += (dx as f32) * sensitivity;
                    state.camera.pitch -= (dy as f32) * sensitivity;
                    let max_pitch = 89.0_f32.to_radians();
                    state.camera.pitch = state.camera.pitch.clamp(-max_pitch, max_pitch);
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
