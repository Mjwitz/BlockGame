use std::sync::Arc;
use std::time::Instant;

use blockgame_core::{
    block::{BlockId, BlockRegistry},
    crafting::{CraftingGrid, CraftingManager},
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
    physics::PlayerPhysics,
    player::PlayerStats,
    raycast::{raycast_voxel, RaycastHit},
    tool::ToolTier,
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, DayNightCycle, UiBuilder, UiMesh, UiPipeline, VoxelPipeline,
    WireframePipeline,
};
use glam::{IVec3, Vec3};
use wgpu::util::DeviceExt;
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
    pub player_stats: PlayerStats,
    pub death_handled: bool,
    camera: Camera,
    camera_uniform: CameraUniform,
    pub day_night: DayNightCycle,
    targeted_block: Option<RaycastHit>,
    wireframe_buffer: Option<wgpu::Buffer>,
    // Milestone 5: Inventory, Crafting, and UI
    pub inventory: Inventory,
    pub active_hotbar_slot: usize,
    pub item_registry: Arc<ItemRegistry>,
    pub crafting_manager: CraftingManager,
    pub crafting_grid: CraftingGrid,
    pub crafting_result: Option<ItemSlot>,
    pub inventory_open: bool,
    pub cursor_held: ItemSlot,
    pub cursor_pos: [f32; 2],
    pub ui_pipeline: UiPipeline,
    pub ui_mesh: UiMesh,
    pub ui_vertex_buffer: Option<wgpu::Buffer>,
    pub ui_index_buffer: Option<wgpu::Buffer>,
    pub ui_index_count: u32,
    pub item_drop_gpu_mesh: Option<blockgame_render::GpuMeshBuffers>,
    pub animal_gpu_mesh: Option<blockgame_render::GpuMeshBuffers>,
    pub total_time: f32,
    pub walk_distance: f32,
    pub punch_timer: f32,
    // Hold-to-break mining state
    pub mouse_left_down: bool,
    pub breaking_voxel: Option<IVec3>,
    pub break_progress: f32,
    pub cracking_buffer: Option<(wgpu::Buffer, u32)>,
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

