use std::sync::Arc;
use std::time::Instant;

use blockgame_core::block::BlockRegistry;
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, VoxelPipeline,
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
    world_manager: WorldManager,
    camera: Camera,
    camera_uniform: CameraUniform,
    // Movement state
    moving_forward: bool,
    moving_backward: bool,
    moving_left: bool,
    moving_right: bool,
    moving_up: bool,
    moving_down: bool,
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
            .with_title("BlockGame - Milestone 2 (Infinite Procedural World)")
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

        let mut world_manager =
            WorldManager::new(self.seed, self.render_distance, self.registry.clone());
        world_manager.init_spawn_area(&device, &pipeline);

        // Position camera floating above the spawn hills
        let camera = Camera::new(
            Vec3::new(0.0, 38.0, 0.0),
            (-90.0_f32).to_radians(),
            (-15.0_f32).to_radians(),
        );

        let mut camera_uniform = CameraUniform::default();
        camera.update_uniform(&mut camera_uniform, width as f32 / height as f32);
        pipeline.update_camera(&queue, &camera_uniform);

        self.state = Some(AppState {
            window,
            surface,
            device,
            queue,
            surface_config,
            depth_view,
            pipeline,
            world_manager,
            camera,
            camera_uniform,
            moving_forward: false,
            moving_backward: false,
            moving_left: false,
            moving_right: false,
            moving_up: false,
            moving_down: false,
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
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            } => {
                state.mouse_captured = true;
                let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                state.window.set_cursor_visible(false);
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
                    KeyCode::Space => state.moving_up = pressed,
                    KeyCode::ShiftLeft => state.moving_down = pressed,
                    KeyCode::Escape if pressed => {
                        state.mouse_captured = false;
                        let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame_time).as_secs_f32().min(0.1);
                state.last_frame_time = now;

                // Flying spectator movement speed (22 blocks per second)
                let speed = 22.0 * dt;
                let forward = state.camera.forward();
                let right = state.camera.right();

                if state.moving_forward {
                    state.camera.position += forward * speed;
                }
                if state.moving_backward {
                    state.camera.position -= forward * speed;
                }
                if state.moving_right {
                    state.camera.position += right * speed;
                }
                if state.moving_left {
                    state.camera.position -= right * speed;
                }
                if state.moving_up {
                    state.camera.position.y += speed;
                }
                if state.moving_down {
                    state.camera.position.y -= speed;
                }

                // Stream chunks around the player
                state.world_manager.update(
                    state.camera.position,
                    &state.device,
                    &state.pipeline,
                );

                let aspect = state.surface_config.width as f32 / state.surface_config.height as f32;
                state.camera.update_uniform(&mut state.camera_uniform, aspect);
                state.pipeline.update_camera(&state.queue, &state.camera_uniform);

                // Render frame
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

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Voxel Main Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.53,
                                    g: 0.81,
                                    b: 0.98,
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

                    render_pass.set_pipeline(&state.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);

                    state.world_manager.render(&mut render_pass);
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
