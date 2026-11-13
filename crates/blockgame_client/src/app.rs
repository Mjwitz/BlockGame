use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use blockgame_core::{
    block::{BlockId, BlockRegistry},
    crafting::{CraftingGrid, CraftingManager},
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
    physics::PlayerPhysics,
    player::PlayerStats,
    raycast::{raycast_voxel_with_water, RaycastHit},
    save::{
        delete_saved_world, list_saved_worlds, load_world_from_dir, parse_or_hash_seed,
        save_world_to_dir, unique_world_folder_name, SavedPlayerState, WorldMetadata,
    },
    tool::ToolTier,
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, DayNightCycle, UiBuilder, UiMesh, UiPipeline, VoxelPipeline,
    WireframePipeline,
};
use glam::{IVec3, Vec3};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::world_manager::WorldManager;

#[derive(Debug, Clone)]
pub struct JeiRecipeModalState {
    pub target_item: ItemId,
    pub is_usage_mode: bool,
    pub recipe_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppScreen {
    Title,
    WorldSelect {
        selected_index: Option<usize>,
    },
    CreateWorld {
        name_input: String,
        seed_input: String,
        focused_field: usize,
    },
    Settings {
        return_to_pause: bool,
    },
    MultiplayerLobby {
        username_input: String,
        password_input: String,
        address_input: String,
        skin_id: u8,
        focused_field: usize,
        status_message: Option<(String, bool)>,
    },
    InGame,
    Paused {
        status_message: Option<String>,
    },
}

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
    pub respawn_point: Option<Vec3>,
    pub is_sleeping: bool,
    camera: Camera,
    camera_uniform: CameraUniform,
    pub day_night: DayNightCycle,
    targeted_block: Option<RaycastHit>,
    wireframe_buffer: Option<wgpu::Buffer>,
    // Screen & World Persistence State
    pub screen: AppScreen,
    pub saves_root: PathBuf,
    pub saved_worlds: Vec<WorldMetadata>,
    pub active_world_metadata: Option<WorldMetadata>,
    pub render_distance: i32,
    pub registry: Arc<BlockRegistry>,
    // Milestone 5: Inventory, Crafting, and UI
    pub inventory: Inventory,
    pub armor: blockgame_core::item::ArmorEquipment,
    pub active_hotbar_slot: usize,
    pub item_registry: Arc<ItemRegistry>,
    pub crafting_manager: CraftingManager,
    pub crafting_grid: CraftingGrid,
    pub crafting_result: Option<ItemSlot>,
    pub inventory_open: bool,
    pub active_chest_pos: Option<IVec3>,
    pub active_furnace_pos: Option<IVec3>,
    pub shift_down: bool,
    pub jei_search_text: String,
    pub jei_search_focused: bool,
    pub jei_page: usize,
    pub jei_recipe_modal: Option<JeiRecipeModalState>,
    pub cursor_held: ItemSlot,
    pub cursor_pos: [f32; 2],
    pub ui_pipeline: UiPipeline,
    pub ui_mesh: UiMesh,
    pub ui_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub item_drop_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub animal_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub monster_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub arrow_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub celestial_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub falling_block_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub player_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    // Multiplayer & 3D Player Model State
    pub mp_client: Option<blockgame_core::net::MultiplayerClient>,
    pub mp_hosted_server: Option<blockgame_core::net::MultiplayerServer>,
    pub local_username: String,
    pub local_skin_id: u8,
    pub third_person: bool,
    pub chat_open: bool,
    pub chat_input: String,
    pub tab_held: bool,
    pub net_sync_timer: f32,
    pub next_mp_drop_nonce: u32,
    pub local_chat_messages: Vec<blockgame_core::net::ChatEntry>,
    pub total_time: f32,
    pub walk_distance: f32,
    pub punch_timer: f32,
    pub last_footstep_dist: f32,
    pub break_hit_timer: f32,
    pub was_in_water: bool,
    pub underwater_sound_timer: f32,
    pub audio: crate::audio::AudioManager,
    pub music_scheduler: crate::audio::MusicScheduler,
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
    pub fn refresh_saved_worlds(&mut self) {
        self.saved_worlds = list_saved_worlds(&self.saves_root);
    }

    /// Spawns an item drop into the world. In multiplayer, automatically assigns a unique drop ID,
    /// spawns it locally with 0ms visual latency, and broadcasts `ClientPacket::SpawnItemDrop` to the server.
    pub fn spawn_drop_synced(&mut self, pos: Vec3, vel: Vec3, item: ItemSlot) {
        if item.is_empty() {
            return;
        }
        if let Some(client) = &mut self.mp_client {
            let drop_id = ((client.player_id as u64) << 40)
                | (self.next_mp_drop_nonce as u64 & 0x000000FFFFFFFFFF);
            self.next_mp_drop_nonce = self.next_mp_drop_nonce.wrapping_add(1);
            self.world_manager
                .spawn_item_drop_with_id_and_velocity(drop_id, pos, vel, item);
            client.send(&blockgame_core::net::ClientPacket::SpawnItemDrop {
                drop_id,
                position: pos,
                velocity: vel,
                item,
            });
        } else {
            self.world_manager
                .spawn_item_drop_with_velocity(pos, vel, item);
        }
    }

    pub fn current_local_player_model(&self) -> blockgame_core::net::PlayerModelState {
        let swing_anim = if self.punch_timer > 0.0 {
            1.0 - (self.punch_timer / 0.25)
        } else {
            0.0
        };
        blockgame_core::net::PlayerModelState {
            player_id: self
                .mp_client
                .as_ref()
                .map(|c| c.player_id)
                .unwrap_or(0),
            username: if self.local_username.is_empty() {
                "Explorer".to_string()
            } else {
                self.local_username.clone()
            },
            position: self.player.position,
            velocity: self.player.velocity,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            held_item: self.inventory.get(self.active_hotbar_slot).item_id,
            walk_anim: self.walk_distance,
            swing_anim,
            hurt_timer: 0.0,
            is_sneaking: self.shift_down && !self.inventory_open && !self.player.flying,
            skin_id: self.local_skin_id,
            health: self.player_stats.health,
            max_health: self.player_stats.max_health,
        }
    }

    pub fn disconnect_multiplayer(&mut self) {
        let model = self.current_local_player_model();
        let inv = self.inventory.clone();
        let hotbar = self.active_hotbar_slot;
        let stats = self.player_stats.clone();
        if let Some(mut client) = self.mp_client.take() {
            client.send(&blockgame_core::net::ClientPacket::PlayerStateUpdate {
                position: model.position,
                velocity: model.velocity,
                yaw: model.yaw,
                pitch: model.pitch,
                held_item: model.held_item,
                walk_anim: model.walk_anim,
                swing_anim: model.swing_anim,
                is_sneaking: model.is_sneaking,
                health: model.health,
            });
            client.send(&blockgame_core::net::ClientPacket::PlayerInventorySync {
                inventory: inv,
                active_hotbar_slot: hotbar,
                stats,
            });
            client.disconnect();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        if let Some(mut hosted) = self.mp_hosted_server.take() {
            hosted.shutdown();
        }
        self.chat_open = false;
        self.chat_input.clear();
        self.tab_held = false;
    }

    pub fn join_multiplayer_server(
        &mut self,
        username: &str,
        password: &str,
        addr: &str,
        skin_id: u8,
    ) -> Result<String, String> {
        self.disconnect_multiplayer();

        let (client, auth_pkt) =
            blockgame_core::net::MultiplayerClient::connect_and_authenticate(
                addr, username, password, skin_id,
            )?;

        let (seed, day_night_time, initial_state, assigned_skin, welcome_msg) =
            if let blockgame_core::net::ServerPacket::AuthResponse {
                seed,
                day_night_time,
                player_state,
                skin_id: assigned_skin,
                message,
                ..
            } = auth_pkt
            {
                (seed, day_night_time, player_state, assigned_skin, message)
            } else {
                return Err("Invalid authentication response packet".to_string());
            };

        let mut world_manager =
            WorldManager::new(seed, self.render_distance, self.registry.clone());
        world_manager.init_area_around(initial_state.position, &self.device, &self.pipeline);

        self.world_manager = world_manager;
        self.player = PlayerPhysics::new(initial_state.position);
        self.player.velocity = initial_state.velocity;
        self.player_stats = initial_state.stats;
        self.death_handled = self.player_stats.is_dead;
        self.inventory = initial_state.inventory;
        self.active_hotbar_slot = initial_state.active_hotbar_slot.min(8);
        self.camera.position = self.player.eye_position();
        self.camera.yaw = initial_state.yaw;
        self.camera.pitch = initial_state.pitch;
        self.day_night = DayNightCycle::new();
        self.day_night.time = day_night_time;
        self.total_time = 0.0;
        self.walk_distance = 0.0;
        self.last_footstep_dist = 0.0;
        self.break_hit_timer = 0.0;
        self.was_in_water = false;
        self.underwater_sound_timer = 0.0;
        self.punch_timer = 0.0;
        self.inventory_open = false;
        self.active_chest_pos = None;
        self.active_furnace_pos = None;
        self.jei_recipe_modal = None;
        self.jei_search_focused = false;
        self.cursor_held = ItemSlot::EMPTY;
        self.mouse_left_down = false;
        self.breaking_voxel = None;
        self.break_progress = 0.0;
        self.cracking_buffer = None;
        self.active_world_metadata = None;
        self.local_username = username.trim().to_string();
        self.local_skin_id = assigned_skin % 4;
        self.chat_open = false;
        self.chat_input.clear();
        self.tab_held = false;
        self.net_sync_timer = 0.0;
        self.mp_client = Some(client);

        self.screen = AppScreen::InGame;
        self.mouse_captured = !self.player_stats.is_dead;
        if self.mouse_captured {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
            self.window.set_cursor_visible(false);
        } else {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }

        Ok(welcome_msg)
    }

    pub fn host_and_play_multiplayer(
        &mut self,
        username: &str,
        password: &str,
        addr: &str,
        skin_id: u8,
    ) -> Result<String, String> {
        self.disconnect_multiplayer();

        let bind_addr = if addr.trim().is_empty() {
            format!("0.0.0.0:{}", blockgame_core::net::DEFAULT_SERVER_PORT)
        } else if !addr.contains(':') {
            format!("{}:{}", addr.trim(), blockgame_core::net::DEFAULT_SERVER_PORT)
        } else {
            addr.trim().to_string()
        };

        let saves_root = self.saves_root.clone();
        let seed = self.world_manager.generator.seed;
        let generator = Arc::new(blockgame_gen::TerrainGenerator::new(seed));

        let mut server = blockgame_core::net::MultiplayerServer::start(
            &bind_addr,
            saves_root,
            "Hosted_Multiplayer".to_string(),
            "Hosted Multiplayer World".to_string(),
            seed,
            None,
            move |pos: blockgame_core::chunk::ChunkPos| {
                let mut chunk = generator.generate_chunk(pos);
                generator.populate_chunk_features(&mut chunk);
                chunk
            },
        )
        .map_err(|e| format!("Failed to bind server on {}: {}", bind_addr, e))?;

        let port = server.local_addr().port();
        let connect_addr = format!("127.0.0.1:{}", port);

        match self.join_multiplayer_server(username, password, &connect_addr, skin_id) {
            Ok(msg) => {
                self.mp_hosted_server = Some(server);
                Ok(msg)
            }
            Err(e) => {
                server.shutdown();
                Err(e)
            }
        }
    }

    pub fn save_active_world(&mut self) -> bool {
        if self.mp_client.is_some() {
            let model = self.current_local_player_model();
            let inv = self.inventory.clone();
            let hotbar = self.active_hotbar_slot;
            let stats = self.player_stats.clone();
            if let Some(client) = &mut self.mp_client {
                client.send(&blockgame_core::net::ClientPacket::PlayerStateUpdate {
                    position: model.position,
                    velocity: model.velocity,
                    yaw: model.yaw,
                    pitch: model.pitch,
                    held_item: model.held_item,
                    walk_anim: model.walk_anim,
                    swing_anim: model.swing_anim,
                    is_sneaking: model.is_sneaking,
                    health: model.health,
                });
                client.send(&blockgame_core::net::ClientPacket::PlayerInventorySync {
                    inventory: inv,
                    active_hotbar_slot: hotbar,
                    stats,
                });
            }
            return true;
        }
        let Some(mut meta) = self.active_world_metadata.clone() else {
            return false;
        };
        meta.total_time = self.total_time;
        meta.day_night_time = self.day_night.time;

        let player_state = SavedPlayerState {
            position: self.player.position,
            velocity: self.player.velocity,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            stats: self.player_stats.clone(),
            inventory: self.inventory.clone(),
            active_hotbar_slot: self.active_hotbar_slot,
            rank: "default".to_string(),
        };

        let mut save_data = self.world_manager.capture_save_data(meta, player_state);
        match save_world_to_dir(&self.saves_root, &mut save_data) {
            Ok(path) => {
                log::info!(
                    "Saved world '{}' to {:?}",
                    save_data.metadata.name,
                    path
                );
                self.active_world_metadata = Some(save_data.metadata);
                true
            }
            Err(e) => {
                log::error!("Failed to save world: {:?}", e);
                false
            }
        }
    }

    pub fn create_and_start_world(&mut self, name_input: &str, seed_input: &str) {
        let display_name = if name_input.trim().is_empty() {
            "New World".to_string()
        } else {
            name_input.trim().to_string()
        };
        let seed = parse_or_hash_seed(seed_input);
        let folder_name = unique_world_folder_name(&self.saves_root, &display_name);

        let mut meta = WorldMetadata::new(&display_name, seed);
        meta.folder_name = folder_name;

        let mut world_manager =
            WorldManager::new(seed, self.render_distance, self.registry.clone());
        world_manager.init_spawn_area(&self.device, &self.pipeline);

        // Find safe surface spawn height at (0, 0)
        let spawn_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(0, y, 0));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };

        self.world_manager = world_manager;
        self.player = PlayerPhysics::new(Vec3::new(0.5, spawn_y, 0.5));
        self.player_stats = PlayerStats::new();
        self.death_handled = false;
        self.inventory = Inventory::starter_inventory();
        self.active_hotbar_slot = 0;
        self.camera.position = self.player.eye_position();
        self.camera.yaw = (-90.0_f32).to_radians();
        self.camera.pitch = (-10.0_f32).to_radians();
        self.day_night = DayNightCycle::new();
        self.total_time = 0.0;
        self.walk_distance = 0.0;
        self.last_footstep_dist = 0.0;
        self.break_hit_timer = 0.0;
        self.was_in_water = false;
        self.underwater_sound_timer = 0.0;
        self.punch_timer = 0.0;
        self.inventory_open = false;
        self.active_chest_pos = None;
        self.active_furnace_pos = None;
        self.jei_recipe_modal = None;
        self.jei_search_focused = false;
        self.cursor_held = ItemSlot::EMPTY;
        self.mouse_left_down = false;
        self.breaking_voxel = None;
        self.break_progress = 0.0;
        self.cracking_buffer = None;
        self.active_world_metadata = Some(meta);

        // Immediately save initial state to disk so the world folder exists
        self.save_active_world();