impl AppState {
    pub fn respawn_player(&mut self) {
        self.player_stats.reset();
        self.death_handled = false;
        self.player.velocity = Vec3::ZERO;
        self.player.fall_distance = 0.0;
        self.mouse_left_down = false;
        self.break_progress = 0.0;
        self.breaking_voxel = None;
        self.cracking_buffer = None;

        // Find safe surface spawn height at (0, 0)
        let spawn_y = {
            let world_guard = self.world_manager.world().lock().unwrap();
            let mut y = 60;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(0, y, 0));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };
        self.player.position = Vec3::new(0.5, spawn_y, 0.5);
        self.camera.position = self.player.eye_position();
        self.camera.pitch = 0.0;
        self.mouse_captured = true;
        let _ = self.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
        self.window.set_cursor_visible(false);
    }
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
            .with_title("BlockGame - Milestone 5 (Gameplay Loop, Inventory & Crafting)")
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
        let ui_pipeline = UiPipeline::new(
            &device,
            surface_format,
            &pipeline.texture_bind_group_layout,
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

        let item_registry = Arc::new(ItemRegistry::new());
        let inventory = Inventory::starter_inventory();
        let crafting_manager = CraftingManager::new();
        let crafting_grid = CraftingGrid::new_2x2();

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
            player_stats: PlayerStats::new(),
            death_handled: false,
            camera,
            camera_uniform,
            day_night,
            targeted_block: None,
            wireframe_buffer: None,
            inventory,
            active_hotbar_slot: 0,
            item_registry,
            crafting_manager,
            crafting_grid,
            crafting_result: None,
            inventory_open: false,
            cursor_held: ItemSlot::EMPTY,
            cursor_pos: [width as f32 * 0.5, height as f32 * 0.5],
            ui_pipeline,
            ui_mesh: UiMesh::new(),
            ui_vertex_buffer: None,
            ui_index_buffer: None,
            ui_index_count: 0,
            item_drop_gpu_mesh: None,
            animal_gpu_mesh: None,
            total_time: 0.0,
            walk_distance: 0.0,
            punch_timer: 0.0,
            mouse_left_down: false,
            breaking_voxel: None,
            break_progress: 0.0,
            cracking_buffer: None,
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
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                if scroll > 0.0 {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 8) % 9;
                } else if scroll < 0.0 {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 1) % 9;
                }
            }
            WindowEvent::MouseInput {
                button,
                state: mouse_btn_state,
                ..
            } => {
                let pressed = mouse_btn_state == ElementState::Pressed;

                // If player is dead, left click on Respawn button respawns
                if state.player_stats.is_dead {
                    if pressed && button == MouseButton::Left {
                        let screen_w = state.surface_config.width as f32;
                        let screen_h = state.surface_config.height as f32;
                        let btn_w = 200.0;
                        let btn_h = 44.0;
                        let btn_x = (screen_w - btn_w) * 0.5;
                        let btn_y = screen_h * 0.50;
                        if state.cursor_pos[0] >= btn_x && state.cursor_pos[0] <= btn_x + btn_w
                            && state.cursor_pos[1] >= btn_y && state.cursor_pos[1] <= btn_y + btn_h
                        {
                            state.respawn_player();
                        }
                    }
                    return;
                }

                if button == MouseButton::Left {
                    state.mouse_left_down = pressed;
                    if !pressed {
                        state.break_progress = 0.0;
                        state.breaking_voxel = None;
                        state.cracking_buffer = None;
                    }
                }

                if state.inventory_open {
                    if pressed {
                        let is_right = button == MouseButton::Right;
                        handle_inventory_click(state, is_right);
                    }
                    return;
                }

                if !state.mouse_captured {
                    if pressed {
                        state.mouse_captured = true;
                        let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                        state.window.set_cursor_visible(false);
                    }
                    return;
                }

                match button {
                    MouseButton::Left => {
                        if pressed {
                            state.punch_timer = 0.25; // Trigger punch animation
                            let held_slot = state.inventory.get(state.active_hotbar_slot);
                            let attack_damage = if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                match def.tool_tier {
                                    Some(ToolTier::Wood) => 4.0,
                                    Some(ToolTier::Stone) => 5.0,
                                    Some(ToolTier::Iron) => 6.0,
                                    Some(ToolTier::Diamond) => 7.0,
                                    _ => 1.0,
                                }
                            } else {
                                1.0
                            };

                            let hit_animal = state.world_manager.attack_animal(
                                state.camera.position,
                                state.camera.forward(),
                                3.5,
                                attack_damage,
                            );
                            if hit_animal {
                                state.mouse_left_down = false;
                                state.breaking_voxel = None;
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;
                            }
                        }
                    }
                    MouseButton::Right if pressed => {
                        // 1. Check if targeting an interactive block (e.g. Crafting Table)
                        if let Some(hit) = state.targeted_block {
                            let block_id = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard.get_block(hit.hit_voxel)
                            };
                            if block_id == BlockId::CRAFTING_TABLE {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;

                                // Return any items left in current grid
                                for slot in state.crafting_grid.slots.drain(..) {
                                    if !slot.is_empty() {
                                        if let Some(rem) = state.inventory.add_item(slot, &state.item_registry) {
                                            state.world_manager.spawn_item_drop(state.player.position, rem);
                                        }
                                    }
                                }

                                state.crafting_grid = CraftingGrid::new_3x3();
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                        }

                        let held_slot = state.inventory.get(state.active_hotbar_slot);

                        // 2. Hoe Tilling (Grass or Dirt with Hoe -> Farmland)
                        if !held_slot.is_empty() {
                            if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                if def.tool_type == Some(blockgame_core::tool::ToolType::Hoe) {
                                    if let Some(hit) = state.targeted_block {
                                        if hit.hit_face == blockgame_core::direction::Direction::PosY {
                                            let tilled = state.world_manager.till_block(hit.hit_voxel, &state.device, &state.pipeline);
                                            if tilled {
                                                state.punch_timer = 0.2;
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 3. Wheat Seed Planting (Seeds on Farmland -> Wheat Crop)
                        if held_slot.item_id == ItemId::WHEAT_SEEDS {
                            if let Some(hit) = state.targeted_block {
                                let block_beneath = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    world_guard.get_block(hit.hit_voxel)
                                };
                                if block_beneath == BlockId::FARMLAND && hit.hit_face == blockgame_core::direction::Direction::PosY {
                                    let planted = state.world_manager.place_block(
                                        hit.place_voxel,
                                        BlockId::WHEAT_0,
                                        &state.player.aabb(),
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    if planted {
                                        state.world_manager.plant_crop(hit.place_voxel);
                                        state.inventory.slots[state.active_hotbar_slot].take(1);
                                        state.punch_timer = 0.2;
                                        return;
                                    }
                                }
                            }
                        }

                        // 4. Check if holding food and hungry or hurt (eat food!)
                        if !held_slot.is_empty() {
                            if let Some(food) = state.item_registry.get_food(held_slot.item_id) {
                                if state.player_stats.eat(food.food_points, food.saturation) {
                                    state.inventory.slots[state.active_hotbar_slot].take(1);
                                    state.punch_timer = 0.2; // Eating bite animation
                                    return;
                                }
                            }
                        }

                        // 5. Otherwise place active block from hotbar slot
                        let slot = state.inventory.get(state.active_hotbar_slot);
                        if !slot.is_empty() {
                            if let Some(def) = state.item_registry.get(slot.item_id) {
                                if let Some(block_id) = def.placeable_block {
                                    if let Some(hit) = state.targeted_block {
                                        let placed = state.world_manager.place_block(
                                            hit.place_voxel,
                                            block_id,
                                            &state.player.aabb(),
                                            &state.device,
                                            &state.pipeline,
                                        );
                                        if placed {
                                            state.inventory.slots[state.active_hotbar_slot].take(1);
                                        }
                                    }
                                }
                            }
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

                // When dead, Space or Enter triggers respawn
                if state.player_stats.is_dead {
                    if pressed && (key == KeyCode::Space || key == KeyCode::Enter) {
                        state.respawn_player();
                    }
                    return;
                }

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
                    KeyCode::KeyE if pressed => {
                        state.mouse_left_down = false;
                        state.break_progress = 0.0;
                        state.breaking_voxel = None;
                        state.cracking_buffer = None;
                        if state.inventory_open {
                            // Safely return crafting grid items and cursor held item to inventory
                            for slot in state.crafting_grid.slots.drain(..) {
                                if !slot.is_empty() {
                                    if let Some(rem) = state.inventory.add_item(slot, &state.item_registry) {
                                        state.world_manager.spawn_item_drop(state.player.position, rem);
                                    }
                                }
                            }
                            if !state.cursor_held.is_empty() {
                                let held = std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                                if let Some(rem) = state.inventory.add_item(held, &state.item_registry) {
                                    state.world_manager.spawn_item_drop(state.player.position, rem);
                                }
                            }
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        } else {
                            // Open 2x2 backpack inventory
                            state.crafting_grid = CraftingGrid::new_2x2();
                            state.inventory_open = true;
                            state.mouse_captured = false;
                            let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                    }
                    KeyCode::Escape if pressed => {
                        state.mouse_left_down = false;
                        state.break_progress = 0.0;
                        state.breaking_voxel = None;
                        state.cracking_buffer = None;
                        if state.inventory_open {
                            for slot in state.crafting_grid.slots.drain(..) {
                                if !slot.is_empty() {
                                    if let Some(rem) = state.inventory.add_item(slot, &state.item_registry) {
                                        state.world_manager.spawn_item_drop(state.player.position, rem);
                                    }
                                }
                            }
                            if !state.cursor_held.is_empty() {
                                let held = std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                                if let Some(rem) = state.inventory.add_item(held, &state.item_registry) {
                                    state.world_manager.spawn_item_drop(state.player.position, rem);
                                }
                            }
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        } else {
                            state.mouse_captured = false;
                            let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                    }
                    KeyCode::KeyT if pressed => {
                        // Advance time of day by 3 hours (3000 ticks)
                        state.day_night.time = (state.day_night.time + 3000.0).rem_euclid(24000.0);
                        log::info!("Time of day advanced to: {:.1} ticks", state.day_night.time);
                    }
                    // Hotbar slot selection 1-9
                    KeyCode::Digit1 if pressed => state.active_hotbar_slot = 0,
                    KeyCode::Digit2 if pressed => state.active_hotbar_slot = 1,
                    KeyCode::Digit3 if pressed => state.active_hotbar_slot = 2,
                    KeyCode::Digit4 if pressed => state.active_hotbar_slot = 3,
                    KeyCode::Digit5 if pressed => state.active_hotbar_slot = 4,
                    KeyCode::Digit6 if pressed => state.active_hotbar_slot = 5,
                    KeyCode::Digit7 if pressed => state.active_hotbar_slot = 6,
                    KeyCode::Digit8 if pressed => state.active_hotbar_slot = 7,
                    KeyCode::Digit9 if pressed => state.active_hotbar_slot = 8,
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame_time).as_secs_f32().min(0.1);
                state.last_frame_time = now;

                // Update dynamic Day/Night celestial cycle
                state.day_night.update(dt);
                state.total_time += dt;

                // 1. Calculate player wish direction relative to camera yaw
                let forward = Vec3::new(state.camera.yaw.cos(), 0.0, state.camera.yaw.sin()).normalize();
                let right = Vec3::new(-state.camera.yaw.sin(), 0.0, state.camera.yaw.cos()).normalize();

                let mut wish_dir = Vec3::ZERO;
                if !state.player_stats.is_dead {
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
                }

                // 2. Step player physics against voxel world
                let fall_damage = {
                    let world_guard = state.world_manager.world().lock().unwrap();
                    state.player.update(
                        dt,
                        wish_dir,
                        state.sprinting,
                        state.jumping,
                        &world_guard,
                        &self.registry,
                    )
                };

                // Apply fall damage if landed on ground
                if fall_damage > 0.0 && !state.player_stats.is_dead {
                    state.player_stats.take_damage(fall_damage);
                }

                // Update walking distance & movement exhaustion (advances ONLY when moving)
                let is_moving = (state.moving_forward || state.moving_backward || state.moving_left || state.moving_right)
                    && (state.player.on_ground || state.player.flying);
                let horiz_speed = (state.player.velocity.x * state.player.velocity.x + state.player.velocity.z * state.player.velocity.z).sqrt();
                if is_moving && horiz_speed > 0.1 && !state.player_stats.is_dead {
                    state.walk_distance += dt * (if state.sprinting { 8.5 } else { 5.5 });
                    if state.player.on_ground {
                        state.player_stats.add_exhaustion(dt * if state.sprinting { 0.1 } else { 0.02 });
                    }
                }

                // Update player stats (regeneration, starvation, void damage at y < -25.0)
                state.player_stats.update(dt, state.player.position.y);

                // Handle death transition and item scattering
                if state.player_stats.is_dead {
                    if !state.death_handled {
                        state.death_handled = true;
                        state.inventory_open = false;
                        state.mouse_left_down = false;
                        state.break_progress = 0.0;
                        state.breaking_voxel = None;
                        state.cracking_buffer = None;

                        // Scatter all 36 inventory slots as item drops in world
                        for slot in state.inventory.slots.iter_mut() {
                            if !slot.is_empty() {
                                let drop_slot = std::mem::replace(slot, ItemSlot::EMPTY);
                                state.world_manager.spawn_item_drop(state.player.position, drop_slot);
                            }
                        }
                        // Scatter crafting grid items
                        for slot in state.crafting_grid.slots.drain(..) {
                            if !slot.is_empty() {
                                state.world_manager.spawn_item_drop(state.player.position, slot);
                            }
                        }
                        // Scatter cursor held item
                        if !state.cursor_held.is_empty() {
                            let drop_slot = std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                            state.world_manager.spawn_item_drop(state.player.position, drop_slot);
                        }

                        // Unlock mouse so user can click the Respawn button
                        state.mouse_captured = false;
                        let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    }
                    // Cinematic death camera tilt
                    let target_pitch = 28.0_f32.to_radians();
                    state.camera.pitch = (state.camera.pitch + dt * 0.8).min(target_pitch);
                }

                // Update punch animation timer
                let punch_time = if state.punch_timer > 0.0 {
                    state.punch_timer = (state.punch_timer - dt).max(0.0);
                    1.0 - (state.punch_timer / 0.25)
                } else {
                    0.0
                };

                // 3. Update Item Drops & Magnetism Pickup (only when alive)
                if !state.player_stats.is_dead {
                    let collected = state.world_manager.update_item_drops(dt, state.player.position);
                    for item in collected {
                        let overflow = state.inventory.add_item(item, &state.item_registry);
                        if let Some(remaining) = overflow {
                            state.world_manager.spawn_item_drop(state.player.position, remaining);
                        }
                    }
                } else {
                    let _ = state.world_manager.update_item_drops(dt, state.player.position);
                }

                // 3b. Update Animals & Planted Crops
                let animal_drops = state.world_manager.update_animals(dt);
                for (pos, drops) in animal_drops {
                    for drop_slot in drops {
                        state.world_manager.spawn_item_drop(pos, drop_slot);
                    }
                }
                state.world_manager.update_crops(dt, &state.device, &state.pipeline);

                // 4. Update Camera position and atmosphere uniform
                state.camera.position = state.player.eye_position();
                let aspect = state.surface_config.width as f32 / state.surface_config.height as f32;
                state.camera.update_uniform_with_atmosphere(&mut state.camera_uniform, aspect, &state.day_night);
                state.pipeline.update_camera(&state.queue, &state.camera_uniform);

                // 5. Perform Fast Voxel Traversal (DDA) raycast from crosshair
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
                            let block = world_guard.get_block(h.hit_voxel);
                            let wire_buf = if block == BlockId::TORCH {
                                WireframePipeline::create_custom_box_buffer(
                                    &state.device,
                                    h.hit_voxel,
                                    Vec3::new(0.35, 0.0, 0.35),
                                    Vec3::new(0.65, 0.65, 0.65),
                                )
                            } else {
                                WireframePipeline::create_box_buffer(&state.device, h.hit_voxel)
                            };
                            state.wireframe_buffer = Some(wire_buf);
                        }
                        state.targeted_block = Some(h);
                    } else {
                        state.targeted_block = None;
                        state.wireframe_buffer = None;
                    }
                }

                // 5b. Process hold-to-break mining mechanics
                if state.mouse_left_down && state.mouse_captured && !state.inventory_open && !state.player_stats.is_dead {
                    if state.punch_timer <= 0.05 {
                        state.punch_timer = 0.25; // Continuous swing loop while holding down break
                    }

                    if let Some(hit) = state.targeted_block {
                        let target_voxel = hit.hit_voxel;
                        if state.breaking_voxel != Some(target_voxel) {
                            state.breaking_voxel = Some(target_voxel);
                            state.break_progress = 0.0;
                            state.cracking_buffer = None;
                        }

                        let block_at_voxel = {
                            let world_guard = state.world_manager.world().lock().unwrap();
                            world_guard.get_block(target_voxel)
                        };

                        if block_at_voxel.is_air() {
                            state.breaking_voxel = None;
                            state.break_progress = 0.0;
                            state.cracking_buffer = None;
                        } else {
                            let held_slot = state.inventory.get(state.active_hotbar_slot);
                            let (held_tool_type, held_tool_tier) = if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                (def.tool_type, def.tool_tier.unwrap_or(ToolTier::None))
                            } else {
                                (None, ToolTier::None)
                            };

                            let break_time = self.registry.get_break_time(block_at_voxel, held_tool_type, held_tool_tier);
                            if break_time.is_infinite() {
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;
                            } else {
                                state.break_progress += dt / break_time;
                                if state.break_progress >= 1.0 {
                                    // Block break completed!
                                    if let Some(broken) = state.world_manager.break_block(
                                        target_voxel,
                                        &state.device,
                                        &state.pipeline,
                                    ) {
                                        state.player_stats.add_exhaustion(0.025);
                                        if let Some((drop_item, drop_count)) = self.registry.get_drop(broken, held_tool_type, held_tool_tier) {
                                            state.world_manager.spawn_item_drop(
                                                target_voxel.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                                ItemSlot::new(drop_item, drop_count),
                                            );
                                        }
                                        // 15% chance to drop Apple from Leaves
                                        if broken == BlockId::LEAVES {
                                            let hash = (target_voxel.x.wrapping_mul(73856093) ^ target_voxel.y.wrapping_mul(19349663) ^ target_voxel.z.wrapping_mul(83492791)) % 100;
                                            if hash.abs() < 15 {
                                                state.world_manager.spawn_item_drop(
                                                    target_voxel.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                                    ItemSlot::new(ItemId::APPLE, 1),
                                                );
                                            }
                                        }
                                        // 12% chance to drop Wheat Seeds from Grass
                                        if broken == BlockId::GRASS {
                                            let hash = (target_voxel.x.wrapping_mul(87654321) ^ target_voxel.y.wrapping_mul(23456789) ^ target_voxel.z.wrapping_mul(98765432)) % 100;
                                            if hash.abs() < 12 {
                                                state.world_manager.spawn_item_drop(
                                                    target_voxel.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                                    ItemSlot::new(ItemId::WHEAT_SEEDS, 1),
                                                );
                                            }
                                        }
                                        // Ripe Wheat Crop drops additional 1..2 seeds
                                        if broken == BlockId::WHEAT_3 {
                                            let hash = (target_voxel.x.wrapping_mul(1234567) ^ target_voxel.z.wrapping_mul(7654321)) % 2;
                                            let seed_count = (hash.abs() + 1) as u16;
                                            state.world_manager.spawn_item_drop(
                                                target_voxel.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                                ItemSlot::new(ItemId::WHEAT_SEEDS, seed_count),
                                            );
                                        }
                                    }
                                    state.breaking_voxel = None;
                                    state.break_progress = 0.0;
                                    state.cracking_buffer = None;
                                } else {
                                    let stage = ((state.break_progress * 10.0).floor() as usize).min(9);
                                    let (crack_buf, count) = WireframePipeline::create_cracking_box_buffer(
                                        &state.device,
                                        target_voxel,
                                        stage,
                                    );
                                    state.cracking_buffer = Some((crack_buf, count));
                                }
                            }
                        }
                    } else {
                        state.breaking_voxel = None;
                        state.break_progress = 0.0;
                        state.cracking_buffer = None;
                    }
                } else if state.breaking_voxel.is_some() {
                    state.breaking_voxel = None;
                    state.break_progress = 0.0;
                    state.cracking_buffer = None;
                }

                // 6. Update chunk streaming around player
                state.world_manager.update(
                    state.player.position,
                    &state.device,
                    &state.pipeline,
                );

                // 7. Build 2D UI Overlay (HUD or Inventory/Crafting Screen)
                let screen_w = state.surface_config.width as f32;
                let screen_h = state.surface_config.height as f32;
                state.ui_pipeline.update_screen_size(&state.queue, screen_w, screen_h);

                if state.inventory_open {
                    state.crafting_result = state
                        .crafting_manager
                        .find_match(&state.crafting_grid)
                        .map(|(_, out)| out);
                    UiBuilder::build_inventory_screen(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        &state.inventory,
                        state.active_hotbar_slot,
                        &state.crafting_grid,
                        state.crafting_result,
                        &state.cursor_held,
                        state.cursor_pos,
                        &state.item_registry,
                    );
                } else {
                    UiBuilder::build_hud(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        &state.inventory,
                        state.active_hotbar_slot,
                        &state.item_registry,
                        state.walk_distance,
                        punch_time,
                        &state.player_stats,
                    );
                }

                if !state.ui_mesh.indices.is_empty() {
                    let vertex_buf = state.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Vertex Buffer"),
                        contents: bytemuck::cast_slice(&state.ui_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let index_buf = state.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Index Buffer"),
                        contents: bytemuck::cast_slice(&state.ui_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    state.ui_vertex_buffer = Some(vertex_buf);
                    state.ui_index_buffer = Some(index_buf);
                    state.ui_index_count = state.ui_mesh.indices.len() as u32;
                } else {
                    state.ui_index_count = 0;
                }

                // Upload dynamic 3D Item Drops mesh
                if !state.world_manager.item_drops.is_empty() {
                    let drops_mesh = blockgame_render::build_item_drops_mesh(
                        &state.world_manager.item_drops,
                        &state.item_registry,
                    );
                    state.item_drop_gpu_mesh = state.pipeline.upload_mesh(&state.device, &drops_mesh);
                } else {
                    state.item_drop_gpu_mesh = None;
                }

                // Upload dynamic 3D Animals mesh
                if !state.world_manager.animals.is_empty() {
                    let animals_mesh = blockgame_render::build_animals_mesh(&state.world_manager.animals);
                    state.animal_gpu_mesh = state.pipeline.upload_mesh(&state.device, &animals_mesh);
                } else {
                    state.animal_gpu_mesh = None;
                }

                // 8. Render frame
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

                // 3D Voxel Terrain Pass
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

                    // Render voxel terrain meshes
                    render_pass.set_pipeline(&state.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    state.world_manager.render(&mut render_pass);

                    // Render floating 3D item drops
                    if let Some(drop_mesh) = &state.item_drop_gpu_mesh {
                        drop_mesh.draw(&mut render_pass);
                    }

                    // Render active 3D animals
                    if let Some(animal_mesh) = &state.animal_gpu_mesh {
                        animal_mesh.draw(&mut render_pass);
                    }

                    // Render targeted block selection wireframe or progressive cracking overlay
                    if let Some((crack_buf, vertex_count)) = &state.cracking_buffer {
                        render_pass.set_pipeline(&state.wireframe_pipeline.render_pipeline);
                        render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, crack_buf.slice(..));
                        render_pass.draw(0..*vertex_count, 0..1);
                    } else if let Some(wireframe_buf) = &state.wireframe_buffer {
                        render_pass.set_pipeline(&state.wireframe_pipeline.render_pipeline);
                        render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, wireframe_buf.slice(..));
                        render_pass.draw(0..24, 0..1);
                    }
                }

                // 2D UI Overlay Pass (Draws HUD or Inventory over 3D world)
                {
                    let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("UI Overlay Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    if state.ui_index_count > 0 {
                        if let (Some(vb), Some(ib)) = (&state.ui_vertex_buffer, &state.ui_index_buffer) {
                            ui_pass.set_pipeline(&state.ui_pipeline.render_pipeline);
                            ui_pass.set_bind_group(0, &state.ui_pipeline.screen_bind_group, &[]);
                            ui_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                            ui_pass.set_vertex_buffer(0, vb.slice(..));
                            ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                            ui_pass.draw_indexed(0..state.ui_index_count, 0, 0..1);
                        }
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
            if state.mouse_captured && !state.player_stats.is_dead {
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

fn handle_inventory_click(state: &mut AppState, is_right_click: bool) {
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    let slot_size = 40.0;
    let slot_gap = 4.0;

    // 1. Dynamic Crafting Grid (2x2 or 3x3)
    let grid_w = state.crafting_grid.width;
    let grid_h = state.crafting_grid.height;

    let craft_x = (screen_w - ((grid_w as f32 + 2.0) * (slot_size + slot_gap) + 20.0)) * 0.5;
    let craft_y = screen_h * 0.5 - (if grid_h == 3 { 185.0 } else { 160.0 });

    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let sx = craft_x + (gx as f32) * (slot_size + slot_gap);
            let sy = craft_y + (gy as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                let mut slot = state.crafting_grid.get(gx, gy);
                click_slot_raw(&mut slot, &mut state.cursor_held, is_right_click, &state.item_registry);
                state.crafting_grid.set(gx, gy, slot);
                return;
            }
        }
    }

    // 2. Crafting Result Slot
    let result_x = craft_x + (grid_w as f32) * (slot_size + slot_gap) + 28.0;
    let result_y = craft_y + ((grid_h as f32 - 1.0) * (slot_size + slot_gap) * 0.5);
    let result_size = slot_size + 6.0;

    if cx >= result_x && cx < result_x + result_size && cy >= result_y && cy < result_y + result_size {
        if state.crafting_result.is_some() {
            if let Some(crafted) = state.crafting_manager.craft(&mut state.crafting_grid) {
                if state.cursor_held.is_empty() {
                    state.cursor_held = crafted;
                } else if state.cursor_held.can_stack_with(&crafted) {
                    state.cursor_held.count += crafted.count;
                }
            }
        }
        return;
    }

    // 3. Backpack Storage Grid (3 rows x 9 columns)
    let inv_w = 9.0 * (slot_size + slot_gap);
    let inv_start_x = (screen_w - inv_w) * 0.5;
    let inv_start_y = craft_y + (grid_h as f32) * (slot_size + slot_gap) + 24.0;

    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = inv_start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_start_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                state.inventory.click_slot(slot_idx, &mut state.cursor_held, is_right_click, &state.item_registry);
                return;
            }
        }
    }

    // 4. Hotbar Slots
    let hotbar_y = inv_start_y + 3.0 * (slot_size + slot_gap) + 16.0;
    for i in 0..9 {
        let sx = inv_start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;

        if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
            state.inventory.click_slot(i, &mut state.cursor_held, is_right_click, &state.item_registry);
            return;
        }
    }
}

fn click_slot_raw(
    slot: &mut ItemSlot,
    held_cursor: &mut ItemSlot,
    split_half: bool,
    registry: &ItemRegistry,
) {
    if held_cursor.is_empty() {
        if !slot.is_empty() {
            if split_half {
                *held_cursor = slot.split_half();
            } else {
                *held_cursor = *slot;
                *slot = ItemSlot::EMPTY;
            }
        }
    } else {
        if slot.is_empty() {
            if split_half {
                *slot = held_cursor.take(1);
            } else {
                let max_stack = registry.max_stack_size(held_cursor.item_id);
                let transfer = max_stack.min(held_cursor.count);
                *slot = held_cursor.take(transfer);
            }
        } else if slot.can_stack_with(held_cursor) {
            let max_stack = registry.max_stack_size(slot.item_id);
            if slot.count < max_stack {
                if split_half {
                    slot.count += 1;
                    held_cursor.take(1);
                } else {
                    let space = max_stack - slot.count;
                    let transfer = space.min(held_cursor.count);
                    slot.count += transfer;
                    held_cursor.take(transfer);
                }
            }
        } else if !split_half {
            std::mem::swap(slot, held_cursor);
        }
    }
}