        self.screen = AppScreen::InGame;
        self.mouse_captured = true;
        let _ = self
            .window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined);
        self.window.set_cursor_visible(false);
    }

    pub fn load_and_start_world(&mut self, folder_name: &str) {
        let save_data = match load_world_from_dir(&self.saves_root, folder_name) {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to load world '{}': {:?}", folder_name, e);
                return;
            }
        };

        let seed = save_data.metadata.seed;
        let mut world_manager =
            WorldManager::new(seed, self.render_distance, self.registry.clone());
        world_manager.apply_save_data(&save_data);
        world_manager.init_area_around(
            save_data.player.position,
            &self.device,
            &self.pipeline,
        );

        self.world_manager = world_manager;
        self.player = PlayerPhysics::new(save_data.player.position);
        self.player.velocity = save_data.player.velocity;
        self.player_stats = save_data.player.stats;
        self.death_handled = self.player_stats.is_dead;
        self.inventory = save_data.player.inventory;
        self.active_hotbar_slot = save_data.player.active_hotbar_slot.min(8);
        self.camera.position = self.player.eye_position();
        self.camera.yaw = save_data.player.yaw;
        self.camera.pitch = save_data.player.pitch;
        self.day_night = DayNightCycle::new();
        self.day_night.time = save_data.metadata.day_night_time;
        self.total_time = save_data.metadata.total_time;
        self.walk_distance = 0.0;
        self.last_footstep_dist = 0.0;
        self.break_hit_timer = 0.0;
        self.was_in_water = false;
        self.underwater_sound_timer = 0.0;
        self.punch_timer = 0.0;
        self.inventory_open = false;
        self.active_chest_pos = None;
        self.active_furnace_pos = None;
        self.jei_recipe_modal = None;
        self.jei_search_focused = false;
        self.cursor_held = ItemSlot::EMPTY;
        self.mouse_left_down = false;
        self.breaking_voxel = None;
        self.break_progress = 0.0;
        self.cracking_buffer = None;
        self.active_world_metadata = Some(save_data.metadata);

        self.screen = AppScreen::InGame;
        self.mouse_captured = !self.player_stats.is_dead;
        if self.mouse_captured {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
            self.window.set_cursor_visible(false);
        } else {
            let _ = self
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::None);
            self.window.set_cursor_visible(true);
        }
    }

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
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
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
        let _ = self
            .window
            .set_cursor_grab(winit::window::CursorGrabMode::Confined);
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
            .with_title("BlockGame")
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

        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::AutoNoVsync)
        {
            wgpu::PresentMode::AutoNoVsync
        } else {
            wgpu::PresentMode::AutoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let (_depth_tex, depth_view) = create_depth_texture(&device, width, height);

        let atlas = TextureAtlas::generate_default();
        let pipeline = VoxelPipeline::new(&device, &queue, surface_format, &atlas);
        let wireframe_pipeline =
            WireframePipeline::new(&device, surface_format, &pipeline.camera_bind_group_layout);
        let ui_pipeline =
            UiPipeline::new(&device, surface_format, &pipeline.texture_bind_group_layout);

        let mut world_manager =
            WorldManager::new(self.seed, self.render_distance, self.registry.clone());
        world_manager.init_spawn_area(&device, &pipeline);

        // Spawn player atop the surface
        let spawn_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(0, y, 0));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };
        let player = PlayerPhysics::new(Vec3::new(0.5, spawn_y, 0.5));

        let camera = Camera::new(
            player.eye_position(),
            (-90.0_f32).to_radians(),
            (-10.0_f32).to_radians(),
        );

        let day_night = DayNightCycle::new();
        let mut camera_uniform = CameraUniform::default();
        camera.update_uniform_with_atmosphere(
            &mut camera_uniform,
            width as f32 / height as f32,
            &day_night,
        );
        pipeline.update_camera(&queue, &camera_uniform);

        let item_registry = Arc::new(ItemRegistry::new());
        let inventory = Inventory::starter_inventory();
        let crafting_manager = CraftingManager::new();
        let crafting_grid = CraftingGrid::new_2x2();

        let saves_root = PathBuf::from("saves");
        let saved_worlds = list_saved_worlds(&saves_root);

        // Ensure cursor is visible and unlocked on the Title Screen at launch
        let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
        window.set_cursor_visible(true);

        let audio = crate::audio::AudioManager::new();
        let music_scheduler = crate::audio::MusicScheduler::new(audio.music_track_count());

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
            respawn_point: None,
            is_sleeping: false,
            camera,
            camera_uniform,
            day_night,
            targeted_block: None,
            wireframe_buffer: None,
            screen: AppScreen::Title,
            saves_root,
            saved_worlds,
            active_world_metadata: None,
            render_distance: self.render_distance,
            registry: self.registry.clone(),
            inventory,
            armor: blockgame_core::item::ArmorEquipment::new(),
            active_hotbar_slot: 0,
            item_registry,
            crafting_manager,
            crafting_grid,
            crafting_result: None,
            inventory_open: false,
            active_chest_pos: None,
            active_furnace_pos: None,
            shift_down: false,
            jei_search_text: String::new(),
            jei_search_focused: false,
            jei_page: 0,
            jei_recipe_modal: None,
            cursor_held: ItemSlot::EMPTY,
            cursor_pos: [width as f32 * 0.5, height as f32 * 0.5],
            ui_pipeline,
            ui_mesh: UiMesh::new(),
            ui_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new("Dynamic UI Mesh"),
            item_drop_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Item Drop Mesh",
            ),
            animal_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new("Dynamic Animal Mesh"),
            monster_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new("Dynamic Monster Mesh"),
            arrow_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new("Dynamic Arrow Mesh"),
            celestial_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Celestial Mesh",
            ),
            falling_block_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Falling Block Mesh",
            ),
            player_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Player Model Mesh",
            ),
            mp_client: None,
            mp_hosted_server: None,
            local_username: "Player1".to_string(),
            local_skin_id: 0,
            third_person: false,
            chat_open: false,
            chat_input: String::new(),
            tab_held: false,
            net_sync_timer: 0.0,
            next_mp_drop_nonce: 1,
            local_chat_messages: Vec::new(),
            total_time: 0.0,
            walk_distance: 0.0,
            punch_timer: 0.0,
            last_footstep_dist: 0.0,
            break_hit_timer: 0.0,
            was_in_water: false,
            underwater_sound_timer: 0.0,
            audio,
            music_scheduler,
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
                if matches!(
                    state.screen,
                    AppScreen::InGame
                        | AppScreen::Paused { .. }
                        | AppScreen::Settings {
                            return_to_pause: true
                        }
                ) {
                    state.save_active_world();
                }
                state.disconnect_multiplayer();
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state
                        .surface
                        .configure(&state.device, &state.surface_config);
                    let (_depth_tex, depth_view) =
                        create_depth_texture(&state.device, new_size.width, new_size.height);
                    state.depth_view = depth_view;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_pos = [position.x as f32, position.y as f32];
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if state.screen != AppScreen::InGame {
                    return;
                }
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

                // Handle Title Screen, World Select, Create World, Settings, Multiplayer Lobby, and Pause Menu clicks
                if state.screen != AppScreen::InGame {
                    if pressed && button == MouseButton::Left {
                        let screen_w = state.surface_config.width as f32;
                        let screen_h = state.surface_config.height as f32;
                        let cpos = state.cursor_pos;

                        match state.screen.clone() {
                            AppScreen::Title => {
                                let rects = UiBuilder::title_screen_button_rects(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[0][0],
                                    rects[0][1],
                                    rects[0][2],
                                    rects[0][3],
                                ) {
                                    // Singleplayer
                                    state.refresh_saved_worlds();
                                    let sel = if state.saved_worlds.is_empty() {
                                        None
                                    } else {
                                        Some(0)
                                    };
                                    state.screen = AppScreen::WorldSelect {
                                        selected_index: sel,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[1][0],
                                    rects[1][1],
                                    rects[1][2],
                                    rects[1][3],
                                ) {
                                    // Multiplayer Lobby
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input: state.local_username.clone(),
                                        password_input: String::new(),
                                        address_input: "127.0.0.1:25565".to_string(),
                                        skin_id: state.local_skin_id,
                                        focused_field: 0,
                                        status_message: None,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[2][0],
                                    rects[2][1],
                                    rects[2][2],
                                    rects[2][3],
                                ) {
                                    // Settings
                                    state.screen = AppScreen::Settings {
                                        return_to_pause: false,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[3][0],
                                    rects[3][1],
                                    rects[3][2],
                                    rects[3][3],
                                ) {
                                    // Quit Game
                                    event_loop.exit();
                                }
                            }
                            AppScreen::WorldSelect { selected_index } => {
                                let (rows, btns) = UiBuilder::world_select_layout(
                                    screen_w,
                                    screen_h,
                                    state.saved_worlds.len(),
                                );
                                for (i, r) in rows.iter().enumerate() {
                                    if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3]) {
                                        if selected_index == Some(i) {
                                            // Clicking already-selected world directly launches it
                                            let folder = state.saved_worlds[i].folder_name.clone();
                                            state.load_and_start_world(&folder);
                                            return;
                                        } else {
                                            state.screen = AppScreen::WorldSelect {
                                                selected_index: Some(i),
                                            };
                                            return;
                                        }
                                    }
                                }
                                let has_sel = selected_index
                                    .map(|idx| idx < state.saved_worlds.len())
                                    .unwrap_or(false);
                                if has_sel
                                    && UiBuilder::is_rect_hovered(
                                        cpos,
                                        btns[0][0],
                                        btns[0][1],
                                        btns[0][2],
                                        btns[0][3],
                                    )
                                {
                                    // Play Selected World
                                    if let Some(idx) = selected_index {
                                        let folder = state.saved_worlds[idx].folder_name.clone();
                                        state.load_and_start_world(&folder);
                                    }
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    btns[1][0],
                                    btns[1][1],
                                    btns[1][2],
                                    btns[1][3],
                                ) {
                                    // Create New World
                                    state.screen = AppScreen::CreateWorld {
                                        name_input: "New World".to_string(),
                                        seed_input: String::new(),
                                        focused_field: 0,
                                    };
                                } else if has_sel
                                    && UiBuilder::is_rect_hovered(
                                        cpos,
                                        btns[2][0],
                                        btns[2][1],
                                        btns[2][2],
                                        btns[2][3],
                                    )
                                {
                                    // Delete World
                                    if let Some(idx) = selected_index {
                                        let folder = state.saved_worlds[idx].folder_name.clone();
                                        let _ = delete_saved_world(&state.saves_root, &folder);
                                        state.refresh_saved_worlds();
                                        let next_sel = if state.saved_worlds.is_empty() {
                                            None
                                        } else {
                                            Some(idx.min(state.saved_worlds.len() - 1))
                                        };
                                        state.screen = AppScreen::WorldSelect {
                                            selected_index: next_sel,
                                        };
                                    }
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    btns[3][0],
                                    btns[3][1],
                                    btns[3][2],
                                    btns[3][3],
                                ) {
                                    // Back
                                    state.screen = AppScreen::Title;
                                }
                            }
                            AppScreen::CreateWorld {
                                name_input,
                                seed_input,
                                focused_field,
                            } => {
                                let (name_r, seed_r, create_r, cancel_r) =
                                    UiBuilder::create_world_layout(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    name_r[0],
                                    name_r[1],
                                    name_r[2],
                                    name_r[3],
                                ) {
                                    state.screen = AppScreen::CreateWorld {
                                        name_input,
                                        seed_input,
                                        focused_field: 0,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    seed_r[0],
                                    seed_r[1],
                                    seed_r[2],
                                    seed_r[3],
                                ) {
                                    state.screen = AppScreen::CreateWorld {
                                        name_input,
                                        seed_input,
                                        focused_field: 1,
                                    };
                                } else if !name_input.trim().is_empty()
                                    && UiBuilder::is_rect_hovered(
                                        cpos,
                                        create_r[0],
                                        create_r[1],
                                        create_r[2],
                                        create_r[3],
                                    )
                                {
                                    state.create_and_start_world(&name_input, &seed_input);
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    cancel_r[0],
                                    cancel_r[1],
                                    cancel_r[2],
                                    cancel_r[3],
                                ) {
                                    state.refresh_saved_worlds();
                                    let sel = if state.saved_worlds.is_empty() {
                                        None
                                    } else {
                                        Some(0)
                                    };
                                    state.screen = AppScreen::WorldSelect {
                                        selected_index: sel,
                                    };
                                } else {
                                    let _ = focused_field;
                                }
                            }
                            AppScreen::Paused { .. } => {
                                let rects = UiBuilder::pause_menu_button_rects(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[0][0],
                                    rects[0][1],
                                    rects[0][2],
                                    rects[0][3],
                                ) {
                                    // Back to Game
                                    state.screen = AppScreen::InGame;
                                    state.mouse_captured = true;
                                    let _ = state
                                        .window
                                        .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                    state.window.set_cursor_visible(false);
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[1][0],
                                    rects[1][1],
                                    rects[1][2],
                                    rects[1][3],
                                ) {
                                    // Save World
                                    let ok = state.save_active_world();
                                    state.screen = AppScreen::Paused {
                                        status_message: Some(
                                            if ok {
                                                if state.mp_client.is_some() {
                                                    "Synced player state with server!".to_string()
                                                } else {
                                                    "World saved successfully!".to_string()
                                                }
                                            } else {
                                                "Failed to save world.".to_string()
                                            },
                                        ),
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[2][0],
                                    rects[2][1],
                                    rects[2][2],
                                    rects[2][3],
                                ) {
                                    // Settings
                                    state.screen = AppScreen::Settings {
                                        return_to_pause: true,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[3][0],
                                    rects[3][1],
                                    rects[3][2],
                                    rects[3][3],
                                ) {
                                    // Save & Quit to Title
                                    state.save_active_world();
                                    state.disconnect_multiplayer();
                                    state.active_world_metadata = None;
                                    // Ensure all chunks around the panorama center are loaded on GPU
                                    state.world_manager.init_area_around(
                                        state.player.position,
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    state.screen = AppScreen::Title;
                                    state.mouse_captured = false;
                                    let _ = state
                                        .window
                                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                                    state.window.set_cursor_visible(true);
                                }
                            }
                            AppScreen::Settings { return_to_pause } => {
                                let back_r = UiBuilder::notice_back_button_rect(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    back_r[0],
                                    back_r[1],
                                    back_r[2],
                                    back_r[3],
                                ) {
                                    state.screen = if return_to_pause {
                                        AppScreen::Paused {
                                            status_message: None,
                                        }
                                    } else {
                                        AppScreen::Title
                                    };
                                }
                            }
                            AppScreen::MultiplayerLobby {
                                username_input,
                                password_input,
                                address_input,
                                skin_id,
                                focused_field,
                                status_message,
                            } => {
                                let (user_r, pass_r, addr_r, [skin_r, join_r, host_r, back_r]) =
                                    UiBuilder::multiplayer_lobby_layout(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos, user_r[0], user_r[1], user_r[2], user_r[3],
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
                                        skin_id,
                                        focused_field: 0,
                                        status_message,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, pass_r[0], pass_r[1], pass_r[2], pass_r[3],
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
                                        skin_id,
                                        focused_field: 1,
                                        status_message,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, addr_r[0], addr_r[1], addr_r[2], addr_r[3],
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
                                        skin_id,
                                        focused_field: 2,
                                        status_message,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, skin_r[0], skin_r[1], skin_r[2], skin_r[3],
                                ) {
                                    let next_skin = (skin_id + 1) % 4;
                                    state.local_skin_id = next_skin;
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
                                        skin_id: next_skin,
                                        focused_field,
                                        status_message,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, join_r[0], join_r[1], join_r[2], join_r[3],
                                ) {
                                    if let Err(err) = state.join_multiplayer_server(
                                        &username_input,
                                        &password_input,
                                        &address_input,
                                        skin_id,
                                    ) {
                                        state.screen = AppScreen::MultiplayerLobby {
                                            username_input,
                                            password_input,
                                            address_input,
                                            skin_id,
                                            focused_field,
                                            status_message: Some((err, true)),
                                        };
                                    }
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, host_r[0], host_r[1], host_r[2], host_r[3],
                                ) {
                                    if let Err(err) = state.host_and_play_multiplayer(
                                        &username_input,
                                        &password_input,
                                        &address_input,
                                        skin_id,
                                    ) {
                                        state.screen = AppScreen::MultiplayerLobby {
                                            username_input,
                                            password_input,
                                            address_input,
                                            skin_id,
                                            focused_field,
                                            status_message: Some((err, true)),
                                        };
                                    }
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, back_r[0], back_r[1], back_r[2], back_r[3],
                                ) {
                                    state.screen = AppScreen::Title;
                                }
                            }
                            AppScreen::InGame => {}
                        }
                    }
                    return;
                }

                // If player is dead, left click on Respawn button respawns
                if state.player_stats.is_dead {
                    if pressed && button == MouseButton::Left {
                        let screen_w = state.surface_config.width as f32;
                        let screen_h = state.surface_config.height as f32;
                        let btn_w = 200.0;
                        let btn_h = 44.0;
                        let btn_x = (screen_w - btn_w) * 0.5;
                        let btn_y = screen_h * 0.50;
                        if state.cursor_pos[0] >= btn_x
                            && state.cursor_pos[0] <= btn_x + btn_w
                            && state.cursor_pos[1] >= btn_y
                            && state.cursor_pos[1] <= btn_y + btn_h
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
                        if state.jei_recipe_modal.is_some() {
                            handle_recipe_modal_click(state, is_right);
                            return;
                        }
                        if handle_jei_panel_click(state, is_right) {
                            return;
                        }
                        if state.active_furnace_pos.is_some() {
                            handle_furnace_click(state, is_right);
                            if let (Some(pos), Some(client)) =
                                (state.active_furnace_pos, &mut state.mp_client)
                            {
                                if let Some(f) = state
                                    .world_manager
                                    .world()
                                    .lock()
                                    .unwrap()
                                    .get_furnace(pos)
                                    .cloned()
                                {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::ContainerUpdate {
                                            pos,
                                            is_furnace: true,
                                            slots: vec![f.input, f.fuel, f.output],
                                            burn_time: f.burn_time_remaining,
                                            max_burn_time: f.max_burn_time,
                                            cook_progress: f.cook_progress,
                                        },
                                    );
                                }
                            }
                        } else if state.active_chest_pos.is_some() {
                            handle_chest_click(state, is_right);
                            if let (Some(pos), Some(client)) =
                                (state.active_chest_pos, &mut state.mp_client)
                            {
                                if let Some(slots) = state
                                    .world_manager
                                    .world()
                                    .lock()
                                    .unwrap()
                                    .get_chest(pos)
                                    .cloned()
                                {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::ContainerUpdate {
                                            pos,
                                            is_furnace: false,
                                            slots,
                                            burn_time: 0.0,
                                            max_burn_time: 0.0,
                                            cook_progress: 0.0,
                                        },
                                    );
                                }
                            }
                        } else {
                            handle_inventory_click(state, is_right);
                        }
                    }
                    return;
                }

                if !state.mouse_captured {
                    if pressed {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                        state.window.set_cursor_visible(false);
                    }
                    return;
                }

                match button {
                    MouseButton::Left => {
                        if pressed {
                            state.punch_timer = 0.25; // Trigger punch animation
                            let held_slot = state.inventory.get(state.active_hotbar_slot);
                            let attack_damage =
                                if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                    match (def.tool_type, def.tool_tier) {
                                        (Some(blockgame_core::tool::ToolType::Sword), Some(tier)) => match tier {
                                            ToolTier::Wood | ToolTier::Gold => 4.0,
                                            ToolTier::Stone => 5.0,
                                            ToolTier::Copper => 5.5,
                                            ToolTier::Iron => 6.0,
                                            ToolTier::Diamond => 7.0,
                                            ToolTier::None => 1.0,
                                        },
                                        (Some(blockgame_core::tool::ToolType::Axe), Some(tier)) => match tier {
                                            ToolTier::Wood | ToolTier::Gold => 3.0,
                                            ToolTier::Stone => 4.0,
                                            ToolTier::Copper => 4.5,
                                            ToolTier::Iron => 5.0,
                                            ToolTier::Diamond => 6.0,
                                            ToolTier::None => 1.0,
                                        },
                                        (Some(blockgame_core::tool::ToolType::Pickaxe), Some(tier)) => match tier {
                                            ToolTier::Wood | ToolTier::Gold => 2.0,
                                            ToolTier::Stone => 3.0,
                                            ToolTier::Copper => 3.5,
                                            ToolTier::Iron => 4.0,
                                            ToolTier::Diamond => 5.0,
                                            ToolTier::None => 1.0,
                                        },
                                        (Some(blockgame_core::tool::ToolType::Shovel), Some(tier)) => match tier {
                                            ToolTier::Wood | ToolTier::Gold => 1.5,
                                            ToolTier::Stone => 2.0,
                                            ToolTier::Copper => 2.5,
                                            ToolTier::Iron => 3.0,
                                            ToolTier::Diamond => 4.0,
                                            ToolTier::None => 1.0,
                                        },
                                        _ => 1.0,
                                    }
                                } else {
                                    1.0
                                };

                            // Check PvP hit against remote multiplayer players within 3.5 blocks
                            let mut hit_remote_player = None;
                            if let Some(client) = &state.mp_client {
                                let ray_origin = state.player.eye_position();
                                let ray_dir = state.camera.forward();
                                let mut best_dist = 3.6_f32;
                                for (&pid, rp) in &client.remote_players {
                                    if rp.health <= 0.0 {
                                        continue;
                                    }
                                    let p_center = rp.position + Vec3::new(0.0, 0.9, 0.0);
                                    let to_p = p_center - ray_origin;
                                    let dist = to_p.length();
                                    if dist <= best_dist && dist > 0.1 {
                                        let dir_to_p = to_p / dist;
                                        if ray_dir.dot(dir_to_p) > 0.85 {
                                            best_dist = dist;
                                            hit_remote_player = Some(pid);
                                        }
                                    }
                                }
                            }

                            if let Some(target_pid) = hit_remote_player {
                                if let Some(client) = &mut state.mp_client {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::AttackPlayer {
                                            target_player_id: target_pid,
                                            damage: attack_damage,
                                        },
                                    );
                                }
                                state.audio.play_hurt();
                                state.mouse_left_down = false;
                                state.breaking_voxel = None;
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;

                                let mut held = state.inventory.get(state.active_hotbar_slot);
                                if let Some(def) = state.item_registry.get(held.item_id) {
                                    if let Some(max_dur) = def.durability {
                                        let dmg = if def.tool_type
                                            == Some(blockgame_core::tool::ToolType::Sword)
                                        {
                                            1
                                        } else {
                                            2
                                        };
                                        if held.damage_tool(dmg, max_dur) {
                                            state.audio.play_tool_break();
                                        }
                                        state.inventory.set(state.active_hotbar_slot, held);
                                    }
                                }
                            } else {
                                let hit_monster = state.world_manager.attack_monster(
                                    state.player.eye_position(),
                                    state.camera.forward(),
                                    3.5,
                                    attack_damage,
                                );
                                let hit_entity = hit_monster
                                    || state.world_manager.attack_animal(
                                        state.player.eye_position(),
                                        state.camera.forward(),
                                        3.5,
                                        attack_damage,
                                    );
                                if hit_entity {
                                    state.mouse_left_down = false;
                                    state.breaking_voxel = None;
                                    state.break_progress = 0.0;
                                    state.cracking_buffer = None;

                                    let mut held = state.inventory.get(state.active_hotbar_slot);
                                    if let Some(def) = state.item_registry.get(held.item_id) {
                                        if let Some(max_dur) = def.durability {
                                            let dmg = if def.tool_type
                                                == Some(blockgame_core::tool::ToolType::Sword)
                                            {
                                                1
                                            } else {
                                                2
                                            };
                                            if held.damage_tool(dmg, max_dur) {
                                                state.audio.play_tool_break();
                                            }
                                            state.inventory.set(state.active_hotbar_slot, held);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    MouseButton::Right if pressed => {
                        // 1. Check if targeting an interactive block (e.g. Furnace, Chest, or Crafting Table)
                        if let Some(hit) = state.targeted_block {
                            let block_id = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard.get_block(hit.hit_voxel)
                            };
                            if block_id == BlockId::FURNACE || block_id == BlockId::FURNACE_LIT {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;

                                {
                                    let mut world_guard =
                                        state.world_manager.world().lock().unwrap();
                                    world_guard.get_or_create_furnace(hit.hit_voxel);
                                }

                                state.active_chest_pos = None;
                                state.active_furnace_pos = Some(hit.hit_voxel);
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id == BlockId::CHEST {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;

                                {
                                    let mut world_guard =
                                        state.world_manager.world().lock().unwrap();
                                    world_guard.get_or_create_chest(hit.hit_voxel);
                                }

                                state.active_furnace_pos = None;
                                state.active_chest_pos = Some(hit.hit_voxel);
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id == BlockId::CRAFTING_TABLE {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;

                                // Return any items left in current grid
                                let grid_items: Vec<_> =
                                    state.crafting_grid.slots.drain(..).collect();
                                for slot in grid_items {
                                    if !slot.is_empty() {
                                        if let Some(rem) =
                                            state.inventory.add_item(slot, &state.item_registry)
                                        {
                                            state.spawn_drop_synced(
                                                state.player.position,
                                                Vec3::new(0.0, 2.0, 0.0),
                                                rem,
                                            );
                                        }
                                    }
                                }

                                state.active_chest_pos = None;
                                state.active_furnace_pos = None;
                                state.crafting_grid = CraftingGrid::new_3x3();
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id.is_door() {
                                let next_block = block_id.toggled_door();
                                let pair_pos = if block_id.is_door_top() {
                                    hit.hit_voxel - IVec3::Y
                                } else {
                                    hit.hit_voxel + IVec3::Y
                                };
                                let pair_block = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    world_guard.get_block(pair_pos)
                                };
                                state.world_manager.apply_remote_block_update(
                                    hit.hit_voxel,
                                    next_block,
                                    &state.device,
                                    &state.pipeline,
                                );
                                if pair_block.is_door() {
                                    let next_pair = pair_block.toggled_door();
                                    state.world_manager.apply_remote_block_update(
                                        pair_pos,
                                        next_pair,
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::BlockChange {
                                                pos: pair_pos,
                                                block_id: next_pair,
                                            },
                                        );
                                    }
                                }
                                if let Some(client) = &mut state.mp_client {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::BlockChange {
                                            pos: hit.hit_voxel,
                                            block_id: next_block,
                                        },
                                    );
                                }
                                state.audio.play_block_place(
                                    next_block,
                                    hit.hit_voxel.as_vec3() + Vec3::splat(0.5),
                                    state.camera.position,
                                    state.camera.yaw,
                                );
                                state.punch_timer = 0.15;
                                return;
                            }
                            if block_id.is_bed() {
                                state.respawn_point = Some(Vec3::new(
                                    hit.hit_voxel.x as f32 + 0.5,
                                    hit.hit_voxel.y as f32 + 0.65,
                                    hit.hit_voxel.z as f32 + 0.5,
                                ));
                                let is_night = state.day_night.sun_height() <= 0.05;
                                if let Some(client) = &mut state.mp_client {
                                    if is_night {
                                        state.is_sleeping = !state.is_sleeping;
                                        client.send(&blockgame_core::net::ClientPacket::SetSleeping {
                                            sleeping: state.is_sleeping,
                                        });
                                        let msg = if state.is_sleeping {
                                            "You are sleeping in bed. Move to wake up."
                                        } else {
                                            "You left the bed."
                                        };
                                        state.local_chat_messages.push(
                                            blockgame_core::net::ChatEntry {
                                                sender: "System".to_string(),
                                                message: msg.to_string(),
                                                is_system: true,
                                                age: 0.0,
                                            },
                                        );
                                    } else {
                                        state.local_chat_messages.push(
                                            blockgame_core::net::ChatEntry {
                                                sender: "System".to_string(),
                                                message: "You can only sleep at night. Respawn point set.".to_string(),
                                                is_system: true,
                                                age: 0.0,
                                            },
                                        );
                                    }
                                } else if is_night {
                                    state.day_night.set_time(1000.0);
                                    state.player_stats.health = (state.player_stats.health + 4.0)
                                        .min(state.player_stats.max_health);
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: "System".to_string(),
                                            message: "You slept through the night. Good morning!".to_string(),
                                            is_system: true,
                                            age: 0.0,
                                        },
                                    );
                                } else {
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: "System".to_string(),
                                            message: "You can only sleep at night. Respawn point set.".to_string(),
                                            is_system: true,
                                            age: 0.0,
                                        },
                                    );
                                }
                                if state.local_chat_messages.len() > 25 {
                                    state.local_chat_messages.remove(0);
                                }
                                state.punch_timer = 0.15;
                                return;
                            }
                        }

                        let held_slot = state.inventory.get(state.active_hotbar_slot);

                        // 1b. Bow Firing (consumes 1 Arrow from inventory)
                        if held_slot.item_id == ItemId::BOW {
                            let mut arrow_slot_idx = None;
                            for idx in 0..state.inventory.len() {
                                let s = state.inventory.get(idx);
                                if s.item_id == ItemId::ARROW && s.count > 0 {
                                    arrow_slot_idx = Some(idx);
                                    break;
                                }
                            }
                            if let Some(idx) = arrow_slot_idx {
                                let mut arrow_slot = state.inventory.get(idx);
                                arrow_slot.take(1);
                                state.inventory.set(idx, arrow_slot);

                                let mut bow_slot = state.inventory.get(state.active_hotbar_slot);
                                if let Some(def) = state.item_registry.get(ItemId::BOW) {
                                    if let Some(max_dur) = def.durability {
                                        if bow_slot.damage_tool(1, max_dur) {
                                            state.audio.play_tool_break();
                                        }
                                        state.inventory.set(state.active_hotbar_slot, bow_slot);
                                    }
                                }

                                let dir = state.camera.forward().normalize_or_zero();
                                let spawn_pos = state.player.eye_position() + dir * 0.45;
                                state.world_manager.spawn_player_arrow(spawn_pos, dir * 28.0, 6.0);
                                state.punch_timer = 0.15;
                            }
                            return;
                        }

                        // 1c. Quick-Equip Armor on Right-Click
                        if !held_slot.is_empty() {
                            if let Some((armor_slot, _)) = state.item_registry.get_armor(held_slot.item_id) {
                                let idx = armor_slot.index();
                                let prev = std::mem::replace(&mut state.armor.slots[idx], held_slot);
                                state.inventory.set(state.active_hotbar_slot, prev);
                                state.punch_timer = 0.12;
                                return;
                            }
                        }

                        // 1d. Animal Breeding: Right-click an animal while holding Wheat
                        if held_slot.item_id == ItemId::WHEAT {
                            if state.world_manager.feed_animal(
                                state.player.eye_position(),
                                state.camera.forward(),
                                4.0,
                            ) {
                                let mut slot = state.inventory.get(state.active_hotbar_slot);
                                slot.take(1);
                                state.inventory.set(state.active_hotbar_slot, slot);
                                state.punch_timer = 0.15;
                                return;
                            }
                        }

                        // 2. Hoe Tilling (Grass or Dirt with Hoe -> Farmland)
                        if !held_slot.is_empty() {
                            if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                if def.tool_type == Some(blockgame_core::tool::ToolType::Hoe) {
                                    if let Some(hit) = state.targeted_block {
                                        if hit.hit_face
                                            == blockgame_core::direction::Direction::PosY
                                        {
                                            let tilled = state.world_manager.till_block(
                                                hit.hit_voxel,
                                                &state.device,
                                                &state.pipeline,
                                            );
                                            if tilled {
                                                if let Some(client) = &mut state.mp_client {
                                                    let new_b = state
                                                        .world_manager
                                                        .world()
                                                        .lock()
                                                        .unwrap()
                                                        .get_block(hit.hit_voxel);
                                                    client.send(
                                                        &blockgame_core::net::ClientPacket::BlockChange {
                                                            pos: hit.hit_voxel,
                                                            block_id: new_b,
                                                        },
                                                    );
                                                }
                                                state.punch_timer = 0.2;
                                                let mut held =
                                                    state.inventory.get(state.active_hotbar_slot);
                                                if let Some(def) =
                                                    state.item_registry.get(held.item_id)
                                                {
                                                    if let Some(max_dur) = def.durability {
                                                        if held.damage_tool(1, max_dur) {
                                                            state.audio.play_tool_break();
                                                        }
                                                        state.inventory.set(
                                                            state.active_hotbar_slot,
                                                            held,
                                                        );
                                                    }
                                                }
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
                                if block_beneath.is_farmland()
                                    && hit.hit_face == blockgame_core::direction::Direction::PosY
                                {
                                    let planted = state.world_manager.place_block(
                                        hit.place_voxel,
                                        BlockId::WHEAT_0,
                                        &state.player.aabb(),
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    if planted {
                                        if let Some(client) = &mut state.mp_client {
                                            client.send(
                                                &blockgame_core::net::ClientPacket::BlockChange {
                                                    pos: hit.place_voxel,
                                                    block_id: BlockId::WHEAT_0,
                                                },
                                            );
                                        }
                                        state.audio.play_block_place(
                                            BlockId::WHEAT_0,
                                            hit.place_voxel.as_vec3() + Vec3::splat(0.5),
                                            state.camera.position,
                                            state.camera.yaw,
                                        );
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

                        // 4b. Empty Bucket: Scoop Water
                        if held_slot.item_id == ItemId::BUCKET {
                            if let Some(hit) = state.targeted_block {
                                let is_water = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    world_guard.get_block(hit.hit_voxel).is_water()
                                };
                                if is_water {
                                    let scooped = state.world_manager.scoop_water(
                                        hit.hit_voxel,
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    if scooped {
                                        if let Some(client) = &mut state.mp_client {
                                            client.send(
                                                &blockgame_core::net::ClientPacket::BlockChange {
                                                    pos: hit.hit_voxel,
                                                    block_id: BlockId::AIR,
                                                },
                                            );
                                        }
                                        state.punch_timer = 0.2;
                                        if held_slot.count <= 1 {
                                            state.inventory.set(
                                                state.active_hotbar_slot,
                                                ItemSlot::new(ItemId::WATER_BUCKET, 1),
                                            );
                                        } else {
                                            state.inventory.slots[state.active_hotbar_slot].take(1);
                                            let overflow = state.inventory.add_item(
                                                ItemSlot::new(ItemId::WATER_BUCKET, 1),
                                                &state.item_registry,
                                            );
                                            if let Some(left) = overflow {
                                                state.spawn_drop_synced(
                                                    state.player.position,
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    left,
                                                );
                                            }
                                        }
                                        return;
                                    }
                                }
                            }
                        }

                        // 4c. Water Bucket: Place Water Source
                        if held_slot.item_id == ItemId::WATER_BUCKET {
                            if let Some(hit) = state.targeted_block {
                                let target_pos = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    let hit_b = world_guard.get_block(hit.hit_voxel);
                                    if hit_b.is_fragile() {
                                        hit.hit_voxel
                                    } else {
                                        hit.place_voxel
                                    }
                                };
                                let placed = state.world_manager.place_water(
                                    target_pos,
                                    &state.device,
                                    &state.pipeline,
                                );
                                if placed {
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::BlockChange {
                                                pos: target_pos,
                                                block_id: BlockId::WATER,
                                            },
                                        );
                                    }
                                    state.audio.play_block_place(
                                        BlockId::WATER,
                                        target_pos.as_vec3() + Vec3::splat(0.5),
                                        state.camera.position,
                                        state.camera.yaw,
                                    );
                                    state.punch_timer = 0.2;
                                    state.inventory.set(
                                        state.active_hotbar_slot,
                                        ItemSlot::new(ItemId::BUCKET, 1),
                                    );
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
                                        let target_is_water = {
                                            let world_guard =
                                                state.world_manager.world().lock().unwrap();
                                            world_guard.get_block(hit.hit_voxel).is_water()
                                        };
                                        let target_pos =
                                            if target_is_water && block_id != BlockId::LILY_PAD {
                                                hit.hit_voxel
                                            } else {
                                                hit.place_voxel
                                            };
                                        let dx = state.player.position.x - (target_pos.x as f32 + 0.5);
                                        let dz = state.player.position.z - (target_pos.z as f32 + 0.5);
                                        let facing = if dx.abs() > dz.abs() {
                                            if dx > 0.0 { 2 } else { 3 }
                                        } else if dz > 0.0 {
                                            0
                                        } else {
                                            1
                                        };
                                        let placed = state.world_manager.place_block_with_facing(
                                            target_pos,
                                            block_id,
                                            facing,
                                            &state.player.aabb(),
                                            &state.device,
                                            &state.pipeline,
                                        );
                                        if placed {
                                            if let Some(client) = &mut state.mp_client {
                                                let actual_block = state
                                                    .world_manager
                                                    .world()
                                                    .lock()
                                                    .unwrap()
                                                    .get_block(target_pos);
                                                client.send(
                                                    &blockgame_core::net::ClientPacket::BlockChange {
                                                        pos: target_pos,
                                                        block_id: actual_block,
                                                    },
                                                );
                                                if actual_block == BlockId::OAK_DOOR_CLOSED {
                                                    client.send(
                                                        &blockgame_core::net::ClientPacket::BlockChange {
                                                            pos: target_pos + IVec3::Y,
                                                            block_id: BlockId::OAK_DOOR_TOP_CLOSED,
                                                        },
                                                    );
                                                }
                                            }
                                            state.audio.play_block_place(
                                                block_id,
                                                target_pos.as_vec3() + Vec3::splat(0.5),
                                                state.camera.position,
                                                state.camera.yaw,
                                            );
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
                        text,
                        ..
                    },
                ..
            } => {
                let pressed = key_state == ElementState::Pressed;

                // Handle keyboard input on non-gameplay screens (CreateWorld, MultiplayerLobby, etc.)
                if state.screen != AppScreen::InGame {
                    if !pressed {
                        return;
                    }
                    match state.screen.clone() {
                        AppScreen::CreateWorld {
                            mut name_input,
                            mut seed_input,
                            mut focused_field,
                        } => {
                            if key == KeyCode::Tab {
                                focused_field = (focused_field + 1) % 2;
                                state.screen = AppScreen::CreateWorld {
                                    name_input,
                                    seed_input,
                                    focused_field,
                                };
                            } else if key == KeyCode::Escape {
                                state.refresh_saved_worlds();
                                let sel = if state.saved_worlds.is_empty() {
                                    None
                                } else {
                                    Some(0)
                                };
                                state.screen = AppScreen::WorldSelect {
                                    selected_index: sel,
                                };
                            } else if key == KeyCode::Enter {
                                if !name_input.trim().is_empty() {
                                    state.create_and_start_world(&name_input, &seed_input);
                                }
                            } else if key == KeyCode::Backspace {
                                if focused_field == 0 {
                                    name_input.pop();
                                } else {
                                    seed_input.pop();
                                }
                                state.screen = AppScreen::CreateWorld {
                                    name_input,
                                    seed_input,
                                    focused_field,
                                };
                            } else if let Some(t) = text {
                                for ch in t.chars() {
                                    if !ch.is_control() {
                                        if focused_field == 0 && name_input.len() < 32 {
                                            name_input.push(ch);
                                        } else if focused_field == 1 && seed_input.len() < 32 {
                                            seed_input.push(ch);
                                        }
                                    }
                                }
                                state.screen = AppScreen::CreateWorld {
                                    name_input,
                                    seed_input,
                                    focused_field,
                                };
                            }
                        }
                        AppScreen::Paused { .. } => {
                            if key == KeyCode::Escape {
                                state.screen = AppScreen::InGame;
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
                        }
                        AppScreen::WorldSelect { selected_index } => {
                            if key == KeyCode::Escape {
                                state.screen = AppScreen::Title;
                            } else if key == KeyCode::Enter {
                                if let Some(idx) = selected_index {
                                    if idx < state.saved_worlds.len() {
                                        let folder = state.saved_worlds[idx].folder_name.clone();
                                        state.load_and_start_world(&folder);
                                    }
                                }
                            }
                        }
                        AppScreen::Settings { return_to_pause } => {
                            if key == KeyCode::Escape {
                                state.screen = if return_to_pause {
                                    AppScreen::Paused {
                                        status_message: None,
                                    }
                                } else {
                                    AppScreen::Title
                                };
                            }
                        }
                        AppScreen::MultiplayerLobby {
                            mut username_input,
                            mut password_input,
                            mut address_input,
                            skin_id,
                            mut focused_field,
                            status_message,
                        } => {
                            if key == KeyCode::Tab {
                                focused_field = (focused_field + 1) % 3;
                                state.screen = AppScreen::MultiplayerLobby {
                                    username_input,
                                    password_input,
                                    address_input,
                                    skin_id,
                                    focused_field,
                                    status_message,
                                };
                            } else if key == KeyCode::Escape {
                                state.screen = AppScreen::Title;
                            } else if key == KeyCode::Enter {
                                if let Err(err) = state.join_multiplayer_server(
                                    &username_input,
                                    &password_input,
                                    &address_input,
                                    skin_id,
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
                                        skin_id,
                                        focused_field,
                                        status_message: Some((err, true)),
                                    };
                                }
                            } else if key == KeyCode::Backspace {
                                match focused_field {
                                    0 => {
                                        username_input.pop();
                                        state.local_username = username_input.clone();
                                    }
                                    1 => {
                                        password_input.pop();
                                    }
                                    _ => {
                                        address_input.pop();
                                    }
                                }
                                state.screen = AppScreen::MultiplayerLobby {
                                    username_input,
                                    password_input,
                                    address_input,
                                    skin_id,
                                    focused_field,
                                    status_message: None,
                                };
                            } else if let Some(t) = text {
                                for ch in t.chars() {
                                    if !ch.is_control() {
                                        match focused_field {
                                            0 => {
                                                if (ch.is_ascii_alphanumeric() || ch == '_')
                                                    && username_input.len() < 16
                                                {
                                                    username_input.push(ch);
                                                    state.local_username = username_input.clone();
                                                }
                                            }
                                            1 => {
                                                if password_input.len() < 32 {
                                                    password_input.push(ch);
                                                }
                                            }
                                            _ => {
                                                if address_input.len() < 48 {
                                                    address_input.push(ch);
                                                }
                                            }
                                        }
                                    }
                                }
                                state.screen = AppScreen::MultiplayerLobby {
                                    username_input,
                                    password_input,
                                    address_input,
                                    skin_id,
                                    focused_field,
                                    status_message: None,
                                };
                            }
                        }
                        AppScreen::Title | AppScreen::InGame => {}
                    }
                    return;
                }

                // When dead, Space or Enter triggers respawn
                if state.player_stats.is_dead {
                    if pressed && (key == KeyCode::Space || key == KeyCode::Enter) {
                        state.respawn_player();
                    }
                    return;
                }

                // In-game Chat Input when chat is open
                if state.chat_open {
                    if pressed {
                        if key == KeyCode::Escape {
                            state.chat_open = false;
                            state.chat_input.clear();
                        } else if key == KeyCode::Enter {
                            let msg = state.chat_input.trim().to_string();
                            if !msg.is_empty() {
                                if let Some(client) = &mut state.mp_client {
                                    client.send(&blockgame_core::net::ClientPacket::Chat {
                                        message: msg,
                                    });
                                } else {
                                    let sender = if state.local_username.is_empty() {
                                        "Player".to_string()
                                    } else {
                                        state.local_username.clone()
                                    };
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender,
                                            message: msg,
                                            is_system: false,
                                            age: 0.0,
                                        },
                                    );
                                    if state.local_chat_messages.len() > 25 {
                                        state.local_chat_messages.remove(0);
                                    }
                                }
                            }
                            state.chat_open = false;
                            state.chat_input.clear();
                        } else if key == KeyCode::Backspace {
                            state.chat_input.pop();
                        } else if let Some(t) = text {
                            for ch in t.chars() {
                                if !ch.is_control() && state.chat_input.len() < 120 {
                                    state.chat_input.push(ch);
                                }
                            }
                        }
                    }
                    return;
                }

                // Search Bar text entry when focused
                if state.jei_search_focused && state.inventory_open {
                    if pressed {
                        if key == KeyCode::Escape || key == KeyCode::Enter {
                            state.jei_search_focused = false;
                        } else if key == KeyCode::Backspace {
                            state.jei_search_text.pop();
                            state.jei_page = 0;
                        } else if let Some(t) = text {
                            for ch in t.chars() {
                                if !ch.is_control() {
                                    state.jei_search_text.push(ch);
                                    state.jei_page = 0;
                                }
                            }
                        }
                    }
                    return;
                }

                // Recipe Modal key navigation (Esc / E to close, Left / Right to cycle)
                if state.jei_recipe_modal.is_some() && state.inventory_open {
                    if pressed {
                        if key == KeyCode::Escape || key == KeyCode::KeyE {
                            state.jei_recipe_modal = None;
                            return;
                        }
                        if key == KeyCode::ArrowLeft {
                            if let Some(m) = &mut state.jei_recipe_modal {
                                let total = if m.is_usage_mode {
                                    state
                                        .crafting_manager
                                        .get_recipes_using_input(m.target_item)
                                        .len()
                                } else {
                                    state
                                        .crafting_manager
                                        .get_recipes_for_output(m.target_item)
                                        .len()
                                };
                                if total > 1 {
                                    m.recipe_index = (m.recipe_index + total - 1) % total;
                                }
                            }
                            return;
                        }
                        if key == KeyCode::ArrowRight {
                            if let Some(m) = &mut state.jei_recipe_modal {
                                let total = if m.is_usage_mode {
                                    state
                                        .crafting_manager
                                        .get_recipes_using_input(m.target_item)
                                        .len()
                                } else {
                                    state
                                        .crafting_manager
                                        .get_recipes_for_output(m.target_item)
                                        .len()
                                };
                                if total > 1 {
                                    m.recipe_index = (m.recipe_index + 1) % total;
                                }
                            }
                            return;
                        }
                    }
                }

                match key {
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => state.shift_down = pressed,
                    KeyCode::Tab if !state.inventory_open => {
                        state.tab_held = pressed;
                    }
                    KeyCode::F5 if pressed && !state.inventory_open => {
                        state.third_person = !state.third_person;
                    }
                    KeyCode::KeyR
                        if pressed && state.inventory_open && !state.jei_search_focused =>
                    {
                        if let Some(id) = find_hovered_item(state) {
                            state.jei_recipe_modal = Some(JeiRecipeModalState {
                                target_item: id,
                                is_usage_mode: false,
                                recipe_index: 0,
                            });
                            return;
                        }
                    }
                    KeyCode::KeyU
                        if pressed && state.inventory_open && !state.jei_search_focused =>
                    {
                        if let Some(id) = find_hovered_item(state) {
                            state.jei_recipe_modal = Some(JeiRecipeModalState {
                                target_item: id,
                                is_usage_mode: true,
                                recipe_index: 0,
                            });
                            return;
                        }
                    }
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
                        state.active_chest_pos = None;
                        state.active_furnace_pos = None;
                        state.jei_recipe_modal = None;
                        state.jei_search_focused = false;
                        if state.inventory_open {
                            // Safely return crafting grid items and cursor held item to inventory
                            let grid_items: Vec<_> =
                                state.crafting_grid.slots.drain(..).collect();
                            for slot in grid_items {
                                if !slot.is_empty() {
                                    if let Some(rem) =
                                        state.inventory.add_item(slot, &state.item_registry)
                                    {
                                        state.spawn_drop_synced(
                                            state.player.position,
                                            Vec3::new(0.0, 2.0, 0.0),
                                            rem,
                                        );
                                    }
                                }
                            }
                            if !state.cursor_held.is_empty() {
                                let held =
                                    std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                                if let Some(rem) =
                                    state.inventory.add_item(held, &state.item_registry)
                                {
                                    state.spawn_drop_synced(
                                        state.player.position,
                                        Vec3::new(0.0, 2.0, 0.0),
                                        rem,
                                    );
                                }
                            }
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        } else {
                            // Open 2x2 backpack inventory
                            state.crafting_grid = CraftingGrid::new_2x2();
                            state.inventory_open = true;
                            state.mouse_captured = false;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                    }
                    KeyCode::Escape if pressed => {
                        state.mouse_left_down = false;
                        state.break_progress = 0.0;
                        state.breaking_voxel = None;
                        state.cracking_buffer = None;
                        state.active_chest_pos = None;
                        state.active_furnace_pos = None;
                        if state.inventory_open {
                            let grid_items: Vec<_> =
                                state.crafting_grid.slots.drain(..).collect();
                            for slot in grid_items {
                                if !slot.is_empty() {
                                    if let Some(rem) =
                                        state.inventory.add_item(slot, &state.item_registry)
                                    {
                                        state.spawn_drop_synced(
                                            state.player.position,
                                            Vec3::new(0.0, 2.0, 0.0),
                                            rem,
                                        );
                                    }
                                }
                            }
                            if !state.cursor_held.is_empty() {
                                let held =
                                    std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                                if let Some(rem) =
                                    state.inventory.add_item(held, &state.item_registry)
                                {
                                    state.spawn_drop_synced(
                                        state.player.position,
                                        Vec3::new(0.0, 2.0, 0.0),
                                        rem,
                                    );
                                }
                            }
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        } else {
                            // Open Pause Menu
                            state.moving_forward = false;
                            state.moving_backward = false;
                            state.moving_left = false;
                            state.moving_right = false;
                            state.jumping = false;
                            state.sprinting = false;
                            state.screen = AppScreen::Paused {
                                status_message: None,
                            };
                            state.mouse_captured = false;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                    }
                    KeyCode::KeyT if pressed && !state.inventory_open => {
                        state.moving_forward = false;
                        state.moving_backward = false;
                        state.moving_left = false;
                        state.moving_right = false;
                        state.jumping = false;
                        state.sprinting = false;
                        state.chat_open = true;
                        state.chat_input.clear();
                    }
                    KeyCode::KeyY if pressed && !state.inventory_open => {
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
                let dt = now
                    .duration_since(state.last_frame_time)
                    .as_secs_f32()
                    .min(0.1);
                state.last_frame_time = now;
                state.music_scheduler.update(dt, &mut state.audio);

                let screen_w = state.surface_config.width as f32;
                let screen_h = state.surface_config.height as f32;
                let aspect = screen_w / screen_h;
                state
                    .ui_pipeline
                    .update_screen_size(&state.queue, screen_w, screen_h);

                if state.screen == AppScreen::InGame {
                    // Age local chat messages
                    for entry in &mut state.local_chat_messages {
                        entry.age += dt;
                    }

                    // 0. Poll Multiplayer Network Packets & Sync Player State
                    let mut kick_reason: Option<String> = None;
                    if let Some(client) = &mut state.mp_client {
                        for entry in &mut client.chat_history {
                            entry.age += dt;
                        }
                        for rp in client.remote_players.values_mut() {
                            if rp.hurt_timer > 0.0 {
                                rp.hurt_timer = (rp.hurt_timer - dt).max(0.0);
                            }
                        }
                        let packets = client.poll_packets();
                        for pkt in packets {
                            match pkt {
                                blockgame_core::net::ServerPacket::WorldDeltaSync {
                                    modified_chunks,
                                    chests,
                                    furnaces,
                                    item_drops,
                                } => {
                                    state.world_manager.apply_world_delta_sync(
                                        &modified_chunks,
                                        &chests,
                                        &furnaces,
                                        &item_drops,
                                        &state.device,
                                        &state.pipeline,
                                    );
                                }
                                blockgame_core::net::ServerPacket::BlockUpdate {
                                    pos,
                                    block_id,
                                } => {
                                    state.world_manager.apply_remote_block_update(
                                        pos,
                                        block_id,
                                        &state.device,
                                        &state.pipeline,
                                    );
                                }
                                blockgame_core::net::ServerPacket::ContainerSync {
                                    pos,
                                    is_furnace,
                                    slots,
                                    burn_time,
                                    max_burn_time,
                                    cook_progress,
                                } => {
                                    let mut world = state.world_manager.world().lock().unwrap();
                                    if is_furnace {
                                        let f = blockgame_core::world::FurnaceState {
                                            input: slots.first().copied().unwrap_or(ItemSlot::EMPTY),
                                            fuel: slots.get(1).copied().unwrap_or(ItemSlot::EMPTY),
                                            output: slots.get(2).copied().unwrap_or(ItemSlot::EMPTY),
                                            burn_time_remaining: burn_time,
                                            max_burn_time,
                                            cook_progress,
                                        };
                                        world.furnaces.insert(pos, f);
                                    } else {
                                        world.chest_containers.insert(pos, slots);
                                    }
                                }
                                blockgame_core::net::ServerPacket::ItemDropSpawned {
                                    drop_id,
                                    position,
                                    velocity,
                                    item,
                                } => {
                                    state.world_manager.spawn_item_drop_with_id_and_velocity(
                                        drop_id, position, velocity, item,
                                    );
                                }
                                blockgame_core::net::ServerPacket::ItemDropCollected {
                                    drop_id,
                                    collector_player_id,
                                    item,
                                } => {
                                    state.world_manager.remove_item_drop(drop_id);
                                    if collector_player_id == client.player_id && !item.is_empty() {
                                        let overflow = state
                                            .inventory
                                            .add_item(item, &state.item_registry);
                                        if let Some(remaining) = overflow {
                                            let drop_id = ((client.player_id as u64) << 40)
                                                | (state.next_mp_drop_nonce as u64
                                                    & 0x000000FFFFFFFFFF);
                                            state.next_mp_drop_nonce =
                                                state.next_mp_drop_nonce.wrapping_add(1);
                                            let ppos = state.player.position;
                                            state
                                                .world_manager
                                                .spawn_item_drop_with_id_and_velocity(
                                                    drop_id,
                                                    ppos,
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    remaining,
                                                );
                                            client.send(
                                                &blockgame_core::net::ClientPacket::SpawnItemDrop {
                                                    drop_id,
                                                    position: ppos,
                                                    velocity: Vec3::new(0.0, 2.0, 0.0),
                                                    item: remaining,
                                                },
                                            );
                                        }
                                        let inv = state.inventory.clone();
                                        let hotbar = state.active_hotbar_slot;
                                        let stats = state.player_stats.clone();
                                        client.send(
                                            &blockgame_core::net::ClientPacket::PlayerInventorySync {
                                                inventory: inv,
                                                active_hotbar_slot: hotbar,
                                                stats,
                                            },
                                        );
                                    }
                                }
                                blockgame_core::net::ServerPacket::PlayerJoined { player } => {
                                    if player.player_id != client.player_id {
                                        client.remote_players.insert(player.player_id, player);
                                    }
                                }
                                blockgame_core::net::ServerPacket::PlayerLeft { player_id, .. } => {
                                    client.remote_players.remove(&player_id);
                                }
                                blockgame_core::net::ServerPacket::PlayerDamaged {
                                    target_player_id,
                                    attacker_id,
                                    new_health,
                                    ..
                                } => {
                                    if target_player_id == client.player_id {
                                        state.player_stats.health = new_health;
                                        if new_health <= 0.0 {
                                            state.player_stats.is_dead = true;
                                        }
                                        if let Some(attacker) =
                                            client.remote_players.get(&attacker_id)
                                        {
                                            let kb_dir = (state.player.position - attacker.position)
                                                .with_y(0.0)
                                                .normalize_or_zero();
                                            state.player.velocity +=
                                                kb_dir * 4.5 + Vec3::new(0.0, 3.2, 0.0);
                                        }
                                        state.audio.play_hurt();
                                    } else if let Some(rp) =
                                        client.remote_players.get_mut(&target_player_id)
                                    {
                                        rp.health = new_health;
                                        rp.hurt_timer = 0.35;
                                    }
                                }
                                blockgame_core::net::ServerPacket::WorldSnapshot {
                                    day_night_time,
                                    players,
                                } => {
                                    state.day_night.time = day_night_time;
                                    if !blockgame_core::net::is_night_ticks(day_night_time) {
                                        state.is_sleeping = false;
                                    }
                                    let local_id = client.player_id;
                                    client.remote_players.clear();
                                    for p in players {
                                        if p.player_id != local_id {
                                            client.remote_players.insert(p.player_id, p);
                                        }
                                    }
                                }
                                blockgame_core::net::ServerPacket::ChatBroadcast {
                                    sender,
                                    message,
                                    is_system,
                                } => {
                                    client.chat_history.push(blockgame_core::net::ChatEntry {
                                        sender,
                                        message,
                                        is_system,
                                        age: 0.0,
                                    });
                                    if client.chat_history.len() > 30 {
                                        client.chat_history.remove(0);
                                    }
                                }
                                blockgame_core::net::ServerPacket::Kick { reason } => {
                                    kick_reason = Some(reason);
                                }
                                blockgame_core::net::ServerPacket::TeleportPlayer {
                                    position,
                                    yaw,
                                    pitch,
                                } => {
                                    state.player.position = position;
                                    state.player.velocity = Vec3::ZERO;
                                    state.camera.yaw = yaw;
                                    state.camera.pitch = pitch;
                                }

                                blockgame_core::net::ServerPacket::ReceiveItem { item } => {
                                    if !item.is_empty() {
                                        let overflow = state
                                            .inventory
                                            .add_item(item, &state.item_registry);
                                        if let Some(remaining) = overflow {
                                            let drop_id = ((client.player_id as u64) << 40)
                                                | (state.next_mp_drop_nonce as u64
                                                    & 0x000000FFFFFFFFFF);
                                            state.next_mp_drop_nonce =
                                                state.next_mp_drop_nonce.wrapping_add(1);
                                            let ppos = state.player.position;
                                            state
                                                .world_manager
                                                .spawn_item_drop_with_id_and_velocity(
                                                    drop_id,
                                                    ppos,
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    remaining,
                                                );
                                            client.send(
                                                &blockgame_core::net::ClientPacket::SpawnItemDrop {
                                                    drop_id,
                                                    position: ppos,
                                                    velocity: Vec3::new(0.0, 2.0, 0.0),
                                                    item: remaining,
                                                },
                                            );
                                        }
                                        let inv = state.inventory.clone();
                                        let hotbar = state.active_hotbar_slot;
                                        let stats = state.player_stats.clone();
                                        client.send(
                                            &blockgame_core::net::ClientPacket::PlayerInventorySync {
                                                inventory: inv,
                                                active_hotbar_slot: hotbar,
                                                stats,
                                            },
                                        );
                                    }
                                }
                                _ => {}
                            }

                        }

                        if !client.connected && kick_reason.is_none() {
                            kick_reason = Some("Connection to server lost.".to_string());
                        }
                    }

                    if let Some(reason) = kick_reason {
                        state.disconnect_multiplayer();
                        state.mouse_captured = false;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                        state.screen = AppScreen::MultiplayerLobby {
                            username_input: state.local_username.clone(),
                            password_input: String::new(),
                            address_input: "127.0.0.1:25565".to_string(),
                            skin_id: state.local_skin_id,
                            focused_field: 0,
                            status_message: Some((reason, true)),
                        };
                        return;
                    }

                    // Update dynamic Day/Night celestial cycle
                    state.day_night.update(dt);
                    state.total_time += dt;

                    // 1. Calculate player wish direction relative to camera yaw
                    let forward =
                        Vec3::new(state.camera.yaw.cos(), 0.0, state.camera.yaw.sin()).normalize();
                    let right =
                        Vec3::new(-state.camera.yaw.sin(), 0.0, state.camera.yaw.cos()).normalize();

                    let mut wish_dir = Vec3::ZERO;
                    if !state.player_stats.is_dead {
                        if state.moving_forward {
                            wish_dir += forward;
                        }
                        if state.moving_backward {
                            wish_dir -= forward;
                        }
                        if state.moving_right {
                            wish_dir += right;
                        }
                        if state.moving_left {
                            wish_dir -= right;
                        }
                        if wish_dir.length_squared() > 0.0 {
                            wish_dir = wish_dir.normalize();
                            if state.is_sleeping {
                                state.is_sleeping = false;
                                if let Some(client) = &mut state.mp_client {
                                    client.send(&blockgame_core::net::ClientPacket::SetSleeping {
                                        sleeping: false,
                                    });
                                }
                            }
                        }

                        // If flying, vertical movement with Space / Shift
                        if state.player.flying {
                            if state.jumping {
                                wish_dir.y += 1.0;
                            }
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

                    // Check if player's head is submerged in water, player is in water, or touching cactus
                    let head_block_pos = IVec3::new(
                        state.player.position.x.floor() as i32,
                        (state.player.position.y + 1.62).floor() as i32,
                        state.player.position.z.floor() as i32,
                    );
                    let (is_head_submerged, is_in_water, is_touching_cactus) = {
                        let world = state.world_manager.world().lock().unwrap();
                        let submerged = world.get_block(head_block_pos).is_water();
                        let in_water = state.player.is_in_water(&world);
                        let touching_cactus = state.player.is_touching_cactus(&world);
                        (submerged, in_water, touching_cactus)
                    };

                    // Water splash on entry and exit
                    if is_in_water && !state.was_in_water {
                        state.audio.play_water_splash(0.9);
                    } else if !is_in_water && state.was_in_water {
                        state.audio.play_water_splash(0.7);
                    }
                    state.was_in_water = is_in_water;

                    // Underwater ambient bubbling when head is submerged
                    if is_head_submerged && !state.player_stats.is_dead {
                        state.underwater_sound_timer += dt;
                        if state.underwater_sound_timer >= 2.2 {
                            state.underwater_sound_timer = 0.0;
                            state.audio.play_underwater_ambience();
                        }
                    } else {
                        state.underwater_sound_timer = 1.0;
                    }

                    // Apply damage and trigger hurt audio on health decrease
                    let prev_health = state.player_stats.health;
                    let armor_mult = state.armor.damage_multiplier(&state.item_registry);
                    if fall_damage > 0.0 && !state.player_stats.is_dead {
                        if state.player_stats.take_damage(fall_damage * armor_mult)
                            && state.armor.damage_armor(fall_damage, &state.item_registry)
                        {
                            state.audio.play_tool_break();
                        }
                    }
                    if is_touching_cactus {
                        if state.player_stats.take_damage(1.0 * armor_mult)
                            && state.armor.damage_armor(1.0, &state.item_registry)
                        {
                            state.audio.play_tool_break();
                        }
                    }

                    // Update walking distance & movement exhaustion (advances ONLY when moving)
                    let is_moving = (state.moving_forward
                        || state.moving_backward
                        || state.moving_left
                        || state.moving_right)
                        && (state.player.on_ground || state.player.flying || is_in_water);
                    let horiz_speed = (state.player.velocity.x * state.player.velocity.x
                        + state.player.velocity.z * state.player.velocity.z)
                        .sqrt();
                    if is_moving && horiz_speed > 0.1 && !state.player_stats.is_dead {
                        state.walk_distance += dt * (if state.sprinting { 8.5 } else { 5.5 });
                        if state.player.on_ground || is_in_water {
                            if state.player.on_ground {
                                state
                                    .player_stats
                                    .add_exhaustion(dt * if state.sprinting { 0.1 } else { 0.02 });
                            }

                            // Dynamic footstep / water swimming audio based on surface and stride interval
                            let stride_dist = if state.sprinting { 2.4 } else { 1.6 };
                            if state.walk_distance - state.last_footstep_dist >= stride_dist {
                                state.last_footstep_dist = state.walk_distance;
                                if is_in_water {
                                    let swim_vol = if state.sprinting { 0.95 } else { 0.70 };
                                    state.audio.play_water_swim(swim_vol);
                                } else {
                                    let foot_block_pos = IVec3::new(
                                        state.player.position.x.floor() as i32,
                                        (state.player.position.y - 0.2).floor() as i32,
                                        state.player.position.z.floor() as i32,
                                    );
                                    let ground_block = {
                                        let world = state.world_manager.world().lock().unwrap();
                                        world.get_block(foot_block_pos)
                                    };
                                    let material =
                                        crate::audio::SoundMaterial::from_block(ground_block);
                                    let volume_scale = if state.sprinting { 1.0 } else { 0.75 };
                                    state.audio.play_footstep(material, volume_scale);
                                }
                            }
                        }
                    }

                    // Update player stats (regeneration, starvation, void damage, breath/drowning)
                    state
                        .player_stats
                        .update(dt, state.player.position.y, is_head_submerged);

                    if state.player_stats.health < prev_health && !state.player_stats.is_dead {
                        state.audio.play_hurt();
                    }

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
                            let mut drops_to_scatter = Vec::new();
                            for (i, slot) in state.inventory.slots.iter_mut().enumerate() {
                                if !slot.is_empty() {
                                    let drop_slot = std::mem::replace(slot, ItemSlot::EMPTY);
                                    let angle = (i as f32) * 0.35;
                                    let vel = Vec3::new(angle.cos() * 2.0, 3.0, angle.sin() * 2.0);
                                    drops_to_scatter.push((vel, drop_slot));
                                }
                            }
                            // Scatter crafting grid items
                            for (i, slot) in state.crafting_grid.slots.drain(..).enumerate() {
                                if !slot.is_empty() {
                                    let angle = (i as f32) * 0.7;
                                    let vel = Vec3::new(angle.cos() * 2.0, 3.0, angle.sin() * 2.0);
                                    drops_to_scatter.push((vel, slot));
                                }
                            }
                            // Scatter equipped armor
                            for (i, slot) in state.armor.slots.iter_mut().enumerate() {
                                if !slot.is_empty() {
                                    let drop_slot = std::mem::replace(slot, ItemSlot::EMPTY);
                                    let angle = (i as f32) * 1.1;
                                    let vel = Vec3::new(angle.cos() * 2.0, 3.0, angle.sin() * 2.0);
                                    drops_to_scatter.push((vel, drop_slot));
                                }
                            }
                            // Scatter cursor held item
                            if !state.cursor_held.is_empty() {
                                let drop_slot =
                                    std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                                drops_to_scatter.push((Vec3::new(0.0, 3.0, 0.0), drop_slot));
                            }
                            for (vel, slot) in drops_to_scatter {
                                state.spawn_drop_synced(state.player.position, vel, slot);
                            }

                            // Unlock mouse so user can click the Respawn button
                            state.mouse_captured = false;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::None);
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

                    // Send periodic PlayerStateUpdate to multiplayer server (20 Hz)
                    if state.mp_client.is_some() {
                        state.net_sync_timer += dt;
                        if state.net_sync_timer >= 0.05 {
                            state.net_sync_timer = 0.0;
                            let model = state.current_local_player_model();
                            let inv = state.inventory.clone();
                            let hotbar = state.active_hotbar_slot;
                            let stats = state.player_stats.clone();
                            if let Some(client) = &mut state.mp_client {
                                client.send(
                                    &blockgame_core::net::ClientPacket::PlayerStateUpdate {
                                        position: model.position,
                                        velocity: model.velocity,
                                        yaw: model.yaw,
                                        pitch: model.pitch,
                                        held_item: model.held_item,
                                        walk_anim: model.walk_anim,
                                        swing_anim: model.swing_anim,
                                        is_sneaking: model.is_sneaking,
                                        health: model.health,
                                    },
                                );
                                client.send(
                                    &blockgame_core::net::ClientPacket::PlayerInventorySync {
                                        inventory: inv,
                                        active_hotbar_slot: hotbar,
                                        stats,
                                    },
                                );
                            }
                        }
                    }

                    // 3. Update Item Drops & Magnetism Pickup (only when alive)
                    if state.mp_client.is_some() {
                        if !state.player_stats.is_dead {
                            let pickable_ids = state
                                .world_manager
                                .update_item_drops_multiplayer(dt, state.player.position);
                            if let Some(client) = &mut state.mp_client {
                                for drop_id in pickable_ids {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::PickupItemDrop {
                                            drop_id,
                                        },
                                    );
                                }
                            }
                        } else {
                            let _ = state
                                .world_manager
                                .update_item_drops_multiplayer(dt, state.player.position);
                        }
                    } else if !state.player_stats.is_dead {
                        let collected = state
                            .world_manager
                            .update_item_drops(dt, state.player.position);
                        for item in collected {
                            let overflow = state.inventory.add_item(item, &state.item_registry);
                            if let Some(remaining) = overflow {
                                state
                                    .world_manager
                                    .spawn_item_drop(state.player.position, remaining);
                            }
                        }
                    } else {
                        let _ = state
                            .world_manager
                            .update_item_drops(dt, state.player.position);
                    }

                    // 3b. Update Animals, Monsters, Arrows & Planted Crops
                    let animal_drops = state.world_manager.update_animals(dt);
                    for (pos, drops) in animal_drops {
                        for drop_slot in drops {
                            state.spawn_drop_synced(pos, Vec3::new(0.0, 2.0, 0.0), drop_slot);
                        }
                    }
                    let (monster_dmg, monster_drops) = state.world_manager.update_monsters(
                        dt,
                        state.player.position,
                        state.day_night.sun_intensity(),
                    );
                    for (pos, drops) in monster_drops {
                        for drop_slot in drops {
                            state.spawn_drop_synced(pos, Vec3::new(0.0, 2.0, 0.0), drop_slot);
                        }
                    }
                    let arrow_dmg = state.world_manager.update_arrows(
                        dt,
                        state.player.position,
                        state.player.half_width,
                        state.player.height,
                    );
                    let total_hostile_dmg = monster_dmg + arrow_dmg;
                    if total_hostile_dmg > 0.0
                        && !state.player_stats.is_dead
                        && !state.player.flying
                    {
                        let armor_mult = state.armor.damage_multiplier(&state.item_registry);
                        if state
                            .player_stats
                            .take_damage(total_hostile_dmg * armor_mult)
                            && state
                                .armor
                                .damage_armor(total_hostile_dmg, &state.item_registry)
                        {
                            state.audio.play_tool_break();
                        }
                    }
                    state
                        .world_manager
                        .update_crops(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_farmland(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_cacti(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_decaying_leaves(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_furnaces(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_falling_blocks(dt, &state.device, &state.pipeline);

                    // 4. Update Camera position (First-Person or F5 Third-Person) and atmosphere uniform
                    let eye_pos = state.player.eye_position()
                        - if state.shift_down && !state.inventory_open && !state.player.flying {
                            Vec3::new(0.0, 0.18, 0.0)
                        } else {
                            Vec3::ZERO
                        };
                    state.camera.position = if state.third_person {
                        let back_dir = -state.camera.forward();
                        eye_pos + back_dir * 3.8 + Vec3::new(0.0, 0.35, 0.0)
                    } else {
                        eye_pos
                    };
                    let camera_block_pos = IVec3::new(
                        state.camera.position.x.floor() as i32,
                        state.camera.position.y.floor() as i32,
                        state.camera.position.z.floor() as i32,
                    );
                    let is_camera_underwater = {
                        let world = state.world_manager.world().lock().unwrap();
                        world.get_block(camera_block_pos).is_water()
                    };
                    let player_pos = state.player.position;
                    let current_biome = state.world_manager.generator.get_biome(
                        player_pos.x.floor() as i32,
                        player_pos.z.floor() as i32,
                        player_pos.y.floor() as i32,
                    );
                    let biome_fog = current_biome.fog_color();
                    let biome_tint = current_biome.sky_tint();
                    state.camera.update_uniform_with_atmosphere_water_tint(
                        &mut state.camera_uniform,
                        aspect,
                        &state.day_night,
                        is_camera_underwater,
                        biome_fog,
                        biome_tint,
                    );
                    state
                        .pipeline
                        .update_camera(&state.queue, &state.camera_uniform);

                    // 5. Perform Fast Voxel Traversal (DDA) raycast from player eye_pos
                    {
                        let world_guard = state.world_manager.world().lock().unwrap();
                        let held_item = state.inventory.get(state.active_hotbar_slot).item_id;
                        let is_placeable = state
                            .item_registry
                            .get(held_item)
                            .and_then(|d| d.placeable_block)
                            .is_some();
                        let is_bucket =
                            held_item == ItemId::BUCKET || held_item == ItemId::WATER_BUCKET;
                        let mut hit = raycast_voxel_with_water(
                            eye_pos,
                            state.camera.forward(),
                            5.0, // 5 blocks reach
                            &world_guard,
                            &self.registry,
                            held_item == ItemId::LILY_PAD || held_item == ItemId::BUCKET,
                        );
                        if hit.is_none() && (is_placeable || is_bucket) {
                            hit = raycast_voxel_with_water(
                                eye_pos,
                                state.camera.forward(),
                                5.0,
                                &world_guard,
                                &self.registry,
                                true,
                            );
                        }

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
                    if state.mouse_left_down
                        && state.mouse_captured
                        && !state.inventory_open
                        && !state.player_stats.is_dead
                    {
                        if state.punch_timer <= 0.05 {
                            state.punch_timer = 0.25; // Continuous swing loop while holding down break
                        }

                        if let Some(hit) = state.targeted_block {
                            let target_voxel = hit.hit_voxel;
                            if state.breaking_voxel != Some(target_voxel) {
                                state.breaking_voxel = Some(target_voxel);
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;
                                state.break_hit_timer = 0.25; // Trigger immediate initial hit
                            }

                            let block_at_voxel = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard.get_block(target_voxel)
                            };

                            if block_at_voxel.is_air() {
                                state.breaking_voxel = None;
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;
                                state.break_hit_timer = 0.0;
                            } else {
                                let held_slot = state.inventory.get(state.active_hotbar_slot);
                                let (held_tool_type, held_tool_tier) =
                                    if let Some(def) = state.item_registry.get(held_slot.item_id) {
                                        (def.tool_type, def.tool_tier.unwrap_or(ToolTier::None))
                                    } else {
                                        (None, ToolTier::None)
                                    };

                                let break_time = self.registry.get_break_time(
                                    block_at_voxel,
                                    held_tool_type,
                                    held_tool_tier,
                                );
                                if break_time.is_infinite() {
                                    state.break_progress = 0.0;
                                    state.cracking_buffer = None;
                                    state.break_hit_timer = 0.0;
                                } else {
                                    state.break_progress += dt / break_time;
                                    state.break_hit_timer += dt;
                                    if state.break_hit_timer >= 0.25 {
                                        state.break_hit_timer = 0.0;
                                        state.audio.play_block_hit(
                                            block_at_voxel,
                                            target_voxel.as_vec3() + Vec3::splat(0.5),
                                            state.camera.position,
                                            state.camera.yaw,
                                        );
                                    }
                                    if state.break_progress >= 1.0 {
                                        // Block break completed!
                                        if let Some(broken) = state.world_manager.break_block(
                                            target_voxel,
                                            &state.device,
                                            &state.pipeline,
                                        ) {
                                            if let Some(client) = &mut state.mp_client {
                                                let post_break_block = state
                                                    .world_manager
                                                    .world()
                                                    .lock()
                                                    .unwrap()
                                                    .get_block(target_voxel);
                                                client.send(
                                                    &blockgame_core::net::ClientPacket::BlockChange {
                                                        pos: target_voxel,
                                                        block_id: post_break_block,
                                                    },
                                                );
                                                if broken.is_door() {
                                                    let pair_pos = if broken.is_door_top() {
                                                        target_voxel - IVec3::Y
                                                    } else {
                                                        target_voxel + IVec3::Y
                                                    };
                                                    client.send(
                                                        &blockgame_core::net::ClientPacket::BlockChange {
                                                            pos: pair_pos,
                                                            block_id: BlockId::AIR,
                                                        },
                                                    );
                                                }
                                            }
                                            state.audio.play_block_break(
                                                broken,
                                                target_voxel.as_vec3() + Vec3::splat(0.5),
                                                state.camera.position,
                                                state.camera.yaw,
                                            );
                                            state.player_stats.add_exhaustion(0.025);
                                            let mut held =
                                                state.inventory.get(state.active_hotbar_slot);
                                            if let Some(def) =
                                                state.item_registry.get(held.item_id)
                                            {
                                                if let Some(max_dur) = def.durability {
                                                    let dmg = if def.tool_type
                                                        == Some(
                                                            blockgame_core::tool::ToolType::Sword,
                                                        ) {
                                                        2
                                                    } else {
                                                        1
                                                    };
                                                    if held.damage_tool(dmg, max_dur) {
                                                        state.audio.play_tool_break();
                                                    }
                                                    state.inventory.set(
                                                        state.active_hotbar_slot,
                                                        held,
                                                    );
                                                }
                                            }
                                            if let Some((drop_item, drop_count)) = self
                                                .registry
                                                .get_drop(broken, held_tool_type, held_tool_tier)
                                            {
                                                let drop_pos = target_voxel.as_vec3()
                                                    + Vec3::new(0.5, 0.5, 0.5);
                                                let drop_slot =
                                                    ItemSlot::new(drop_item, drop_count);
                                                state.spawn_drop_synced(
                                                    drop_pos,
                                                    Vec3::new(0.0, 2.5, 0.0),
                                                    drop_slot,
                                                );
                                            }
                                            // 15% chance to drop Apple from Leaves
                                            if broken == BlockId::LEAVES {
                                                let hash = (target_voxel.x.wrapping_mul(73856093)
                                                    ^ target_voxel.y.wrapping_mul(19349663)
                                                    ^ target_voxel.z.wrapping_mul(83492791))
                                                    % 100;
                                                if hash.abs() < 15 {
                                                    state.spawn_drop_synced(
                                                        target_voxel.as_vec3()
                                                            + Vec3::new(0.5, 0.5, 0.5),
                                                        Vec3::new(0.0, 2.0, 0.0),
                                                        ItemSlot::new(ItemId::APPLE, 1),
                                                    );
                                                }
                                            }
                                            // 12% chance to drop Wheat Seeds from Grass
                                            if broken == BlockId::GRASS {
                                                let hash = (target_voxel.x.wrapping_mul(87654321)
                                                    ^ target_voxel.y.wrapping_mul(23456789)
                                                    ^ target_voxel.z.wrapping_mul(98765432))
                                                    % 100;
                                                if hash.abs() < 12 {
                                                    state.spawn_drop_synced(
                                                        target_voxel.as_vec3()
                                                            + Vec3::new(0.5, 0.5, 0.5),
                                                        Vec3::new(0.0, 2.0, 0.0),
                                                        ItemSlot::new(ItemId::WHEAT_SEEDS, 1),
                                                    );
                                                }
                                            }
                                            // Ripe Wheat Crop drops additional 1..2 seeds
                                            if broken == BlockId::WHEAT_3 {
                                                let hash = (target_voxel.x.wrapping_mul(1234567)
                                                    ^ target_voxel.z.wrapping_mul(7654321))
                                                    % 2;
                                                let seed_count = (hash.abs() + 1) as u16;
                                                state.spawn_drop_synced(
                                                    target_voxel.as_vec3()
                                                        + Vec3::new(0.5, 0.5, 0.5),
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    ItemSlot::new(ItemId::WHEAT_SEEDS, seed_count),
                                                );
                                            }
                                        }
                                        state.breaking_voxel = None;
                                        state.break_progress = 0.0;
                                        state.cracking_buffer = None;
                                        state.break_hit_timer = 0.0;
                                    } else {
                                        let stage =
                                            ((state.break_progress * 10.0).floor() as usize).min(9);
                                        let (crack_buf, count) =
                                            WireframePipeline::create_cracking_box_buffer(
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
                            state.break_hit_timer = 0.0;
                        }
                    } else if state.breaking_voxel.is_some() {
                        state.breaking_voxel = None;
                        state.break_progress = 0.0;
                        state.cracking_buffer = None;
                        state.break_hit_timer = 0.0;
                    }

                    // 6. Update chunk streaming around player
                    state
                        .world_manager
                        .update(state.player.position, &state.device, &state.pipeline);

                    if let Some(furnace_pos) = state.active_furnace_pos {
                        let dist_sq = (state.player.position
                            - (furnace_pos.as_vec3() + Vec3::splat(0.5)))
                        .length_squared();
                        if dist_sq > 36.0 {
                            state.active_furnace_pos = None;
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        }
                    }

                    if let Some(chest_pos) = state.active_chest_pos {
                        let dist_sq = (state.player.position
                            - (chest_pos.as_vec3() + Vec3::splat(0.5)))
                        .length_squared();
                        if dist_sq > 36.0 {
                            state.active_chest_pos = None;
                            state.inventory_open = false;
                            state.mouse_captured = true;
                            let _ = state
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                            state.window.set_cursor_visible(false);
                        }
                    }

                    // 7. Build 2D UI Overlay (HUD or Inventory/Crafting Screen)
                    if state.inventory_open {
                        // 1. Prepare JEI Item Index Catalog & Filter
                        let search_lower = state.jei_search_text.trim().to_lowercase();
                        let catalog: Vec<ItemId> = state
                            .item_registry
                            .all_items()
                            .into_iter()
                            .filter(|def| {
                                if search_lower.is_empty() {
                                    true
                                } else {
                                    def.display_name.to_lowercase().contains(&search_lower)
                                }
                            })
                            .map(|def| def.id)
                            .collect();
                        let items_per_page = 6 * 8;
                        let total_pages =
                            ((catalog.len() + items_per_page - 1) / items_per_page).max(1);
                        if state.jei_page >= total_pages {
                            state.jei_page = total_pages.saturating_sub(1);
                        }

                        let jei_params = blockgame_render::ui::JeiParams {
                            catalog_items: &catalog,
                            page: state.jei_page,
                            total_pages,
                            search_query: &state.jei_search_text,
                            search_focused: state.jei_search_focused,
                        };

                        // 2. Prepare Recipe Modal State if active
                        let display_recipe_storage: Option<blockgame_core::crafting::DisplayRecipe>;
                        let mut modal_params: Option<blockgame_render::ui::RecipeModalParams> =
                            None;
                        if let Some(m) = &state.jei_recipe_modal {
                            let recipes = if m.is_usage_mode {
                                state
                                    .crafting_manager
                                    .get_recipes_using_input(m.target_item)
                            } else {
                                state.crafting_manager.get_recipes_for_output(m.target_item)
                            };
                            let total_rec = recipes.len();
                            if total_rec > 0 {
                                let clamped_idx = m.recipe_index.min(total_rec - 1);
                                display_recipe_storage = Some(recipes[clamped_idx].clone());
                            } else {
                                let item_name = state
                                    .item_registry
                                    .get(m.target_item)
                                    .map(|d| d.display_name)
                                    .unwrap_or("Item");
                                display_recipe_storage =
                                    Some(blockgame_core::crafting::DisplayRecipe {
                                        name: item_name,
                                        category:
                                            blockgame_core::crafting::RecipeCategory::WorldGen,
                                        inputs: [None; 9],
                                        output: ItemSlot::new(m.target_item, 1),
                                    });
                            }
                            if let Some(ref disp) = display_recipe_storage {
                                modal_params = Some(blockgame_render::ui::RecipeModalParams {
                                    target_item: m.target_item,
                                    recipe: disp,
                                    recipe_index: m.recipe_index.min(total_rec.max(1) - 1),
                                    total_recipes: total_rec.max(1),
                                    is_usage_mode: m.is_usage_mode,
                                });
                            }
                        }

                        if let Some(furnace_pos) = state.active_furnace_pos {
                            let furnace = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard
                                    .get_furnace(furnace_pos)
                                    .cloned()
                                    .unwrap_or_default()
                            };
                            UiBuilder::build_furnace_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &furnace,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                state.cursor_pos,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(chest_pos) = state.active_chest_pos {
                            let chest_slots = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard
                                    .get_chest(chest_pos)
                                    .cloned()
                                    .unwrap_or_else(|| vec![ItemSlot::EMPTY; 27])
                            };
                            UiBuilder::build_chest_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &chest_slots,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                state.cursor_pos,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else {
                            state.crafting_result = state
                                .crafting_manager
                                .find_match(&state.crafting_grid)
                                .map(|(_, out)| out);
                            UiBuilder::build_inventory_screen_with_armor(
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
                                Some(&jei_params),
                                modal_params.as_ref(),
                                &state.armor.slots,
                            );
                        }
                    } else {
                        UiBuilder::build_hud_with_armor(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            &state.inventory,
                            state.active_hotbar_slot,
                            &state.item_registry,
                            state.walk_distance,
                            punch_time,
                            &state.player_stats,
                            state.armor.total_defense(&state.item_registry),
                        );

                        // Draw floating 3D-to-2D username nametags for remote players (and local player in F5 third-person)
                        let mut nametag_models = Vec::new();
                        if let Some(client) = &state.mp_client {
                            for rp in client.remote_players.values() {
                                if rp.health > 0.0 {
                                    nametag_models.push(rp.clone());
                                }
                            }
                        }
                        if state.third_person && !state.player_stats.is_dead {
                            nametag_models.push(state.current_local_player_model());
                        }
                        if !nametag_models.is_empty() {
                            UiBuilder::draw_player_nametags(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                state.camera.build_view_projection_matrix(aspect),
                                state.camera.position,
                                &nametag_models,
                            );
                        }

                        // Draw Multiplayer Chat, Tab Player List, and Server Status Overlays
                        let mut online_models = vec![state.current_local_player_model()];
                        if let Some(client) = &state.mp_client {
                            for rp in client.remote_players.values() {
                                online_models.push(rp.clone());
                            }
                        }
                        let local_name = if state.local_username.is_empty() {
                            "Explorer".to_string()
                        } else {
                            state.local_username.clone()
                        };
                        let server_addr = state
                            .mp_client
                            .as_ref()
                            .map(|c| c.server_addr.clone())
                            .unwrap_or_default();
                        let chat_slice = if let Some(client) = &state.mp_client {
                            client.chat_history.as_slice()
                        } else {
                            state.local_chat_messages.as_slice()
                        };
                        UiBuilder::draw_multiplayer_hud_overlays(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            chat_slice,
                            state.chat_open,
                            &state.chat_input,
                            state.tab_held,
                            &online_models,
                            &local_name,
                            state.mp_hosted_server.is_some(),
                            &server_addr,
                        );
                    }
                } else if let AppScreen::Paused { ref status_message } = state.screen {
                    // Game is paused: keep camera at player eye position without stepping simulation
                    state.camera.position = state.player.eye_position();
                    let camera_block_pos = IVec3::new(
                        state.camera.position.x.floor() as i32,
                        state.camera.position.y.floor() as i32,
                        state.camera.position.z.floor() as i32,
                    );
                    let is_camera_underwater = {
                        let world = state.world_manager.world().lock().unwrap();
                        world.get_block(camera_block_pos).is_water()
                    };
                    let player_pos = state.player.position;
                    let current_biome = state.world_manager.generator.get_biome(
                        player_pos.x.floor() as i32,
                        player_pos.z.floor() as i32,
                        player_pos.y.floor() as i32,
                    );
                    let biome_fog = current_biome.fog_color();
                    let biome_tint = current_biome.sky_tint();
                    state.camera.update_uniform_with_atmosphere_water_tint(
                        &mut state.camera_uniform,
                        aspect,
                        &state.day_night,
                        is_camera_underwater,
                        biome_fog,
                        biome_tint,
                    );
                    state
                        .pipeline
                        .update_camera(&state.queue, &state.camera_uniform);

                    UiBuilder::build_pause_menu(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        state.cursor_pos,
                        status_message.as_deref(),
                    );
                } else {
                    // Menu screens (Title, WorldSelect, CreateWorld, Settings, MultiplayerLobby):
                    // Slowly orbit camera over the currently loaded 3D terrain around player.position
                    state.total_time += dt;
                    state.world_manager.update(
                        state.player.position,
                        &state.device,
                        &state.pipeline,
                    );

                    let angle = state.total_time * 0.12;
                    let center_x = state.player.position.x;
                    let center_z = state.player.position.z;
                    let cam_x = center_x + angle.cos() * 24.0;
                    let cam_z = center_z + angle.sin() * 24.0;

                    // Compute a constant elevation from fixed points around the orbit center
                    // (independent of `angle`) so the camera stays at the exact same height while rotating.
                    let constant_surface_y = {
                        let world_guard = state.world_manager.world().lock().unwrap();
                        let top_y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
                        let mut max_y = 40;
                        let mut check_col = |sx: i32, sz: i32| {
                            let mut y = top_y;
                            while y > 1 {
                                if !world_guard.get_block(IVec3::new(sx, y, sz)).is_air() {
                                    if y > max_y {
                                        max_y = y;
                                    }
                                    break;
                                }
                                y -= 1;
                            }
                        };
                        check_col(center_x.floor() as i32, center_z.floor() as i32);
                        for i in 0..16 {
                            let a = (i as f32) * (std::f32::consts::TAU / 16.0);
                            let sx = (center_x + a.cos() * 24.0).floor() as i32;
                            let sz = (center_z + a.sin() * 24.0).floor() as i32;
                            check_col(sx, sz);
                        }
                        max_y as f32
                    };

                    let target_cam_y = (constant_surface_y + 14.0)
                        .max(state.player.position.y + 18.0)
                        .max(58.0);
                    state.camera.position = Vec3::new(cam_x, target_cam_y, cam_z);
                    state.camera.yaw = angle + std::f32::consts::PI;
                    state.camera.pitch = -0.24;
                    state.camera.update_uniform_with_atmosphere(
                        &mut state.camera_uniform,
                        aspect,
                        &state.day_night,
                    );
                    state
                        .pipeline
                        .update_camera(&state.queue, &state.camera_uniform);
                    state.targeted_block = None;
                    state.wireframe_buffer = None;
                    state.cracking_buffer = None;

                    match &state.screen {
                        AppScreen::Title => {
                            UiBuilder::build_title_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                state.cursor_pos,
                            );
                        }
                        AppScreen::WorldSelect { selected_index } => {
                            UiBuilder::build_world_select_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &state.saved_worlds,
                                *selected_index,
                                state.cursor_pos,
                            );
                        }
                        AppScreen::CreateWorld {
                            name_input,
                            seed_input,
                            focused_field,
                        } => {
                            UiBuilder::build_create_world_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                name_input,
                                seed_input,
                                *focused_field,
                                state.cursor_pos,
                            );
                        }
                        AppScreen::Settings { .. } => {
                            let render_dist_line =
                                format!("Render Distance: {} chunks", state.render_distance);
                            let lines = [
                                render_dist_line.as_str(),
                                "Controls: WASD Move | Space Jump | Ctrl Sprint | F Fly | F5 3rd Person",
                                "T Chat | Tab Player List | E Inventory | R Recipe | U Uses | Esc Pause",
                                "Textures: Faithful 32x Procedural Atlas + 3D Player Skins",
                            ];
                            UiBuilder::build_notice_or_settings_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                "Settings",
                                &lines,
                                state.cursor_pos,
                            );
                        }
                        AppScreen::MultiplayerLobby {
                            username_input,
                            password_input,
                            address_input,
                            skin_id,
                            focused_field,
                            status_message,
                        } => {
                            let status_ref = status_message.as_ref().map(|(m, err)| (m.as_str(), *err));
                            UiBuilder::build_multiplayer_lobby_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                username_input,
                                password_input,
                                address_input,
                                *skin_id,
                                *focused_field,
                                status_ref,
                                state.cursor_pos,
                            );
                        }
                        AppScreen::InGame | AppScreen::Paused { .. } => {}
                    }
                }

                state.ui_dynamic_mesh.update(
                    &state.device,
                    &state.queue,
                    &state.ui_mesh.vertices,
                    &state.ui_mesh.indices,
                );

                // Upload dynamic 3D Item Drops mesh
                if !state.world_manager.item_drops.is_empty() {
                    let drops_mesh = blockgame_render::build_item_drops_mesh(
                        &state.world_manager.item_drops,
                        &state.item_registry,
                    );
                    state.item_drop_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &drops_mesh.vertices,
                        &drops_mesh.indices,
                    );
                } else {
                    state.item_drop_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Animals mesh
                if !state.world_manager.animals.is_empty() {
                    let animals_mesh =
                        blockgame_render::build_animals_mesh(&state.world_manager.animals);
                    state.animal_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &animals_mesh.vertices,
                        &animals_mesh.indices,
                    );
                } else {
                    state.animal_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Monsters mesh
                if !state.world_manager.monsters.is_empty() {
                    let monsters_mesh = blockgame_render::build_monsters_mesh_animated(
                        &state.world_manager.monsters,
                        state.total_time,
                    );
                    state.monster_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &monsters_mesh.vertices,
                        &monsters_mesh.indices,
                    );
                } else {
                    state.monster_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Arrows mesh
                if !state.world_manager.arrows.is_empty() {
                    let arrows_mesh =
                        blockgame_render::build_arrows_mesh(&state.world_manager.arrows);
                    state.arrow_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &arrows_mesh.vertices,
                        &arrows_mesh.indices,
                    );
                } else {
                    state.arrow_dynamic_mesh.clear();
                }

                // Upload Sun & Moon celestial sky mesh
                {
                    let celestial_mesh = blockgame_render::build_celestial_mesh(
                        state.camera.position,
                        state.day_night.sun_direction(),
                    );
                    state.celestial_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &celestial_mesh.vertices,
                        &celestial_mesh.indices,
                    );
                }

                // Upload dynamic 3D Falling Blocks mesh
                if !state.world_manager.falling_blocks.is_empty() {
                    let falling_mesh = blockgame_render::build_falling_blocks_mesh(
                        &state.world_manager.falling_blocks,
                        state.world_manager.registry(),
                    );
                    state.falling_block_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &falling_mesh.vertices,
                        &falling_mesh.indices,
                    );
                } else {
                    state.falling_block_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Articulated Players mesh (remote multiplayer players + local player in 3rd-person or menu)
                let mut players_to_render = Vec::new();
                if let Some(client) = &state.mp_client {
                    for rp in client.remote_players.values() {
                        players_to_render.push(rp.clone());
                    }
                }
                if state.third_person || state.screen != AppScreen::InGame {
                    players_to_render.push(state.current_local_player_model());
                }
                if !players_to_render.is_empty() {
                    let players_mesh = blockgame_render::build_players_mesh(
                        &players_to_render,
                        &state.item_registry,
                    );
                    state.player_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &players_mesh.vertices,
                        &players_mesh.indices,
                    );
                } else {
                    state.player_dynamic_mesh.clear();
                }

                // 8. Render frame
                let output = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost) => {
                        state
                            .surface
                            .configure(&state.device, &state.surface_config);
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
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Frame Command Encoder"),
                        });

                let camera_block_pos = IVec3::new(
                    state.camera.position.x.floor() as i32,
                    state.camera.position.y.floor() as i32,
                    state.camera.position.z.floor() as i32,
                );
                let is_camera_underwater = {
                    let world = state.world_manager.world().lock().unwrap();
                    world.get_block(camera_block_pos).is_water()
                };
                let clear_color = if is_camera_underwater {
                    wgpu::Color {
                        r: 0.05,
                        g: 0.22,
                        b: 0.45,
                        a: 1.0,
                    }
                } else {
                    let sky = state.day_night.sky_color();
                    wgpu::Color {
                        r: sky[0] as f64,
                        g: sky[1] as f64,
                        b: sky[2] as f64,
                        a: 1.0,
                    }
                };

                // 3D Voxel Terrain Pass
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Main Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear_color),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &state.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    // Render Sun and Moon in the sky behind all terrain
                    render_pass.set_pipeline(&state.pipeline.celestial_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    state.celestial_dynamic_mesh.draw(&mut render_pass);

                    // Render voxel terrain meshes with frustum culling
                    render_pass.set_pipeline(&state.pipeline.render_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    let aspect =
                        state.surface_config.width as f32 / state.surface_config.height as f32;
                    let frustum = state.camera.build_frustum(aspect);
                    state
                        .world_manager
                        .render_with_frustum(&mut render_pass, Some(&frustum));

                    // Render floating 3D item drops
                    state.item_drop_dynamic_mesh.draw(&mut render_pass);

                    // Render active 3D animals
                    state.animal_dynamic_mesh.draw(&mut render_pass);

                    // Render active 3D monsters and arrow projectiles
                    state.monster_dynamic_mesh.draw(&mut render_pass);
                    state.arrow_dynamic_mesh.draw(&mut render_pass);

                    // Render falling gravity blocks (Sand, Gravel, Red Sand)
                    state.falling_block_dynamic_mesh.draw(&mut render_pass);

                    // Render 3D articulated player models (remote players & 3rd-person local player)
                    state.player_dynamic_mesh.draw(&mut render_pass);

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

                    // Render translucent water meshes after all opaque geometry
                    render_pass.set_pipeline(&state.pipeline.water_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    state
                        .world_manager
                        .render_water_with_frustum(&mut render_pass, Some(&frustum));
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

                    if state.ui_dynamic_mesh.index_count > 0 {
                        ui_pass.set_pipeline(&state.ui_pipeline.render_pipeline);
                        ui_pass.set_bind_group(0, &state.ui_pipeline.screen_bind_group, &[]);
                        ui_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                        state.ui_dynamic_mesh.draw(&mut ui_pass);
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
            if state.screen == AppScreen::InGame
                && state.mouse_captured
                && !state.player_stats.is_dead
            {
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

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;

    // 1. Dynamic Crafting Grid (2x2 or 3x3)
    let grid_w = state.crafting_grid.width;
    let grid_h = state.crafting_grid.height;

    let craft_x = (screen_w - ((grid_w as f32 + 2.0) * (slot_size + slot_gap) + 24.0)) * 0.5;
    let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });

    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let sx = craft_x + (gx as f32) * (slot_size + slot_gap);
            let sy = craft_y + (gy as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                let mut slot = state.crafting_grid.get(gx, gy);
                click_slot_raw(
                    &mut slot,
                    &mut state.cursor_held,
                    is_right_click,
                    &state.item_registry,
                );
                state.crafting_grid.set(gx, gy, slot);
                return;
            }
        }
    }

    // 2. Crafting Result Slot
    let result_x = craft_x + (grid_w as f32) * (slot_size + slot_gap) + 32.0;
    let result_y = craft_y + ((grid_h as f32 - 1.0) * (slot_size + slot_gap) * 0.5);
    let result_size = slot_size + 8.0;

    if cx >= result_x
        && cx < result_x + result_size
        && cy >= result_y
        && cy < result_y + result_size
    {
        if state.crafting_result.is_some() {
            if state.shift_down {
                while let Some(crafted) = state.crafting_manager.craft(&mut state.crafting_grid) {
                    if let Some(rem) = state.inventory.add_item(crafted, &state.item_registry) {
                        state.spawn_drop_synced(
                            state.player.position,
                            Vec3::new(0.0, 2.0, 0.0),
                            rem,
                        );
                        break;
                    }
                    if state
                        .crafting_manager
                        .find_match(&state.crafting_grid)
                        .is_none()
                    {
                        break;
                    }
                }
            } else if let Some(crafted) = state.crafting_manager.craft(&mut state.crafting_grid) {
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
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let inv_start_x = (screen_w - inv_w) * 0.5;
    let inv_start_y = craft_y + (grid_h as f32) * (slot_size + slot_gap) + 24.0;

    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = inv_start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_start_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if state.shift_down {
                    let item = state.inventory.get(slot_idx);
                    if !item.is_empty() {
                        for h in 0..9 {
                            if state.inventory.get(h).is_empty() {
                                state.inventory.set(h, item);
                                state.inventory.set(slot_idx, ItemSlot::EMPTY);
                                break;
                            }
                        }
                    }
                } else {
                    state.inventory.click_slot(
                        slot_idx,
                        &mut state.cursor_held,
                        is_right_click,
                        &state.item_registry,
                    );
                }
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
            if state.shift_down {
                let item = state.inventory.get(i);
                if !item.is_empty() {
                    for b in 9..36 {
                        if state.inventory.get(b).is_empty() {
                            state.inventory.set(b, item);
                            state.inventory.set(i, ItemSlot::EMPTY);
                            break;
                        }
                    }
                }
            } else {
                state.inventory.click_slot(
                    i,
                    &mut state.cursor_held,
                    is_right_click,
                    &state.item_registry,
                );
            }
            return;
        }
    }

    // 5. Armor Equipment Slots
    for idx in 0..4 {
        let rect = UiBuilder::armor_slot_rect(screen_w, screen_h, grid_h, idx);
        if cx >= rect[0]
            && cx < rect[0] + rect[2]
            && cy >= rect[1]
            && cy < rect[1] + rect[3]
        {
            if state.cursor_held.is_empty() {
                if !state.armor.slots[idx].is_empty() {
                    if state.shift_down {
                        if let Some(rem) =
                            state.inventory.add_item(state.armor.slots[idx], &state.item_registry)
                        {
                            state.armor.slots[idx] = rem;
                        } else {
                            state.armor.slots[idx] = ItemSlot::EMPTY;
                        }
                    } else {
                        state.cursor_held =
                            std::mem::replace(&mut state.armor.slots[idx], ItemSlot::EMPTY);
                    }
                }
            } else if let Some((armor_slot, _)) =
                state.item_registry.get_armor(state.cursor_held.item_id)
            {
                if armor_slot.index() == idx {
                    let prev =
                        std::mem::replace(&mut state.armor.slots[idx], state.cursor_held);
                    state.cursor_held = prev;
                }
            }
            return;
        }
    }
}

fn handle_chest_click(state: &mut AppState, is_right_click: bool) {
    let Some(chest_pos) = state.active_chest_pos else {
        return;
    };
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;

    // 1. Chest Container Slots (3 rows x 9 columns = 27 slots)
    let chest_y = screen_h * 0.5 - 220.0;
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = chest_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                let mut world_guard = state.world_manager.world().lock().unwrap();
                let chest_slots = world_guard.get_or_create_chest(chest_pos);
                if slot_idx < chest_slots.len() {
                    if state.shift_down {
                        let item = chest_slots[slot_idx];
                        if !item.is_empty() {
                            if let Some(rem) = state.inventory.add_item(item, &state.item_registry)
                            {
                                chest_slots[slot_idx] = rem;
                            } else {
                                chest_slots[slot_idx] = ItemSlot::EMPTY;
                            }
                        }
                    } else {
                        click_slot_raw(
                            &mut chest_slots[slot_idx],
                            &mut state.cursor_held,
                            is_right_click,
                            &state.item_registry,
                        );
                    }
                }
                return;
            }
        }
    }

    // 2. Backpack Storage Grid (3 rows x 9 columns)
    let inv_y = chest_y + 3.0 * (slot_size + slot_gap) + 26.0;
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if state.shift_down {
                    let slot_val = state.inventory.slots[slot_idx];
                    if !slot_val.is_empty() {
                        let mut world_guard = state.world_manager.world().lock().unwrap();
                        let chest_slots = world_guard.get_or_create_chest(chest_pos);
                        let rem = add_to_chest_slots(chest_slots, slot_val, &state.item_registry);
                        state.inventory.slots[slot_idx] = rem.unwrap_or(ItemSlot::EMPTY);
                    }
                } else {
                    state.inventory.click_slot(
                        slot_idx,
                        &mut state.cursor_held,
                        is_right_click,
                        &state.item_registry,
                    );
                }
                return;
            }
        }
    }

    // 3. Hotbar Slots
    let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
    for i in 0..9 {
        let sx = start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;

        if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
            if state.shift_down {
                let slot_val = state.inventory.slots[i];
                if !slot_val.is_empty() {
                    let mut world_guard = state.world_manager.world().lock().unwrap();
                    let chest_slots = world_guard.get_or_create_chest(chest_pos);
                    let rem = add_to_chest_slots(chest_slots, slot_val, &state.item_registry);
                    state.inventory.slots[i] = rem.unwrap_or(ItemSlot::EMPTY);
                }
            } else {
                state.inventory.click_slot(
                    i,
                    &mut state.cursor_held,
                    is_right_click,
                    &state.item_registry,
                );
            }
            return;
        }
    }
}

fn handle_furnace_click(state: &mut AppState, is_right_click: bool) {
    let Some(furnace_pos) = state.active_furnace_pos else {
        return;
    };
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;

    let furnace_top_y = screen_h * 0.5 - 220.0;
    let center_x = start_x + (inv_w - slot_size) * 0.5 - 55.0;
    let input_y = furnace_top_y + 10.0;
    let flame_size = 24.0;
    let flame_y = input_y + slot_size + 8.0;
    let fuel_y = flame_y + flame_size + 8.0;

    let arrow_x = center_x + slot_size + 24.0;
    let _arrow_y = input_y + (fuel_y + slot_size - input_y) * 0.5 - 7.0;
    let out_x = arrow_x + 64.0;
    let out_y = input_y + (fuel_y + slot_size - input_y) * 0.5 - 28.0;
    let out_size = 56.0;

    // 1. Furnace Input Slot
    if cx >= center_x && cx < center_x + slot_size && cy >= input_y && cy < input_y + slot_size {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        let furnace = world_guard.get_or_create_furnace(furnace_pos);
        if state.shift_down {
            let item = furnace.input;
            if !item.is_empty() {
                if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                    furnace.input = rem;
                } else {
                    furnace.input = ItemSlot::EMPTY;
                }
            }
        } else {
            click_slot_raw(
                &mut furnace.input,
                &mut state.cursor_held,
                is_right_click,
                &state.item_registry,
            );
        }
        return;
    }

    // 2. Furnace Fuel Slot
    if cx >= center_x && cx < center_x + slot_size && cy >= fuel_y && cy < fuel_y + slot_size {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        let furnace = world_guard.get_or_create_furnace(furnace_pos);
        if state.shift_down {
            let item = furnace.fuel;
            if !item.is_empty() {
                if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                    furnace.fuel = rem;
                } else {
                    furnace.fuel = ItemSlot::EMPTY;
                }
            }
        } else {
            click_slot_raw(
                &mut furnace.fuel,
                &mut state.cursor_held,
                is_right_click,
                &state.item_registry,
            );
        }
        return;
    }

    // 3. Furnace Output Slot
    if cx >= out_x && cx < out_x + out_size && cy >= out_y && cy < out_y + out_size {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        let furnace = world_guard.get_or_create_furnace(furnace_pos);
        if state.shift_down {
            let item = furnace.output;
            if !item.is_empty() {
                if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                    furnace.output = rem;
                } else {
                    furnace.output = ItemSlot::EMPTY;
                }
            }
        } else {
            // Can only TAKE from output slot, cannot place items into it
            if state.cursor_held.is_empty() {
                state.cursor_held = furnace.output;
                furnace.output = ItemSlot::EMPTY;
            } else if state.cursor_held.can_stack_with(&furnace.output) {
                let max_stack = state.item_registry.max_stack_size(furnace.output.item_id);
                let space = max_stack - state.cursor_held.count;
                let take = space.min(furnace.output.count);
                state.cursor_held.count += take;
                furnace.output.count -= take;
                if furnace.output.count == 0 {
                    furnace.output = ItemSlot::EMPTY;
                }
            }
        }
        return;
    }

    // 4. Backpack Storage Grid (3 rows x 9 columns)
    let inv_y = fuel_y + slot_size + 24.0;
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if state.shift_down {
                    let item = state.inventory.get(slot_idx);
                    if !item.is_empty() {
                        let mut world_guard = state.world_manager.world().lock().unwrap();
                        let furnace = world_guard.get_or_create_furnace(furnace_pos);
                        // Priority 1: If item is fuel, try transfer into fuel slot
                        if blockgame_core::crafting::get_fuel_burn_time(item.item_id) > 0.0 {
                            if furnace.fuel.is_empty() {
                                furnace.fuel = item;
                                state.inventory.set(slot_idx, ItemSlot::EMPTY);
                                return;
                            } else if furnace.fuel.can_stack_with(&item) {
                                let max_stack = state.item_registry.max_stack_size(item.item_id);
                                let space = max_stack - furnace.fuel.count;
                                let transfer = space.min(item.count);
                                furnace.fuel.count += transfer;
                                let remaining = item.count - transfer;
                                state.inventory.set(
                                    slot_idx,
                                    if remaining > 0 {
                                        ItemSlot::new(item.item_id, remaining)
                                    } else {
                                        ItemSlot::EMPTY
                                    },
                                );
                                return;
                            }
                        }
                        // Priority 2: If item is smeltable, try transfer into input slot
                        if blockgame_core::crafting::get_smelting_output(item.item_id).is_some() {
                            if furnace.input.is_empty() {
                                furnace.input = item;
                                state.inventory.set(slot_idx, ItemSlot::EMPTY);
                                return;
                            } else if furnace.input.can_stack_with(&item) {
                                let max_stack = state.item_registry.max_stack_size(item.item_id);
                                let space = max_stack - furnace.input.count;
                                let transfer = space.min(item.count);
                                furnace.input.count += transfer;
                                let remaining = item.count - transfer;
                                state.inventory.set(
                                    slot_idx,
                                    if remaining > 0 {
                                        ItemSlot::new(item.item_id, remaining)
                                    } else {
                                        ItemSlot::EMPTY
                                    },
                                );
                                return;
                            }
                        }
                    }
                } else {
                    state.inventory.click_slot(
                        slot_idx,
                        &mut state.cursor_held,
                        is_right_click,
                        &state.item_registry,
                    );
                }
                return;
            }
        }
    }

    // 5. Hotbar Slots
    let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
    for i in 0..9 {
        let sx = start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;

        if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
            if state.shift_down {
                let item = state.inventory.get(i);
                if !item.is_empty() {
                    let mut world_guard = state.world_manager.world().lock().unwrap();
                    let furnace = world_guard.get_or_create_furnace(furnace_pos);
                    if blockgame_core::crafting::get_fuel_burn_time(item.item_id) > 0.0 {
                        if furnace.fuel.is_empty() {
                            furnace.fuel = item;
                            state.inventory.set(i, ItemSlot::EMPTY);
                            return;
                        } else if furnace.fuel.can_stack_with(&item) {
                            let max_stack = state.item_registry.max_stack_size(item.item_id);
                            let space = max_stack - furnace.fuel.count;
                            let transfer = space.min(item.count);
                            furnace.fuel.count += transfer;
                            let remaining = item.count - transfer;
                            state.inventory.set(
                                i,
                                if remaining > 0 {
                                    ItemSlot::new(item.item_id, remaining)
                                } else {
                                    ItemSlot::EMPTY
                                },
                            );
                            return;
                        }
                    }
                    if blockgame_core::crafting::get_smelting_output(item.item_id).is_some() {
                        if furnace.input.is_empty() {
                            furnace.input = item;
                            state.inventory.set(i, ItemSlot::EMPTY);
                            return;
                        } else if furnace.input.can_stack_with(&item) {
                            let max_stack = state.item_registry.max_stack_size(item.item_id);
                            let space = max_stack - furnace.input.count;
                            let transfer = space.min(item.count);
                            furnace.input.count += transfer;
                            let remaining = item.count - transfer;
                            state.inventory.set(
                                i,
                                if remaining > 0 {
                                    ItemSlot::new(item.item_id, remaining)
                                } else {
                                    ItemSlot::EMPTY
                                },
                            );
                            return;
                        }
                    }
                }
            } else {
                state.inventory.click_slot(
                    i,
                    &mut state.cursor_held,
                    is_right_click,
                    &state.item_registry,
                );
            }
            return;
        }
    }
}

fn add_to_chest_slots(
    chest_slots: &mut [ItemSlot],
    mut item: ItemSlot,
    registry: &ItemRegistry,
) -> Option<ItemSlot> {
    if item.is_empty() {
        return None;
    }
    let max_stack = registry.max_stack_size(item.item_id);

    // 1. Try to merge into existing stacks
    for slot in chest_slots.iter_mut() {
        if slot.can_stack_with(&item) && slot.count < max_stack {
            let space = max_stack - slot.count;
            let transfer = space.min(item.count);
            slot.count += transfer;
            item.count -= transfer;
            if item.count == 0 {
                return None;
            }
        }
    }

    // 2. Put remainder into first empty slot
    for slot in chest_slots.iter_mut() {
        if slot.is_empty() {
            let transfer = max_stack.min(item.count);
            *slot = ItemSlot::new(item.item_id, transfer);
            item.count -= transfer;
            if item.count == 0 {
                return None;
            }
        }
    }

    if item.count > 0 {
        Some(item)
    } else {
        None
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

fn handle_recipe_modal_click(state: &mut AppState, is_right_click: bool) -> bool {
    let Some(modal) = state.jei_recipe_modal.clone() else {
        return false;
    };
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    let modal_w = 460.0;
    let modal_h = 340.0;
    let mx = (screen_w - modal_w) * 0.5;
    let my = (screen_h - modal_h) * 0.5;

    // Check if clicked outside modal: close modal!
    if cx < mx || cx > mx + modal_w || cy < my || cy > my + modal_h {
        state.jei_recipe_modal = None;
        return true;
    }

    // Close button [X]
    let close_size = 24.0;
    let close_x = mx + modal_w - close_size - 6.0;
    let close_y = my + 6.0;
    if cx >= close_x && cx <= close_x + close_size && cy >= close_y && cy <= close_y + close_size {
        state.jei_recipe_modal = None;
        return true;
    }

    // Recipe cycle controls (< and >)
    let recipes = if modal.is_usage_mode {
        state
            .crafting_manager
            .get_recipes_using_input(modal.target_item)
    } else {
        state
            .crafting_manager
            .get_recipes_for_output(modal.target_item)
    };
    let total_recipes = recipes.len();

    let sub_y = my + 44.0;
    let btn_w = 22.0;
    let btn_h = 20.0;
    let prev_rx = mx + 20.0;
    let next_rx = mx + 50.0;

    if total_recipes > 1 {
        if cx >= prev_rx && cx <= prev_rx + btn_w && cy >= sub_y && cy <= sub_y + btn_h {
            if let Some(m) = &mut state.jei_recipe_modal {
                m.recipe_index = (m.recipe_index + total_recipes - 1) % total_recipes;
            }
            return true;
        }
        if cx >= next_rx && cx <= next_rx + btn_w && cy >= sub_y && cy <= sub_y + btn_h {
            if let Some(m) = &mut state.jei_recipe_modal {
                m.recipe_index = (m.recipe_index + 1) % total_recipes;
            }
            return true;
        }
    }

    // Active recipe slots (3x3 ingredients & output)
    if let Some(recipe) = recipes.get(modal.recipe_index) {
        let box_x = mx + 20.0;
        let box_y = my + 76.0;
        let slot_size = UiBuilder::SLOT_SIZE;
        let slot_gap = UiBuilder::SLOT_GAP;
        let grid_start_x = box_x + 28.0;
        let grid_start_y = box_y + 24.0;

        // 3x3 Inputs
        for gy in 0..3 {
            for gx in 0..3 {
                let idx = gy * 3 + gx;
                let sx = grid_start_x + (gx as f32) * (slot_size + slot_gap);
                let sy = grid_start_y + (gy as f32) * (slot_size + slot_gap);

                if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                    if let Some(ing_id) = recipe.inputs[idx] {
                        state.jei_recipe_modal = Some(JeiRecipeModalState {
                            target_item: ing_id,
                            is_usage_mode: is_right_click,
                            recipe_index: 0,
                        });
                    }
                    return true;
                }
            }
        }

        // Output Slot
        let arrow_x = grid_start_x + 3.0 * (slot_size + slot_gap) + 24.0;
        let out_x = arrow_x + 64.0;
        let out_y = grid_start_y + (slot_size + slot_gap) * 1.5 - 28.0;
        let out_size = 56.0;

        if cx >= out_x && cx <= out_x + out_size && cy >= out_y && cy <= out_y + out_size {
            let out_slot = recipe.output;
            if !out_slot.is_empty() {
                state.jei_recipe_modal = Some(JeiRecipeModalState {
                    target_item: out_slot.item_id,
                    is_usage_mode: is_right_click,
                    recipe_index: 0,
                });
            }
            return true;
        }
    }

    true
}

fn handle_jei_panel_click(state: &mut AppState, is_right_click: bool) -> bool {
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    let slot_size = UiBuilder::JEI_SLOT_SIZE;
    let slot_gap = UiBuilder::JEI_SLOT_GAP;
    let cols = UiBuilder::JEI_COLS;
    let rows = UiBuilder::JEI_ROWS;
    let panel_w = (cols as f32) * (slot_size + slot_gap) - slot_gap;
    let start_x = screen_w - panel_w - 20.0;
    let start_y = screen_h * 0.5 - 210.0;

    let page_y = start_y - 28.0;
    let btn_w = 26.0;
    let btn_h = 20.0;
    let search_y = start_y + (rows as f32) * (slot_size + slot_gap) + 8.0;
    let search_h = 24.0;

    // Filter catalog
    let search_lower = state.jei_search_text.trim().to_lowercase();
    let catalog: Vec<ItemId> = state
        .item_registry
        .all_items()
        .into_iter()
        .filter(|def| {
            if search_lower.is_empty() {
                true
            } else {
                def.display_name.to_lowercase().contains(&search_lower)
            }
        })
        .map(|def| def.id)
        .collect();
    let items_per_page = cols * rows;
    let total_pages = ((catalog.len() + items_per_page - 1) / items_per_page).max(1);

    // Prev Button [<]
    let prev_x = start_x;
    if cx >= prev_x && cx <= prev_x + btn_w && cy >= page_y && cy <= page_y + btn_h {
        if state.jei_page > 0 {
            state.jei_page -= 1;
        } else {
            state.jei_page = total_pages.saturating_sub(1);
        }
        return true;
    }

    // Next Button [>]
    let next_x = start_x + panel_w - btn_w;
    if cx >= next_x && cx <= next_x + btn_w && cy >= page_y && cy <= page_y + btn_h {
        state.jei_page = (state.jei_page + 1) % total_pages;
        return true;
    }

    // Search Bar
    if cx >= start_x && cx <= start_x + panel_w && cy >= search_y && cy <= search_y + search_h {
        if is_right_click {
            state.jei_search_text.clear();
            state.jei_page = 0;
        }
        state.jei_search_focused = true;
        return true;
    }

    // Item Catalog Grid
    let page_offset = state.jei_page * items_per_page;
    for r in 0..rows {
        for c in 0..cols {
            let slot_idx = page_offset + r * cols + c;
            let sx = start_x + (c as f32) * (slot_size + slot_gap);
            let sy = start_y + (r as f32) * (slot_size + slot_gap);

            if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                if slot_idx < catalog.len() {
                    let clicked_id = catalog[slot_idx];
                    state.jei_recipe_modal = Some(JeiRecipeModalState {
                        target_item: clicked_id,
                        is_usage_mode: is_right_click,
                        recipe_index: 0,
                    });
                }
                return true;
            }
        }
    }

    // Check if clicked anywhere inside JEI panel bounding box
    if cx >= start_x - 6.0
        && cx <= start_x + panel_w + 6.0
        && cy >= page_y - 6.0
        && cy <= search_y + search_h + 6.0
    {
        state.jei_search_focused = false;
        return true;
    }

    // Outside JEI panel -> unfocus search bar
    state.jei_search_focused = false;
    false
}

fn find_hovered_item(state: &AppState) -> Option<ItemId> {
    let screen_w = state.surface_config.width as f32;
    let screen_h = state.surface_config.height as f32;
    let cx = state.cursor_pos[0];
    let cy = state.cursor_pos[1];

    // 1. Check JEI Grid
    let slot_size_jei = UiBuilder::JEI_SLOT_SIZE;
    let slot_gap_jei = UiBuilder::JEI_SLOT_GAP;
    let cols = UiBuilder::JEI_COLS;
    let rows = UiBuilder::JEI_ROWS;
    let panel_w = (cols as f32) * (slot_size_jei + slot_gap_jei) - slot_gap_jei;
    let start_x_jei = screen_w - panel_w - 20.0;
    let start_y_jei = screen_h * 0.5 - 210.0;

    let search_lower = state.jei_search_text.trim().to_lowercase();
    let catalog: Vec<ItemId> = state
        .item_registry
        .all_items()
        .into_iter()
        .filter(|def| {
            if search_lower.is_empty() {
                true
            } else {
                def.display_name.to_lowercase().contains(&search_lower)
            }
        })
        .map(|def| def.id)
        .collect();
    let items_per_page = cols * rows;
    let page_offset = state.jei_page * items_per_page;

    for r in 0..rows {
        for c in 0..cols {
            let slot_idx = page_offset + r * cols + c;
            let sx = start_x_jei + (c as f32) * (slot_size_jei + slot_gap_jei);
            let sy = start_y_jei + (r as f32) * (slot_size_jei + slot_gap_jei);

            if cx >= sx && cx <= sx + slot_size_jei && cy >= sy && cy <= sy + slot_size_jei {
                if slot_idx < catalog.len() {
                    return Some(catalog[slot_idx]);
                }
            }
        }
    }

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;

    // 2. Check Furnace Slots if Furnace open
    if let Some(furnace_pos) = state.active_furnace_pos {
        let furnace_top_y = screen_h * 0.5 - 220.0;
        let center_x = start_x + (inv_w - slot_size) * 0.5 - 55.0;
        let input_y = furnace_top_y + 10.0;
        let flame_size = 24.0;
        let flame_y = input_y + slot_size + 8.0;
        let fuel_y = flame_y + flame_size + 8.0;

        let arrow_x = center_x + slot_size + 24.0;
        let out_x = arrow_x + 64.0;
        let out_y = input_y + (fuel_y + slot_size - input_y) * 0.5 - 28.0;
        let out_size = 56.0;

        let world_guard = state.world_manager.world().lock().unwrap();
        if let Some(furnace) = world_guard.get_furnace(furnace_pos) {
            // Input
            if cx >= center_x
                && cx <= center_x + slot_size
                && cy >= input_y
                && cy <= input_y + slot_size
            {
                if !furnace.input.is_empty() {
                    return Some(furnace.input.item_id);
                }
            }
            // Fuel
            if cx >= center_x
                && cx <= center_x + slot_size
                && cy >= fuel_y
                && cy <= fuel_y + slot_size
            {
                if !furnace.fuel.is_empty() {
                    return Some(furnace.fuel.item_id);
                }
            }
            // Output
            if cx >= out_x && cx <= out_x + out_size && cy >= out_y && cy <= out_y + out_size {
                if !furnace.output.is_empty() {
                    return Some(furnace.output.item_id);
                }
            }
        }
    } else if let Some(chest_pos) = state.active_chest_pos {
        let chest_y = screen_h * 0.5 - 220.0;

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = chest_y + (row as f32) * (slot_size + slot_gap);

                if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                    let world_guard = state.world_manager.world().lock().unwrap();
                    if let Some(chest) = world_guard.get_chest(chest_pos) {
                        if let Some(slot) = chest.get(slot_idx) {
                            if !slot.is_empty() {
                                return Some(slot.item_id);
                            }
                        }
                    }
                }
            }
        }
    } else {
        // 2b. Check Crafting Grid & Result Slot
        let grid_w = state.crafting_grid.width;
        let grid_h = state.crafting_grid.height;
        let craft_x = (screen_w - ((grid_w as f32 + 2.0) * (slot_size + slot_gap) + 24.0)) * 0.5;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });

        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let sx = craft_x + (gx as f32) * (slot_size + slot_gap);
                let sy = craft_y + (gy as f32) * (slot_size + slot_gap);

                if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                    let slot = state.crafting_grid.get(gx, gy);
                    if !slot.is_empty() {
                        return Some(slot.item_id);
                    }
                }
            }
        }

        // Result slot
        let result_x = craft_x + (grid_w as f32) * (slot_size + slot_gap) + 32.0;
        let result_y = craft_y + ((grid_h as f32 - 1.0) * (slot_size + slot_gap) * 0.5);
        let r_size = slot_size + 8.0;
        if cx >= result_x && cx <= result_x + r_size && cy >= result_y && cy <= result_y + r_size {
            if let Some(res) = state.crafting_result {
                if !res.is_empty() {
                    return Some(res.item_id);
                }
            }
        }
    }

    // 3. Check Backpack and Hotbar
    let inv_start_y = if state.active_furnace_pos.is_some() {
        let furnace_top_y = screen_h * 0.5 - 220.0;
        let input_y = furnace_top_y + 10.0;
        let flame_size = 24.0;
        let flame_y = input_y + slot_size + 8.0;
        let fuel_y = flame_y + flame_size + 8.0;
        fuel_y + slot_size + 24.0
    } else if state.active_chest_pos.is_some() {
        screen_h * 0.5 - 220.0 + 3.0 * (slot_size + slot_gap) + 26.0
    } else {
        let grid_h = state.crafting_grid.height;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });
        craft_y + (grid_h as f32) * (slot_size + slot_gap) + 24.0
    };

    // Backpack (3x9)
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_start_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                let slot = state.inventory.get(slot_idx);
                if !slot.is_empty() {
                    return Some(slot.item_id);
                }
            }
        }
    }

    // Hotbar (1x9)
    let hotbar_y = inv_start_y + 3.0 * (slot_size + slot_gap) + 16.0;
    for i in 0..9 {
        let sx = start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;

        if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
            let slot = state.inventory.get(i);
            if !slot.is_empty() {
                return Some(slot.item_id);
            }
        }
    }

    None
}
