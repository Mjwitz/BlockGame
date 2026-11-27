use std::collections::HashMap;
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
    skills::{
        break_speed_skill_for_block, skill_xp_for_broken_block, PlayerSkills, SkillKind,
    },
    tool::ToolTier,
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, ui::ChatActionHitbox, DayNightCycle, UiBuilder, UiMesh, UiPipeline, VoxelPipeline,
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
        game_mode: blockgame_core::GameMode,
    },
    Settings {
        return_to_pause: bool,
    },
    MultiplayerLobby {
        username_input: String,
        password_input: String,
        address_input: String,
        focused_field: usize,
        status_message: Option<(String, bool)>,
    },
    CharacterCreator {
        tab: blockgame_render::CharacterCreatorTab,
        draft: blockgame_core::CharacterAppearance,
        initial_snapshot: blockgame_core::CharacterAppearance,
        selected_preset_name: Option<String>,
        preset_name_input: String,
        preset_input_focused: bool,
        preview_yaw: f32,
        face_zoom: bool,
        walk_preview: bool,
        active_color_target: usize,
        status_message: Option<String>,
        return_to_multiplayer: bool,
    },
    InGame,
    Paused {
        status_message: Option<String>,
    },
    Guide {
        page: usize,
    },
    Achievements {
        return_to_pause: bool,
    },
}

#[derive(Debug, Clone)]
pub enum FishingSession {
    Idle,
    Waiting {
        bobber_pos: Vec3,
        bobber_vel: Vec3,
        landed: bool,
        wait_timer: f32,
        is_ocean: bool,
        rod_slot: usize,
        tackle: blockgame_core::FishingTackleStats,
    },
    Biting {
        bobber_pos: Vec3,
        bite_timer: f32,
        species: blockgame_core::FishSpecies,
        rod_slot: usize,
        tackle: blockgame_core::FishingTackleStats,
    },
    Minigame {
        bobber_pos: Vec3,
        game: blockgame_core::FishingMinigameState,
        rod_slot: usize,
        tackle: blockgame_core::FishingTackleStats,
    },
}

#[derive(Debug, Clone)]
pub struct EatingState {
    pub slot: usize,
    pub timer: f32,
    pub duration: f32,
    pub crunch_accum: f32,
}

pub struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    supported_present_modes: Vec<wgpu::PresentMode>,
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
    pub settings: crate::settings::GameSettings,
    pub active_settings_slider: Option<usize>,
    pub settings_tab: usize,
    pub listening_keybind: Option<usize>,
    pub listening_controller_bind: Option<usize>,
    pub gamepad_backend: crate::controller::GilrsBackend,
    pub input_router: crate::controller::InputRouter,
    pub quick_menu_open: bool,
    pub hud_toast: Option<(String, f32)>,
    pub controller_move_axes: [f32; 2],
    pub controller_sneaking: bool,
    pub controller_jumping: bool,
    pub controller_sprinting: bool,
    pub controller_mine_held: bool,
    pub controller_use_held: bool,
    pub controller_ptt_held: bool,
    pub controller_player_list_held: bool,
    pub ui_scroll_cooldown: f32,
    pub last_rumble_health: f32,
    pub registry: Arc<BlockRegistry>,
    // Milestone 5: Inventory, Crafting, and UI
    pub inventory: Inventory,
    pub armor: blockgame_core::item::ArmorEquipment,
    pub active_hotbar_slot: usize,
    pub hotbar_activity_timer: f32,
    pub hotbar_tuck_progress: f32,
    pub item_registry: Arc<ItemRegistry>,
    pub crafting_manager: CraftingManager,
    pub crafting_grid: CraftingGrid,
    pub crafting_result: Option<ItemSlot>,
    pub inventory_open: bool,
    pub active_chest_pos: Option<IVec3>,
    pub active_furnace_pos: Option<IVec3>,
    pub active_cooking_pot_pos: Option<IVec3>,
    pub active_shop_snapshot: Option<blockgame_core::ShopUiSnapshot>,
    pub shop_tab: blockgame_render::ShopUiTab,
    pub shop_selected_enhance_slot: Option<usize>,
    pub shop_status_message: String,
    pub eating: Option<EatingState>,
    pub active_coal_gen_pos: Option<IVec3>,
    pub active_battery_pos: Option<IVec3>,
    pub active_electric_furnace_pos: Option<IVec3>,
    pub active_storage_terminal_pos: Option<IVec3>,
    pub storage_search_text: String,
    pub storage_search_focused: bool,
    pub storage_sort_mode: blockgame_core::storage::StorageSortMode,
    pub storage_page: usize,
    pub chest_page: usize,
    pub backpacks: blockgame_core::storage::BackpackRegistry,
    pub active_backpack_id: Option<u16>,
    pub backpack_page: usize,
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
    pub wind_turbine_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub player_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub player_held_items_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub preview_character_dynamic_mesh: blockgame_render::DynamicMeshBuffers,
    pub character_pipeline: blockgame_render::CharacterPipeline,
    // Multiplayer & 3D Player Model State
    pub mp_client: Option<blockgame_core::net::MultiplayerClient>,
    pub mp_hosted_server: Option<blockgame_core::net::MultiplayerServer>,
    pub local_username: String,
    pub character_profile: blockgame_core::CharacterProfile,
    pub creator_dragging_preview: bool,
    pub creator_last_drag_x: f32,
    pub creator_active_rgb_slider: Option<usize>,
    pub creator_randomize_counter: u64,
    pub third_person: bool,
    pub chat_open: bool,
    pub chat_input: String,
    pub chat_scroll: usize,
    pub chat_hitboxes: Vec<ChatActionHitbox>,
    pub player_homes: HashMap<String, (Vec3, f32, f32)>,
    pub tab_held: bool,
    pub net_sync_timer: f32,
    pub next_mp_drop_nonce: u32,
    pub local_chat_messages: Vec<blockgame_core::net::ChatEntry>,
    pub total_time: f32,
    pub fps_display: u32,
    pub fps_frame_count: u32,
    pub fps_timer: Instant,
    pub walk_distance: f32,
    pub punch_timer: f32,
    pub last_footstep_dist: f32,
    pub break_hit_timer: f32,
    pub was_in_water: bool,
    pub underwater_sound_timer: f32,
    pub audio: crate::audio::AudioManager,
    pub music_scheduler: crate::audio::MusicScheduler,
    pub mining_music_grace: f32,
    pub creature_sound_timer: f32,
    pub voice_capture: crate::voice::VoiceCaptureManager,
    pub ptt_down: bool,
    pub voice_transmitting: bool,
    pub speaking_players: HashMap<u32, f32>,
    pub directory_servers: Vec<blockgame_core::directory::DirectoryServerEntry>,
    pub selected_directory_idx: Option<usize>,
    pub directory_loading: bool,
    pub directory_error: Option<String>,
    pub directory_rx: Option<
        std::sync::mpsc::Receiver<Result<Vec<blockgame_core::directory::DirectoryServerEntry>, String>>,
    >,
    // Hold-to-break mining state
    pub mouse_left_down: bool,
    pub mouse_right_down: bool,
    pub breaking_voxel: Option<IVec3>,
    pub break_progress: f32,
    pub creative_break_cooldown: f32,
    pub cracking_buffer: Option<(wgpu::Buffer, u32)>,
    pub fishing_state: FishingSession,
    pub fishing_rng: u64,
    // MMO Skills System
    pub player_skills: PlayerSkills,
    pub skills_open: bool,
    pub selected_skill_idx: usize,
    pub skill_toast: Option<(String, String, f32)>,
    pub skills_dirty: bool,
    pub agility_xp_timer: f32,
    // Achievements System (Issue #18, ADR 0007)
    pub achievements: blockgame_core::AchievementEngine,
    pub achievement_toasts: crate::achievement_toast::AchievementToastQueue,
    pub achievements_open: bool,
    pub achievements_category: usize,
    pub achievements_scroll: usize,
    pub biome_check_timer: f32,
    // Movement inputs
    moving_forward: bool,
    moving_backward: bool,
    moving_left: bool,
    moving_right: bool,
    jumping: bool,
    last_jump_tap_time: f32,
    sprinting: bool,
    mouse_captured: bool,
    last_frame_time: Instant,
}

impl AppState {
    /// Returns true if the player is currently underground (earthen/stone ceiling above or deep cavern).
    pub fn is_player_underground(&self) -> bool {
        if self.screen != AppScreen::InGame {
            return false;
        }
        let px = self.player.position.x.floor() as i32;
        let py = (self.player.position.y + 1.8).floor() as i32;
        let pz = self.player.position.z.floor() as i32;
        let world_guard = self.world_manager.world().lock().unwrap();
        let world_height =
            blockgame_core::chunk::VERTICAL_CHUNKS * blockgame_core::chunk::CHUNK_SIZE_Y as i32;
        let max_y = (py + 28).min(world_height - 1);
        for y in (py + 1)..=max_y {
            let block = world_guard.get_block(IVec3::new(px, y, pz));
            if matches!(
                block,
                BlockId::STONE
                    | BlockId::DIRT
                    | BlockId::GRASS
                    | BlockId::COBBLESTONE
                    | BlockId::BEDROCK
                    | BlockId::COAL_ORE
                    | BlockId::COPPER_ORE
                    | BlockId::IRON_ORE
                    | BlockId::GOLD_ORE
                    | BlockId::DIAMOND_ORE
                    | BlockId::SAND
                    | BlockId::RED_SAND
                    | BlockId::SANDSTONE
                    | BlockId::GRAVEL
                    | BlockId::CLAY
                    | BlockId::TERRACOTTA
                    | BlockId::PODZOL
                    | BlockId::MOSSY_COBBLESTONE
                    | BlockId::SNOW_BLOCK
                    | BlockId::ICE
            ) {
                return true;
            }
        }
        self.player.position.y < 34.0
    }

    pub fn refresh_saved_worlds(&mut self) {
        self.saved_worlds = list_saved_worlds(&self.saves_root);
    }

    /// Spawns a background request to fetch discoverable servers from `self.settings.directory_url`.
    pub fn refresh_directory_servers(&mut self) {
        if self.directory_loading {
            return;
        }
        let url = self.settings.directory_url.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.directory_loading = true;
        self.directory_error = None;
        self.directory_rx = Some(rx);
        std::thread::Builder::new()
            .name("directory-fetch".to_string())
            .spawn(move || {
                let res = blockgame_core::directory::fetch_servers(&url);
                let _ = tx.send(res);
            })
            .ok();
    }

    /// Polls any in-flight directory server list fetch.
    pub fn poll_directory_servers(&mut self) {
        if let Some(rx) = &self.directory_rx {
            if let Ok(res) = rx.try_recv() {
                self.directory_loading = false;
                self.directory_rx = None;
                match res {
                    Ok(list) => {
                        self.directory_servers = list;
                        self.directory_error = None;
                        if let Some(sel) = self.selected_directory_idx {
                            if sel >= self.directory_servers.len() {
                                self.selected_directory_idx = None;
                            }
                        }
                    }
                    Err(e) => {
                        self.directory_error = Some(e);
                    }
                }
            }
        }
    }

    /// Applies `self.settings` live to camera FOV, render distance, audio volumes, VSync, and fullscreen mode.
    pub fn apply_settings(&mut self) {
        self.settings.clamp_all();
        self.render_distance = self.settings.render_distance;
        self.world_manager
            .set_render_distance(self.settings.render_distance);
        self.camera.fovy = self.settings.fov_degrees.to_radians();
        self.audio.set_volumes(
            self.settings.master_volume,
            self.settings.sfx_volume,
            self.settings.music_volume,
        );
        self.audio.set_voice_volume(self.settings.voice_volume);
        self.audio.set_creature_volume(self.settings.creature_volume);
        let desired_fs = if self.settings.fullscreen {
            Some(winit::window::Fullscreen::Borderless(None))
        } else {
            None
        };
        if self.window.fullscreen() != desired_fs {
            self.window.set_fullscreen(desired_fs);
        }

        let desired_present = if self.settings.vsync {
            if self
                .supported_present_modes
                .contains(&wgpu::PresentMode::AutoVsync)
            {
                wgpu::PresentMode::AutoVsync
            } else if self
                .supported_present_modes
                .contains(&wgpu::PresentMode::Fifo)
            {
                wgpu::PresentMode::Fifo
            } else {
                self.surface_config.present_mode
            }
        } else if self
            .supported_present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else if self
            .supported_present_modes
            .contains(&wgpu::PresentMode::AutoNoVsync)
        {
            wgpu::PresentMode::AutoNoVsync
        } else if self
            .supported_present_modes
            .contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else {
            self.surface_config.present_mode
        };

        if self.surface_config.present_mode != desired_present {
            self.surface_config.present_mode = desired_present;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    /// Returns `(virtual_screen_w, virtual_screen_h, virtual_cursor_pos)` scaled by `settings.ui_scale`.
    #[inline]
    pub fn ui_metrics(&self) -> (f32, f32, [f32; 2]) {
        let scale = self.settings.ui_scale.max(0.5);
        (
            self.surface_config.width as f32 / scale,
            self.surface_config.height as f32 / scale,
            [self.cursor_pos[0] / scale, self.cursor_pos[1] / scale],
        )
    }

    /// Builds a `SettingsUiState` snapshot for `UiBuilder::build_settings_screen`.
    pub fn settings_ui_state(&self) -> blockgame_render::ui::SettingsUiState {
        let active_controller_label = match self.input_router.active_controller_info() {
            Some(info) => format!("{} ({})", info.name, info.family.display_name()),
            None => "No Controller Connected".to_string(),
        };
        let active_glyph_family = self
            .input_router
            .resolved_glyph_family(self.settings.controller_glyph_style);
        blockgame_render::ui::SettingsUiState {
            render_distance: self.settings.render_distance,
            fov_degrees: self.settings.fov_degrees,
            mouse_sensitivity: self.settings.mouse_sensitivity,
            master_volume: self.settings.master_volume,
            sfx_volume: self.settings.sfx_volume,
            music_volume: self.settings.music_volume,
            invert_y: self.settings.invert_y,
            fullscreen: self.settings.fullscreen,
            vsync: self.settings.vsync,
            view_bobbing: self.settings.view_bobbing,
            show_debug_hud: self.settings.show_debug_hud,
            ui_scale: self.settings.ui_scale,
            voice_mode: self.settings.voice_mode,
            voice_volume: self.settings.voice_volume,
            creature_volume: self.settings.creature_volume,
            voice_input_device: self.settings.voice_input_device.clone(),
            controller_sensitivity: self.settings.controller_sensitivity,
            controller_invert_y: self.settings.controller_invert_y,
            controller_deadzone: self.settings.controller_deadzone,
            controller_rumble_intensity: self.settings.controller_rumble_intensity,
            controller_glyph_style: self.settings.controller_glyph_style,
            controller_swap_confirm_cancel: self.settings.controller_swap_confirm_cancel,
            controller_sneak_toggle: self.settings.controller_sneak_toggle,
            active_controller_label,
            active_glyph_family,
        }
    }

    /// Dispatches a scaled controller rumble effect if in Controller Input Mode.
    pub fn trigger_controller_rumble(&mut self, effect: crate::controller::RumbleEffect) {
        self.input_router.trigger_rumble(
            &mut self.gamepad_backend,
            effect,
            self.settings.controller_rumble_intensity,
        );
    }

    /// Updates a specific settings slider (`0..7` General, `100..=102` Controller) from horizontal cursor position and applies it live.
    pub fn update_settings_slider(
        &mut self,
        slider_idx: usize,
        cursor_x: f32,
        screen_w: f32,
        screen_h: f32,
    ) {
        let r = if slider_idx >= 100 {
            UiBuilder::settings_controller_slider_rect(screen_w, screen_h, slider_idx - 100)
        } else {
            UiBuilder::settings_slider_rect(screen_w, screen_h, slider_idx)
        };
        let t = ((cursor_x - (r[0] + 2.0)) / (r[2] - 4.0).max(1.0)).clamp(0.0, 1.0);
        match slider_idx {
            0 => {
                self.settings.render_distance = (2.0 + t * 10.0).round() as i32;
            }
            1 => {
                self.settings.fov_degrees = (50.0 + t * 60.0).round();
            }
            2 => {
                self.settings.mouse_sensitivity = 0.2 + t * 2.3;
            }
            3 => {
                self.settings.master_volume = t;
            }
            4 => {
                self.settings.sfx_volume = t;
            }
            5 => {
                self.settings.music_volume = t;
            }
            6 => {
                self.settings.voice_volume = t;
            }
            7 => {
                self.settings.creature_volume = t;
            }
            100 => {
                self.settings.controller_sensitivity = 0.2 + t * 2.3;
            }
            101 => {
                self.settings.controller_deadzone = 0.05 + t * 0.40;
            }
            102 => {
                self.settings.controller_rumble_intensity = t;
            }
            _ => {}
        }
        self.apply_settings();
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
            prefix: self
                .mp_client
                .as_ref()
                .map(|c| c.local_prefix.clone())
                .unwrap_or_default(),
            position: self.player.position,
            velocity: self.player.velocity,
            yaw: self.camera.yaw,
            pitch: self.camera.pitch,
            held_item: self.inventory.get(self.active_hotbar_slot).item_id,
            walk_anim: self.walk_distance,
            swing_anim,
            hurt_timer: 0.0,
            is_sneaking: self.shift_down && !self.inventory_open && !self.skills_open && !self.player.flying,
            appearance: self.character_profile.active_appearance,
            health: self.player_stats.health,
            max_health: self.player_stats.max_health,
        }
    }

    pub fn character_preset_label(&self) -> String {
        self.character_profile
            .selected_preset_name
            .clone()
            .unwrap_or_else(|| "Custom".to_string())
    }

    pub fn open_character_creator(&mut self, return_to_multiplayer: bool) {
        self.creator_dragging_preview = false;
        self.creator_active_rgb_slider = None;
        self.screen = AppScreen::CharacterCreator {
            tab: blockgame_render::CharacterCreatorTab::Body,
            draft: self.character_profile.active_appearance,
            initial_snapshot: self.character_profile.active_appearance,
            selected_preset_name: self.character_profile.selected_preset_name.clone(),
            preset_name_input: String::new(),
            preset_input_focused: false,
            preview_yaw: std::f32::consts::FRAC_PI_2,
            face_zoom: false,
            walk_preview: false,
            active_color_target: 0,
            status_message: None,
            return_to_multiplayer,
        };
    }

    pub fn build_creator_ui_state(&self) -> Option<blockgame_render::CharacterCreatorUiState> {
        if let AppScreen::CharacterCreator {
            tab,
            draft,
            selected_preset_name,
            preset_name_input,
            preset_input_focused,
            face_zoom,
            walk_preview,
            active_color_target,
            status_message,
            ..
        } = &self.screen
        {
            Some(blockgame_render::CharacterCreatorUiState {
                active_tab: *tab,
                appearance: *draft,
                presets: self.character_profile.all_presets(),
                selected_preset_name: selected_preset_name.clone(),
                preset_name_input: preset_name_input.clone(),
                preset_input_focused: *preset_input_focused,
                face_zoom: *face_zoom,
                walk_preview: *walk_preview,
                active_color_target: *active_color_target,
                status_message: status_message.clone(),
            })
        } else {
            None
        }
    }

    pub fn apply_creator_rgb_slider(&mut self, channel: usize, ui_x: f32, screen_w: f32, screen_h: f32) {
        let Some(ui_state) = self.build_creator_ui_state() else {
            return;
        };
        let layout = UiBuilder::character_creator_layout(screen_w, screen_h, &ui_state);
        if channel >= 3 {
            return;
        }
        let tr = layout.rgb_slider_rects[channel];
        if tr[2] <= 1.0 {
            return;
        }
        let frac = ((ui_x - tr[0]) / tr[2]).clamp(0.0, 1.0);
        let val = (frac * 255.0).round() as u8;

        if let AppScreen::CharacterCreator {
            tab,
            draft,
            active_color_target,
            selected_preset_name,
            ..
        } = &mut self.screen
        {
            let target_color = match (*tab, *active_color_target) {
                (blockgame_render::CharacterCreatorTab::Body, _) => Some(&mut draft.skin_tone),
                (blockgame_render::CharacterCreatorTab::FaceHair, 0) => Some(&mut draft.eye_color),
                (blockgame_render::CharacterCreatorTab::FaceHair, _) => Some(&mut draft.hair_color),
                (blockgame_render::CharacterCreatorTab::Clothing, 0) => Some(&mut draft.shirt_primary),
                (blockgame_render::CharacterCreatorTab::Clothing, 1) => Some(&mut draft.shirt_accent),
                (blockgame_render::CharacterCreatorTab::Clothing, 2) => Some(&mut draft.pants_color),
                (blockgame_render::CharacterCreatorTab::Clothing, _) => Some(&mut draft.shoe_color),
                _ => None,
            };
            if let Some(col) = target_color {
                match channel {
                    0 => col.r = val,
                    1 => col.g = val,
                    _ => col.b = val,
                }
                *selected_preset_name = None;
            }
        }
    }

    /// Grants XP to an MMO skill (unless in Creative mode), triggers level-up toast/audio, and marks dirty for MP sync.
    pub fn grant_skill_xp(&mut self, skill: SkillKind, amount: u64) {
        if amount == 0 || self.player_stats.game_mode.is_creative() {
            return;
        }
        let lvlup = self.player_skills.add_xp(skill, amount);
        self.skills_dirty = true;
        if let Some(lu) = lvlup {
            self.record_achievement_event(blockgame_core::AchievementEvent::SkillLevelReached {
                level: lu.new_level as u32,
            });
            self.audio.play_fishing_catch();
            let title = format!("{} Level {}!", lu.skill.name(), lu.new_level);
            let subtitle = if let Some(first_ms) = lu.unlocked_milestones.first() {
                format!("Milestone Unlocked: {}!", first_ms.name)
            } else {
                self.player_skills.perks_summary(lu.skill)
            };
            self.skill_toast = Some((title, subtitle, 4.5));
            for m in &lu.unlocked_milestones {
                let msg = format!(
                    "[Skills] {} Lv {} Milestone Unlocked - {}: {}",
                    lu.skill.name(),
                    m.level,
                    m.name,
                    m.description
                );
                if let Some(client) = &mut self.mp_client {
                    client.chat_history.push(blockgame_core::net::ChatEntry {
                        sender: "Skills".to_string(),
                        prefix: String::new(),
                        message: msg,
                        is_system: true,
                        age: 0.0,
                    });
                    if client.chat_history.len() > 100 {
                        client.chat_history.remove(0);
                    }
                } else {
                    self.local_chat_messages.push(blockgame_core::net::ChatEntry {
                        sender: "Skills".to_string(),
                        prefix: String::new(),
                        message: msg,
                        is_system: true,
                        age: 0.0,
                    });
                    if self.local_chat_messages.len() > 100 {
                        self.local_chat_messages.remove(0);
                    }
                }
            }
        }
    }

    /// Feeds a gameplay event into the shared AchievementEngine, enqueues HUD toasts
    /// with chime audio, notifies multiplayer servers for optional chat broadcast,
    /// and persists unlocks atomically to `achievements.yaml`.
    pub fn record_achievement_event(&mut self, event: blockgame_core::AchievementEvent) {
        if self.player_stats.game_mode.is_creative() {
            return;
        }
        let unlocked = self.achievements.record_event(&event);
        if unlocked.is_empty() {
            return;
        }
        for def in unlocked {
            let icon_tile = self
                .item_registry
                .get(def.icon)
                .map_or(0, |d| d.icon_tile as u16);
            let toast = crate::achievement_toast::AchievementToast::new(
                def.title,
                def.description,
                icon_tile,
            );
            if self.achievement_toasts.push(toast) {
                self.audio.play_achievement_unlock();
            }
            if let Some(client) = &mut self.mp_client {
                client.send(&blockgame_core::net::ClientPacket::AchievementUnlocked {
                    achievement_id: def.id.to_string(),
                });
            }
        }
        self.save_achievements_if_dirty();
    }

    /// Checks whether the player currently wears a full set of Diamond Armor.
    pub fn check_full_diamond_armor_achievement(&mut self) {
        if blockgame_core::is_full_diamond_armor(&self.armor.slots) {
            self.record_achievement_event(
                blockgame_core::AchievementEvent::EquippedFullDiamondArmor,
            );
        }
    }

    /// Atomically persists `achievements.yaml` if any unlock or Career Stat changed.
    pub fn save_achievements_if_dirty(&mut self) {
        if let Err(err) = self
            .achievements
            .save_if_dirty(std::path::Path::new(blockgame_core::DEFAULT_ACHIEVEMENTS_PATH))
        {
            log::warn!("Failed to save achievements.yaml: {}", err);
        }
    }

    pub fn disconnect_multiplayer(&mut self) {
        self.save_achievements_if_dirty();
        self.achievements_open = false;
        let model = self.current_local_player_model();
        let inv = self.inventory.clone();
        let hotbar = self.active_hotbar_slot;
        let stats = self.player_stats.clone();
        let skills = self.player_skills.clone();
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
            client.send(&blockgame_core::net::ClientPacket::SkillsSync { skills });
            client.disconnect();
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        if let Some(mut hosted) = self.mp_hosted_server.take() {
            hosted.shutdown();
        }
        self.chat_open = false;
        self.chat_input.clear();
        self.chat_scroll = 0;
        self.chat_hitboxes.clear();
        self.tab_held = false;
        self.skills_open = false;
        self.skill_toast = None;
        self.skills_dirty = false;
        self.world_manager.is_multiplayer = false;
    }

    pub fn join_multiplayer_server(
        &mut self,
        username: &str,
        password: &str,
        addr: &str,
        appearance: blockgame_core::CharacterAppearance,
    ) -> Result<String, String> {
        self.disconnect_multiplayer();

        let (client, auth_pkt) =
            blockgame_core::net::MultiplayerClient::connect_and_authenticate(
                addr, username, password, appearance,
            )?;

        let (seed, day_night_time, initial_state, assigned_appearance, welcome_msg) =
            if let blockgame_core::net::ServerPacket::AuthResponse {
                seed,
                day_night_time,
                player_state,
                appearance: assigned_appearance,
                message,
                ..
            } = auth_pkt
            {
                (seed, day_night_time, player_state, assigned_appearance, message)
            } else {
                return Err("Invalid authentication response packet".to_string());
            };

        let mut world_manager =
            WorldManager::new(seed, self.render_distance, self.registry.clone());
        world_manager.is_multiplayer = true;
        world_manager.init_area_around(initial_state.position, &self.device, &self.pipeline);

        self.world_manager = world_manager;
        self.player = PlayerPhysics::new(initial_state.position);
        self.player.velocity = initial_state.velocity;
        self.player_stats = initial_state.stats;
        self.player_skills = initial_state.skills;
        self.skills_open = false;
        self.selected_skill_idx = 0;
        self.skill_toast = None;
        self.skills_dirty = false;
        self.agility_xp_timer = 0.0;
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
        self.active_cooking_pot_pos = None;
        self.active_coal_gen_pos = None;
        self.active_battery_pos = None;
        self.active_electric_furnace_pos = None;
        self.active_storage_terminal_pos = None;
        self.storage_search_focused = false;
        self.active_backpack_id = None;
        self.backpacks = blockgame_core::storage::BackpackRegistry::default();
        self.jei_recipe_modal = None;
        self.jei_search_focused = false;
        self.cursor_held = ItemSlot::EMPTY;
        self.mouse_left_down = false;
        self.breaking_voxel = None;
        self.break_progress = 0.0;
        self.cracking_buffer = None;
        self.active_world_metadata = None;
        self.local_username = username.trim().to_string();
        self.character_profile.active_appearance = assigned_appearance.sanitized();
        self.chat_open = false;
        self.chat_input.clear();
        self.chat_scroll = 0;
        self.chat_hitboxes.clear();
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
        appearance: blockgame_core::CharacterAppearance,
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
        let hosted_dir = saves_root.join("Hosted_Multiplayer");
        let seed = blockgame_core::save::load_world_metadata(&hosted_dir)
            .map(|m| m.seed)
            .unwrap_or(self.world_manager.generator.seed);
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

        match self.join_multiplayer_server(username, password, &connect_addr, appearance) {
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
            let skills = self.player_skills.clone();
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
                client.send(&blockgame_core::net::ClientPacket::SkillsSync { skills });
            }
            self.skills_dirty = false;
            self.save_achievements_if_dirty();
            return true;
        }
        self.save_achievements_if_dirty();
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
            homes: self.player_homes.clone(),
            skills: self.player_skills.clone(),
        };

        let mut save_data = self.world_manager.capture_save_data(meta, player_state);
        match save_world_to_dir(&self.saves_root, &mut save_data) {
            Ok(path) => {
                let _ = blockgame_core::save::save_backpack_file(&path, &self.backpacks);
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

    pub fn create_and_start_world(
        &mut self,
        name_input: &str,
        seed_input: &str,
        game_mode: blockgame_core::GameMode,
    ) {
        let display_name = if name_input.trim().is_empty() {
            "New World".to_string()
        } else {
            name_input.trim().to_string()
        };
        let seed = parse_or_hash_seed(seed_input);
        let folder_name = unique_world_folder_name(&self.saves_root, &display_name);

        let mut meta = WorldMetadata::new(&display_name, seed);
        meta.folder_name = folder_name;
        meta.game_mode = game_mode;
        meta.achievements_enabled = game_mode == blockgame_core::GameMode::Survival;
        self.achievements.set_eligible(meta.achievements_enabled);
        self.achievements_open = false;

        let mut world_manager =
            WorldManager::new(seed, self.render_distance, self.registry.clone());
        let (spawn_x, _, spawn_z) = world_manager.generator.find_spawn_coords();
        world_manager.init_area_around(
            Vec3::new(spawn_x as f32 + 0.5, 64.0, spawn_z as f32 + 0.5),
            &self.device,
            &self.pipeline,
        );

        // Find safe surface spawn height at the dry-land spawn coordinates
        let spawn_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(spawn_x, y, spawn_z));
                if !block.is_air() && block != BlockId::TALL_GRASS {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };

        self.world_manager = world_manager;
        self.player = PlayerPhysics::new(Vec3::new(spawn_x as f32 + 0.5, spawn_y, spawn_z as f32 + 0.5));
        self.player_stats = PlayerStats::new();
        self.player_stats.game_mode = game_mode;
        self.player_skills = PlayerSkills::new();
        self.skills_open = false;
        self.selected_skill_idx = 0;
        self.skill_toast = None;
        self.skills_dirty = false;
        self.agility_xp_timer = 0.0;
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
        self.active_cooking_pot_pos = None;
        self.active_coal_gen_pos = None;
        self.active_battery_pos = None;
        self.active_electric_furnace_pos = None;
        self.active_storage_terminal_pos = None;
        self.storage_search_focused = false;
        self.active_backpack_id = None;
        self.backpacks = blockgame_core::storage::BackpackRegistry::default();
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
        let world_dir = self.saves_root.join(folder_name);
        self.backpacks = blockgame_core::save::load_backpack_file(&world_dir)
            .ok()
            .flatten()
            .unwrap_or_default();

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
        self.player_skills = save_data.player.skills;
        self.skills_open = false;
        self.selected_skill_idx = 0;
        self.skill_toast = None;
        self.skills_dirty = false;
        self.agility_xp_timer = 0.0;
        self.death_handled = self.player_stats.is_dead;
        self.inventory = save_data.player.inventory;
        self.active_hotbar_slot = save_data.player.active_hotbar_slot.min(8);
        self.player_homes = save_data.player.homes;
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
        self.active_cooking_pot_pos = None;
        self.active_coal_gen_pos = None;
        self.active_battery_pos = None;
        self.active_electric_furnace_pos = None;
        self.active_storage_terminal_pos = None;
        self.storage_search_focused = false;
        self.active_backpack_id = None;
        self.jei_recipe_modal = None;
        self.jei_search_focused = false;
        self.cursor_held = ItemSlot::EMPTY;
        self.mouse_left_down = false;
        self.breaking_voxel = None;
        self.break_progress = 0.0;
        self.cracking_buffer = None;
        self.achievements.set_eligible(
            save_data.metadata.achievements_enabled
                && save_data.metadata.game_mode == blockgame_core::GameMode::Survival,
        );
        self.achievements_open = false;
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
        self.active_shop_snapshot = None;
        self.world_manager
            .villages
            .clear_guard_hostility_on_player_death("singleplayer");
        self.player.velocity = Vec3::ZERO;
        self.player.fall_distance = 0.0;
        self.mouse_left_down = false;
        self.break_progress = 0.0;
        self.breaking_voxel = None;
        self.cracking_buffer = None;

        // Find safe surface spawn height at this world's dry-land spawn
        let (spawn_x, _, spawn_z) = self.world_manager.generator.find_spawn_coords();
        self.world_manager.init_area_around(
            Vec3::new(spawn_x as f32 + 0.5, 64.0, spawn_z as f32 + 0.5),
            &self.device,
            &self.pipeline,
        );
        let spawn_y = {
            let world_guard = self.world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(spawn_x, y, spawn_z));
                if !block.is_air() && block != BlockId::TALL_GRASS {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };
        self.player.position = Vec3::new(spawn_x as f32 + 0.5, spawn_y, spawn_z as f32 + 0.5);
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

fn load_window_icon(target_size: Option<u32>) -> Option<winit::window::Icon> {
    let bytes = include_bytes!("../../../assets/icon.png");
    let decoded = match image::load_from_memory(bytes) {
        Ok(img) => img,
        Err(e) => {
            log::warn!("Failed to decode embedded application icon: {}", e);
            return None;
        }
    };
    let rgba = match target_size {
        Some(size) if decoded.width() != size || decoded.height() != size => {
            image::imageops::resize(&decoded.to_rgba8(), size, size, image::imageops::FilterType::Lanczos3)
        }
        _ => decoded.into_rgba8(),
    };
    let (width, height) = rgba.dimensions();
    match winit::window::Icon::from_rgba(rgba.into_raw(), width, height) {
        Ok(icon) => Some(icon),
        Err(e) => {
            log::warn!("Failed to build winit window icon ({}x{}): {}", width, height, e);
            None
        }
    }
}

impl ApplicationHandler for BlockGameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // 32x32 for crisp top-left titlebar (ICON_SMALL), 128x128 for taskbar / Alt+Tab (ICON_BIG)
        let titlebar_icon = load_window_icon(Some(32));
        let taskbar_icon = load_window_icon(Some(128));
        #[allow(unused_mut)]
        let mut window_attrs = WindowAttributes::default()
            .with_title("Roots & Ridges")
            .with_inner_size(PhysicalSize::new(1280, 720))
            .with_window_icon(titlebar_icon.clone().or_else(|| taskbar_icon.clone()));

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            window_attrs = window_attrs.with_taskbar_icon(taskbar_icon.clone());
        }

        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;
            window_attrs = WindowAttributesExtWayland::with_name(
                window_attrs,
                "roots-and-ridges",
                "roots-and-ridges",
            );
            window_attrs = WindowAttributesExtX11::with_name(
                window_attrs,
                "roots-and-ridges",
                "roots-and-ridges",
            );
        }

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        // Re-apply icons after HWND creation to guarantee WM_SETICON updates both titlebar and taskbar
        window.set_window_icon(titlebar_icon.or_else(|| taskbar_icon.clone()));
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowExtWindows;
            window.set_taskbar_icon(taskbar_icon);
        }

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
        let character_pipeline = blockgame_render::CharacterPipeline::new(
            &device,
            surface_format,
            &pipeline.camera_bind_group_layout,
        );
        let wireframe_pipeline =
            WireframePipeline::new(&device, surface_format, &pipeline.camera_bind_group_layout);
        let ui_pipeline =
            UiPipeline::new(&device, surface_format, &pipeline.texture_bind_group_layout);

        let mut world_manager =
            WorldManager::new(self.seed, self.render_distance, self.registry.clone());
        let (spawn_x, _, spawn_z) = world_manager.generator.find_spawn_coords();
        world_manager.init_area_around(
            Vec3::new(spawn_x as f32 + 0.5, 64.0, spawn_z as f32 + 0.5),
            &device,
            &pipeline,
        );

        // Spawn player atop the dry-land surface
        let spawn_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(spawn_x, y, spawn_z));
                if !block.is_air() && block != BlockId::TALL_GRASS {
                    break;
                }
                y -= 1;
            }
            (y + 2) as f32
        };
        let player = PlayerPhysics::new(Vec3::new(spawn_x as f32 + 0.5, spawn_y, spawn_z as f32 + 0.5));

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

        let settings = crate::settings::GameSettings::load(std::path::Path::new("settings.cfg"));
        let mut camera = camera;
        camera.fovy = settings.fov_degrees.to_radians();
        if settings.fullscreen {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let audio = crate::audio::AudioManager::new();
        audio.set_volumes(
            settings.master_volume,
            settings.sfx_volume,
            settings.music_volume,
        );
        audio.set_voice_volume(settings.voice_volume);
        audio.set_creature_volume(settings.creature_volume);
        let music_scheduler = crate::audio::MusicScheduler::new(audio.music_track_count());
        let voice_capture = crate::voice::VoiceCaptureManager::new();

        self.state = Some(AppState {
            window,
            surface,
            device,
            queue,
            surface_config,
            supported_present_modes: surface_caps.present_modes,
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
            render_distance: settings.render_distance,
            settings,
            active_settings_slider: None,
            settings_tab: 0,
            listening_keybind: None,
            listening_controller_bind: None,
            gamepad_backend: crate::controller::GilrsBackend::new(),
            input_router: crate::controller::InputRouter::new(),
            quick_menu_open: false,
            hud_toast: None,
            controller_move_axes: [0.0, 0.0],
            controller_sneaking: false,
            controller_jumping: false,
            controller_sprinting: false,
            controller_mine_held: false,
            controller_use_held: false,
            controller_ptt_held: false,
            controller_player_list_held: false,
            ui_scroll_cooldown: 0.0,
            last_rumble_health: 20.0,
            registry: self.registry.clone(),
            inventory,
            armor: blockgame_core::item::ArmorEquipment::new(),
            active_hotbar_slot: 0,
            hotbar_activity_timer: 4.0,
            hotbar_tuck_progress: 0.0,
            item_registry,
            crafting_manager,
            crafting_grid,
            crafting_result: None,
            inventory_open: false,
            active_chest_pos: None,
            active_furnace_pos: None,
            active_cooking_pot_pos: None,
            active_shop_snapshot: None,
            shop_tab: blockgame_render::ShopUiTab::Buy,
            shop_selected_enhance_slot: None,
            shop_status_message: String::new(),
            eating: None,
            active_coal_gen_pos: None,
            active_battery_pos: None,
            active_electric_furnace_pos: None,
            active_storage_terminal_pos: None,
            storage_search_text: String::new(),
            storage_search_focused: false,
            storage_sort_mode: blockgame_core::storage::StorageSortMode::CountDesc,
            storage_page: 0,
            chest_page: 0,
            backpacks: blockgame_core::storage::BackpackRegistry::default(),
            active_backpack_id: None,
            backpack_page: 0,
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
            wind_turbine_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Wind Turbine Rotor Mesh",
            ),
            player_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Player Model Mesh",
            ),
            player_held_items_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Player Held Items Mesh",
            ),
            preview_character_dynamic_mesh: blockgame_render::DynamicMeshBuffers::new(
                "Dynamic Character Preview Mesh",
            ),
            character_pipeline,
            mp_client: None,
            mp_hosted_server: None,
            local_username: "Player1".to_string(),
            character_profile: blockgame_core::CharacterProfile::load(std::path::Path::new(
                "characters.yaml",
            )),
            creator_dragging_preview: false,
            creator_last_drag_x: 0.0,
            creator_active_rgb_slider: None,
            creator_randomize_counter: 1,
            third_person: false,
            chat_open: false,
            chat_input: String::new(),
            chat_scroll: 0,
            chat_hitboxes: Vec::new(),
            player_homes: HashMap::new(),
            tab_held: false,
            net_sync_timer: 0.0,
            next_mp_drop_nonce: 1,
            local_chat_messages: Vec::new(),
            voice_capture,
            ptt_down: false,
            voice_transmitting: false,
            speaking_players: HashMap::new(),
            directory_servers: Vec::new(),
            selected_directory_idx: None,
            directory_loading: false,
            directory_error: None,
            directory_rx: None,
            total_time: 0.0,
            fps_display: 0,
            fps_frame_count: 0,
            fps_timer: Instant::now(),
            walk_distance: 0.0,
            punch_timer: 0.0,
            last_footstep_dist: 0.0,
            break_hit_timer: 0.0,
            was_in_water: false,
            underwater_sound_timer: 0.0,
            audio,
            music_scheduler,
            mining_music_grace: 0.0,
            creature_sound_timer: 4.0,
            mouse_left_down: false,
            mouse_right_down: false,
            breaking_voxel: None,
            break_progress: 0.0,
            creative_break_cooldown: 0.0,
            cracking_buffer: None,
            fishing_state: FishingSession::Idle,
            fishing_rng: 0x517cc1b727220a95,
            player_skills: PlayerSkills::new(),
            skills_open: false,
            selected_skill_idx: 0,
            skill_toast: None,
            skills_dirty: false,
            agility_xp_timer: 0.0,
            achievements: blockgame_core::AchievementEngine::load_default(std::path::Path::new(
                blockgame_core::DEFAULT_ACHIEVEMENTS_PATH,
            )),
            achievement_toasts: crate::achievement_toast::AchievementToastQueue::new(),
            achievements_open: false,
            achievements_category: 0,
            achievements_scroll: 0,
            biome_check_timer: 0.0,
            moving_forward: false,
            moving_backward: false,
            moving_left: false,
            moving_right: false,
            jumping: false,
            last_jump_tap_time: -10.0,
            sprinting: false,
            mouse_captured: false,
            last_frame_time: Instant::now(),
        });
        if let Some(s) = self.state.as_mut() {
            s.apply_settings();
            s.input_router.sync_initial_gamepads(&s.gamepad_backend);
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
                    if dx.abs() > 0.1 || dy.abs() > 0.1 {
                        state.input_router.on_keyboard_mouse_activity(None);
                    }
                    let sensitivity = 0.003 * state.settings.mouse_sensitivity;
                    let y_sign = if state.settings.invert_y { -1.0 } else { 1.0 };
                    state.camera.yaw += (dx as f32) * sensitivity;
                    state.camera.pitch -= (dy as f32) * sensitivity * y_sign;
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.poll_controller_frame(event_loop);
        } else if let Some(state) = &mut self.state {
            match &event {
                WindowEvent::CursorMoved { position, .. } => {
                    let scale = state.settings.ui_scale.clamp(0.75, 1.5);
                    state.input_router.on_keyboard_mouse_activity(Some([
                        position.x as f32 / scale,
                        position.y as f32 / scale,
                    ]));
                    if !state.mouse_captured || state.screen != AppScreen::InGame {
                        state.window.set_cursor_visible(true);
                    }
                }
                WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. } => {
                    state.input_router.on_keyboard_mouse_activity(None);
                    if !state.mouse_captured || state.screen != AppScreen::InGame {
                        state.window.set_cursor_visible(true);
                    }
                }
                _ => {}
            }
        }
        self.dispatch_event(event_loop, InternalEvent::Window(event));
    }
}

enum InternalEvent {
    Window(WindowEvent),
    SyntheticMouse {
        button: MouseButton,
        state: ElementState,
    },
    SyntheticKey {
        key: KeyCode,
        state: ElementState,
        text: Option<winit::keyboard::SmolStr>,
    },
}

impl BlockGameApp {
    fn dispatch_event(&mut self, event_loop: &ActiveEventLoop, event: InternalEvent) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        match event {
            InternalEvent::Window(WindowEvent::CloseRequested) => {
                if matches!(
                    state.screen,
                    AppScreen::InGame
                        | AppScreen::Paused { .. }
                        | AppScreen::Settings {
                            return_to_pause: true
                        }
                        | AppScreen::Achievements {
                            return_to_pause: true
                        }
                ) {
                    state.save_active_world();
                }
                state.disconnect_multiplayer();
                event_loop.exit();
            }
            InternalEvent::Window(WindowEvent::Resized(new_size)) => {
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
            InternalEvent::Window(WindowEvent::CursorMoved { position, .. }) => {
                state.cursor_pos = [position.x as f32, position.y as f32];
                if matches!(state.screen, AppScreen::Settings { .. }) {
                    if let Some(slider_idx) = state.active_settings_slider {
                        let (screen_w, screen_h, ui_cpos) = state.ui_metrics();
                        state.update_settings_slider(
                            slider_idx,
                            ui_cpos[0],
                            screen_w,
                            screen_h,
                        );
                    }
                } else if matches!(state.screen, AppScreen::CharacterCreator { .. }) {
                    let (screen_w, screen_h, ui_cpos) = state.ui_metrics();
                    if state.creator_dragging_preview {
                        let dx = ui_cpos[0] - state.creator_last_drag_x;
                        state.creator_last_drag_x = ui_cpos[0];
                        if let AppScreen::CharacterCreator { preview_yaw, .. } = &mut state.screen {
                            *preview_yaw -= dx * 0.014;
                        }
                    } else if let Some(ch) = state.creator_active_rgb_slider {
                        state.apply_creator_rgb_slider(ch, ui_cpos[0], screen_w, screen_h);
                    }
                }
            }
            InternalEvent::Window(WindowEvent::MouseWheel { delta, .. }) => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                if matches!(state.screen, AppScreen::Achievements { .. })
                    || (state.screen == AppScreen::InGame && state.achievements_open)
                {
                    if scroll > 0.0 {
                        state.achievements_scroll = state.achievements_scroll.saturating_sub(1);
                    } else if scroll < 0.0 {
                        state.achievements_scroll = (state.achievements_scroll + 1).min(16);
                    }
                    return;
                }
                if state.screen != AppScreen::InGame {
                    return;
                }
                if state.chat_open {
                    if scroll > 0.0 {
                        state.chat_scroll = (state.chat_scroll + 1).min(100);
                    } else if scroll < 0.0 {
                        state.chat_scroll = state.chat_scroll.saturating_sub(1);
                    }
                    return;
                }
                if state.skills_open || state.achievements_open || state.inventory_open {
                    return;
                }
                if scroll > 0.0 {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 8) % 9;
                    state.hotbar_activity_timer = 4.0;
                } else if scroll < 0.0 {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 1) % 9;
                    state.hotbar_activity_timer = 4.0;
                }
            }
            InternalEvent::Window(WindowEvent::MouseInput {
                button,
                state: mouse_btn_state,
                ..
            })
            | InternalEvent::SyntheticMouse {
                button,
                state: mouse_btn_state,
            } => {
                let pressed = mouse_btn_state == ElementState::Pressed;
                if !pressed && button == MouseButton::Left {
                    if state.active_settings_slider.take().is_some() {
                        state.settings.save(std::path::Path::new("settings.cfg"));
                    }
                    state.creator_dragging_preview = false;
                    state.creator_active_rgb_slider = None;
                }

                // Handle Quick-Menu clicks when open in-game
                if state.screen == AppScreen::InGame && state.quick_menu_open {
                    if pressed && button == MouseButton::Left {
                        let (screen_w, screen_h, cpos) = state.ui_metrics();
                        for idx in 0..4 {
                            let r = UiBuilder::quick_menu_item_rect(screen_w, screen_h, idx);
                            if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3]) {
                                state.quick_menu_open = false;
                                match idx {
                                    0 => {
                                        state.skills_open = true;
                                        state.mouse_captured = false;
                                    }
                                    1 => {
                                        state.achievements_open = true;
                                        state.mouse_captured = false;
                                    }
                                    2 => {
                                        state.mouse_captured = false;
                                        state.screen = AppScreen::Guide { page: 0 };
                                    }
                                    3 => {
                                        state.settings.show_debug_hud =
                                            !state.settings.show_debug_hud;
                                        state.settings.save(std::path::Path::new("settings.cfg"));
                                        state.mouse_captured = true;
                                        let _ = state.window.set_cursor_grab(
                                            winit::window::CursorGrabMode::Locked,
                                        );
                                        state.window.set_cursor_visible(false);
                                    }
                                    _ => {}
                                }
                                break;
                            }
                        }
                    }
                    return;
                }

                // Handle Title Screen, World Select, Create World, Settings, Multiplayer Lobby, and Pause Menu clicks
                if state.screen != AppScreen::InGame {
                    if pressed && button == MouseButton::Left {
                        let (screen_w, screen_h, cpos) = state.ui_metrics();

                        match state.screen.clone() {
                            AppScreen::Title => {
                                let rects = UiBuilder::title_screen_button_rects(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_SINGLEPLAYER][0],
                                    rects[UiBuilder::TITLE_BTN_SINGLEPLAYER][1],
                                    rects[UiBuilder::TITLE_BTN_SINGLEPLAYER][2],
                                    rects[UiBuilder::TITLE_BTN_SINGLEPLAYER][3],
                                ) {
                                    // Singleplayer / Continue Expedition
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
                                    rects[UiBuilder::TITLE_BTN_NEW_WORLD][0],
                                    rects[UiBuilder::TITLE_BTN_NEW_WORLD][1],
                                    rects[UiBuilder::TITLE_BTN_NEW_WORLD][2],
                                    rects[UiBuilder::TITLE_BTN_NEW_WORLD][3],
                                ) {
                                    // New Expedition
                                    state.screen = AppScreen::CreateWorld {
                                        name_input: "New World".to_string(),
                                        seed_input: String::new(),
                                        focused_field: 0,
                                        game_mode: blockgame_core::GameMode::Survival,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_MULTIPLAYER][0],
                                    rects[UiBuilder::TITLE_BTN_MULTIPLAYER][1],
                                    rects[UiBuilder::TITLE_BTN_MULTIPLAYER][2],
                                    rects[UiBuilder::TITLE_BTN_MULTIPLAYER][3],
                                ) {
                                    // Multiplayer Lobby
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input: state.local_username.clone(),
                                        password_input: String::new(),
                                        address_input: "127.0.0.1:25565".to_string(),
                                        focused_field: 0,
                                        status_message: None,
                                    };
                                    state.refresh_directory_servers();
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_CHARACTER_CREATOR][0],
                                    rects[UiBuilder::TITLE_BTN_CHARACTER_CREATOR][1],
                                    rects[UiBuilder::TITLE_BTN_CHARACTER_CREATOR][2],
                                    rects[UiBuilder::TITLE_BTN_CHARACTER_CREATOR][3],
                                ) {
                                    state.open_character_creator(false);
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_SETTINGS][0],
                                    rects[UiBuilder::TITLE_BTN_SETTINGS][1],
                                    rects[UiBuilder::TITLE_BTN_SETTINGS][2],
                                    rects[UiBuilder::TITLE_BTN_SETTINGS][3],
                                ) {
                                    // Settings
                                    state.screen = AppScreen::Settings {
                                        return_to_pause: false,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_GUIDE][0],
                                    rects[UiBuilder::TITLE_BTN_GUIDE][1],
                                    rects[UiBuilder::TITLE_BTN_GUIDE][2],
                                    rects[UiBuilder::TITLE_BTN_GUIDE][3],
                                ) {
                                    // How to Play & Guide
                                    state.screen = AppScreen::Guide { page: 0 };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_ACHIEVEMENTS][0],
                                    rects[UiBuilder::TITLE_BTN_ACHIEVEMENTS][1],
                                    rects[UiBuilder::TITLE_BTN_ACHIEVEMENTS][2],
                                    rects[UiBuilder::TITLE_BTN_ACHIEVEMENTS][3],
                                ) {
                                    // Achievements
                                    state.achievements_scroll = 0;
                                    state.screen = AppScreen::Achievements {
                                        return_to_pause: false,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_QUIT][0],
                                    rects[UiBuilder::TITLE_BTN_QUIT][1],
                                    rects[UiBuilder::TITLE_BTN_QUIT][2],
                                    rects[UiBuilder::TITLE_BTN_QUIT][3],
                                ) {
                                    // Quit Game
                                    event_loop.exit();
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[UiBuilder::TITLE_BTN_QUICK_RESUME][0],
                                    rects[UiBuilder::TITLE_BTN_QUICK_RESUME][1],
                                    rects[UiBuilder::TITLE_BTN_QUICK_RESUME][2],
                                    rects[UiBuilder::TITLE_BTN_QUICK_RESUME][3],
                                ) {
                                    // Quick Resume Card Action
                                    state.refresh_saved_worlds();
                                    if let Some(recent) = state.saved_worlds.first() {
                                        let folder = recent.folder_name.clone();
                                        state.load_and_start_world(&folder);
                                    } else {
                                        state.screen = AppScreen::CreateWorld {
                                            name_input: "New World".to_string(),
                                            seed_input: String::new(),
                                            focused_field: 0,
                                            game_mode: blockgame_core::GameMode::Survival,
                                        };
                                    }
                                }
                            }
                            AppScreen::Achievements { return_to_pause } => {
                                let (tab_rects, close_rect, _) =
                                    UiBuilder::achievements_screen_rects(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(
                                    cpos,
                                    close_rect[0],
                                    close_rect[1],
                                    close_rect[2],
                                    close_rect[3],
                                ) {
                                    state.screen = if return_to_pause {
                                        AppScreen::Paused {
                                            status_message: None,
                                        }
                                    } else {
                                        AppScreen::Title
                                    };
                                } else {
                                    for (i, r) in tab_rects.iter().enumerate() {
                                        if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3]) {
                                            state.achievements_category = i;
                                            state.achievements_scroll = 0;
                                            break;
                                        }
                                    }
                                }
                            }
                            AppScreen::Guide { page } => {
                                let btns = UiBuilder::guide_screen_button_rects(screen_w, screen_h);
                                if UiBuilder::is_rect_hovered(cpos, btns[0][0], btns[0][1], btns[0][2], btns[0][3]) {
                                    let new_page = if page == 0 { 2 } else { page - 1 };
                                    state.screen = AppScreen::Guide { page: new_page };
                                } else if UiBuilder::is_rect_hovered(cpos, btns[1][0], btns[1][1], btns[1][2], btns[1][3]) {
                                    state.screen = AppScreen::Guide { page: (page + 1) % 3 };
                                } else if UiBuilder::is_rect_hovered(cpos, btns[2][0], btns[2][1], btns[2][2], btns[2][3]) {
                                    state.screen = AppScreen::Title;
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
                                        game_mode: blockgame_core::GameMode::Survival,
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
                                game_mode,
                            } => {
                                let (name_r, seed_r, create_r, cancel_r) =
                                    UiBuilder::create_world_layout(screen_w, screen_h);
                                let mode_r = UiBuilder::create_world_gamemode_rect(screen_w, screen_h);
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
                                        game_mode,
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
                                        game_mode,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    mode_r[0],
                                    mode_r[1],
                                    mode_r[2],
                                    mode_r[3],
                                ) {
                                    let next_mode = if game_mode.is_creative() {
                                        blockgame_core::GameMode::Survival
                                    } else {
                                        blockgame_core::GameMode::Creative
                                    };
                                    state.screen = AppScreen::CreateWorld {
                                        name_input,
                                        seed_input,
                                        focused_field,
                                        game_mode: next_mode,
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
                                    state.create_and_start_world(&name_input, &seed_input, game_mode);
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
                                    // Achievements
                                    state.achievements_scroll = 0;
                                    state.screen = AppScreen::Achievements {
                                        return_to_pause: true,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[3][0],
                                    rects[3][1],
                                    rects[3][2],
                                    rects[3][3],
                                ) {
                                    // Settings
                                    state.screen = AppScreen::Settings {
                                        return_to_pause: true,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos,
                                    rects[4][0],
                                    rects[4][1],
                                    rects[4][2],
                                    rects[4][3],
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
                                let mut handled = false;
                                for tab_idx in 0..3 {
                                    let r = UiBuilder::settings_tab_rect(screen_w, screen_h, tab_idx);
                                    if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3]) {
                                        state.settings_tab = tab_idx;
                                        state.active_settings_slider = None;
                                        state.listening_keybind = None;
                                        state.listening_controller_bind = None;
                                        handled = true;
                                        break;
                                    }
                                }

                                if !handled && state.settings_tab == 0 {
                                    for idx in 0..8 {
                                        let r = UiBuilder::settings_slider_rect(
                                            screen_w, screen_h, idx,
                                        );
                                        if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3])
                                        {
                                            state.active_settings_slider = Some(idx);
                                            state.update_settings_slider(
                                                idx, cpos[0], screen_w, screen_h,
                                            );
                                            handled = true;
                                            break;
                                        }
                                    }

                                    if !handled {
                                        for idx in 0..8 {
                                            let r = UiBuilder::settings_toggle_rect(
                                                screen_w, screen_h, idx,
                                            );
                                            if UiBuilder::is_rect_hovered(
                                                cpos, r[0], r[1], r[2], r[3],
                                            ) {
                                                match idx {
                                                    0 => {
                                                        state.settings.invert_y =
                                                            !state.settings.invert_y
                                                    }
                                                    1 => {
                                                        state.settings.fullscreen =
                                                            !state.settings.fullscreen
                                                    }
                                                    2 => {
                                                        state.settings.vsync =
                                                            !state.settings.vsync
                                                    }
                                                    3 => {
                                                        state.settings.view_bobbing =
                                                            !state.settings.view_bobbing
                                                    }
                                                    4 => {
                                                        state.settings.show_debug_hud =
                                                            !state.settings.show_debug_hud
                                                    }
                                                    5 => {
                                                        state.settings.cycle_ui_scale();
                                                    }
                                                    6 => {
                                                        state.settings.cycle_voice_mode();
                                                    }
                                                    7 => {
                                                        state.settings.voice_input_device = state
                                                            .voice_capture
                                                            .cycle_input_device(
                                                                &state.settings.voice_input_device,
                                                            );
                                                    }
                                                    _ => {}
                                                }
                                                state.apply_settings();
                                                state
                                                    .settings
                                                    .save(std::path::Path::new("settings.cfg"));
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }
                                } else if !handled && state.settings_tab == 1 {
                                    for idx in 0..crate::settings::KeyBinds::COUNT {
                                        let r = UiBuilder::settings_keybind_rect(
                                            screen_w, screen_h, idx,
                                        );
                                        if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3])
                                        {
                                            state.listening_keybind = Some(idx);
                                            state.listening_controller_bind = None;
                                            handled = true;
                                            break;
                                        }
                                    }
                                } else if !handled && state.settings_tab == 2 {
                                    for idx in 0..3 {
                                        let r = UiBuilder::settings_controller_slider_rect(
                                            screen_w, screen_h, idx,
                                        );
                                        if UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3])
                                        {
                                            let slider_id = 100 + idx;
                                            state.active_settings_slider = Some(slider_id);
                                            state.update_settings_slider(
                                                slider_id, cpos[0], screen_w, screen_h,
                                            );
                                            handled = true;
                                            break;
                                        }
                                    }
                                    if !handled {
                                        for idx in 0..4 {
                                            let r = UiBuilder::settings_controller_toggle_rect(
                                                screen_w, screen_h, idx,
                                            );
                                            if UiBuilder::is_rect_hovered(
                                                cpos, r[0], r[1], r[2], r[3],
                                            ) {
                                                match idx {
                                                    0 => {
                                                        state.settings.controller_invert_y =
                                                            !state.settings.controller_invert_y;
                                                    }
                                                    1 => {
                                                        state.settings.controller_glyph_style = state
                                                            .settings
                                                            .controller_glyph_style
                                                            .cycle_next();
                                                    }
                                                    2 => {
                                                        state.settings.controller_swap_confirm_cancel =
                                                            !state.settings.controller_swap_confirm_cancel;
                                                    }
                                                    3 => {
                                                        state.settings.controller_sneak_toggle =
                                                            !state.settings.controller_sneak_toggle;
                                                    }
                                                    _ => {}
                                                }
                                                state
                                                    .settings
                                                    .save(std::path::Path::new("settings.cfg"));
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }
                                    if !handled {
                                        for idx in 0..crate::controller::ControllerBindings::COUNT {
                                            let r = UiBuilder::settings_controller_bind_rect(
                                                screen_w, screen_h, idx,
                                            );
                                            if UiBuilder::is_rect_hovered(
                                                cpos, r[0], r[1], r[2], r[3],
                                            ) {
                                                state.listening_controller_bind = Some(idx);
                                                state.listening_keybind = None;
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }
                                }

                                if !handled {
                                    let reset_r =
                                        UiBuilder::settings_reset_button_rect(screen_w, screen_h);
                                    if UiBuilder::is_rect_hovered(
                                        cpos, reset_r[0], reset_r[1], reset_r[2], reset_r[3],
                                    ) {
                                        if state.settings_tab == 2 {
                                            let defaults = crate::settings::GameSettings::default();
                                            state.settings.controller_sensitivity =
                                                defaults.controller_sensitivity;
                                            state.settings.controller_invert_y =
                                                defaults.controller_invert_y;
                                            state.settings.controller_deadzone =
                                                defaults.controller_deadzone;
                                            state.settings.controller_rumble_intensity =
                                                defaults.controller_rumble_intensity;
                                            state.settings.controller_glyph_style =
                                                defaults.controller_glyph_style;
                                            state.settings.controller_swap_confirm_cancel =
                                                defaults.controller_swap_confirm_cancel;
                                            state.settings.controller_sneak_toggle =
                                                defaults.controller_sneak_toggle;
                                            state.settings.controller_binds =
                                                defaults.controller_binds;
                                        } else {
                                            state.settings =
                                                crate::settings::GameSettings::default();
                                        }
                                        state.listening_keybind = None;
                                        state.listening_controller_bind = None;
                                        state.apply_settings();
                                        state.settings.save(std::path::Path::new("settings.cfg"));
                                        handled = true;
                                    }
                                }

                                if !handled {
                                    let done_r =
                                        UiBuilder::settings_done_button_rect(screen_w, screen_h);
                                    if UiBuilder::is_rect_hovered(
                                        cpos, done_r[0], done_r[1], done_r[2], done_r[3],
                                    ) {
                                        state.listening_keybind = None;
                                        state.listening_controller_bind = None;
                                        state.settings.save(std::path::Path::new("settings.cfg"));
                                        state.screen = if return_to_pause {
                                            AppScreen::Paused {
                                                status_message: None,
                                            }
                                        } else {
                                            AppScreen::Title
                                        };
                                    }
                                }
                            }
                            AppScreen::MultiplayerLobby {
                                username_input,
                                password_input,
                                address_input,
                                focused_field,
                                status_message,
                            } => {
                                let (user_r, pass_r, addr_r, [char_r, join_r, host_r, back_r]) =
                                    UiBuilder::multiplayer_lobby_layout(screen_w, screen_h);
                                let active_appearance = state.character_profile.active_appearance;
                                if UiBuilder::is_rect_hovered(
                                    cpos, user_r[0], user_r[1], user_r[2], user_r[3],
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
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
                                        focused_field: 2,
                                        status_message,
                                    };
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, char_r[0], char_r[1], char_r[2], char_r[3],
                                ) {
                                    state.open_character_creator(true);
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, join_r[0], join_r[1], join_r[2], join_r[3],
                                ) {
                                    if let Err(err) = state.join_multiplayer_server(
                                        &username_input,
                                        &password_input,
                                        &address_input,
                                        active_appearance,
                                    ) {
                                        state.screen = AppScreen::MultiplayerLobby {
                                            username_input,
                                            password_input,
                                            address_input,
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
                                        active_appearance,
                                    ) {
                                        state.screen = AppScreen::MultiplayerLobby {
                                            username_input,
                                            password_input,
                                            address_input,
                                            focused_field,
                                            status_message: Some((err, true)),
                                        };
                                    }
                                } else if UiBuilder::is_rect_hovered(
                                    cpos, back_r[0], back_r[1], back_r[2], back_r[3],
                                ) {
                                    state.screen = AppScreen::Title;
                                } else {
                                    let (_, refresh_r, row_rects) =
                                        UiBuilder::multiplayer_browser_layout(screen_w, screen_h);
                                    if UiBuilder::is_rect_hovered(
                                        cpos,
                                        refresh_r[0],
                                        refresh_r[1],
                                        refresh_r[2],
                                        refresh_r[3],
                                    ) {
                                        state.refresh_directory_servers();
                                        state.screen = AppScreen::MultiplayerLobby {
                                            username_input,
                                            password_input,
                                            address_input,
                                            focused_field,
                                            status_message,
                                        };
                                    } else {
                                        let mut clicked_row = None;
                                        for (idx, r) in row_rects.iter().enumerate() {
                                            if idx < state.directory_servers.len()
                                                && UiBuilder::is_rect_hovered(
                                                    cpos, r[0], r[1], r[2], r[3],
                                                )
                                            {
                                                clicked_row = Some(idx);
                                                break;
                                            }
                                        }
                                        if let Some(idx) = clicked_row {
                                            let srv = &state.directory_servers[idx];
                                            let new_addr = format!("{}:{}", srv.host, srv.port);
                                            let already_selected =
                                                state.selected_directory_idx == Some(idx);
                                            state.selected_directory_idx = Some(idx);
                                            if already_selected
                                                && username_input.trim().len() >= 3
                                            {
                                                if let Err(err) = state.join_multiplayer_server(
                                                    &username_input,
                                                    &password_input,
                                                    &new_addr,
                                                    active_appearance,
                                                ) {
                                                    state.screen = AppScreen::MultiplayerLobby {
                                                        username_input,
                                                        password_input,
                                                        address_input: new_addr,
                                                        focused_field,
                                                        status_message: Some((err, true)),
                                                    };
                                                }
                                            } else {
                                                state.screen = AppScreen::MultiplayerLobby {
                                                    username_input,
                                                    password_input,
                                                    address_input: new_addr,
                                                    focused_field,
                                                    status_message: None,
                                                };
                                            }
                                        } else {
                                            state.screen = AppScreen::MultiplayerLobby {
                                                username_input,
                                                password_input,
                                                address_input,
                                                focused_field,
                                                status_message,
                                            };
                                        }
                                    }
                                }
                            }
                            AppScreen::CharacterCreator {
                                mut tab,
                                mut draft,
                                initial_snapshot,
                                mut selected_preset_name,
                                mut preset_name_input,
                                mut preset_input_focused,
                                preview_yaw,
                                mut face_zoom,
                                mut walk_preview,
                                mut active_color_target,
                                mut status_message,
                                return_to_multiplayer,
                            } => {
                                use blockgame_core::character::{
                                    BodyFrame, EyeStyle, FacialHairStyle, HairStyle, PantsStyle,
                                    ShirtStyle, ShoeStyle,
                                };
                                use blockgame_render::CharacterCreatorTab;

                                let ui_state = blockgame_render::CharacterCreatorUiState {
                                    active_tab: tab,
                                    appearance: draft,
                                    presets: state.character_profile.all_presets(),
                                    selected_preset_name: selected_preset_name.clone(),
                                    preset_name_input: preset_name_input.clone(),
                                    preset_input_focused,
                                    face_zoom,
                                    walk_preview,
                                    active_color_target,
                                    status_message: status_message.clone(),
                                };
                                let layout = UiBuilder::character_creator_layout(
                                    screen_w, screen_h, &ui_state,
                                );
                                let hit = |r: [f32; 4]| {
                                    UiBuilder::is_rect_hovered(cpos, r[0], r[1], r[2], r[3])
                                };

                                preset_input_focused = false;
                                let mut exit_to_menu = false;

                                if hit(layout.preview_viewport) {
                                    state.creator_dragging_preview = true;
                                    state.creator_last_drag_x = cpos[0];
                                } else if hit(layout.btn_zoom) {
                                    face_zoom = !face_zoom;
                                } else if hit(layout.btn_walk) {
                                    walk_preview = !walk_preview;
                                } else if hit(layout.btn_randomize) {
                                    state.creator_randomize_counter =
                                        state.creator_randomize_counter.wrapping_add(1);
                                    let seed = state.creator_randomize_counter
                                        ^ ((state.total_time * 1000.0) as u64).wrapping_mul(0x9E37_79B9);
                                    draft = blockgame_core::CharacterAppearance::randomized(seed);
                                    selected_preset_name = None;
                                    status_message = Some("Randomized character!".to_string());
                                } else if hit(layout.btn_revert) {
                                    draft = initial_snapshot;
                                    selected_preset_name =
                                        state.character_profile.selected_preset_name.clone();
                                    status_message = Some("Reverted session edits.".to_string());
                                } else if hit(layout.btn_done) {
                                    state.character_profile.active_appearance = draft.sanitized();
                                    state.character_profile.selected_preset_name =
                                        selected_preset_name.clone();
                                    state
                                        .character_profile
                                        .save(std::path::Path::new("characters.yaml"));
                                    exit_to_menu = true;
                                } else {
                                    // Check tabs
                                    let mut handled = false;
                                    for (i, r) in layout.tab_rects.iter().enumerate() {
                                        if hit(*r) {
                                            tab = CharacterCreatorTab::ALL[i];
                                            active_color_target = 0;
                                            status_message = None;
                                            handled = true;
                                            break;
                                        }
                                    }

                                    // Presets tab interactions
                                    if !handled && tab == CharacterCreatorTab::Presets {
                                        if hit(layout.preset_name_input) {
                                            preset_input_focused = true;
                                            handled = true;
                                        } else if hit(layout.btn_save_preset) {
                                            let saved = state
                                                .character_profile
                                                .save_custom_preset(&preset_name_input, draft);
                                            state
                                                .character_profile
                                                .save(std::path::Path::new("characters.yaml"));
                                            selected_preset_name = Some(saved.clone());
                                            preset_name_input.clear();
                                            status_message =
                                                Some(format!("Saved preset '{}'.", saved));
                                            handled = true;
                                        } else {
                                            for (i, (row_r, del_r)) in
                                                layout.preset_rows.iter().enumerate()
                                            {
                                                if let Some(dr) = del_r {
                                                    if hit(*dr) {
                                                        if let Some(p) = ui_state.presets.get(i) {
                                                            let name = p.name.clone();
                                                            if state
                                                                .character_profile
                                                                .delete_custom_preset(&name)
                                                            {
                                                                state.character_profile.save(
                                                                    std::path::Path::new(
                                                                        "characters.yaml",
                                                                    ),
                                                                );
                                                                if selected_preset_name.as_deref()
                                                                    == Some(name.as_str())
                                                                {
                                                                    selected_preset_name = None;
                                                                }
                                                                status_message = Some(format!(
                                                                    "Deleted preset '{}'.",
                                                                    name
                                                                ));
                                                            }
                                                        }
                                                        handled = true;
                                                        break;
                                                    }
                                                }
                                                if hit(*row_r) {
                                                    if let Some(p) = ui_state.presets.get(i) {
                                                        draft = p.appearance;
                                                        selected_preset_name = Some(p.name.clone());
                                                        status_message = Some(format!(
                                                            "Loaded '{}'.",
                                                            p.name
                                                        ));
                                                    }
                                                    handled = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }

                                    // Style option chips
                                    if !handled {
                                        for &(r, group, idx) in &layout.option_buttons {
                                            if hit(r) {
                                                match group {
                                                    0 => draft.body_frame = BodyFrame::ALL[idx],
                                                    1 => draft.eye_style = EyeStyle::ALL[idx],
                                                    2 => draft.hair_style = HairStyle::ALL[idx],
                                                    3 => draft.facial_hair = FacialHairStyle::ALL[idx],
                                                    4 => draft.shirt_style = ShirtStyle::ALL[idx],
                                                    5 => draft.pants_style = PantsStyle::ALL[idx],
                                                    6 => draft.shoe_style = ShoeStyle::ALL[idx],
                                                    _ => {}
                                                }
                                                selected_preset_name = None;
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }

                                    // Color target selector
                                    if !handled {
                                        for &(r, target) in &layout.color_target_buttons {
                                            if hit(r) {
                                                active_color_target = target;
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }

                                    // Color swatch clicks
                                    if !handled {
                                        for &(r, col) in &layout.swatch_rects {
                                            if hit(r) {
                                                match (tab, active_color_target) {
                                                    (CharacterCreatorTab::Body, _) => draft.skin_tone = col,
                                                    (CharacterCreatorTab::FaceHair, 0) => draft.eye_color = col,
                                                    (CharacterCreatorTab::FaceHair, _) => draft.hair_color = col,
                                                    (CharacterCreatorTab::Clothing, 0) => draft.shirt_primary = col,
                                                    (CharacterCreatorTab::Clothing, 1) => draft.shirt_accent = col,
                                                    (CharacterCreatorTab::Clothing, 2) => draft.pants_color = col,
                                                    (CharacterCreatorTab::Clothing, _) => draft.shoe_color = col,
                                                    _ => {}
                                                }
                                                selected_preset_name = None;
                                                handled = true;
                                                break;
                                            }
                                        }
                                    }

                                    // RGB slider drag start
                                    if !handled {
                                        for (ch, r) in layout.rgb_slider_rects.iter().enumerate() {
                                            let expanded = [r[0], r[1] - 4.0, r[2], r[3] + 8.0];
                                            if r[2] > 1.0 && hit(expanded) {
                                                state.creator_active_rgb_slider = Some(ch);
                                                let frac = ((cpos[0] - r[0]) / r[2]).clamp(0.0, 1.0);
                                                let val = (frac * 255.0).round() as u8;
                                                let target_col = match (tab, active_color_target) {
                                                    (CharacterCreatorTab::Body, _) => Some(&mut draft.skin_tone),
                                                    (CharacterCreatorTab::FaceHair, 0) => Some(&mut draft.eye_color),
                                                    (CharacterCreatorTab::FaceHair, _) => Some(&mut draft.hair_color),
                                                    (CharacterCreatorTab::Clothing, 0) => Some(&mut draft.shirt_primary),
                                                    (CharacterCreatorTab::Clothing, 1) => Some(&mut draft.shirt_accent),
                                                    (CharacterCreatorTab::Clothing, 2) => Some(&mut draft.pants_color),
                                                    (CharacterCreatorTab::Clothing, _) => Some(&mut draft.shoe_color),
                                                    _ => None,
                                                };
                                                if let Some(c) = target_col {
                                                    match ch {
                                                        0 => c.r = val,
                                                        1 => c.g = val,
                                                        _ => c.b = val,
                                                    }
                                                    selected_preset_name = None;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }

                                if exit_to_menu {
                                    state.screen = if return_to_multiplayer {
                                        AppScreen::MultiplayerLobby {
                                            username_input: state.local_username.clone(),
                                            password_input: String::new(),
                                            address_input: "127.0.0.1:25565".to_string(),
                                            focused_field: 0,
                                            status_message: None,
                                        }
                                    } else {
                                        AppScreen::Title
                                    };
                                } else {
                                    state.screen = AppScreen::CharacterCreator {
                                        tab,
                                        draft,
                                        initial_snapshot,
                                        selected_preset_name,
                                        preset_name_input,
                                        preset_input_focused,
                                        preview_yaw,
                                        face_zoom,
                                        walk_preview,
                                        active_color_target,
                                        status_message,
                                        return_to_multiplayer,
                                    };
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
                        let (screen_w, screen_h, cpos) = state.ui_metrics();
                        let btn_w = 200.0;
                        let btn_h = 44.0;
                        let btn_x = (screen_w - btn_w) * 0.5;
                        let btn_y = screen_h * 0.50;
                        if cpos[0] >= btn_x
                            && cpos[0] <= btn_x + btn_w
                            && cpos[1] >= btn_y
                            && cpos[1] <= btn_y + btn_h
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

                if state.achievements_open {
                    if pressed && button == MouseButton::Left {
                        let (screen_w, screen_h, ui_cpos) = state.ui_metrics();
                        let (tab_rects, close_rect, _) =
                            UiBuilder::achievements_screen_rects(screen_w, screen_h);
                        if UiBuilder::is_rect_hovered(
                            ui_cpos,
                            close_rect[0],
                            close_rect[1],
                            close_rect[2],
                            close_rect[3],
                        ) {
                            state.achievements_open = false;
                            if !state.inventory_open && !state.chat_open && !state.player_stats.is_dead {
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
                        } else {
                            for (i, r) in tab_rects.iter().enumerate() {
                                if UiBuilder::is_rect_hovered(ui_cpos, r[0], r[1], r[2], r[3]) {
                                    state.achievements_category = i;
                                    state.achievements_scroll = 0;
                                    break;
                                }
                            }
                        }
                    }
                    return;
                }

                if state.skills_open {
                    if pressed && button == MouseButton::Left {
                        let (screen_w, screen_h, ui_cpos) = state.ui_metrics();
                        let (row_rects, close_rect, _) =
                            UiBuilder::skills_screen_rects(screen_w, screen_h);
                        if UiBuilder::is_rect_hovered(
                            ui_cpos,
                            close_rect[0],
                            close_rect[1],
                            close_rect[2],
                            close_rect[3],
                        ) {
                            state.skills_open = false;
                            if !state.inventory_open && !state.chat_open && !state.player_stats.is_dead {
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
                        } else {
                            for (idx, r) in row_rects.iter().enumerate() {
                                if UiBuilder::is_rect_hovered(ui_cpos, r[0], r[1], r[2], r[3]) {
                                    state.selected_skill_idx = idx;
                                    break;
                                }
                            }
                        }
                    }
                    return;
                }

                if state.inventory_open {
                    if pressed {
                        let is_right = button == MouseButton::Right;
                        if let Some(snapshot) = state.active_shop_snapshot.clone() {
                            if !is_right {
                                let (screen_w, screen_h, ui_cursor) = state.ui_metrics();
                                if let Some(action) = UiBuilder::hit_test_shop_screen(
                                    screen_w,
                                    screen_h,
                                    &snapshot,
                                    state.shop_tab,
                                    &state.inventory,
                                    ui_cursor,
                                    &state.item_registry,
                                ) {
                                    let world_ticks = state.world_manager.villages.advance_clock(state.day_night.time);
                                    match action {
                                        blockgame_render::ShopUiAction::Close => {
                                            state.active_shop_snapshot = None;
                                            state.inventory_open = false;
                                            state.shop_status_message.clear();
                                            state.mouse_captured = true;
                                            let _ = state
                                                .window
                                                .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                                                .or_else(|_| {
                                                    state.window.set_cursor_grab(
                                                        winit::window::CursorGrabMode::Confined,
                                                    )
                                                });
                                            state.window.set_cursor_visible(false);
                                        }
                                        blockgame_render::ShopUiAction::SwitchTab(tab) => {
                                            state.shop_tab = tab;
                                            state.shop_status_message.clear();
                                        }
                                        blockgame_render::ShopUiAction::SelectEnhanceSlot(idx) => {
                                            state.shop_selected_enhance_slot = Some(idx);
                                        }
                                        blockgame_render::ShopUiAction::Buy(offer_idx) => {
                                            if let Some(client) = &mut state.mp_client {
                                                client.send(
                                                    &blockgame_core::net::ClientPacket::ShopBuy {
                                                        npc_id: snapshot.npc_id,
                                                        offer_idx,
                                                    },
                                                );
                                            } else {
                                                let res = state.world_manager.villages.buy_item(
                                                    snapshot.npc_id,
                                                    "singleplayer",
                                                    offer_idx,
                                                    &mut state.inventory,
                                                    &state.item_registry,
                                                    world_ticks,
                                                );
                                                let (msg, spent_opt) = match res {
                                                    Ok(s) => {
                                                        let m = if let Some(t) = s.leveled_up_to {
                                                            format!("Purchased! Shop Tier {} unlocked!", t)
                                                        } else {
                                                            "Purchase complete!".to_string()
                                                        };
                                                        (m, Some(s.coins_delta.unsigned_abs() as u64))
                                                    }
                                                    Err(e) => (e.message(), None),
                                                };
                                                state.shop_status_message = msg;
                                                state.active_shop_snapshot = state
                                                    .world_manager
                                                    .villages
                                                    .build_shop_snapshot(
                                                        snapshot.npc_id,
                                                        "singleplayer",
                                                        world_ticks,
                                                    );
                                                if let Some(coins_spent) = spent_opt {
                                                    let tier = state
                                                        .active_shop_snapshot
                                                        .as_ref()
                                                        .map_or(1, |sn| sn.shop_tier);
                                                    let bought_item_id = state
                                                        .active_shop_snapshot
                                                        .as_ref()
                                                        .and_then(|sn| {
                                                            sn.buy_offers
                                                                .iter()
                                                                .find(|o| o.offer_idx == offer_idx)
                                                                .map(|o| o.item_id.0)
                                                        });
                                                    state.record_achievement_event(
                                                        blockgame_core::AchievementEvent::ShopTradeCompleted {
                                                            coins_spent,
                                                            new_shop_tier: tier,
                                                        },
                                                    );
                                                    if let Some(item_id) = bought_item_id {
                                                        state.record_achievement_event(
                                                            blockgame_core::AchievementEvent::ItemObtained {
                                                                item_id,
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        blockgame_render::ShopUiAction::Sell(offer_idx) => {
                                            if let Some(client) = &mut state.mp_client {
                                                client.send(
                                                    &blockgame_core::net::ClientPacket::ShopSell {
                                                        npc_id: snapshot.npc_id,
                                                        offer_idx,
                                                    },
                                                );
                                            } else {
                                                let res = state.world_manager.villages.sell_item(
                                                    snapshot.npc_id,
                                                    "singleplayer",
                                                    offer_idx,
                                                    &mut state.inventory,
                                                    &state.item_registry,
                                                    world_ticks,
                                                );
                                                let (msg, ok) = match res {
                                                    Ok(s) => (
                                                        format!("Sold for +{} Gold Coins!", s.coins_delta),
                                                        true,
                                                    ),
                                                    Err(e) => (e.message(), false),
                                                };
                                                state.shop_status_message = msg;
                                                state.active_shop_snapshot = state
                                                    .world_manager
                                                    .villages
                                                    .build_shop_snapshot(
                                                        snapshot.npc_id,
                                                        "singleplayer",
                                                        world_ticks,
                                                    );
                                                if ok {
                                                    let tier = state
                                                        .active_shop_snapshot
                                                        .as_ref()
                                                        .map_or(1, |sn| sn.shop_tier);
                                                    state.record_achievement_event(
                                                        blockgame_core::AchievementEvent::ShopTradeCompleted {
                                                            coins_spent: 0,
                                                            new_shop_tier: tier,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                        blockgame_render::ShopUiAction::ApplyEnhance(offer_idx) => {
                                            if let Some(inv_idx) = state.shop_selected_enhance_slot {
                                                if let Some(client) = &mut state.mp_client {
                                                    client.send(
                                                        &blockgame_core::net::ClientPacket::ShopEnhance {
                                                            npc_id: snapshot.npc_id,
                                                            inv_slot_idx: inv_idx as u16,
                                                            offer_idx,
                                                        },
                                                    );
                                                } else {
                                                    let res = state
                                                        .world_manager
                                                        .villages
                                                        .enhance_item(
                                                            snapshot.npc_id,
                                                            "singleplayer",
                                                            inv_idx,
                                                            offer_idx,
                                                            &mut state.inventory,
                                                            &state.item_registry,
                                                            world_ticks,
                                                        );
                                                    let (msg, spent_opt) = match res {
                                                        Ok(s) => {
                                                            state.shop_selected_enhance_slot = None;
                                                            let m = if let Some(t) = s.leveled_up_to {
                                                                format!(
                                                                    "Enhanced! Shop Tier {} unlocked!",
                                                                    t
                                                                )
                                                            } else {
                                                                "Item enhanced and restored to full durability!".to_string()
                                                            };
                                                            (m, Some(s.coins_delta.unsigned_abs() as u64))
                                                        }
                                                        Err(e) => (e.message(), None),
                                                    };
                                                    state.shop_status_message = msg;
                                                    state.active_shop_snapshot = state
                                                        .world_manager
                                                        .villages
                                                        .build_shop_snapshot(
                                                            snapshot.npc_id,
                                                            "singleplayer",
                                                            world_ticks,
                                                        );
                                                    if let Some(coins_spent) = spent_opt {
                                                        let tier = state
                                                            .active_shop_snapshot
                                                            .as_ref()
                                                            .map_or(1, |sn| sn.shop_tier);
                                                        state.record_achievement_event(
                                                            blockgame_core::AchievementEvent::ShopTradeCompleted {
                                                                coins_spent,
                                                                new_shop_tier: tier,
                                                            },
                                                        );
                                                        state.record_achievement_event(
                                                            blockgame_core::AchievementEvent::EnhancementApplied,
                                                        );
                                                    }
                                                }
                                            } else {
                                                state.shop_status_message =
                                                    "Select an unenhanced item on the left first."
                                                        .to_string();
                                            }
                                        }
                                    }
                                }
                            }
                            return;
                        }
                        if state.jei_recipe_modal.is_some() {
                            handle_recipe_modal_click(state, is_right);
                            return;
                        }
                        if handle_jei_panel_click(state, is_right) {
                            return;
                        }
                        if state.active_cooking_pot_pos.is_some() {
                            handle_cooking_pot_click(state, is_right);
                            if let (Some(pos), Some(client)) =
                                (state.active_cooking_pot_pos, &mut state.mp_client)
                            {
                                if let Some(pot) = state
                                    .world_manager
                                    .world()
                                    .lock()
                                    .unwrap()
                                    .get_cooking_pot(pos)
                                    .cloned()
                                {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::CookingPotUpdate {
                                            pos,
                                            ingredients: pot.ingredients,
                                            fuel: pot.fuel,
                                            output: pot.output,
                                            burn_time: pot.burn_time_remaining,
                                            max_burn_time: pot.max_burn_time,
                                            cook_progress: pot.cook_progress,
                                        },
                                    );
                                }
                            }
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
                        } else if state.active_coal_gen_pos.is_some() {
                            handle_coal_generator_click(state, is_right);
                        } else if state.active_battery_pos.is_some() {
                            handle_battery_click(state, is_right);
                        } else if state.active_electric_furnace_pos.is_some() {
                            handle_electric_furnace_click(state, is_right);
                        } else if state.active_storage_terminal_pos.is_some() {
                            handle_storage_terminal_click(state, is_right);
                        } else if state.active_backpack_id.is_some() {
                            handle_backpack_click(state, is_right);
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
                            state.check_full_diamond_armor_achievement();
                        }
                    }
                    return;
                }

                if state.chat_open {
                    if pressed && button == MouseButton::Left {
                        let (_, _, ui_cpos) = state.ui_metrics();
                        let mut clicked_command = None;
                        for hb in &state.chat_hitboxes {
                            if ui_cpos[0] >= hb.rect[0]
                                && ui_cpos[0] <= hb.rect[0] + hb.rect[2]
                                && ui_cpos[1] >= hb.rect[1]
                                && ui_cpos[1] <= hb.rect[1] + hb.rect[3]
                            {
                                clicked_command = Some(hb.command.clone());
                                break;
                            }
                        }

                        if let Some(cmd) = clicked_command {
                            if let Some(client) = &mut state.mp_client {
                                client.send(&blockgame_core::net::ClientPacket::Chat {
                                    message: cmd,
                                });
                            } else {
                                state.local_chat_messages.push(blockgame_core::net::ChatEntry {
                                    sender: if state.local_username.is_empty() {
                                        "Player".to_string()
                                    } else {
                                        state.local_username.clone()
                                    },
                                    prefix: String::new(),
                                    message: cmd,
                                    is_system: false,
                                    age: 0.0,
                                });
                                if state.local_chat_messages.len() > 100 {
                                    state.local_chat_messages.remove(0);
                                }
                            }
                            state.chat_open = false;
                            state.chat_input.clear();
                            state.chat_scroll = 0;
                            if !state.inventory_open && !state.player_stats.is_dead {
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
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
                        if matches!(state.fishing_state, FishingSession::Minigame { .. }) {
                            return;
                        }
                        if pressed {
                            state.hotbar_activity_timer = 4.0;
                            state.punch_timer = 0.25; // Trigger punch animation
                            let held_slot = state.inventory.get(state.active_hotbar_slot);
                            let base_attack_damage =
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
                            let held_slot_for_boss = state.inventory.get(state.active_hotbar_slot);
                            let base_attack_damage = held_slot_for_boss
                                .item_id
                                .boss_weapon_damage()
                                .unwrap_or(base_attack_damage);
                            let weapon_lifesteal =
                                held_slot_for_boss.item_id.boss_weapon_lifesteal();
                            let crit_roll =
                                blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                    < state.player_skills.combat_crit_chance();
                            let mut attack_damage = base_attack_damage
                                * held_slot_for_boss.weapon_damage_multiplier()
                                * state.player_skills.combat_damage_multiplier()
                                + state.player_skills.combat_flat_bonus();
                            if crit_roll {
                                attack_damage *= 1.5;
                            }

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
                                state.grant_skill_xp(SkillKind::Combat, 15);
                                state.mouse_left_down = false;
                                state.breaking_voxel = None;
                                state.break_progress = 0.0;
                                state.cracking_buffer = None;

                                if !state.player_stats.game_mode.is_creative() {
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
                            } else {
                                // In multiplayer, also check synced server-authoritative boss/temple monsters
                                let hit_mp_boss = if state.mp_client.is_some() {
                                    state.world_manager.hit_test_mp_temple_monster(
                                        state.player.eye_position(),
                                        state.camera.forward(),
                                        4.2,
                                    )
                                } else {
                                    None
                                };
                                let hit_monster = if hit_mp_boss.is_none() {
                                    state.world_manager.attack_monster(
                                        state.player.eye_position(),
                                        state.camera.forward(),
                                        4.0,
                                        attack_damage,
                                    )
                                } else {
                                    None
                                };
                                let hit_animal = if hit_mp_boss.is_none() && hit_monster.is_none() {
                                    state.world_manager.attack_animal(
                                        state.player.eye_position(),
                                        state.camera.forward(),
                                        3.5,
                                        base_attack_damage,
                                    )
                                } else {
                                    None
                                };
                                if hit_mp_boss.is_none()
                                    && hit_monster.is_none()
                                    && hit_animal.is_none()
                                {
                                    if let Some((npc_id, _is_guard, _name, _dist)) =
                                        state.world_manager.raycast_village_npc(
                                            state.player.eye_position(),
                                            state.camera.forward(),
                                            3.8,
                                        )
                                    {
                                        let kb = state.camera.forward();
                                        if let Some(client) = &mut state.mp_client {
                                            client.send(
                                                &blockgame_core::net::ClientPacket::AttackVillageNpc {
                                                    npc_id,
                                                    damage: attack_damage,
                                                    knockback: kb,
                                                },
                                            );
                                        } else {
                                            let world_ticks =
                                                state.world_manager.villages.advance_clock(state.day_night.time);
                                            let hit = state.world_manager.villages.damage_npc(
                                                npc_id,
                                                attack_damage,
                                                kb,
                                                Some("singleplayer"),
                                                world_ticks,
                                            );
                                            if hit.is_some() {
                                                state.record_achievement_event(
                                                    blockgame_core::AchievementEvent::VillageGuardProvoked,
                                                );
                                            }
                                        }
                                        state.audio.play_hurt();
                                        state.mouse_left_down = false;
                                        state.breaking_voxel = None;
                                        state.break_progress = 0.0;
                                        state.cracking_buffer = None;
                                        return;
                                    }
                                }
                                if let Some((mid, mkind, mpos, invulnerable)) = hit_mp_boss {
                                    if !invulnerable {
                                        if let Some(client) = &mut state.mp_client {
                                            client.send(
                                                &blockgame_core::net::ClientPacket::AttackTempleMonster {
                                                    monster_id: mid,
                                                    damage: attack_damage,
                                                    knockback: state.camera.forward(),
                                                },
                                            );
                                        }
                                        state.audio.play_monster_hurt(
                                            mkind,
                                            mpos,
                                            state.player.position,
                                            state.camera.yaw,
                                        );
                                        state.grant_skill_xp(SkillKind::Combat, 15);
                                        if weapon_lifesteal > 0.0 && !state.player_stats.is_dead {
                                            state.player_stats.health = (state.player_stats.health
                                                + attack_damage * weapon_lifesteal)
                                                .min(state.player_stats.max_health);
                                        }
                                    }
                                } else if let Some((mkind, mpos, killed)) = hit_monster {
                                    state.audio.play_monster_hurt(
                                        mkind,
                                        mpos,
                                        state.player.position,
                                        state.camera.yaw,
                                    );
                                    let xp = if killed {
                                        if mkind.is_boss() { 250 } else { 37 }
                                    } else {
                                        12
                                    };
                                    state.grant_skill_xp(SkillKind::Combat, xp);
                                    if weapon_lifesteal > 0.0 && !state.player_stats.is_dead {
                                        state.player_stats.health = (state.player_stats.health
                                            + attack_damage * weapon_lifesteal)
                                            .min(state.player_stats.max_health);
                                    }
                                    if killed {
                                        state.record_achievement_event(
                                            blockgame_core::AchievementEvent::MonsterKilled {
                                                is_boss: mkind.is_boss(),
                                                boss_key: blockgame_core::boss_achievement_key(mkind)
                                                    .map(str::to_string),
                                            },
                                        );
                                    }
                                    if killed && !state.player_stats.is_dead {
                                        let ls = state.player_skills.combat_lifesteal_on_kill();
                                        if ls > 0.0 {
                                            state.player_stats.health = (state.player_stats.health + ls)
                                                .min(state.player_stats.max_health);
                                        }
                                    }
                                } else if let Some((animal_id, akind, apos)) = hit_animal {
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::AttackAnimal {
                                                animal_id,
                                                damage: base_attack_damage,
                                                knockback: state.camera.forward().normalize_or_zero(),
                                            },
                                        );
                                    }
                                    state.audio.play_animal_hurt(
                                        akind,
                                        apos,
                                        state.player.position,
                                        state.camera.yaw,
                                    );
                                }
                                let hit_entity = hit_mp_boss.is_some()
                                    || hit_monster.is_some()
                                    || hit_animal.is_some();
                                if hit_entity {
                                    state.mouse_left_down = false;
                                    state.breaking_voxel = None;
                                    state.break_progress = 0.0;
                                    state.cracking_buffer = None;

                                    if !state.player_stats.game_mode.is_creative() {
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
                    }
                    MouseButton::Right => {
                        state.mouse_right_down = pressed;
                        if !pressed
                            || matches!(state.fishing_state, FishingSession::Minigame { .. })
                        {
                            return;
                        }
                        state.hotbar_activity_timer = 4.0;
                        // 1. Check if targeting an interactive block (e.g. Temple Portal, Furnace, Chest, or Crafting Table)
                        if let Some(hit) = state.targeted_block {
                            let block_id = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard.get_block(hit.hit_voxel)
                            };
                            if block_id.is_temple_interactive() {
                                if block_id == BlockId::TEMPLE_PORTAL {
                                    if let Some(placement) = state
                                        .world_manager
                                        .find_temple_placement_at_portal(hit.hit_voxel)
                                    {
                                        state.world_manager.ensure_arena_built_local(
                                            placement,
                                            &state.device,
                                            &state.pipeline,
                                        );
                                    }
                                }
                                if state.mp_client.is_some() {
                                    let placement = state
                                        .world_manager
                                        .find_temple_placement_at_portal(hit.hit_voxel);
                                    let (tid, kind, ow_orig, p_pos) = placement
                                        .map(|p| (p.temple_id, p.kind.to_u8(), p.origin, p.portal_pos))
                                        .unwrap_or((0, 0, IVec3::ZERO, hit.hit_voxel));
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::TempleInteract {
                                                pos: hit.hit_voxel,
                                                temple_id: tid,
                                                kind,
                                                overworld_origin: ow_orig,
                                                portal_pos: p_pos,
                                            },
                                        );
                                    }
                                } else if let Some(dest) = state
                                    .world_manager
                                    .interact_temple_block_sp(hit.hit_voxel, state.player.position)
                                {
                                    state.player.position = dest;
                                    state.player.velocity = Vec3::ZERO;
                                    if block_id == BlockId::TEMPLE_PORTAL {
                                        state.camera.yaw = std::f32::consts::FRAC_PI_2;
                                        state.camera.pitch = 0.0;
                                    }
                                }
                                return;
                            }
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
                                state.active_cooking_pot_pos = None;
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id.is_cooking_pot() {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;

                                {
                                    let mut world_guard =
                                        state.world_manager.world().lock().unwrap();
                                    world_guard.get_or_create_cooking_pot(hit.hit_voxel);
                                }

                                state.active_chest_pos = None;
                                state.active_furnace_pos = None;
                                state.active_coal_gen_pos = None;
                                state.active_battery_pos = None;
                                state.active_electric_furnace_pos = None;
                                state.active_storage_terminal_pos = None;
                                state.active_backpack_id = None;
                                state.active_cooking_pot_pos = Some(hit.hit_voxel);
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id.is_chest() {
                                if state.world_manager.is_chest_locked(hit.hit_voxel) {
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: String::new(),
                                            prefix: String::new(),
                                            message: "&cThis treasure chest is sealed until the temple boss is defeated!".to_string(),
                                            is_system: true,
                                            age: 0.0,
                                        },
                                    );
                                    return;
                                }
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
                                state.active_cooking_pot_pos = None;
                                state.active_coal_gen_pos = None;
                                state.active_battery_pos = None;
                                state.active_electric_furnace_pos = None;
                                state.active_storage_terminal_pos = None;
                                state.active_backpack_id = None;
                                state.active_chest_pos = Some(hit.hit_voxel);
                                state.chest_page = 0;
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id == BlockId::STORAGE_TERMINAL {
                                let storage_ready = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    let info = blockgame_core::storage::collect_storage_network(
                                        &world_guard,
                                        hit.hit_voxel,
                                    );
                                    info.is_powered && !info.chests.is_empty()
                                };
                                if storage_ready {
                                    state.record_achievement_event(
                                        blockgame_core::AchievementEvent::PoweredStorageTerminalOpened,
                                    );
                                }
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;
                                state.active_chest_pos = None;
                                state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_coal_gen_pos = None;
                                state.active_battery_pos = None;
                                state.active_electric_furnace_pos = None;
                                state.active_backpack_id = None;
                                state.active_storage_terminal_pos = Some(hit.hit_voxel);
                                state.storage_page = 0;
                                state.storage_search_text.clear();
                                state.storage_search_focused = false;
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if block_id == BlockId::COAL_GENERATOR
                                || block_id.is_battery_box()
                                || block_id == BlockId::ELECTRIC_FURNACE
                            {
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;
                                {
                                    let mut world_guard =
                                        state.world_manager.world().lock().unwrap();
                                    if block_id == BlockId::COAL_GENERATOR {
                                        world_guard.get_or_create_coal_generator(hit.hit_voxel);
                                    } else if block_id.is_battery_box() {
                                        world_guard.get_or_create_battery(hit.hit_voxel);
                                    } else {
                                        world_guard.get_or_create_electric_furnace(hit.hit_voxel);
                                    }
                                }
                                state.active_chest_pos = None;
                                state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_coal_gen_pos =
                                    (block_id == BlockId::COAL_GENERATOR).then_some(hit.hit_voxel);
                                state.active_battery_pos =
                                    block_id.is_battery_box().then_some(hit.hit_voxel);
                                state.active_electric_furnace_pos =
                                    (block_id == BlockId::ELECTRIC_FURNACE).then_some(hit.hit_voxel);
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                                return;
                            }
                            if matches!(
                                block_id,
                                BlockId::SOLAR_PANEL
                                    | BlockId::ADVANCED_SOLAR_PANEL
                                    | BlockId::WIND_TURBINE
                                    | BlockId::ELECTRIC_LAMP
                                    | BlockId::ELECTRIC_LAMP_LIT
                                    | BlockId::COPPER_CABLE
                                    | BlockId::GOLD_CABLE
                                    | BlockId::DIAMOND_CABLE
                            ) {
                                let held_is_placeable = {
                                    let slot = state.inventory.get(state.active_hotbar_slot);
                                    !slot.is_empty()
                                        && state
                                            .item_registry
                                            .get(slot.item_id)
                                            .and_then(|d| d.placeable_block)
                                            .is_some()
                                };
                                if !held_is_placeable {
                                    let is_daytime = state.day_night.sun_direction().y > 0.0;
                                    let msg = {
                                        let world_guard =
                                            state.world_manager.world().lock().unwrap();
                                        let s = blockgame_core::power::get_network_summary(
                                            &world_guard,
                                            hit.hit_voxel,
                                            is_daytime,
                                        );
                                        let block_line = match block_id {
                                            BlockId::SOLAR_PANEL => {
                                                let out = blockgame_core::power::solar_panel_output_eu_s(
                                                    &world_guard,
                                                    hit.hit_voxel,
                                                    is_daytime,
                                                );
                                                format!("Solar Panel: Provides +{:.0} EU/s", out)
                                            }
                                            BlockId::ADVANCED_SOLAR_PANEL => {
                                                let out = blockgame_core::power::solar_panel_output_eu_s(
                                                    &world_guard,
                                                    hit.hit_voxel,
                                                    is_daytime,
                                                );
                                                format!("Advanced Solar Panel: Provides +{:.0} EU/s", out)
                                            }
                                            BlockId::WIND_TURBINE => {
                                                let out = blockgame_core::power::wind_turbine_output_eu_s(
                                                    &world_guard,
                                                    hit.hit_voxel,
                                                );
                                                format!("Wind Turbine: Provides +{:.0} EU/s", out)
                                            }
                                            BlockId::ELECTRIC_LAMP_LIT => {
                                                "Electric Lamp: Consumes -5 EU/s (Lit)".to_string()
                                            }
                                            BlockId::ELECTRIC_LAMP => {
                                                "Electric Lamp: Consumes -5 EU/s (Unpowered)"
                                                    .to_string()
                                            }
                                            BlockId::GOLD_CABLE => {
                                                "Gold Cable: Max 1,000 EU/s".to_string()
                                            }
                                            BlockId::DIAMOND_CABLE => {
                                                "Diamond Cable: Max 5,000 EU/s".to_string()
                                            }
                                            _ => "Copper Cable: Max 200 EU/s".to_string(),
                                        };
                                        let cap_note = match s.cable_limit_eu_s {
                                            Some(lim) => format!(" | Cable Cap: {:.0} EU/s", lim),
                                            None => String::new(),
                                        };
                                        format!(
                                            "{} | Grid: +{:.0} EU/s Gen / -{:.0} EU/s Use{} | Stored: {:.0}/{:.0} EU",
                                            block_line,
                                            s.total_gen_eu_s,
                                            s.total_demand_eu_s,
                                            cap_note,
                                            s.total_stored_eu,
                                            s.total_capacity_eu
                                        )
                                    };
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: "Power".to_string(),
                                            prefix: String::new(),
                                            message: msg,
                                            is_system: true,
                                            age: 0.0,
                                        },
                                    );
                                    return;
                                }
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
                                state.active_cooking_pot_pos = None;
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
                                                prefix: String::new(),
                                                message: msg.to_string(),
                                                is_system: true,
                                                age: 0.0,
                                            },
                                        );
                                    } else {
                                        state.local_chat_messages.push(
                                            blockgame_core::net::ChatEntry {
                                                sender: "System".to_string(),
                                                prefix: String::new(),
                                                message: "You can only sleep at night. Respawn point set.".to_string(),
                                                is_system: true,
                                                age: 0.0,
                                            },
                                        );
                                    }
                                } else if is_night {
                                    state.day_night.set_time(1000.0);
                                    state.record_achievement_event(
                                        blockgame_core::AchievementEvent::SleptInBed,
                                    );
                                    state.player_stats.health = (state.player_stats.health + 4.0)
                                        .min(state.player_stats.max_health);
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: "System".to_string(),
                                            prefix: String::new(),
                                            message: "You slept through the night. Good morning!".to_string(),
                                            is_system: true,
                                            age: 0.0,
                                        },
                                    );
                                } else {
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender: "System".to_string(),
                                            prefix: String::new(),
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

                        // 1a. Portable Backpack Open (each backpack item has its own unique ID & inventory)
                        if blockgame_core::storage::BackpackState::is_backpack_item(held_slot.item_id) {
                            let mut slot = held_slot;
                            if let Some(bp_id) = state.backpacks.ensure_slot_id(&mut slot) {
                                state.inventory.set(state.active_hotbar_slot, slot);
                                state.mouse_left_down = false;
                                state.break_progress = 0.0;
                                state.breaking_voxel = None;
                                state.cracking_buffer = None;
                                state.active_chest_pos = None;
                                state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_coal_gen_pos = None;
                                state.active_battery_pos = None;
                                state.active_electric_furnace_pos = None;
                                state.active_storage_terminal_pos = None;
                                state.storage_search_focused = false;
                                state.active_backpack_id = Some(bp_id);
                                state.backpack_page = 0;
                                state.inventory_open = true;
                                state.mouse_captured = false;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::None);
                                state.window.set_cursor_visible(true);
                            }
                            return;
                        }

                        // 1a. Right-click a Village Shopkeeper to open the 3-tab Shop UI
                        if let Some((npc_id, is_guard, _name, _dist)) =
                            state.world_manager.raycast_village_npc(
                                state.player.eye_position(),
                                state.camera.forward(),
                                4.5,
                            )
                        {
                            if !is_guard {
                                if let Some(client) = &mut state.mp_client {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::InteractShopkeeper {
                                            npc_id,
                                        },
                                    );
                                } else {
                                    let world_ticks = state.world_manager.villages.advance_clock(state.day_night.time);
                                    if let Some(snap) = state
                                        .world_manager
                                        .villages
                                        .build_shop_snapshot(npc_id, "singleplayer", world_ticks)
                                    {
                                        state.active_shop_snapshot = Some(snap);
                                        state.shop_tab = blockgame_render::ShopUiTab::Buy;
                                        state.shop_selected_enhance_slot = None;
                                        state.shop_status_message.clear();
                                        state.inventory_open = true;
                                        state.active_chest_pos = None;
                                        state.active_furnace_pos = None;
                                        state.active_cooking_pot_pos = None;
                                        state.mouse_captured = false;
                                        let _ = state
                                            .window
                                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                                        state.window.set_cursor_visible(true);
                                    }
                                }
                                return;
                            }
                        }

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
                            let crit_roll =
                                blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                    < state.player_skills.combat_crit_chance();
                            let mut bow_damage = 6.0
                                * held_slot.weapon_damage_multiplier()
                                * state.player_skills.combat_damage_multiplier()
                                + state.player_skills.combat_flat_bonus();
                            if crit_roll {
                                bow_damage *= 1.5;
                            }
                            if state.player_stats.game_mode.is_creative() {
                                let dir = state.camera.forward().normalize_or_zero();
                                let spawn_pos = state.player.eye_position() + dir * 0.45;
                                state.world_manager.spawn_player_arrow(
                                    spawn_pos,
                                    dir * 28.0,
                                    bow_damage,
                                );
                                state.punch_timer = 0.15;
                                state.trigger_controller_rumble(
                                    crate::controller::RumbleEffect::BowRelease,
                                );
                            } else if let Some(idx) = arrow_slot_idx {
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
                                state.world_manager.spawn_player_arrow(
                                    spawn_pos,
                                    dir * 28.0,
                                    bow_damage,
                                );
                                state.punch_timer = 0.15;
                                state.trigger_controller_rumble(
                                    crate::controller::RumbleEffect::BowRelease,
                                );
                            }
                            return;
                        }

                        // 1b-2. Fishing Rod Casting / Reeling / Hooking
                        if let Some(rod_kind) =
                            blockgame_core::FishingRodKind::from_item(held_slot.item_id)
                        {
                            match state.fishing_state.clone() {
                                FishingSession::Idle => {
                                    let mut tackle = blockgame_core::FishingTackleStats::from_inventory(
                                        rod_kind,
                                        &state.inventory,
                                    )
                                    .with_rod_enhancement(held_slot.enhancement);
                                    tackle.bite_speed_mult *=
                                        state.player_skills.fishing_bite_speed_mult();
                                    tackle.bar_height = (tackle.bar_height
                                        + state.player_skills.fishing_bar_height_bonus())
                                    .clamp(0.20, 0.58);
                                    tackle.treasure_chance = (tackle.treasure_chance
                                        + state.player_skills.fishing_treasure_chance_bonus())
                                    .clamp(0.0, 0.75);
                                    if let Some(b_idx) = tackle.bait_slot {
                                        if !state.player_stats.game_mode.is_creative() {
                                            let mut b_slot = state.inventory.get(b_idx);
                                            b_slot.take(1);
                                            state.inventory.set(b_idx, b_slot);
                                        }
                                    } else {
                                        tackle.bait_count = 0;
                                    }
                                    let dir = state.camera.forward().normalize_or_zero();
                                    let start_pos = state.player.eye_position() + dir * 0.6;
                                    let vel = dir * 13.5 + Vec3::new(0.0, 2.8, 0.0);
                                    let wait_time = (4.5
                                        + blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                            * 7.5)
                                        / tackle.bite_speed_mult;
                                    state.fishing_state = FishingSession::Waiting {
                                        bobber_pos: start_pos,
                                        bobber_vel: vel,
                                        landed: false,
                                        wait_timer: wait_time.max(0.8),
                                        is_ocean: false,
                                        rod_slot: state.active_hotbar_slot,
                                        tackle,
                                    };
                                    state.audio.play_fishing_cast();
                                    state.punch_timer = 0.18;
                                }
                                FishingSession::Waiting { .. } => {
                                    state.fishing_state = FishingSession::Idle;
                                    state.audio.play_fishing_cast();
                                    state.punch_timer = 0.12;
                                }
                                FishingSession::Biting {
                                    bobber_pos,
                                    species,
                                    rod_slot,
                                    tackle,
                                    ..
                                } => {
                                    let game =
                                        blockgame_core::FishingMinigameState::new(species, &tackle);
                                    state.fishing_state = FishingSession::Minigame {
                                        bobber_pos,
                                        game,
                                        rod_slot,
                                        tackle,
                                    };
                                    state.audio.play_fishing_splash();
                                    state.punch_timer = 0.15;
                                }
                                FishingSession::Minigame { .. } => {}
                            }
                            return;
                        }

                        // 1c. Quick-Equip Armor on Right-Click
                        if !held_slot.is_empty() {
                            if let Some((armor_slot, _)) = state.item_registry.get_armor(held_slot.item_id) {
                                let idx = armor_slot.index();
                                let prev = std::mem::replace(&mut state.armor.slots[idx], held_slot);
                                state.inventory.set(state.active_hotbar_slot, prev);
                                state.check_full_diamond_armor_achievement();
                                state.punch_timer = 0.12;
                                return;
                            }
                        }

                        // 1d. Animal Breeding & Cow Milking (Issue #12)
                        if !held_slot.is_empty() {
                            let eye = state.player.eye_position();
                            let fwd = state.camera.forward().normalize_or_zero();
                            let mut hit_idx: Option<usize> = None;
                            let mut best_dist = 4.0_f32;
                            for (idx, a) in state.world_manager.animals.iter().enumerate() {
                                if a.dead {
                                    continue;
                                }
                                let center = a.position + Vec3::new(0.0, a.kind.dimensions().1 * 0.5, 0.0);
                                let to = center - eye;
                                let proj = to.dot(fwd);
                                if proj > 0.0 && proj < best_dist {
                                    let perp = (to - fwd * proj).length();
                                    if perp <= a.kind.dimensions().0 + 0.35 {
                                        best_dist = proj;
                                        hit_idx = Some(idx);
                                    }
                                }
                            }
                            if let Some(a_idx) = hit_idx {
                                let animal_id = state.world_manager.animals[a_idx].id;
                                if state.world_manager.animals[a_idx].can_milk(held_slot.item_id) {
                                    let mut slot = state.inventory.get(state.active_hotbar_slot);
                                    slot.take(1);
                                    if slot.is_empty() {
                                        if state.mp_client.is_none() {
                                            state.inventory.set(
                                                state.active_hotbar_slot,
                                                ItemSlot::new(ItemId::MILK_BUCKET, 1),
                                            );
                                        } else {
                                            state.inventory.set(state.active_hotbar_slot, slot);
                                        }
                                    } else {
                                        state.inventory.set(state.active_hotbar_slot, slot);
                                        if state.mp_client.is_none() {
                                            if let Some(rem) = state.inventory.add_item(
                                                ItemSlot::new(ItemId::MILK_BUCKET, 1),
                                                &state.item_registry,
                                            ) {
                                                state.spawn_drop_synced(
                                                    state.player.position + Vec3::new(0.0, 0.5, 0.0),
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    rem,
                                                );
                                            }
                                        }
                                    }
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::InteractAnimal {
                                                animal_id,
                                                held_item: ItemId::BUCKET,
                                            },
                                        );
                                    } else {
                                        state.record_achievement_event(
                                            blockgame_core::AchievementEvent::CowMilked,
                                        );
                                    }
                                    state.punch_timer = 0.15;
                                    return;
                                } else if state.world_manager.animals[a_idx]
                                    .kind
                                    .is_breeding_feed(held_slot.item_id)
                                {
                                    let fed = if state.mp_client.is_some() {
                                        let can = state.world_manager.animals[a_idx].can_breed()
                                            && state.world_manager.animals[a_idx].love_timer <= 0.0;
                                        if can {
                                            if let Some(client) = &mut state.mp_client {
                                                client.send(
                                                    &blockgame_core::net::ClientPacket::InteractAnimal {
                                                        animal_id,
                                                        held_item: held_slot.item_id,
                                                    },
                                                );
                                            }
                                        }
                                        can
                                    } else {
                                        state.world_manager.animals[a_idx]
                                            .try_feed(held_slot.item_id)
                                    };
                                    if fed {
                                        let mut slot = state.inventory.get(state.active_hotbar_slot);
                                        slot.take(1);
                                        state.inventory.set(state.active_hotbar_slot, slot);
                                        state.grant_skill_xp(SkillKind::Farming, 18);
                                        state.punch_timer = 0.15;
                                        return;
                                    }
                                }
                            }
                        }

                        // 1e. Passive Soil Fertilizer Application (Compost / Bone Fertilizer on Farmland)
                        if let Some(tier) = held_slot.item_id.fertilizer_tier() {
                            if let Some(hit) = state.targeted_block {
                                if let Some(updated_block) = state.world_manager.apply_fertilizer(
                                    hit.hit_voxel,
                                    tier,
                                    &state.device,
                                    &state.pipeline,
                                ) {
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::BlockChange {
                                                pos: hit.hit_voxel,
                                                block_id: updated_block,
                                            },
                                        );
                                    }
                                    state.inventory.slots[state.active_hotbar_slot].take(1);
                                    state.grant_skill_xp(SkillKind::Farming, 6);
                                    state.record_achievement_event(
                                        blockgame_core::AchievementEvent::FertilizerApplied,
                                    );
                                    state.punch_timer = 0.18;
                                    return;
                                }
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
                                                state.grant_skill_xp(SkillKind::Farming, 6);
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

                        // 3. Crop Planting (Wheat, Carrots, Potatoes, Tomatoes, Melon/Pumpkin Stems on Farmland)
                        if let Some(crop_block) = held_slot.item_id.plantable_farmland_crop() {
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
                                        crop_block,
                                        &state.player.aabb(),
                                        &state.device,
                                        &state.pipeline,
                                    );
                                    if planted {
                                        if let Some(client) = &mut state.mp_client {
                                            client.send(
                                                &blockgame_core::net::ClientPacket::BlockChange {
                                                    pos: hit.place_voxel,
                                                    block_id: crop_block,
                                                },
                                            );
                                        }
                                        state.audio.play_block_place(
                                            crop_block,
                                            hit.place_voxel.as_vec3() + Vec3::splat(0.5),
                                            state.camera.position,
                                            state.camera.yaw,
                                        );
                                        state.world_manager.plant_crop(hit.place_voxel);
                                        state.inventory.slots[state.active_hotbar_slot].take(1);
                                        state.grant_skill_xp(SkillKind::Farming, 5);
                                        state.punch_timer = 0.2;
                                        return;
                                    }
                                }
                            }
                        }

                        // 4. Hold-Right-Click Eating / Drinking (Issue #12, ~1.2s with animation & crunch audio)
                        if !held_slot.is_empty() {
                            if let Some(food) = state.item_registry.get_food(held_slot.item_id) {
                                if state.player_stats.can_consume(&food) {
                                    state.eating = Some(EatingState {
                                        slot: state.active_hotbar_slot,
                                        timer: 0.0,
                                        duration: food.eat_duration.max(0.4),
                                        crunch_accum: 0.0,
                                    });
                                    state.audio.play_eat_crunch();
                                    state.punch_timer = 0.16;
                                    return;
                                }
                            }
                        }

                        // 4b. Empty Bucket: Scoop Water or Lava
                        if held_slot.item_id == ItemId::BUCKET {
                            if let Some(hit) = state.targeted_block {
                                let target_block = {
                                    let world_guard = state.world_manager.world().lock().unwrap();
                                    world_guard.get_block(hit.hit_voxel)
                                };
                                let filled_bucket = if target_block.is_water() {
                                    Some((
                                        state.world_manager.scoop_water(
                                            hit.hit_voxel,
                                            &state.device,
                                            &state.pipeline,
                                        ),
                                        ItemId::WATER_BUCKET,
                                    ))
                                } else if target_block.is_lava() {
                                    Some((
                                        state.world_manager.scoop_lava(
                                            hit.hit_voxel,
                                            &state.device,
                                            &state.pipeline,
                                        ),
                                        ItemId::LAVA_BUCKET,
                                    ))
                                } else {
                                    None
                                };
                                if let Some((true, bucket_item)) = filled_bucket {
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
                                            ItemSlot::new(bucket_item, 1),
                                        );
                                    } else {
                                        state.inventory.slots[state.active_hotbar_slot].take(1);
                                        let overflow = state.inventory.add_item(
                                            ItemSlot::new(bucket_item, 1),
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

                        // 4c. Water Bucket / Lava Bucket: Place Liquid Source
                        if held_slot.item_id == ItemId::WATER_BUCKET
                            || held_slot.item_id == ItemId::LAVA_BUCKET
                        {
                            let is_lava_bucket = held_slot.item_id == ItemId::LAVA_BUCKET;
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
                                let placed = if is_lava_bucket {
                                    state.world_manager.place_lava(
                                        target_pos,
                                        &state.device,
                                        &state.pipeline,
                                    )
                                } else {
                                    state.world_manager.place_water(
                                        target_pos,
                                        &state.device,
                                        &state.pipeline,
                                    )
                                };
                                if placed {
                                    let liquid_block = if is_lava_bucket {
                                        BlockId::LAVA
                                    } else {
                                        BlockId::WATER
                                    };
                                    if let Some(client) = &mut state.mp_client {
                                        client.send(
                                            &blockgame_core::net::ClientPacket::BlockChange {
                                                pos: target_pos,
                                                block_id: liquid_block,
                                            },
                                        );
                                    }
                                    state.audio.play_block_place(
                                        liquid_block,
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
                                        if state.world_manager.is_block_protected(target_pos) {
                                            state.local_chat_messages.push(
                                                blockgame_core::net::ChatEntry {
                                                    sender: String::new(),
                                                    prefix: String::new(),
                                                    message: "&cThis temple is protected until its boss is defeated!".to_string(),
                                                    is_system: true,
                                                    age: 0.0,
                                                },
                                            );
                                            return;
                                        }
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
                                            if blockgame_core::is_eu_generator_block(block_id) {
                                                state.record_achievement_event(
                                                    blockgame_core::AchievementEvent::GeneratorPlaced,
                                                );
                                            }
                                            if !state.player_stats.game_mode.is_creative() {
                                                state.inventory.slots[state.active_hotbar_slot]
                                                    .take(1);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            InternalEvent::Window(WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: key_state,
                        text,
                        ..
                    },
                ..
            })
            | InternalEvent::SyntheticKey {
                key,
                state: key_state,
                text,
            } => {
                let pressed = key_state == ElementState::Pressed;
                if pressed && key == KeyCode::Escape && state.quick_menu_open {
                    state.quick_menu_open = false;
                    if state.screen == AppScreen::InGame {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::Locked);
                        state.window.set_cursor_visible(false);
                    }
                    return;
                }

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
                            game_mode,
                        } => {
                            if key == KeyCode::Tab {
                                focused_field = (focused_field + 1) % 2;
                                state.screen = AppScreen::CreateWorld {
                                    name_input,
                                    seed_input,
                                    focused_field,
                                    game_mode,
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
                                    state.create_and_start_world(&name_input, &seed_input, game_mode);
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
                                    game_mode,
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
                                    game_mode,
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
                            if let Some(bind_idx) = state.listening_keybind {
                                if key == KeyCode::Escape {
                                    state.listening_keybind = None;
                                } else {
                                    state.settings.keybinds.set_by_index(bind_idx, key);
                                    state.listening_keybind = None;
                                    state.settings.save(std::path::Path::new("settings.cfg"));
                                }
                            } else if key == KeyCode::Escape {
                                state.active_settings_slider = None;
                                state.settings.save(std::path::Path::new("settings.cfg"));
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
                            mut focused_field,
                            status_message,
                        } => {
                            let active_appearance = state.character_profile.active_appearance;
                            if key == KeyCode::Tab {
                                focused_field = (focused_field + 1) % 3;
                                state.screen = AppScreen::MultiplayerLobby {
                                    username_input,
                                    password_input,
                                    address_input,
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
                                    active_appearance,
                                ) {
                                    state.screen = AppScreen::MultiplayerLobby {
                                        username_input,
                                        password_input,
                                        address_input,
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
                                    focused_field,
                                    status_message: None,
                                };
                            }
                        }
                        AppScreen::CharacterCreator {
                            tab,
                            draft,
                            initial_snapshot,
                            mut selected_preset_name,
                            mut preset_name_input,
                            preset_input_focused,
                            preview_yaw,
                            face_zoom,
                            walk_preview,
                            active_color_target,
                            mut status_message,
                            return_to_multiplayer,
                        } => {
                            if key == KeyCode::Escape {
                                state.character_profile.active_appearance = draft.sanitized();
                                state.character_profile.selected_preset_name = selected_preset_name;
                                state
                                    .character_profile
                                    .save(std::path::Path::new("characters.yaml"));
                                state.screen = if return_to_multiplayer {
                                    AppScreen::MultiplayerLobby {
                                        username_input: state.local_username.clone(),
                                        password_input: String::new(),
                                        address_input: "127.0.0.1:25565".to_string(),
                                        focused_field: 0,
                                        status_message: None,
                                    }
                                } else {
                                    AppScreen::Title
                                };
                            } else if preset_input_focused {
                                if key == KeyCode::Enter {
                                    let saved = state
                                        .character_profile
                                        .save_custom_preset(&preset_name_input, draft);
                                    state
                                        .character_profile
                                        .save(std::path::Path::new("characters.yaml"));
                                    selected_preset_name = Some(saved.clone());
                                    preset_name_input.clear();
                                    status_message = Some(format!("Saved preset '{}'.", saved));
                                } else if key == KeyCode::Backspace {
                                    preset_name_input.pop();
                                } else if let Some(t) = text {
                                    for ch in t.chars() {
                                        if !ch.is_control() && preset_name_input.len() < 24 {
                                            preset_name_input.push(ch);
                                        }
                                    }
                                }
                                state.screen = AppScreen::CharacterCreator {
                                    tab,
                                    draft,
                                    initial_snapshot,
                                    selected_preset_name,
                                    preset_name_input,
                                    preset_input_focused,
                                    preview_yaw,
                                    face_zoom,
                                    walk_preview,
                                    active_color_target,
                                    status_message,
                                    return_to_multiplayer,
                                };
                            }
                        }
                        AppScreen::Guide { .. } => {
                            if key == KeyCode::Escape {
                                state.screen = AppScreen::Title;
                            }
                        }
                        AppScreen::Achievements { return_to_pause } => {
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
                            state.chat_scroll = 0;
                            if !state.inventory_open && !state.skills_open && !state.player_stats.is_dead {
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
                        } else if key == KeyCode::Enter {
                            let msg = state.chat_input.trim().to_string();
                            if !msg.is_empty() {
                                if let Some(client) = &mut state.mp_client {
                                    client.send(&blockgame_core::net::ClientPacket::Chat {
                                        message: msg,
                                    });
                                } else if msg.eq_ignore_ascii_case("/skills")
                                    || msg.eq_ignore_ascii_case("/skill")
                                    || msg.to_ascii_lowercase().starts_with("/skills ")
                                    || msg.to_ascii_lowercase().starts_with("/skill ")
                                {
                                    let parts: Vec<&str> = msg.split_whitespace().collect();
                                    let push_sys = |state: &mut AppState, text: String| {
                                        state.local_chat_messages.push(
                                            blockgame_core::net::ChatEntry {
                                                sender: "Skills".to_string(),
                                                prefix: String::new(),
                                                message: text,
                                                is_system: true,
                                                age: 0.0,
                                            },
                                        );
                                        if state.local_chat_messages.len() > 100 {
                                            state.local_chat_messages.remove(0);
                                        }
                                    };
                                    if parts.len() >= 2 {
                                        if let Some(sk) = SkillKind::parse(parts[1]) {
                                            let p = state.player_skills.get(sk);
                                            let need = PlayerSkills::xp_for_next_level(p.level);
                                            push_sys(
                                                state,
                                                format!(
                                                    "{} Lv {} (XP {}/{}) - {}",
                                                    sk.name(),
                                                    p.level,
                                                    p.xp,
                                                    need,
                                                    state.player_skills.perks_summary(sk)
                                                ),
                                            );
                                        } else {
                                            push_sys(
                                                state,
                                                "Unknown skill. Press K to open the Skills menu."
                                                    .to_string(),
                                            );
                                        }
                                    } else {
                                        push_sys(
                                            state,
                                            format!(
                                                "Skills Summary (Total Lv {} | Press K for full menu):",
                                                state.player_skills.total_level()
                                            ),
                                        );
                                        for sk in SkillKind::ALL {
                                            let p = state.player_skills.get(sk);
                                            push_sys(
                                                state,
                                                format!(
                                                    "  {} Lv {} ({})",
                                                    sk.name(),
                                                    p.level,
                                                    state.player_skills.perks_summary(sk)
                                                ),
                                            );
                                        }
                                    }
                                } else {
                                    let sender = if state.local_username.is_empty() {
                                        "Player".to_string()
                                    } else {
                                        state.local_username.clone()
                                    };
                                    state.local_chat_messages.push(
                                        blockgame_core::net::ChatEntry {
                                            sender,
                                            prefix: String::new(),
                                            message: msg,
                                            is_system: false,
                                            age: 0.0,
                                        },
                                    );
                                    if state.local_chat_messages.len() > 100 {
                                        state.local_chat_messages.remove(0);
                                    }
                                }
                            }
                            state.chat_open = false;
                            state.chat_input.clear();
                            state.chat_scroll = 0;
                            if !state.inventory_open && !state.skills_open && !state.player_stats.is_dead {
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            }
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

                // Storage Terminal search bar text entry when focused
                if state.storage_search_focused && state.inventory_open {
                    if pressed {
                        if key == KeyCode::Escape || key == KeyCode::Enter {
                            state.storage_search_focused = false;
                        } else if key == KeyCode::Backspace {
                            state.storage_search_text.pop();
                            state.storage_page = 0;
                        } else if let Some(t) = text {
                            for ch in t.chars() {
                                if !ch.is_control() {
                                    state.storage_search_text.push(ch);
                                    state.storage_page = 0;
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

                let kb = state.settings.keybinds.clone();
                let modal_open = state.inventory_open || state.skills_open;
                if key == kb.player_list && !modal_open {
                    state.tab_held = pressed;
                } else if key == kb.push_to_talk && !modal_open {
                    state.ptt_down = pressed;
                } else if key == kb.debug_hud && pressed && !modal_open {
                    state.settings.show_debug_hud = !state.settings.show_debug_hud;
                    state.settings.save(std::path::Path::new("settings.cfg"));
                } else if key == kb.third_person && pressed && !modal_open {
                    state.third_person = !state.third_person;
                } else if key == kb.forward && !state.skills_open {
                    state.moving_forward = pressed;
                } else if key == kb.backward && !state.skills_open {
                    state.moving_backward = pressed;
                } else if key == kb.left && !state.skills_open {
                    state.moving_left = pressed;
                } else if key == kb.right && !state.skills_open {
                    state.moving_right = pressed;
                } else if key == kb.jump && !state.skills_open {
                    if pressed && !state.jumping && state.player.on_ground && !state.player.flying && !state.player_stats.game_mode.is_creative() {
                        state.grant_skill_xp(SkillKind::Agility, 3);
                    }
                    if pressed && !state.jumping && state.player_stats.game_mode.is_creative() {
                        if state.total_time - state.last_jump_tap_time <= 0.30 {
                            state.player.toggle_flight();
                            state.last_jump_tap_time = -10.0;
                        } else {
                            state.last_jump_tap_time = state.total_time;
                        }
                    }
                    state.jumping = pressed;
                } else if key == kb.sprint && !state.skills_open {
                    state.sprinting = pressed;
                } else if key == kb.flight && pressed && !state.skills_open {
                    if state.player_stats.game_mode.is_creative() || state.player.flying {
                        state.player.toggle_flight();
                        log::info!("Flying mode: {}", state.player.flying);
                    }
                } else if key == kb.skills && pressed && !state.inventory_open {
                    state.mouse_left_down = false;
                    state.break_progress = 0.0;
                    state.breaking_voxel = None;
                    state.cracking_buffer = None;
                    state.achievements_open = false;
                    state.skills_open = !state.skills_open;
                    if state.skills_open {
                        state.moving_forward = false;
                        state.moving_backward = false;
                        state.moving_left = false;
                        state.moving_right = false;
                        state.jumping = false;
                        state.sprinting = false;
                        state.mouse_captured = false;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    } else {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                        state.window.set_cursor_visible(false);
                    }
                } else if key == kb.achievements && pressed && !state.inventory_open {
                    state.mouse_left_down = false;
                    state.break_progress = 0.0;
                    state.breaking_voxel = None;
                    state.cracking_buffer = None;
                    state.skills_open = false;
                    state.achievements_open = !state.achievements_open;
                    if state.achievements_open {
                        state.achievements_scroll = 0;
                        state.moving_forward = false;
                        state.moving_backward = false;
                        state.moving_left = false;
                        state.moving_right = false;
                        state.jumping = false;
                        state.sprinting = false;
                        state.mouse_captured = false;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    } else {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                        state.window.set_cursor_visible(false);
                    }
                } else if key == kb.inventory && pressed && !state.skills_open && !state.achievements_open {
                    state.mouse_left_down = false;
                    state.break_progress = 0.0;
                    state.breaking_voxel = None;
                    state.cracking_buffer = None;
                    state.active_chest_pos = None;
                    state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_shop_snapshot = None;
                    state.active_coal_gen_pos = None;
                    state.active_battery_pos = None;
                    state.active_electric_furnace_pos = None;
                    state.active_storage_terminal_pos = None;
                    state.storage_search_focused = false;
                    state.active_backpack_id = None;
                    state.jei_recipe_modal = None;
                    state.jei_search_focused = false;
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
                        state.crafting_grid = CraftingGrid::new_2x2();
                        state.inventory_open = true;
                        state.mouse_captured = false;
                        let _ = state
                            .window
                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                        state.window.set_cursor_visible(true);
                    }
                } else if (key == kb.chat || key == KeyCode::Slash) && pressed && !modal_open {
                    state.moving_forward = false;
                    state.moving_backward = false;
                    state.moving_left = false;
                    state.moving_right = false;
                    state.jumping = false;
                    state.sprinting = false;
                    state.chat_open = true;
                    state.chat_scroll = 0;
                    if key == KeyCode::Slash {
                        state.chat_input = "/".to_string();
                    } else {
                        state.chat_input.clear();
                    }
                    state.mouse_captured = false;
                    let _ = state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                    state.window.set_cursor_visible(true);
                } else {
                    match key {
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => state.shift_down = pressed,
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
                        KeyCode::Escape if pressed => {
                            state.mouse_left_down = false;
                            state.break_progress = 0.0;
                            state.breaking_voxel = None;
                            state.cracking_buffer = None;
                            state.active_chest_pos = None;
                            state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_shop_snapshot = None;
                            state.active_coal_gen_pos = None;
                            state.active_battery_pos = None;
                            state.active_electric_furnace_pos = None;
                            state.active_storage_terminal_pos = None;
                            state.storage_search_focused = false;
                            state.active_backpack_id = None;
                            if state.achievements_open {
                                state.achievements_open = false;
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            } else if state.skills_open {
                                state.skills_open = false;
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            } else if state.inventory_open {
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
                        KeyCode::KeyB
                            if pressed
                                && !state.skills_open
                                && !state.jei_search_focused
                                && !state.storage_search_focused =>
                        {
                            if state.active_backpack_id.is_some() && state.inventory_open {
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
                                state.active_backpack_id = None;
                                state.inventory_open = false;
                                state.mouse_captured = true;
                                let _ = state
                                    .window
                                    .set_cursor_grab(winit::window::CursorGrabMode::Confined);
                                state.window.set_cursor_visible(false);
                            } else if !modal_open {
                                // Prefer active hotbar slot if it holds a backpack; otherwise first backpack in inventory
                                let target_slot = if blockgame_core::storage::BackpackState::is_backpack_item(
                                    state.inventory.get(state.active_hotbar_slot).item_id,
                                ) {
                                    Some(state.active_hotbar_slot)
                                } else {
                                    (0..state.inventory.len()).find(|&idx| {
                                        blockgame_core::storage::BackpackState::is_backpack_item(
                                            state.inventory.get(idx).item_id,
                                        )
                                    })
                                };
                                if let Some(slot_idx) = target_slot {
                                    let mut slot = state.inventory.get(slot_idx);
                                    if let Some(bp_id) = state.backpacks.ensure_slot_id(&mut slot) {
                                        state.inventory.set(slot_idx, slot);
                                        state.mouse_left_down = false;
                                        state.break_progress = 0.0;
                                        state.breaking_voxel = None;
                                        state.cracking_buffer = None;
                                        state.active_chest_pos = None;
                                        state.active_furnace_pos = None;
                                state.active_cooking_pot_pos = None;
                                state.active_shop_snapshot = None;
                                        state.active_coal_gen_pos = None;
                                        state.active_battery_pos = None;
                                        state.active_electric_furnace_pos = None;
                                        state.active_storage_terminal_pos = None;
                                        state.storage_search_focused = false;
                                        state.active_backpack_id = Some(bp_id);
                                        state.backpack_page = 0;
                                        state.inventory_open = true;
                                        state.mouse_captured = false;
                                        let _ = state
                                            .window
                                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                                        state.window.set_cursor_visible(true);
                                    }
                                } else {
                                    state.local_chat_messages.push(blockgame_core::net::ChatEntry {
                                        sender: "Storage".to_string(),
                                        prefix: String::new(),
                                        message: "Carry a Leather, Iron, or Diamond Backpack in your inventory to open it!"
                                            .to_string(),
                                        is_system: true,
                                        age: 0.0,
                                    });
                                }
                            }
                        }
                        KeyCode::KeyY if pressed && !modal_open => {
                            state.day_night.time =
                                (state.day_night.time + 3000.0).rem_euclid(24000.0);
                            log::info!("Time of day advanced to: {:.1} ticks", state.day_night.time);
                        }
                        KeyCode::Digit1 if pressed && !state.skills_open => { state.active_hotbar_slot = 0; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit2 if pressed && !state.skills_open => { state.active_hotbar_slot = 1; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit3 if pressed && !state.skills_open => { state.active_hotbar_slot = 2; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit4 if pressed && !state.skills_open => { state.active_hotbar_slot = 3; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit5 if pressed && !state.skills_open => { state.active_hotbar_slot = 4; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit6 if pressed && !state.skills_open => { state.active_hotbar_slot = 5; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit7 if pressed && !state.skills_open => { state.active_hotbar_slot = 6; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit8 if pressed && !state.skills_open => { state.active_hotbar_slot = 7; state.hotbar_activity_timer = 4.0; }
                        KeyCode::Digit9 if pressed && !state.skills_open => { state.active_hotbar_slot = 8; state.hotbar_activity_timer = 4.0; }
                        _ => {}
                    }
                }
            }
            InternalEvent::Window(WindowEvent::RedrawRequested) => {
                let now = Instant::now();
                let dt = now
                    .duration_since(state.last_frame_time)
                    .as_secs_f32()
                    .min(0.1);
                state.last_frame_time = now;
                if state.screen == AppScreen::InGame {
                    state.mining_music_grace = (state.mining_music_grace - dt).max(0.0);
                    if !state.inventory_open {
                        if state.hotbar_activity_timer > 0.0 {
                            state.hotbar_activity_timer = (state.hotbar_activity_timer - dt).max(0.0);
                            state.hotbar_tuck_progress = (state.hotbar_tuck_progress - dt * 6.0).max(0.0);
                        } else {
                            state.hotbar_tuck_progress = (state.hotbar_tuck_progress + dt * 2.8).min(1.0);
                        }
                    } else {
                        state.hotbar_activity_timer = 4.0;
                        state.hotbar_tuck_progress = 0.0;
                    }
                } else {
                    state.mining_music_grace = 0.0;
                    state.hotbar_activity_timer = 4.0;
                    state.hotbar_tuck_progress = 0.0;
                }
                let is_underground = state.is_player_underground();
                let is_moving = state.moving_forward
                    || state.moving_backward
                    || state.moving_left
                    || state.moving_right;
                let is_adventuring =
                    state.screen == AppScreen::InGame && !is_underground && state.sprinting && is_moving;
                let is_mining =
                    state.screen == AppScreen::InGame && state.mining_music_grace > 0.0;
                let music_ctx = crate::audio::MusicContext {
                    is_underground,
                    is_adventuring,
                    is_mining,
                };
                state.music_scheduler.update(dt, &mut state.audio, music_ctx);
                if matches!(state.screen, AppScreen::Title | AppScreen::Guide { .. }) && !state.audio.is_music_playing() {
                    state.audio.play_music(crate::audio::MUSIC_TRACK_THE_VIEW_FROM_THE_RIDGE, 2.5);
                }

                let (screen_w, screen_h, ui_cursor) = state.ui_metrics();
                let aspect = screen_w / screen_h;
                state
                    .ui_pipeline
                    .update_screen_size(&state.queue, screen_w, screen_h);

                let is_mp_active = state.mp_client.is_some();
                let is_paused_screen = matches!(state.screen, AppScreen::Paused { .. });
                let is_settings_from_pause =
                    matches!(state.screen, AppScreen::Settings { return_to_pause: true });

                // On multiplayer servers, the world never pauses: packet polling, physics,
                // mob updates, and 3D rendering continue in the background while in Pause / Settings!
                let should_simulate_world =
                    state.screen == AppScreen::InGame || (is_mp_active && (is_paused_screen || is_settings_from_pause));

                // If player took fatal damage while in pause or settings menu on a server, bring up death screen
                if state.player_stats.is_dead
                    && matches!(state.screen, AppScreen::Paused { .. } | AppScreen::Settings { .. })
                {
                    state.screen = AppScreen::InGame;
                }

                if should_simulate_world {
                    // Age local chat messages
                    for entry in &mut state.local_chat_messages {
                        entry.age += dt;
                    }

                    // 0. Poll Multiplayer Network Packets & Sync Player State
                    let mut kick_reason: Option<String> = None;
                    let mut mp_ach_events: Vec<blockgame_core::AchievementEvent> = Vec::new();
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
                                blockgame_core::net::ServerPacket::FluidUpdate {
                                    pos,
                                    block_id,
                                    level,
                                } => {
                                    state.world_manager.apply_remote_fluid_update(
                                        pos,
                                        block_id,
                                        level,
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
                                        if state.is_sleeping {
                                            state.is_sleeping = false;
                                            mp_ach_events.push(
                                                blockgame_core::AchievementEvent::SleptInBed,
                                            );
                                        }
                                    }
                                    let local_id = client.player_id;
                                    client.remote_players.clear();
                                    for p in players {
                                        if p.player_id == local_id {
                                            client.local_prefix = p.prefix;
                                        } else {
                                            client.remote_players.insert(p.player_id, p);
                                        }
                                    }
                                }
                                blockgame_core::net::ServerPacket::ChatBroadcast {
                                    sender,
                                    prefix,
                                    message,
                                    is_system,
                                } => {
                                    client.chat_history.push(blockgame_core::net::ChatEntry {
                                        sender,
                                        prefix,
                                        message,
                                        is_system,
                                        age: 0.0,
                                    });
                                    if client.chat_history.len() > 100 {
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
                                    let obtained_id = if !item.is_empty() {
                                        Some(item.item_id.0)
                                    } else {
                                        None
                                    };
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
                                    if let Some(item_id) = obtained_id {
                                        mp_ach_events.push(
                                            blockgame_core::AchievementEvent::ItemObtained {
                                                item_id,
                                            },
                                        );
                                    }
                                }
                                blockgame_core::net::ServerPacket::GameModeUpdate { game_mode } => {
                                    state.player_stats.game_mode = game_mode;
                                    if !game_mode.is_creative() && state.player.flying {
                                        state.player.flying = false;
                                    }
                                    state.local_chat_messages.push(blockgame_core::net::ChatEntry {
                                        sender: String::new(),
                                        prefix: String::new(),
                                        message: format!(
                                            "Your game mode has been updated to {}.",
                                            game_mode.display_name()
                                        ),
                                        is_system: true,
                                        age: 0.0,
                                    });
                                    if state.local_chat_messages.len() > 100 {
                                        state.local_chat_messages.remove(0);
                                    }
                                }
                                blockgame_core::net::ServerPacket::SkillsUpdate { skills } => {
                                    state.player_skills = skills;
                                    state.skills_dirty = false;
                                }
                                blockgame_core::net::ServerPacket::AchievementEligibility {
                                    enabled,
                                    ..
                                } => {
                                    state.achievements.set_eligible(enabled);
                                }
                                blockgame_core::net::ServerPacket::AchievementEvent { event } => {
                                    mp_ach_events.push(event);
                                }
                                blockgame_core::net::ServerPacket::TempleMonstersSync { monsters } => {
                                    state.world_manager.apply_mp_temple_monsters(&monsters);
                                }
                                blockgame_core::net::ServerPacket::TempleArrowsSync { arrows } => {
                                    state.world_manager.apply_mp_temple_arrows(&arrows);
                                }
                                blockgame_core::net::ServerPacket::BossHudUpdate { hud } => {
                                    state.world_manager.mp_boss_hud = hud;
                                }
                                blockgame_core::net::ServerPacket::TempleProtectionSync { zones } => {
                                    for t in state.world_manager.temples.temples.values_mut() {
                                        t.boss_defeated =
                                            !zones.iter().any(|z| z.contains(t.arena_origin));
                                    }
                                    state.world_manager.temples.synced_protected_zones = zones;
                                }
                                blockgame_core::net::ServerPacket::AnimalsSync { animals } => {
                                    state.world_manager.apply_synced_animals(&animals);
                                }
                                blockgame_core::net::ServerPacket::VillageNpcsSync { npcs } => {
                                    state.world_manager.mp_village_npcs = npcs;
                                }
                                blockgame_core::net::ServerPacket::ShopUiSync {
                                    snapshot,
                                    status_message,
                                } => {
                                    let was_open = state.active_shop_snapshot.is_some();
                                    state.active_shop_snapshot = Some(snapshot);
                                    state.shop_status_message = status_message;
                                    if !was_open {
                                        state.shop_tab = blockgame_render::ShopUiTab::Buy;
                                        state.shop_selected_enhance_slot = None;
                                    }
                                    state.inventory_open = true;
                                    state.active_chest_pos = None;
                                    state.active_furnace_pos = None;
                                    state.active_cooking_pot_pos = None;
                                    state.mouse_captured = false;
                                    let _ = state
                                        .window
                                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                                    state.window.set_cursor_visible(true);
                                }
                                blockgame_core::net::ServerPacket::InventorySync { slots } => {
                                    for (i, slot) in slots.into_iter().enumerate() {
                                        if i < state.inventory.len() {
                                            state.inventory.set(i, slot);
                                        }
                                    }
                                }
                                blockgame_core::net::ServerPacket::CookingPotSync {
                                    pos,
                                    ingredients,
                                    fuel,
                                    output,
                                    burn_time,
                                    max_burn_time,
                                    cook_progress,
                                } => {
                                    let mut world_guard =
                                        state.world_manager.world().lock().unwrap();
                                    let pot = world_guard.get_or_create_cooking_pot(pos);
                                    pot.ingredients = ingredients;
                                    pot.fuel = fuel;
                                    pot.output = output;
                                    pot.burn_time_remaining = burn_time;
                                    pot.max_burn_time = max_burn_time;
                                    pot.cook_progress = cook_progress;
                                }
                                blockgame_core::net::ServerPacket::VoiceBroadcast {
                                    sender_id,
                                    seq,
                                    position,
                                    data,
                                } => {
                                    if state.settings.voice_mode != 0 && state.settings.voice_volume > 0.001 {
                                        let pcm = blockgame_core::voice::decode_adpcm_frame(&data);
                                        if !pcm.is_empty() {
                                            state.audio.push_spatial_voice(
                                                sender_id,
                                                seq,
                                                &pcm,
                                                position,
                                                state.camera.position,
                                                state.camera.yaw,
                                            );
                                            state.speaking_players.insert(sender_id, 0.35);
                                        }
                                    }
                                }
                                _ => {}
                            }

                        }

                        // Capture & transmit local proximity voice frames
                        let frames = state
                            .voice_capture
                            .poll_encoded_frames(
                                state.ptt_down || state.controller_ptt_held,
                                state.settings.voice_mode,
                            );
                        state.voice_transmitting = !frames.is_empty();
                        for (seq, data) in frames {
                            client.send(&blockgame_core::net::ClientPacket::VoiceFrame { seq, data });
                        }

                        if !client.connected && kick_reason.is_none() {
                            kick_reason = Some("Connection to server lost.".to_string());
                        }
                    } else {
                        let _ = state.voice_capture.poll_encoded_frames(false, 0);
                        state.voice_transmitting = false;
                    }
                    for ev in mp_ach_events {
                        state.record_achievement_event(ev);
                    }

                    state.speaking_players.retain(|_, t| {
                        *t -= dt;
                        *t > 0.0
                    });

                    if let Some((_, _, ref mut timer)) = state.skill_toast {
                        *timer -= dt;
                    }
                    if state.skill_toast.as_ref().map(|(_, _, t)| *t <= 0.0).unwrap_or(false) {
                        state.skill_toast = None;
                    }
                    if state.achievement_toasts.tick(dt) {
                        state.audio.play_achievement_unlock();
                    }
                    state.biome_check_timer += dt;
                    if state.biome_check_timer >= 0.5 {
                        state.biome_check_timer = 0.0;
                        let px = state.player.position.x.floor() as i32;
                        let py = state.player.position.y.floor() as i32;
                        let pz = state.player.position.z.floor() as i32;
                        let biome = state.world_manager.generator.get_biome(px, pz, py);
                        let biome_key = format!("{:?}", biome).to_ascii_lowercase();
                        state.record_achievement_event(
                            blockgame_core::AchievementEvent::BiomeVisited { biome_key },
                        );
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
                        } else if state.controller_move_axes[0].abs() > 0.0
                            || state.controller_move_axes[1].abs() > 0.0
                        {
                            // Preserve analog stick magnitude (0.0..=1.0) for variable walk speed
                            wish_dir = forward * state.controller_move_axes[1]
                                + right * state.controller_move_axes[0];
                            let len = wish_dir.length();
                            if len > 1.0 {
                                wish_dir /= len;
                            }
                        }
                        if wish_dir.length_squared() > 0.0 && state.is_sleeping {
                            state.is_sleeping = false;
                            if let Some(client) = &mut state.mp_client {
                                client.send(&blockgame_core::net::ClientPacket::SetSleeping {
                                    sleeping: false,
                                });
                            }
                        }

                        // If flying, vertical movement with Jump (ascend) / Sneak (descend)
                        if state.player.flying {
                            if state.jumping || state.controller_jumping {
                                wish_dir.y += 1.0;
                            }
                            if state.shift_down || state.controller_sneaking {
                                wish_dir.y -= 1.0;
                            }
                        }
                    }

                    // 2. Step player physics against voxel world (with Agility skill + food buff modifiers)
                    let agi_speed = state.player_skills.agility_speed_multiplier()
                        * state.player_stats.speed_buff_multiplier();
                    let agi_jump = state.player_skills.agility_jump_multiplier();
                    let fall_damage = {
                        let world_guard = state.world_manager.world().lock().unwrap();
                        state.player.update_with_modifiers(
                            dt,
                            wish_dir,
                            state.sprinting || state.controller_sprinting,
                            state.jumping || state.controller_jumping,
                            &world_guard,
                            &self.registry,
                            agi_speed,
                            agi_jump,
                        )
                    };

                    // Check if player's head is submerged in water, player is in water, or touching cactus
                    let head_block_pos = IVec3::new(
                        state.player.position.x.floor() as i32,
                        (state.player.position.y + 1.62).floor() as i32,
                        state.player.position.z.floor() as i32,
                    );
                    let (is_head_submerged, is_in_water, is_in_lava, is_touching_cactus) = {
                        let world = state.world_manager.world().lock().unwrap();
                        let submerged = world.get_block(head_block_pos).is_water();
                        let in_water = state.player.is_in_water(&world);
                        let in_lava = !in_water && state.player.is_in_lava(&world);
                        let touching_cactus = state.player.is_touching_cactus(&world);
                        (submerged, in_water, in_lava, touching_cactus)
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
                        let fall_mult = state.player_skills.agility_fall_damage_multiplier();
                        if state.player_stats.take_damage(fall_damage * armor_mult * fall_mult) {
                            if state
                                .armor
                                .damage_armor(fall_damage * fall_mult, &state.item_registry)
                            {
                                state.audio.play_tool_break();
                            }
                            state.grant_skill_xp(
                                SkillKind::Agility,
                                ((fall_damage * 4.0) as u64).max(5),
                            );
                        }
                    }
                    if is_touching_cactus {
                        if state.player_stats.take_damage(1.0 * armor_mult)
                            && state.armor.damage_armor(1.0, &state.item_registry)
                        {
                            state.audio.play_tool_break();
                        }
                    }
                    let lava_raw_dmg = state.player_stats.tick_lava_hazard(
                        dt,
                        is_in_lava,
                        is_in_water,
                        armor_mult,
                    );
                    if lava_raw_dmg > 0.0
                        && state
                            .armor
                            .damage_armor(lava_raw_dmg, &state.item_registry)
                    {
                        state.audio.play_tool_break();
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
                        if (state.sprinting || is_in_water) && !state.player.flying {
                            state.agility_xp_timer += dt;
                            if state.agility_xp_timer >= 0.8 {
                                state.agility_xp_timer -= 0.8;
                                state.grant_skill_xp(SkillKind::Agility, 4);
                            }
                        }
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
                            state.skills_open = false;
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
                            let skills_to_sync = if state.skills_dirty {
                                state.skills_dirty = false;
                                Some(state.player_skills.clone())
                            } else {
                                None
                            };
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
                                if let Some(skills) = skills_to_sync {
                                    client.send(
                                        &blockgame_core::net::ClientPacket::SkillsSync { skills },
                                    );
                                }
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
                            let obtained_id = item.item_id.0;
                            let overflow = state.inventory.add_item(item, &state.item_registry);
                            state.record_achievement_event(
                                blockgame_core::AchievementEvent::ItemObtained {
                                    item_id: obtained_id,
                                },
                            );
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

                    // 3a. Hold-Right-Click Eating / Drinking Tick (Issue #12)
                    if let Some(eat_slot) = state.eating.as_ref().map(|e| e.slot) {
                        let slot_changed = eat_slot != state.active_hotbar_slot;
                        let held = state.inventory.get(state.active_hotbar_slot);
                        let food_opt = state.item_registry.get_food(held.item_id);
                        let still_valid = (state.mouse_right_down || state.controller_use_held)
                            && !slot_changed
                            && !state.inventory_open
                            && !state.skills_open
                            && !state.player_stats.is_dead
                            && food_opt
                                .map(|f| state.player_stats.can_consume(&f))
                                .unwrap_or(false);

                        if !still_valid {
                            // Cancel eating without consuming item (release or hotbar slot switch)
                            state.eating = None;
                        } else if let Some(food) = food_opt {
                            // Slow horizontal movement while eating
                            state.player.velocity.x *= (1.0 - 6.0 * dt).clamp(0.35, 1.0);
                            state.player.velocity.z *= (1.0 - 6.0 * dt).clamp(0.35, 1.0);

                            let mut finished = false;
                            if let Some(eating) = &mut state.eating {
                                eating.timer += dt;
                                eating.crunch_accum += dt;
                                if eating.crunch_accum >= 0.28 {
                                    eating.crunch_accum = 0.0;
                                    state.audio.play_eat_crunch();
                                    state.punch_timer = 0.16; // Chomp/bob animation pulse
                                }
                                if eating.timer >= eating.duration {
                                    finished = true;
                                }
                            }

                            if finished {
                                let food_mult = state.player_skills.farming_food_multiplier();
                                let rng_roll =
                                    blockgame_core::fishing::next_f32(&mut state.fishing_rng);
                                let had_poison = state.player_stats.status_effects.iter().any(|e| {
                                    matches!(
                                        e.kind,
                                        blockgame_core::item::FoodEffectKind::FoodPoisoning
                                            | blockgame_core::item::FoodEffectKind::Poison
                                    )
                                });
                                let is_herbal_tea =
                                    state.inventory.get(state.active_hotbar_slot).item_id
                                        == ItemId::HERBAL_TEA;
                                if state.player_stats.consume_food(&food, food_mult, rng_roll) {
                                    state.inventory.slots[state.active_hotbar_slot].take(1);
                                    state.audio.play_eat_crunch();
                                    state.punch_timer = 0.2;
                                    if had_poison && is_herbal_tea && food.cures_negative_effects {
                                        state.record_achievement_event(
                                            blockgame_core::AchievementEvent::CuredPoisonWithHerbalTea,
                                        );
                                    }
                                }
                                state.eating = None;
                            }
                        }
                    }

                    // 3b. Update Animals, Monsters, Arrows & Planted Crops (ADR-0004: gated in MP)
                    if state.mp_client.is_none() {
                        let held_id = state.inventory.get(state.active_hotbar_slot).item_id;
                        let prev_animal_count = state.world_manager.animals.len();
                        let animal_drops = state.world_manager.update_animals_with_lure(
                            dt,
                            state.player.position,
                            held_id,
                        );
                        if state.world_manager.animals.len() > prev_animal_count {
                            let newborns = state.world_manager.animals[prev_animal_count..]
                                .iter()
                                .filter(|a| a.baby_timer > 0.0)
                                .count();
                            for _ in 0..newborns {
                                state.record_achievement_event(
                                    blockgame_core::AchievementEvent::AnimalBred,
                                );
                            }
                        }
                        for (pos, drop_slot) in animal_drops {
                            state.spawn_drop_synced(pos, Vec3::new(0.0, 2.0, 0.0), drop_slot);
                        }
                    }
                    let (monster_dmg, monster_drops) = state.world_manager.update_monsters(
                        dt,
                        state.player.position,
                        state.day_night.sun_intensity(),
                        state.mp_client.is_some(),
                    );
                    let guard_dmg = state.world_manager.update_villages(
                        dt,
                        state.player.position,
                        state.day_night.time,
                        state.mp_client.is_some(),
                    );
                    if guard_dmg > 0.0 && !state.player_stats.game_mode.is_creative() {
                        let mult = state.armor.damage_multiplier(&state.item_registry);
                        state.player_stats.take_damage(guard_dmg * mult);
                        state.audio.play_hurt();
                    }
                    if let Some(client) = &mut state.mp_client {
                        for bp in state.world_manager.pending_village_blueprints.drain(..) {
                            client.send(
                                &blockgame_core::net::ClientPacket::RegisterVillageBlueprint {
                                    village_id: bp.village_id,
                                    theme: bp.theme,
                                    origin: bp.origin,
                                    shops: bp.shops,
                                    guard_positions: bp.guard_positions,
                                },
                            );
                        }
                    } else {
                        state.world_manager.pending_village_blueprints.clear();
                    }
                    for msg in state.world_manager.pending_temple_messages.drain(..) {
                        state.local_chat_messages.push(blockgame_core::net::ChatEntry {
                            sender: String::new(),
                            prefix: String::new(),
                            message: msg,
                            is_system: true,
                            age: 0.0,
                        });
                        if state.local_chat_messages.len() > 100 {
                            state.local_chat_messages.remove(0);
                        }
                    }
                    for (pos, drops) in monster_drops {
                        for drop_slot in drops {
                            state.spawn_drop_synced(pos, Vec3::new(0.0, 2.0, 0.0), drop_slot);
                        }
                    }
                    let (arrow_dmg, arrow_hits, arrow_kills, mp_arrow_hits) =
                        state.world_manager.update_arrows(
                            dt,
                            state.player.position,
                            state.player.half_width,
                            state.player.height,
                        );
                    if !state.world_manager.pending_mp_npc_arrow_hits.is_empty() {
                        if let Some(client) = &mut state.mp_client {
                            for (npc_id, damage, knockback) in
                                state.world_manager.pending_mp_npc_arrow_hits.drain(..)
                            {
                                client.send(
                                    &blockgame_core::net::ClientPacket::AttackVillageNpc {
                                        npc_id,
                                        damage,
                                        knockback,
                                    },
                                );
                            }
                        } else {
                            state.world_manager.pending_mp_npc_arrow_hits.clear();
                        }
                    }
                    if !mp_arrow_hits.is_empty() {
                        if let Some(client) = &mut state.mp_client {
                            for (monster_id, damage, knockback) in mp_arrow_hits {
                                client.send(
                                    &blockgame_core::net::ClientPacket::AttackTempleMonster {
                                        monster_id,
                                        damage,
                                        knockback,
                                    },
                                );
                            }
                        }
                    }
                    if !state.world_manager.pending_arrow_killed_monsters.is_empty() {
                        let killed_kinds: Vec<_> = state
                            .world_manager
                            .pending_arrow_killed_monsters
                            .drain(..)
                            .collect();
                        for mkind in killed_kinds {
                            state.record_achievement_event(
                                blockgame_core::AchievementEvent::MonsterKilled {
                                    is_boss: mkind.is_boss(),
                                    boss_key: blockgame_core::boss_achievement_key(mkind)
                                        .map(str::to_string),
                                },
                            );
                        }
                    }
                    if arrow_hits > 0 {
                        state.grant_skill_xp(
                            SkillKind::Combat,
                            (arrow_hits as u64) * 10 + (arrow_kills as u64) * 25,
                        );
                        if arrow_kills > 0 && !state.player_stats.is_dead {
                            let ls = state.player_skills.combat_lifesteal_on_kill()
                                * (arrow_kills as f32);
                            if ls > 0.0 {
                                state.player_stats.health = (state.player_stats.health + ls)
                                    .min(state.player_stats.max_health);
                            }
                        }
                    }
                    let total_hostile_dmg = monster_dmg + arrow_dmg;
                    if total_hostile_dmg > 0.0
                        && !state.player_stats.is_dead
                        && !state.player.flying
                    {
                        let armor_mult = state.armor.damage_multiplier(&state.item_registry);
                        let combat_red = 1.0 - state.player_skills.combat_damage_reduction();
                        if state
                            .player_stats
                            .take_damage(total_hostile_dmg * armor_mult * combat_red)
                        {
                            state.audio.play_hurt();
                            if state
                                .armor
                                .damage_armor(total_hostile_dmg * combat_red, &state.item_registry)
                            {
                                state.audio.play_tool_break();
                            }
                        }
                    }

                    // Periodic ambient sounds for nearby animals and monsters
                    state.creature_sound_timer -= dt;
                    if state.creature_sound_timer <= 0.0 {
                        state.creature_sound_timer = 4.0 + 4.0 * state.music_scheduler.random_f32();
                        let ppos = state.player.position;
                        let mut candidates = Vec::new();
                        for animal in &state.world_manager.animals {
                            if !animal.dead && animal.position.distance_squared(ppos) <= 24.0 * 24.0 {
                                candidates.push((true, animal.kind as usize, animal.position));
                            }
                        }
                        for monster in &state.world_manager.monsters {
                            if !monster.dead && monster.position.distance_squared(ppos) <= 24.0 * 24.0 {
                                candidates.push((false, monster.kind as usize, monster.position));
                            }
                        }
                        if !candidates.is_empty() {
                            let pick_idx = (state.music_scheduler.random_f32() * candidates.len() as f32) as usize % candidates.len();
                            let (is_animal, kind_idx, pos) = candidates[pick_idx];
                            if is_animal {
                                let kind = match kind_idx {
                                    0 => blockgame_core::animal::AnimalKind::Cow,
                                    1 => blockgame_core::animal::AnimalKind::Pig,
                                    2 => blockgame_core::animal::AnimalKind::Chicken,
                                    _ => blockgame_core::animal::AnimalKind::Sheep,
                                };
                                state.audio.play_animal_ambient(kind, pos, ppos, state.camera.yaw);
                            } else {
                                let kind = match kind_idx {
                                    0 => blockgame_core::monster::MonsterKind::Zombie,
                                    1 => blockgame_core::monster::MonsterKind::Skeleton,
                                    _ => blockgame_core::monster::MonsterKind::Spider,
                                };
                                state.audio.play_monster_ambient(kind, pos, ppos, state.camera.yaw);
                            }
                        }
                    }

                    // Update active fishing session (bobber flight, bite timer, and Stardew minigame)
                    let held_is_rod = blockgame_core::FishingRodKind::from_item(
                        state.inventory.get(state.active_hotbar_slot).item_id,
                    )
                    .is_some();
                    if !held_is_rod && !matches!(state.fishing_state, FishingSession::Idle) {
                        state.fishing_state = FishingSession::Idle;
                    } else {
                        let player_pos = state.player.position;
                        match &mut state.fishing_state {
                            FishingSession::Idle => {}
                            FishingSession::Waiting {
                                bobber_pos,
                                bobber_vel,
                                landed,
                                wait_timer,
                                is_ocean,
                                ..
                            } => {
                                if bobber_pos.distance(player_pos) > 26.0 {
                                    state.fishing_state = FishingSession::Idle;
                                } else if !*landed {
                                    bobber_vel.y -= 16.0 * dt;
                                    *bobber_pos += *bobber_vel * dt;
                                    let voxel = IVec3::new(
                                        bobber_pos.x.floor() as i32,
                                        bobber_pos.y.floor() as i32,
                                        bobber_pos.z.floor() as i32,
                                    );
                                    let (hit_water, hit_solid, deep_water) = {
                                        let w = state.world_manager.world().lock().unwrap();
                                        let b = w.get_block(voxel);
                                        let is_w = b.is_water();
                                        let is_s = state.registry.is_solid(b);
                                        let mut depth = 0;
                                        if is_w {
                                            for d in 1..=5 {
                                                if w.get_block(voxel - IVec3::new(0, d, 0)).is_water() {
                                                    depth += 1;
                                                } else {
                                                    break;
                                                }
                                            }
                                        }
                                        (is_w, is_s, depth >= 3)
                                    };
                                    if hit_water {
                                        *landed = true;
                                        *bobber_vel = Vec3::ZERO;
                                        bobber_pos.y = voxel.y as f32 + 0.85;
                                        *is_ocean = deep_water;
                                        state.audio.play_fishing_splash();
                                    } else if hit_solid || bobber_pos.y < 0.0 {
                                        state.fishing_state = FishingSession::Idle;
                                    }
                                } else {
                                    *wait_timer -= dt;
                                    if *wait_timer <= 0.0 {
                                        let bpos = *bobber_pos;
                                        let ocean = *is_ocean;
                                        let is_night = blockgame_core::net::is_night_ticks(
                                            state.day_night.time,
                                        );
                                        if let FishingSession::Waiting {
                                            rod_slot,
                                            tackle,
                                            ..
                                        } = state.fishing_state.clone()
                                        {
                                            let species = blockgame_core::roll_fish_species(
                                                &mut state.fishing_rng,
                                                ocean,
                                                is_night,
                                                &tackle,
                                            );
                                            state.audio.play_fishing_bite();
                                            state.trigger_controller_rumble(
                                                crate::controller::RumbleEffect::FishingBite,
                                            );
                                            state.fishing_state = FishingSession::Biting {
                                                bobber_pos: bpos,
                                                bite_timer: 1.85,
                                                species,
                                                rod_slot,
                                                tackle,
                                            };
                                        }
                                    }
                                }
                            }
                            FishingSession::Biting { bite_timer, .. } => {
                                *bite_timer -= dt;
                                if *bite_timer <= 0.0 {
                                    state.audio.play_fishing_splash();
                                    state.fishing_state = FishingSession::Idle;
                                }
                            }
                            FishingSession::Minigame { .. } => {
                                let holding = state.mouse_left_down
                                    || state.mouse_right_down
                                    || state.controller_mine_held
                                    || state.controller_use_held
                                    || state.jumping
                                    || state.controller_jumping;
                                let mut outcome = None;
                                if let FishingSession::Minigame { game, .. } =
                                    &mut state.fishing_state
                                {
                                    let status = game.step(dt, holding, &mut state.fishing_rng);
                                    if status != blockgame_core::MinigameStatus::Active {
                                        outcome = Some((status, game.species, game.perfect));
                                    }
                                }
                                if let Some((status, species, perfect)) = outcome {
                                    if let FishingSession::Minigame {
                                        rod_slot,
                                        tackle,
                                        ..
                                    } = state.fishing_state.clone()
                                    {
                                        state.fishing_state = FishingSession::Idle;
                                        if status == blockgame_core::MinigameStatus::Caught {
                                            state.audio.play_fishing_catch();
                                            state.record_achievement_event(
                                                blockgame_core::AchievementEvent::FishCaught {
                                                    species_key: format!("{:?}", species)
                                                        .to_ascii_lowercase(),
                                                },
                                            );

                                            // Damage rod, line, and lure by 1 durability
                                            let mut rslot = state.inventory.get(rod_slot);
                                            if let Some(def) = state.item_registry.get(rslot.item_id) {
                                                if let Some(max_d) = def.durability {
                                                    if rslot.damage_tool(1, max_d) {
                                                        state.audio.play_tool_break();
                                                    }
                                                    state.inventory.set(rod_slot, rslot);
                                                }
                                            }
                                            if let Some(lidx) = tackle.line_slot {
                                                let mut lslot = state.inventory.get(lidx);
                                                if let Some(def) = state.item_registry.get(lslot.item_id) {
                                                    if let Some(max_d) = def.durability {
                                                        if lslot.damage_tool(1, max_d) {
                                                            state.audio.play_tool_break();
                                                        }
                                                        state.inventory.set(lidx, lslot);
                                                    }
                                                }
                                            }
                                            if let Some(uidx) = tackle.lure_slot {
                                                let mut uslot = state.inventory.get(uidx);
                                                if let Some(def) = state.item_registry.get(uslot.item_id) {
                                                    if let Some(max_d) = def.durability {
                                                        if uslot.damage_tool(1, max_d) {
                                                            state.audio.play_tool_break();
                                                        }
                                                        state.inventory.set(uidx, uslot);
                                                    }
                                                }
                                            }

                                            // Award caught fish (+ Fishing Double Catch skill roll)
                                            let double_catch =
                                                blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                    < state.player_skills.fishing_double_catch_chance();
                                            let fish_count = if double_catch { 2 } else { 1 };
                                            let fish_item = ItemSlot::new(species.item_id, fish_count);
                                            if let Some(left) = state
                                                .inventory
                                                .add_item(fish_item, &state.item_registry)
                                            {
                                                state.spawn_drop_synced(
                                                    state.player.position,
                                                    Vec3::new(0.0, 2.0, 0.0),
                                                    left,
                                                );
                                            }

                                            let mut msg = if perfect {
                                                format!("Perfect catch! Caught {}!", species.name)
                                            } else {
                                                format!("Caught {}!", species.name)
                                            };
                                            if double_catch {
                                                msg.push_str(" (x2 Double Catch!)");
                                            }

                                            // Roll optional bonus treasure chest/loot
                                            let mut got_treasure = false;
                                            if blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                < tackle.treasure_chance
                                            {
                                                got_treasure = true;
                                                let treasure = blockgame_core::roll_treasure_loot(
                                                    &mut state.fishing_rng,
                                                );
                                                let t_name = state
                                                    .item_registry
                                                    .get(treasure.item_id)
                                                    .map(|d| d.display_name)
                                                    .unwrap_or("Treasure");
                                                msg.push_str(&format!(
                                                    " + Bonus Treasure: {}x {}!",
                                                    treasure.count, t_name
                                                ));
                                                if let Some(left) = state
                                                    .inventory
                                                    .add_item(treasure, &state.item_registry)
                                                {
                                                    state.spawn_drop_synced(
                                                        state.player.position,
                                                        Vec3::new(0.0, 2.0, 0.0),
                                                        left,
                                                    );
                                                }
                                            }

                                            let mut fish_xp = 20 + (species.difficulty * 0.8) as u64;
                                            if perfect {
                                                fish_xp = (fish_xp * 135) / 100;
                                            }
                                            if got_treasure {
                                                fish_xp += 25;
                                            }
                                            state.grant_skill_xp(SkillKind::Fishing, fish_xp);

                                            state.local_chat_messages.push(
                                                blockgame_core::net::ChatEntry {
                                                    sender: "Fishing".to_string(),
                                                    prefix: String::new(),
                                                    message: msg,
                                                    is_system: true,
                                                    age: 0.0,
                                                },
                                            );
                                        } else {
                                            state.audio.play_fishing_splash();
                                            state.local_chat_messages.push(
                                                blockgame_core::net::ChatEntry {
                                                    sender: "Fishing".to_string(),
                                                    prefix: String::new(),
                                                    message: format!(
                                                        "The {} got away...",
                                                        species.name
                                                    ),
                                                    is_system: true,
                                                    age: 0.0,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if state.mp_client.is_none() {
                        state.world_manager.update_farming(
                            dt,
                            state.day_night.sun_intensity(),
                            state.player.position,
                            &state.device,
                            &state.pipeline,
                        );
                        state
                            .world_manager
                            .update_furnaces(dt, &state.device, &state.pipeline);
                        state
                            .world_manager
                            .update_cooking_pots(dt, &state.device, &state.pipeline);
                        state
                            .world_manager
                            .tick_fluids(dt, &state.device, &state.pipeline);
                    }
                    state
                        .world_manager
                        .update_cacti(dt, &state.device, &state.pipeline);
                    state
                        .world_manager
                        .update_decaying_leaves(dt, &state.device, &state.pipeline);
                    let is_daytime = state.day_night.sun_direction().y > 0.0;
                    state.world_manager.update_power_networks(
                        dt,
                        is_daytime,
                        &state.device,
                        &state.pipeline,
                    );
                    state
                        .world_manager
                        .update_falling_blocks(dt, &state.device, &state.pipeline);

                    // 4. Update Camera position (First-Person or F5 Third-Person) and atmosphere uniform
                    let eye_pos = state.player.eye_position()
                        - if (state.shift_down || state.controller_sneaking)
                            && !state.inventory_open
                            && !state.skills_open
                            && !state.player.flying
                        {
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

                    // 5. Perform Fast Voxel Traversal (DDA) raycast from player eye_pos (only in active gameplay)
                    if state.screen == AppScreen::InGame {
                        let world_guard = state.world_manager.world().lock().unwrap();
                        let held_item = state.inventory.get(state.active_hotbar_slot).item_id;
                        let is_placeable = state
                            .item_registry
                            .get(held_item)
                            .and_then(|d| d.placeable_block)
                            .is_some();
                        let is_bucket =
                            held_item == ItemId::BUCKET
                                || held_item == ItemId::WATER_BUCKET
                                || held_item == ItemId::LAVA_BUCKET;
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
                    } else {
                        state.targeted_block = None;
                        state.wireframe_buffer = None;
                    }

                    // 5b. Process hold-to-break mining mechanics
                    state.creative_break_cooldown = (state.creative_break_cooldown - dt).max(0.0);
                    let is_creative = state.player_stats.game_mode.is_creative();
                    let mining_held = state.mouse_left_down || state.controller_mine_held;
                    if !mining_held {
                        state.creative_break_cooldown = 0.0;
                    }
                    if mining_held
                        && state.mouse_captured
                        && !state.inventory_open
                        && !state.skills_open
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

                                let base_break_time = self.registry.get_break_time(
                                    block_at_voxel,
                                    held_tool_type,
                                    held_tool_tier,
                                );
                                let break_speed_skill =
                                    break_speed_skill_for_block(block_at_voxel);
                                let skill_speed = match break_speed_skill {
                                    Some(SkillKind::Mining) => {
                                        state.player_skills.mining_speed_multiplier()
                                    }
                                    Some(SkillKind::Woodcutting) => {
                                        state.player_skills.woodcutting_speed_multiplier()
                                    }
                                    Some(SkillKind::Digging) => {
                                        state.player_skills.digging_speed_multiplier()
                                    }
                                    _ => 1.0,
                                };
                                let held_enh_speed = state
                                    .inventory
                                    .get(state.active_hotbar_slot)
                                    .mining_speed_multiplier()
                                    * state.player_stats.mining_haste_multiplier();
                                let break_time = if base_break_time.is_finite() {
                                    base_break_time / (skill_speed.max(1.0) * held_enh_speed)
                                } else {
                                    base_break_time
                                };
                                if state.world_manager.is_block_protected(target_voxel)
                                    || (break_time.is_infinite() && !is_creative)
                                {
                                    state.break_progress = 0.0;
                                    state.cracking_buffer = None;
                                    state.break_hit_timer = 0.0;
                                } else {
                                    if break_speed_skill == Some(SkillKind::Mining)
                                        || state.is_player_underground()
                                    {
                                        state.mining_music_grace = 4.0;
                                    }
                                    if is_creative {
                                        if state.creative_break_cooldown <= 0.0 {
                                            state.break_progress = 1.0;
                                            state.creative_break_cooldown = 0.20;
                                        }
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
                                            state.trigger_controller_rumble(
                                                crate::controller::RumbleEffect::BlockBreak,
                                            );
                                            state.audio.play_block_break(
                                                broken,
                                                target_voxel.as_vec3() + Vec3::splat(0.5),
                                                state.camera.position,
                                                state.camera.yaw,
                                            );
                                            if !is_creative {
                                                state.record_achievement_event(
                                                    blockgame_core::AchievementEvent::BlockMined {
                                                        block_id: broken.0,
                                                    },
                                                );
                                                if blockgame_core::is_stem_fruit_block(broken) {
                                                    let has_stem = {
                                                        let wg = state
                                                            .world_manager
                                                            .world()
                                                            .lock()
                                                            .unwrap();
                                                        [
                                                            IVec3::X,
                                                            IVec3::NEG_X,
                                                            IVec3::Z,
                                                            IVec3::NEG_Z,
                                                        ]
                                                        .iter()
                                                        .any(|&d| {
                                                            let nb = wg.get_block(target_voxel + d);
                                                            (broken == BlockId::MELON
                                                                && nb == BlockId::MELON_STEM_3)
                                                                || (broken == BlockId::PUMPKIN
                                                                    && nb == BlockId::PUMPKIN_STEM_3)
                                                        })
                                                    };
                                                    if has_stem {
                                                        state.record_achievement_event(
                                                            blockgame_core::AchievementEvent::StemFruitHarvested,
                                                        );
                                                    }
                                                }
                                                state.player_stats.add_exhaustion(0.025);
                                                if let Some((sk, xp)) =
                                                    skill_xp_for_broken_block(broken)
                                                {
                                                    state.grant_skill_xp(sk, xp);
                                                }
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
                                                let drop_pos = target_voxel.as_vec3()
                                                    + Vec3::new(0.5, 0.5, 0.5);
                                                if let Some((drop_item, drop_count)) = self
                                                    .registry
                                                    .get_drop(broken, held_tool_type, held_tool_tier)
                                                {
                                                    let drop_slot =
                                                        ItemSlot::new(drop_item, drop_count);
                                                    state.spawn_drop_synced(
                                                        drop_pos,
                                                        Vec3::new(0.0, 2.5, 0.0),
                                                        drop_slot,
                                                    );

                                                    let prospector_chance = state
                                                        .inventory
                                                        .get(state.active_hotbar_slot)
                                                        .prospector_bonus_chance();
                                                    if prospector_chance > 0.0
                                                        && blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                            < prospector_chance
                                                    {
                                                        state.spawn_drop_synced(
                                                            drop_pos,
                                                            Vec3::new(0.25, 2.5, -0.25),
                                                            ItemSlot::new(drop_item, drop_count),
                                                        );
                                                    }

                                                    match break_speed_skill_for_block(broken) {
                                                        Some(SkillKind::Mining) => {
                                                            if blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                                < state.player_skills.mining_bonus_drop_chance()
                                                            {
                                                                state.spawn_drop_synced(
                                                                    drop_pos,
                                                                    Vec3::new(0.3, 2.4, 0.2),
                                                                    ItemSlot::new(drop_item, drop_count),
                                                                );
                                                            }
                                                            let is_ore = matches!(
                                                                broken,
                                                                BlockId::COAL_ORE
                                                                    | BlockId::COPPER_ORE
                                                                    | BlockId::IRON_ORE
                                                                    | BlockId::GOLD_ORE
                                                                    | BlockId::DIAMOND_ORE
                                                                    | BlockId::DEEPSTONE_COAL_ORE
                                                                    | BlockId::DEEPSTONE_COPPER_ORE
                                                                    | BlockId::DEEPSTONE_IRON_ORE
                                                                    | BlockId::DEEPSTONE_GOLD_ORE
                                                                    | BlockId::DEEPSTONE_DIAMOND_ORE
                                                            );
                                                            if is_ore
                                                                && blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                                    < state.player_skills.mining_triple_ore_chance()
                                                            {
                                                                state.spawn_drop_synced(
                                                                    drop_pos,
                                                                    Vec3::new(-0.3, 2.6, -0.2),
                                                                    ItemSlot::new(drop_item, drop_count),
                                                                );
                                                            }
                                                        }
                                                        Some(SkillKind::Woodcutting) => {
                                                            if blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                                < state.player_skills.woodcutting_bonus_drop_chance()
                                                            {
                                                                state.spawn_drop_synced(
                                                                    drop_pos,
                                                                    Vec3::new(0.3, 2.4, 0.2),
                                                                    ItemSlot::new(drop_item, drop_count),
                                                                );
                                                            }
                                                        }
                                                        Some(SkillKind::Digging) => {
                                                            if blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                                < state.player_skills.digging_double_drop_chance()
                                                            {
                                                                state.spawn_drop_synced(
                                                                    drop_pos,
                                                                    Vec3::new(0.3, 2.4, 0.2),
                                                                    ItemSlot::new(drop_item, drop_count),
                                                                );
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                }

                                                // Digging Treasure Seeker / Relic Hunter buried finds
                                                if break_speed_skill_for_block(broken)
                                                    == Some(SkillKind::Digging)
                                                    && blockgame_core::fishing::next_f32(&mut state.fishing_rng)
                                                        < state.player_skills.digging_treasure_chance()
                                                {
                                                    let d_lvl = state
                                                        .player_skills
                                                        .level(SkillKind::Digging);
                                                    let r = blockgame_core::fishing::next_f32(
                                                        &mut state.fishing_rng,
                                                    );
                                                    let treasure_id = if d_lvl >= 750 && r > 0.88 {
                                                        ItemId::DIAMOND
                                                    } else if d_lvl >= 750 && r > 0.74 {
                                                        ItemId::GOLD_INGOT
                                                    } else if d_lvl >= 250 && r > 0.56 {
                                                        ItemId::RAW_IRON
                                                    } else if d_lvl >= 250 && r > 0.42 {
                                                        ItemId::APPLE
                                                    } else if r > 0.28 {
                                                        ItemId::STRING
                                                    } else if r > 0.14 {
                                                        ItemId::CLAY
                                                    } else {
                                                        ItemId::COAL
                                                    };
                                                    state.spawn_drop_synced(
                                                        drop_pos,
                                                        Vec3::new(0.0, 2.8, 0.0),
                                                        ItemSlot::new(treasure_id, 1),
                                                    );
                                                }

                                                // Woodcutting Lv 250+ Tree Feller (with Axe, not sneaking)
                                                if broken.is_log()
                                                    && held_tool_type
                                                        == Some(blockgame_core::tool::ToolType::Axe)
                                                    && !state.shift_down
                                                {
                                                    let max_logs =
                                                        state.player_skills.tree_feller_max_logs();
                                                    if max_logs > 0 {
                                                        let felled = state
                                                            .world_manager
                                                            .fell_connected_logs(
                                                                target_voxel,
                                                                max_logs,
                                                                &state.device,
                                                                &state.pipeline,
                                                            );
                                                        let felled_count = felled.len() as u64;
                                                        for (fpos, fblock) in felled {
                                                            if let Some(client) =
                                                                &mut state.mp_client
                                                            {
                                                                client.send(
                                                                    &blockgame_core::net::ClientPacket::BlockChange {
                                                                        pos: fpos,
                                                                        block_id: BlockId::AIR,
                                                                    },
                                                                );
                                                            }
                                                            let fdrop_pos = fpos.as_vec3()
                                                                + Vec3::new(0.5, 0.5, 0.5);
                                                            if let Some((di, dc)) = self
                                                                .registry
                                                                .get_drop(
                                                                    fblock,
                                                                    held_tool_type,
                                                                    held_tool_tier,
                                                                )
                                                            {
                                                                state.spawn_drop_synced(
                                                                    fdrop_pos,
                                                                    Vec3::new(0.0, 2.0, 0.0),
                                                                    ItemSlot::new(di, dc),
                                                                );
                                                                if blockgame_core::fishing::next_f32(
                                                                    &mut state.fishing_rng,
                                                                ) < state
                                                                    .player_skills
                                                                    .woodcutting_bonus_drop_chance()
                                                                {
                                                                    state.spawn_drop_synced(
                                                                        fdrop_pos,
                                                                        Vec3::new(0.2, 2.2, 0.2),
                                                                        ItemSlot::new(di, dc),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        if felled_count > 0 {
                                                            state.grant_skill_xp(
                                                                SkillKind::Woodcutting,
                                                                felled_count * 16,
                                                            );
                                                        }
                                                    }
                                                }

                                                // 15% chance to drop Apple from Leaves + Leaf Blower skill bonus
                                                if broken.is_leaves() {
                                                    let hash = (target_voxel.x.wrapping_mul(73856093)
                                                        ^ target_voxel.y.wrapping_mul(19349663)
                                                        ^ target_voxel.z.wrapping_mul(83492791))
                                                        % 100;
                                                    if hash.abs() < 15 {
                                                        state.spawn_drop_synced(
                                                            drop_pos,
                                                            Vec3::new(0.0, 2.0, 0.0),
                                                            ItemSlot::new(ItemId::APPLE, 1),
                                                        );
                                                    }
                                                    if state
                                                        .player_skills
                                                        .woodcutting_leaf_bonus_unlocked()
                                                    {
                                                        let r = blockgame_core::fishing::next_f32(
                                                            &mut state.fishing_rng,
                                                        );
                                                        if r < 0.10 {
                                                            state.spawn_drop_synced(
                                                                drop_pos,
                                                                Vec3::new(0.1, 2.2, -0.1),
                                                                ItemSlot::new(ItemId::APPLE, 1),
                                                            );
                                                        } else if r < 0.22 {
                                                            state.spawn_drop_synced(
                                                                drop_pos,
                                                                Vec3::new(-0.1, 2.2, 0.1),
                                                                ItemSlot::new(ItemId::STICK, 1),
                                                            );
                                                        }
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
                                                            drop_pos,
                                                            Vec3::new(0.0, 2.0, 0.0),
                                                            ItemSlot::new(ItemId::WHEAT_SEEDS, 1),
                                                        );
                                                    }
                                                }
                                                // Mature farmland crops: seeds/extra drops, Bone Fertilizer +1 yield, Farming perks & Auto-Replant
                                                if let Some(replant_stage) = broken.base_replant_stage() {
                                                    let (fert_tier, reset_farmland) =
                                                        state.world_manager.consume_fertilizer_on_harvest(
                                                            target_voxel,
                                                            &state.device,
                                                            &state.pipeline,
                                                        );
                                                    if let (Some(reset_blk), Some(client)) =
                                                        (reset_farmland, &mut state.mp_client)
                                                    {
                                                        client.send(
                                                            &blockgame_core::net::ClientPacket::BlockChange {
                                                                pos: target_voxel - IVec3::Y,
                                                                block_id: reset_blk,
                                                            },
                                                        );
                                                    }

                                                    let (primary_drop, seed_drop_opt) = match broken {
                                                        BlockId::WHEAT_3 => {
                                                            (ItemId::WHEAT, Some(ItemId::WHEAT_SEEDS))
                                                        }
                                                        BlockId::CARROTS_3 => (ItemId::CARROT, None),
                                                        BlockId::POTATOES_3 => (ItemId::POTATO, None),
                                                        BlockId::TOMATOES_3 => {
                                                            (ItemId::TOMATO, Some(ItemId::TOMATO_SEEDS))
                                                        }
                                                        _ => (ItemId::WHEAT, None),
                                                    };

                                                    // Bone Fertilizer guaranteed +1 bonus crop yield
                                                    let fert_bonus = fert_tier.bonus_yield();
                                                    if fert_bonus > 0 {
                                                        state.spawn_drop_synced(
                                                            drop_pos,
                                                            Vec3::new(0.15, 2.2, -0.15),
                                                            ItemSlot::new(primary_drop, fert_bonus),
                                                        );
                                                    }

                                                    // Seed-based crops drop additional 1..2 seeds on mature harvest
                                                    if let Some(seed_id) = seed_drop_opt {
                                                        let hash = (target_voxel.x.wrapping_mul(1234567)
                                                            ^ target_voxel.z.wrapping_mul(7654321))
                                                            % 2;
                                                        let seed_count = (hash.abs() + 1) as u16;
                                                        state.spawn_drop_synced(
                                                            drop_pos,
                                                            Vec3::new(0.0, 2.0, 0.0),
                                                            ItemSlot::new(seed_id, seed_count),
                                                        );
                                                    }

                                                    if blockgame_core::fishing::next_f32(
                                                        &mut state.fishing_rng,
                                                    ) < state
                                                        .player_skills
                                                        .farming_bonus_crop_chance()
                                                    {
                                                        state.spawn_drop_synced(
                                                            drop_pos,
                                                            Vec3::new(0.2, 2.3, 0.2),
                                                            ItemSlot::new(primary_drop, 1),
                                                        );
                                                        if let Some(seed_id) = seed_drop_opt {
                                                            state.spawn_drop_synced(
                                                                drop_pos,
                                                                Vec3::new(-0.2, 2.3, -0.2),
                                                                ItemSlot::new(seed_id, 1),
                                                            );
                                                        }
                                                    }

                                                    if state.player_skills.farming_auto_replant() {
                                                        let replanted = state.world_manager.place_block(
                                                            target_voxel,
                                                            replant_stage,
                                                            &state.player.aabb(),
                                                            &state.device,
                                                            &state.pipeline,
                                                        );
                                                        if replanted {
                                                            state
                                                                .world_manager
                                                                .plant_crop(target_voxel);
                                                            if let Some(client) =
                                                                &mut state.mp_client
                                                            {
                                                                client.send(
                                                                    &blockgame_core::net::ClientPacket::BlockChange {
                                                                        pos: target_voxel,
                                                                        block_id: replant_stage,
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        state.breaking_voxel = None;
                                        state.break_progress = 0.0;
                                        state.cracking_buffer = None;
                                        state.break_hit_timer = 0.0;
                                    } else if !is_creative {
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

                    // 7. Build 2D UI Overlay (HUD, Skills Screen, or Inventory/Crafting Screen)
                    if state.screen == AppScreen::InGame {
                        if state.achievements_open {
                            UiBuilder::build_achievements_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &state.achievements,
                                &state.item_registry,
                                state.achievements_category,
                                state.achievements_scroll,
                                Some(state.achievements.is_eligible()),
                                ui_cursor,
                            );
                        } else if state.skills_open {
                            UiBuilder::build_skills_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &state.player_skills,
                                state.selected_skill_idx,
                                ui_cursor,
                            );
                        } else if state.inventory_open {
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
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(shop_snap) = &state.active_shop_snapshot {
                            UiBuilder::build_shop_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                shop_snap,
                                state.shop_tab,
                                state.shop_selected_enhance_slot,
                                &state.shop_status_message,
                                &state.inventory,
                                ui_cursor,
                                &state.item_registry,
                            );
                        } else if let Some(pot_pos) = state.active_cooking_pot_pos {
                            let pot = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                world_guard
                                    .get_cooking_pot(pot_pos)
                                    .cloned()
                                    .unwrap_or_default()
                            };
                            UiBuilder::build_cooking_pot_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &pot,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(gen_pos) = state.active_coal_gen_pos {
                            let is_daytime = state.day_night.sun_direction().y > 0.0;
                            let (gen_state, summary) = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                let g = world_guard
                                    .coal_generators
                                    .get(&gen_pos)
                                    .cloned()
                                    .unwrap_or_default();
                                let s = blockgame_core::power::get_network_summary(
                                    &world_guard,
                                    gen_pos,
                                    is_daytime,
                                );
                                (g, s)
                            };
                            UiBuilder::build_coal_generator_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &gen_state,
                                &summary,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(bat_pos) = state.active_battery_pos {
                            let is_daytime = state.day_night.sun_direction().y > 0.0;
                            let (bat_state, summary) = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                let b = world_guard
                                    .batteries
                                    .get(&bat_pos)
                                    .cloned()
                                    .unwrap_or_default();
                                let s = blockgame_core::power::get_network_summary(
                                    &world_guard,
                                    bat_pos,
                                    is_daytime,
                                );
                                (b, s)
                            };
                            UiBuilder::build_battery_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &bat_state,
                                &summary,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(ef_pos) = state.active_electric_furnace_pos {
                            let is_daytime = state.day_night.sun_direction().y > 0.0;
                            let (ef_state, summary) = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                let ef = world_guard
                                    .electric_furnaces
                                    .get(&ef_pos)
                                    .cloned()
                                    .unwrap_or_default();
                                let s = blockgame_core::power::get_network_summary(
                                    &world_guard,
                                    ef_pos,
                                    is_daytime,
                                );
                                (ef, s)
                            };
                            UiBuilder::build_electric_furnace_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &ef_state,
                                &summary,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(term_pos) = state.active_storage_terminal_pos {
                            let (info, items) = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                let info = blockgame_core::storage::collect_storage_network(
                                    &world_guard,
                                    term_pos,
                                );
                                let items = blockgame_core::storage::list_network_items(
                                    &world_guard,
                                    &info.chests,
                                    &state.storage_search_text,
                                    state.storage_sort_mode,
                                    &state.item_registry,
                                );
                                (info, items)
                            };
                            let total_pages = ((items.len() + 26) / 27).max(1);
                            if state.storage_page >= total_pages {
                                state.storage_page = total_pages - 1;
                            }
                            UiBuilder::build_storage_terminal_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &info,
                                &items,
                                &state.storage_search_text,
                                state.storage_search_focused,
                                state.storage_sort_mode,
                                state.storage_page,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(bp_id) = state.active_backpack_id {
                            let bp_ref = state.backpacks.get(bp_id).cloned().unwrap_or_default();
                            UiBuilder::build_backpack_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                bp_id,
                                &bp_ref,
                                state.backpack_page,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                            );
                        } else if let Some(chest_pos) = state.active_chest_pos {
                            let (chest_title, chest_slots) = {
                                let world_guard = state.world_manager.world().lock().unwrap();
                                let blk = world_guard.get_block(chest_pos);
                                let title = match blk {
                                    BlockId::IRON_CHEST => "Iron Chest (45 Slots)",
                                    BlockId::GOLD_CHEST => "Gold Chest (63 Slots)",
                                    BlockId::DIAMOND_CHEST => "Diamond Chest (81 Slots)",
                                    _ => "Wooden Chest",
                                };
                                let slots = world_guard
                                    .get_chest(chest_pos)
                                    .cloned()
                                    .unwrap_or_else(|| vec![ItemSlot::EMPTY; blk.chest_slot_count().max(27)]);
                                (title, slots)
                            };
                            let total_pages = ((chest_slots.len() + 26) / 27).max(1);
                            if state.chest_page >= total_pages {
                                state.chest_page = total_pages - 1;
                            }
                            UiBuilder::build_chest_screen_paged(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                chest_title,
                                &chest_slots,
                                state.chest_page,
                                &state.inventory,
                                state.active_hotbar_slot,
                                &state.cursor_held,
                                ui_cursor,
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
                                ui_cursor,
                                &state.item_registry,
                                Some(&jei_params),
                                modal_params.as_ref(),
                                &state.armor.slots,
                            );
                        }
                    } else {
                        state.fps_frame_count += 1;
                        let elapsed = state.fps_timer.elapsed().as_secs_f32();
                        if elapsed >= 0.5 {
                            state.fps_display = (state.fps_frame_count as f32 / elapsed).round() as u32;
                            state.fps_frame_count = 0;
                            state.fps_timer = Instant::now();
                        }

                        let bob_dist = if state.settings.view_bobbing {
                            state.walk_distance
                        } else {
                            0.0
                        };
                        UiBuilder::build_hud_with_appearance(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            &state.inventory,
                            state.active_hotbar_slot,
                            &state.item_registry,
                            bob_dist,
                            punch_time,
                            &state.player_stats,
                            state.armor.total_defense(&state.item_registry),
                            state.hotbar_tuck_progress,
                            &state.character_profile.active_appearance,
                        );

                        if let Some(eating) = &state.eating {
                            if eating.duration > 0.0 {
                                let progress = (eating.timer / eating.duration).clamp(0.0, 1.0);
                                UiBuilder::draw_eating_progress_bar(
                                    &mut state.ui_mesh,
                                    screen_w,
                                    screen_h,
                                    progress,
                                );
                            }
                        }

                        if let Some(boss_hud) = state
                            .world_manager
                            .active_boss_hud(state.player.position, state.mp_client.is_some())
                        {
                            UiBuilder::draw_boss_hud(&mut state.ui_mesh, screen_w, &boss_hud);
                        }

                        match &state.fishing_state {
                            FishingSession::Idle => {}
                            FishingSession::Waiting { tackle, .. } => {
                                let mut summary = tackle.rod.display_name().to_string();
                                if let Some(l) = tackle.line {
                                    summary.push_str(&format!(" + {}", l.display_name()));
                                }
                                if let Some(u) = tackle.lure {
                                    summary.push_str(&format!(" + {}", u.display_name()));
                                }
                                if tackle.bait_count > 0 {
                                    summary.push_str(&format!(" | Bait: {}", tackle.bait_count));
                                }
                                let input_mode = state.input_router.input_mode();
                                let glyph_family = state
                                    .input_router
                                    .resolved_glyph_family(state.settings.controller_glyph_style);
                                let use_btn = state.settings.controller_binds.use_item;
                                UiBuilder::build_fishing_hud_with_input_mode(
                                    &mut state.ui_mesh,
                                    screen_w,
                                    screen_h,
                                    &blockgame_render::FishingHudState {
                                        phase: blockgame_render::FishingHudPhase::Waiting,
                                        tackle_summary: summary,
                                    },
                                    input_mode,
                                    glyph_family,
                                    use_btn,
                                );
                            }
                            FishingSession::Biting { .. } => {
                                let input_mode = state.input_router.input_mode();
                                let glyph_family = state
                                    .input_router
                                    .resolved_glyph_family(state.settings.controller_glyph_style);
                                let use_btn = state.settings.controller_binds.use_item;
                                UiBuilder::build_fishing_hud_with_input_mode(
                                    &mut state.ui_mesh,
                                    screen_w,
                                    screen_h,
                                    &blockgame_render::FishingHudState {
                                        phase: blockgame_render::FishingHudPhase::Biting,
                                        tackle_summary: String::new(),
                                    },
                                    input_mode,
                                    glyph_family,
                                    use_btn,
                                );
                            }
                            FishingSession::Minigame { game, .. } => {
                                let fish_tile = state
                                    .item_registry
                                    .get(game.species.item_id)
                                    .map(|d| d.icon_tile as u16)
                                    .unwrap_or(396);
                                let input_mode = state.input_router.input_mode();
                                let glyph_family = state
                                    .input_router
                                    .resolved_glyph_family(state.settings.controller_glyph_style);
                                let use_btn = state.settings.controller_binds.use_item;
                                UiBuilder::build_fishing_hud_with_input_mode(
                                    &mut state.ui_mesh,
                                    screen_w,
                                    screen_h,
                                    &blockgame_render::FishingHudState {
                                        phase: blockgame_render::FishingHudPhase::Minigame {
                                            bar_pos: game.bar_pos,
                                            bar_height: game.bar_height,
                                            fish_pos: game.fish_pos,
                                            progress: game.progress,
                                            fish_in_bar: game.fish_in_bar,
                                            fish_tile,
                                            fish_name: game.species.name.to_string(),
                                            is_rare: game.species.is_rare,
                                        },
                                        tackle_summary: String::new(),
                                    },
                                    input_mode,
                                    glyph_family,
                                    use_btn,
                                );
                            }
                        }

                        if state.settings.show_debug_hud {
                            let pos = state.player.position;
                            let (cpos, _, _, _) =
                                blockgame_core::world::world_to_chunk_coords(pos.floor().as_ivec3());
                            let yaw = state.camera.yaw.rem_euclid(std::f32::consts::TAU);
                            let facing = if !(std::f32::consts::FRAC_PI_4
                                ..3.0 * std::f32::consts::FRAC_PI_4)
                                .contains(&yaw)
                            {
                                if (3.0 * std::f32::consts::FRAC_PI_4
                                    ..5.0 * std::f32::consts::FRAC_PI_4)
                                    .contains(&yaw)
                                {
                                    "West (-X)"
                                } else if (5.0 * std::f32::consts::FRAC_PI_4
                                    ..7.0 * std::f32::consts::FRAC_PI_4)
                                    .contains(&yaw)
                                {
                                    "North (-Z)"
                                } else {
                                    "East (+X)"
                                }
                            } else {
                                "South (+Z)"
                            };
                            UiBuilder::draw_debug_overlay(
                                &mut state.ui_mesh,
                                state.fps_display,
                                [pos.x, pos.y, pos.z],
                                [cpos.0.x, cpos.0.y, cpos.0.z],
                                facing,
                                state.day_night.time,
                            );
                        }

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
                        if let Some((target_npc_id, _, _, _)) = state
                            .world_manager
                            .raycast_village_npc(
                                state.player.eye_position(),
                                state.camera.forward(),
                                10.0,
                            )
                        {
                            let target_pid = 0x8000_0000 | target_npc_id;
                            if let Some(model) = state
                                .world_manager
                                .collect_village_npc_render_models()
                                .into_iter()
                                .find(|m| m.player_id == target_pid)
                            {
                                nametag_models.push(model);
                            }
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
                        state.chat_hitboxes = UiBuilder::draw_multiplayer_hud_overlays(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            chat_slice,
                            state.chat_open,
                            &state.chat_input,
                            state.chat_scroll,
                            ui_cursor,
                            state.tab_held || state.controller_player_list_held,
                            &online_models,
                            &local_name,
                            state.mp_hosted_server.is_some(),
                            &server_addr,
                        );
                        if state.mp_client.is_some() {
                            let ptt_name = crate::settings::key_display_name(
                                state.settings.keybinds.push_to_talk,
                            );
                            let nearby_speakers: Vec<String> = online_models
                                .iter()
                                .filter(|p| state.speaking_players.contains_key(&p.player_id))
                                .map(|p| p.username.clone())
                                .collect();
                            UiBuilder::draw_voice_hud_badge(
                                &mut state.ui_mesh,
                                screen_w,
                                state.settings.voice_mode,
                                state.voice_transmitting,
                                &ptt_name,
                                &nearby_speakers,
                            );
                        }
                        if let Some((ref title, ref subtitle, timer)) = state.skill_toast {
                            let alpha = (timer / 0.8).clamp(0.0, 1.0);
                            UiBuilder::draw_skill_levelup_banner(
                                &mut state.ui_mesh,
                                screen_w,
                                title,
                                subtitle,
                                alpha,
                            );
                        }
                        if let Some(toast) = state.achievement_toasts.active() {
                            UiBuilder::draw_achievement_toast(
                                &mut state.ui_mesh,
                                screen_w,
                                &toast.title,
                                &toast.description,
                                toast.icon_tile,
                                toast.alpha(),
                            );
                        }
                    }
                } else if let AppScreen::Paused { ref status_message } = state.screen {
                        // In multiplayer pause menu: draw floating player nametags in 3D world
                        let mut nametag_models = Vec::new();
                        if let Some(client) = &state.mp_client {
                            for rp in client.remote_players.values() {
                                if rp.health > 0.0 {
                                    nametag_models.push(rp.clone());
                                }
                            }
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
                        let input_mode = state.input_router.input_mode();
                        let glyph_family = state
                            .input_router
                            .resolved_glyph_family(state.settings.controller_glyph_style);
                        UiBuilder::build_pause_menu_with_input_mode(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            ui_cursor,
                            status_message.as_deref(),
                            input_mode,
                            glyph_family,
                        );
                    } else if let AppScreen::Settings { .. } = state.screen {
                        let ui_settings = state.settings_ui_state();
                        let keybinds = state.settings.keybinds.labels_and_keys();
                        let keybind_refs: Vec<(&str, &str)> = keybinds
                            .iter()
                            .map(|(a, k)| (*a, k.as_str()))
                            .collect();
                        let pad_binds = state.settings.controller_binds.labels_and_buttons();
                        UiBuilder::build_settings_screen(
                            &mut state.ui_mesh,
                            screen_w,
                            screen_h,
                            ui_settings,
                            state.settings_tab,
                            state.active_settings_slider,
                            state.listening_keybind,
                            &keybind_refs,
                            state.listening_controller_bind,
                            &pad_binds,
                            ui_cursor,
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

                    let input_mode = state.input_router.input_mode();
                    let glyph_family = state
                        .input_router
                        .resolved_glyph_family(state.settings.controller_glyph_style);
                    UiBuilder::build_pause_menu_with_input_mode(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        ui_cursor,
                        status_message.as_deref(),
                        input_mode,
                        glyph_family,
                    );
                } else {
                    // Menu screens (Title, WorldSelect, CreateWorld, Settings, MultiplayerLobby):
                    // Slowly orbit camera over the currently loaded 3D terrain around player.position
                    state.total_time += dt;
                    if matches!(state.screen, AppScreen::Title | AppScreen::Guide { .. }) {
                        state.day_night.set_time(5500.0);
                    }
                    state.poll_directory_servers();
                    state.world_manager.update(
                        state.player.position,
                        &state.device,
                        &state.pipeline,
                    );

                    let angle = state.total_time * 0.06;
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
                            UiBuilder::build_title_screen_with_recent_world(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                ui_cursor,
                                state.saved_worlds.first(),
                            );
                        }
                        AppScreen::Guide { page } => {
                            let input_mode = state.input_router.input_mode();
                            let glyph_family = state
                                .input_router
                                .resolved_glyph_family(state.settings.controller_glyph_style);
                            UiBuilder::build_guide_screen_with_input_mode(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                *page,
                                ui_cursor,
                                input_mode,
                                glyph_family,
                            );
                        }
                        AppScreen::Achievements { return_to_pause } => {
                            UiBuilder::build_achievements_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &state.achievements,
                                &state.item_registry,
                                state.achievements_category,
                                state.achievements_scroll,
                                if *return_to_pause {
                                    Some(state.achievements.is_eligible())
                                } else {
                                    None
                                },
                                ui_cursor,
                            );
                        }
                        AppScreen::WorldSelect { selected_index } => {
                            UiBuilder::build_world_select_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                &state.saved_worlds,
                                *selected_index,
                                ui_cursor,
                            );
                        }
                        AppScreen::CreateWorld {
                            name_input,
                            seed_input,
                            focused_field,
                            game_mode,
                        } => {
                            UiBuilder::build_create_world_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                name_input,
                                seed_input,
                                *focused_field,
                                *game_mode,
                                ui_cursor,
                            );
                        }
                        AppScreen::Settings { .. } => {
                            let ui_settings = state.settings_ui_state();
                            let keybinds = state.settings.keybinds.labels_and_keys();
                            let keybind_refs: Vec<(&str, &str)> = keybinds
                                .iter()
                                .map(|(a, k)| (*a, k.as_str()))
                                .collect();
                            let pad_binds = state.settings.controller_binds.labels_and_buttons();
                            UiBuilder::build_settings_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                ui_settings,
                                state.settings_tab,
                                state.active_settings_slider,
                                state.listening_keybind,
                                &keybind_refs,
                                state.listening_controller_bind,
                                &pad_binds,
                                ui_cursor,
                            );
                        }
                        AppScreen::MultiplayerLobby {
                            username_input,
                            password_input,
                            address_input,
                            focused_field,
                            status_message,
                        } => {
                            let status_ref = status_message.as_ref().map(|(m, err)| (m.as_str(), *err));
                            let preset_lbl = state.character_preset_label();
                            UiBuilder::build_multiplayer_lobby_screen(
                                &mut state.ui_mesh,
                                screen_w,
                                screen_h,
                                username_input,
                                password_input,
                                address_input,
                                &state.character_profile.active_appearance,
                                &preset_lbl,
                                *focused_field,
                                status_ref,
                                &state.directory_servers,
                                state.selected_directory_idx,
                                state.directory_loading,
                                state.directory_error.as_deref(),
                                ui_cursor,
                            );
                        }
                        AppScreen::CharacterCreator { .. } => {
                            if let Some(creator_ui) = state.build_creator_ui_state() {
                                UiBuilder::build_character_creator_screen(
                                    &mut state.ui_mesh,
                                    screen_w,
                                    screen_h,
                                    &creator_ui,
                                    ui_cursor,
                                );
                            }
                        }
                        AppScreen::InGame | AppScreen::Paused { .. } => {}
                    }
                }

                // Render Quick-Menu overlay when open in gameplay
                if state.screen == AppScreen::InGame && state.quick_menu_open {
                    let cfg = state.settings.controller_config();
                    let (confirm_btn, cancel_btn) =
                        state.input_router.menu_confirm_cancel_buttons(&cfg);
                    let glyph_family = state
                        .input_router
                        .resolved_glyph_family(state.settings.controller_glyph_style);
                    UiBuilder::build_quick_menu(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        state.settings.show_debug_hud,
                        glyph_family,
                        confirm_btn,
                        cancel_btn,
                        ui_cursor,
                    );
                }

                // Render non-blocking HUD toast (e.g., multiplayer controller disconnect)
                if let Some((ref msg, _)) = state.hud_toast {
                    UiBuilder::draw_hud_toast(&mut state.ui_mesh, screen_w, msg);
                }

                // Render JEI / Inventory controller glyph legend bar when in Controller Input Mode
                if state.screen == AppScreen::InGame
                    && state.inventory_open
                    && state.input_router.input_mode() == blockgame_render::InputMode::Controller
                {
                    let glyph_family = state
                        .input_router
                        .resolved_glyph_family(state.settings.controller_glyph_style);
                    UiBuilder::draw_jei_controller_footer(
                        &mut state.ui_mesh,
                        screen_w,
                        screen_h,
                        glyph_family,
                    );
                }

                // Render Virtual Cursor in Controller Input Mode whenever a menu/overlay is active
                if state.input_router.input_mode() == blockgame_render::InputMode::Controller
                    && (!state.mouse_captured || state.screen != AppScreen::InGame)
                {
                    UiBuilder::draw_virtual_cursor(
                        &mut state.ui_mesh,
                        ui_cursor[0],
                        ui_cursor[1],
                        1.0,
                    );
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

                // Upload dynamic 3D Monsters mesh (local + synced MP bosses/temple minions)
                if !state.world_manager.monsters.is_empty()
                    || !state.world_manager.mp_temple_monsters.is_empty()
                {
                    let monsters_mesh = if state.world_manager.mp_temple_monsters.is_empty() {
                        blockgame_render::build_monsters_mesh_animated(
                            &state.world_manager.monsters,
                            state.total_time,
                        )
                    } else {
                        let mut combined = Vec::with_capacity(
                            state.world_manager.monsters.len()
                                + state.world_manager.mp_temple_monsters.len(),
                        );
                        combined.extend_from_slice(&state.world_manager.monsters);
                        combined.extend_from_slice(&state.world_manager.mp_temple_monsters);
                        blockgame_render::build_monsters_mesh_animated(&combined, state.total_time)
                    };
                    state.monster_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &monsters_mesh.vertices,
                        &monsters_mesh.indices,
                    );
                } else {
                    state.monster_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Arrows + Fishing Bobber & Line mesh
                let bobber_info = match &state.fishing_state {
                    FishingSession::Waiting { bobber_pos, landed, .. } => {
                        let y_off = if *landed { (state.total_time * 3.5).sin() * 0.04 } else { 0.0 };
                        Some((*bobber_pos + Vec3::new(0.0, y_off, 0.0), false))
                    }
                    FishingSession::Biting { bobber_pos, .. } => {
                        let y_off = (state.total_time * 24.0).sin() * 0.12 - 0.08;
                        Some((*bobber_pos + Vec3::new(0.0, y_off, 0.0), true))
                    }
                    FishingSession::Minigame { bobber_pos, .. } => {
                        let y_off = (state.total_time * 14.0).sin() * 0.08;
                        Some((*bobber_pos + Vec3::new(0.0, y_off, 0.0), true))
                    }
                    FishingSession::Idle => None,
                };
                if !state.world_manager.arrows.is_empty()
                    || !state.world_manager.mp_temple_arrows.is_empty()
                    || bobber_info.is_some()
                {
                    let mut combined =
                        blockgame_render::build_arrows_mesh(&state.world_manager.arrows);
                    if !state.world_manager.mp_temple_arrows.is_empty() {
                        let mp_mesh = blockgame_render::build_arrows_mesh(
                            &state.world_manager.mp_temple_arrows,
                        );
                        let base = combined.vertices.len() as u32;
                        combined.vertices.extend(mp_mesh.vertices);
                        combined
                            .indices
                            .extend(mp_mesh.indices.into_iter().map(|i| i + base));
                    }
                    if let Some((bpos, is_biting)) = bobber_info {
                        let right = state.camera.forward().cross(Vec3::Y).normalize_or_zero();
                        let rod_tip = state.camera.position
                            + state.camera.forward() * 0.85
                            + right * 0.35
                            - Vec3::new(0.0, 0.12, 0.0);
                        let bobber_mesh =
                            blockgame_render::build_fishing_bobber_mesh(rod_tip, bpos, is_biting);
                        let base = combined.vertices.len() as u32;
                        combined.vertices.extend(bobber_mesh.vertices);
                        combined
                            .indices
                            .extend(bobber_mesh.indices.into_iter().map(|i| i + base));
                    }
                    state.arrow_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &combined.vertices,
                        &combined.indices,
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

                // Upload animated spinning 3D Wind Turbine rotors
                let turbines: Vec<(IVec3, f32, u8)> = {
                    let world_guard = state.world_manager.world().lock().unwrap();
                    world_guard
                        .power_nodes
                        .iter()
                        .filter_map(|&pos| {
                            if world_guard.get_block(pos) == BlockId::WIND_TURBINE {
                                let out = blockgame_core::power::wind_turbine_output_eu_s(
                                    &world_guard,
                                    pos,
                                );
                                let facing = world_guard.get_block_facing(pos);
                                Some((pos, out, facing))
                            } else {
                                None
                            }
                        })
                        .collect()
                };
                if !turbines.is_empty() {
                    let rotor_mesh = blockgame_render::build_wind_turbine_rotors_mesh(
                        &turbines,
                        state.total_time,
                    );
                    state.wind_turbine_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &rotor_mesh.vertices,
                        &rotor_mesh.indices,
                    );
                } else {
                    state.wind_turbine_dynamic_mesh.clear();
                }

                // Upload dynamic 3D Articulated Players mesh (remote multiplayer players + local player in 3rd-person or menu)
                let mut players_to_render = Vec::new();
                if let Some(client) = &state.mp_client {
                    for rp in client.remote_players.values() {
                        players_to_render.push(rp.clone());
                    }
                }
                let render_local_model = state.third_person
                    || matches!(
                        state.screen,
                        AppScreen::Title
                            | AppScreen::Guide { .. }
                            | AppScreen::WorldSelect { .. }
                            | AppScreen::CreateWorld { .. }
                            | AppScreen::MultiplayerLobby { .. }
                            | AppScreen::Settings { return_to_pause: false }
                            | AppScreen::Achievements { return_to_pause: false }
                    );
                if render_local_model {
                    players_to_render.push(state.current_local_player_model());
                }
                players_to_render.extend(state.world_manager.collect_village_npc_render_models());
                if !players_to_render.is_empty() {
                    let players_mesh = blockgame_render::build_character_rig_mesh(
                        &players_to_render,
                        state.total_time,
                    );
                    state.player_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &players_mesh.vertices,
                        &players_mesh.indices,
                    );
                    let held_items_mesh = blockgame_render::build_player_held_items_mesh(
                        &players_to_render,
                        &state.item_registry,
                        state.total_time,
                    );
                    state.player_held_items_dynamic_mesh.update(
                        &state.device,
                        &state.queue,
                        &held_items_mesh.vertices,
                        &held_items_mesh.indices,
                    );
                } else {
                    state.player_dynamic_mesh.clear();
                    state.player_held_items_dynamic_mesh.clear();
                }

                // Upload live 3D Character Creator turntable preview mesh when on CharacterCreator screen
                let mut creator_preview_viewport_phys: Option<[f32; 4]> = None;
                if let AppScreen::CharacterCreator {
                    draft,
                    preview_yaw,
                    face_zoom,
                    walk_preview,
                    ..
                } = state.screen
                {
                    let (ui_w, ui_h, _) = state.ui_metrics();
                    if let Some(creator_ui) = state.build_creator_ui_state() {
                        let layout =
                            UiBuilder::character_creator_layout(ui_w, ui_h, &creator_ui);
                        let pv = layout.preview_viewport;
                        let sx = state.surface_config.width as f32 / ui_w.max(1.0);
                        let sy = state.surface_config.height as f32 / ui_h.max(1.0);
                        let px = (pv[0] * sx).clamp(0.0, state.surface_config.width as f32 - 2.0);
                        let py = (pv[1] * sy).clamp(0.0, state.surface_config.height as f32 - 2.0);
                        let pw = (pv[2] * sx)
                            .clamp(2.0, (state.surface_config.width as f32 - px).max(2.0));
                        let ph = (pv[3] * sy)
                            .clamp(2.0, (state.surface_config.height as f32 - py).max(2.0));
                        creator_preview_viewport_phys = Some([px, py, pw, ph]);

                        let walk_anim = if walk_preview {
                            state.total_time * 5.2
                        } else {
                            0.0
                        };
                        let preview_mesh = blockgame_render::build_single_character_preview_mesh(
                            &draft,
                            preview_yaw,
                            walk_anim,
                            state.total_time,
                        );
                        state.preview_character_dynamic_mesh.update(
                            &state.device,
                            &state.queue,
                            &preview_mesh.vertices,
                            &preview_mesh.indices,
                        );
                        let cam_uni =
                            blockgame_render::preview_camera_uniform(face_zoom, pw / ph.max(1.0));
                        state
                            .character_pipeline
                            .update_preview_camera(&state.queue, &cam_uni);
                    }
                } else {
                    state.preview_character_dynamic_mesh.clear();
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

                    // Render animated spinning 3D Wind Turbine rotor blades
                    state.wind_turbine_dynamic_mesh.draw(&mut render_pass);

                    // Render 3D held items anchored to player hands (textured voxel atlas pass)
                    state.player_held_items_dynamic_mesh.draw(&mut render_pass);

                    // Render 3D stylized low-poly player characters (dedicated vertex-colored pass)
                    if state.player_dynamic_mesh.index_count > 0 {
                        render_pass.set_pipeline(&state.character_pipeline.render_pipeline);
                        render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                        state.player_dynamic_mesh.draw(&mut render_pass);
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

                    // Render translucent water meshes after all opaque geometry
                    render_pass.set_pipeline(&state.pipeline.water_pipeline);
                    render_pass.set_bind_group(0, &state.pipeline.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &state.pipeline.texture_bind_group, &[]);
                    state
                        .world_manager
                        .render_water_with_frustum(
                            &mut render_pass,
                            Some(&frustum),
                            Some(state.camera.position),
                        );
                }

                // Live 3D Character Creator Turntable Preview Pass (sub-viewport on left side)
                if let Some(pv) = creator_preview_viewport_phys {
                    if state.preview_character_dynamic_mesh.index_count > 0 {
                        let mut preview_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Character Creator 3D Preview Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
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
                        preview_pass.set_viewport(pv[0], pv[1], pv[2], pv[3], 0.0, 1.0);
                        preview_pass.set_pipeline(&state.character_pipeline.render_pipeline);
                        preview_pass.set_bind_group(
                            0,
                            &state.character_pipeline.preview_camera_bind_group,
                            &[],
                        );
                        state.preview_character_dynamic_mesh.draw(&mut preview_pass);
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

    /// Polls the `GamepadBackend` through `InputRouter` once per frame and dispatches
    /// gameplay actions, hybrid Virtual Cursor UI navigation, D-pad snapping, and disconnect handling.
    fn poll_controller_frame(&mut self, event_loop: &ActiveEventLoop) {
        let (dt, in_gameplay, is_mp, listening_rebind, screen_size, ui_scale, cfg) = {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            let dt = state.last_frame_time.elapsed().as_secs_f32().clamp(0.001, 0.1);
            // Unified all-source damage rumble (environmental, monsters, bosses, arrows, guards, PvP)
            let cur_hp = state.player_stats.health;
            if state.screen == AppScreen::InGame && cur_hp < state.last_rumble_health {
                let dmg = state.last_rumble_health - cur_hp;
                let effect = if dmg >= 6.0 {
                    crate::controller::RumbleEffect::Explosion
                } else {
                    crate::controller::RumbleEffect::Damage
                };
                state.trigger_controller_rumble(effect);
            }
            state.last_rumble_health = cur_hp;
            // Decay non-blocking HUD toast timer
            if let Some((_, timer)) = &mut state.hud_toast {
                *timer -= dt;
                if *timer <= 0.0 {
                    state.hud_toast = None;
                }
            }
            let in_gameplay = state.screen == AppScreen::InGame
                && state.mouse_captured
                && !state.inventory_open
                && !state.skills_open
                && !state.achievements_open
                && !state.chat_open
                && !state.quick_menu_open
                && !state.player_stats.is_dead;
            let is_mp = state.mp_client.is_some();
            let listening_rebind = state.listening_controller_bind.is_some();
            let (sw, sh, _) = state.ui_metrics();
            let ui_scale = state.settings.ui_scale;
            let cfg = state.settings.controller_config();
            (dt, in_gameplay, is_mp, listening_rebind, [sw, sh], ui_scale, cfg)
        };

        let out = {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            state.input_router.update(
                &mut state.gamepad_backend,
                dt,
                &cfg,
                in_gameplay,
                is_mp,
                listening_rebind,
                screen_size,
                ui_scale,
            )
        };

        // Handle controller button remapping capture in Settings Tab 2
        if let Some(captured_btn) = out.captured_rebind_button {
            if let Some(state) = self.state.as_mut() {
                if let Some(idx) = state.listening_controller_bind.take() {
                    state.settings.controller_binds.set_by_index(idx, captured_btn);
                    state.settings.save(std::path::Path::new("settings.cfg"));
                }
            }
            return;
        }

        // Handle Active Controller disconnect mid-gameplay
        match out.disconnect_action {
            crate::controller::DisconnectAction::PauseSingleplayer => {
                if let Some(state) = self.state.as_mut() {
                    state.mouse_captured = false;
                    let _ = state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                    state.window.set_cursor_visible(true);
                    state.screen = AppScreen::Paused {
                        status_message: Some("Controller disconnected".to_string()),
                    };
                }
                return;
            }
            crate::controller::DisconnectAction::ShowMultiplayerToast => {
                if let Some(state) = self.state.as_mut() {
                    state.hud_toast = Some(("Controller disconnected".to_string(), 4.5));
                }
            }
            crate::controller::DisconnectAction::None => {}
        }

        // Update continuous controller state & camera look in gameplay
        if in_gameplay {
            let mut kb_inventory = KeyCode::KeyE;
            let mut kb_third_person = KeyCode::F5;
            let mut kb_flight = KeyCode::KeyF;
            let mut kb_chat = KeyCode::KeyT;

            if let Some(state) = self.state.as_mut() {
                state.controller_move_axes = out.move_axes;
                state.controller_jumping = out.jump_held;
                state.controller_sneaking = out.sneak_held;
                state.controller_sprinting = out.sprint_held;
                state.controller_mine_held = out.mine_held;
                state.controller_use_held = out.use_held;
                state.controller_ptt_held = out.ptt_held;
                state.controller_player_list_held = out.player_list_held;

                if out.look_delta[0].abs() > 0.0 || out.look_delta[1].abs() > 0.0 {
                    state.camera.yaw += out.look_delta[0];
                    state.camera.pitch += out.look_delta[1];
                    let max_pitch = 89.0_f32.to_radians();
                    state.camera.pitch = state.camera.pitch.clamp(-max_pitch, max_pitch);
                }

                if out.hotbar_next_pressed {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 1) % 9;
                    state.hotbar_activity_timer = 4.0;
                }
                if out.hotbar_prev_pressed {
                    state.active_hotbar_slot = (state.active_hotbar_slot + 8) % 9;
                    state.hotbar_activity_timer = 4.0;
                }
                if out.jump_double_tapped && state.player_stats.game_mode.is_creative() {
                    state.player.flying = !state.player.flying;
                    state.player.velocity.y = 0.0;
                }
                if out.quick_menu_pressed {
                    state.quick_menu_open = true;
                    state.mouse_captured = false;
                    let _ = state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                    state.window.set_cursor_visible(false);
                    state
                        .input_router
                        .set_virtual_cursor([screen_size[0] * 0.5, screen_size[1] * 0.5]);
                }

                kb_inventory = state.settings.keybinds.inventory;
                kb_third_person = state.settings.keybinds.third_person;
                kb_flight = state.settings.keybinds.flight;
                kb_chat = state.settings.keybinds.chat;
            }

            if out.mine_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticMouse {
                        button: MouseButton::Left,
                        state: ElementState::Pressed,
                    },
                );
            }
            if out.mine_released {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticMouse {
                        button: MouseButton::Left,
                        state: ElementState::Released,
                    },
                );
            }
            if out.use_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticMouse {
                        button: MouseButton::Right,
                        state: ElementState::Pressed,
                    },
                );
            }
            if out.use_released {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticMouse {
                        button: MouseButton::Right,
                        state: ElementState::Released,
                    },
                );
            }
            if out.inventory_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: kb_inventory,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            if out.backpack_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: KeyCode::KeyB,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            if out.third_person_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: kb_third_person,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            if out.flight_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: kb_flight,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            if out.chat_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: kb_chat,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            if out.pause_pressed {
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticKey {
                        key: KeyCode::Escape,
                        state: ElementState::Pressed,
                        text: None,
                    },
                );
            }
            return;
        }

        // Release any latched mouse buttons if we exited gameplay while holding RT/LT
        if out.mine_released {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticMouse {
                    button: MouseButton::Left,
                    state: ElementState::Released,
                },
            );
        }
        if out.use_released {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticMouse {
                    button: MouseButton::Right,
                    state: ElementState::Released,
                },
            );
        }

        // Clear held gameplay states while in menus/UI
        let in_controller_mode = {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            state.controller_move_axes = [0.0, 0.0];
            state.controller_jumping = false;
            state.controller_sneaking = false;
            state.controller_sprinting = false;
            state.controller_mine_held = false;
            state.controller_use_held = false;
            state.controller_ptt_held = false;
            state.controller_player_list_held = false;

            let is_ctrl =
                state.input_router.input_mode() == blockgame_render::InputMode::Controller;
            if is_ctrl {
                state.window.set_cursor_visible(false);
                // D-pad directional focus snapping
                if let Some(dir) = out.ui_dpad_snap {
                    let rects = Self::collect_dpad_snap_rects(state, screen_size[0], screen_size[1]);
                    if let Some(target) = crate::controller::find_dpad_snap_target(
                        state.input_router.virtual_cursor(),
                        dir,
                        &rects,
                    ) {
                        state.input_router.set_virtual_cursor(target);
                    }
                }
                let vc = state.input_router.virtual_cursor();
                state.cursor_pos = [vc[0] * ui_scale, vc[1] * ui_scale];

                // Active slider dragging via Virtual Cursor
                if out.ui_confirm_held {
                    if let Some(slider_idx) = state.active_settings_slider {
                        state.update_settings_slider(slider_idx, vc[0], screen_size[0], screen_size[1]);
                    } else if let Some(ch) = state.creator_active_rgb_slider {
                        state.apply_creator_rgb_slider(ch, vc[0], screen_size[0], screen_size[1]);
                    }
                }

                // Right stick: rotate 3D Character Creator preview + rate-limited list scrolling
                if let AppScreen::CharacterCreator { preview_yaw, .. } = &mut state.screen {
                    if out.ui_right_stick[0].abs() > 0.05 {
                        *preview_yaw += out.ui_right_stick[0] * 2.6 * dt;
                    }
                }
                if out.ui_right_stick[1].abs() <= 0.35 {
                    state.ui_scroll_cooldown = 0.0;
                } else {
                    state.ui_scroll_cooldown = (state.ui_scroll_cooldown - dt).max(0.0);
                    if state.ui_scroll_cooldown <= 0.0 {
                        state.ui_scroll_cooldown = 0.12;
                        let up = out.ui_right_stick[1] > 0.0;
                        if matches!(state.screen, AppScreen::Achievements { .. })
                            || (state.screen == AppScreen::InGame && state.achievements_open)
                        {
                            if up {
                                state.achievements_scroll =
                                    state.achievements_scroll.saturating_sub(1);
                            } else {
                                state.achievements_scroll = (state.achievements_scroll + 1).min(16);
                            }
                        } else if matches!(state.screen, AppScreen::MultiplayerLobby { .. }) {
                            let len = state.directory_servers.len();
                            if len > 0 {
                                let cur = state.selected_directory_idx.unwrap_or(0);
                                let next = if up {
                                    cur.saturating_sub(1)
                                } else {
                                    (cur + 1).min(len - 1)
                                };
                                state.selected_directory_idx = Some(next);
                            }
                        } else if state.screen == AppScreen::InGame && state.chat_open {
                            if up {
                                state.chat_scroll = (state.chat_scroll + 1).min(100);
                            } else {
                                state.chat_scroll = state.chat_scroll.saturating_sub(1);
                            }
                        } else if state.screen == AppScreen::InGame && state.inventory_open {
                            if up {
                                state.jei_page = state.jei_page.saturating_sub(1);
                            } else {
                                state.jei_page = state.jei_page.saturating_add(1);
                            }
                        }
                    }
                }

                // LB / RB: switch tabs and pages across screens
                if out.ui_lb_pressed || out.ui_rb_pressed {
                    let forward = out.ui_rb_pressed;
                    match &mut state.screen {
                        AppScreen::Settings { .. } => {
                            state.settings_tab = if forward {
                                (state.settings_tab + 1) % 3
                            } else {
                                (state.settings_tab + 2) % 3
                            };
                            state.listening_keybind = None;
                            state.listening_controller_bind = None;
                        }
                        AppScreen::Guide { page, .. } => {
                            *page = if forward { (*page + 1) % 3 } else { (*page + 2) % 3 };
                        }
                        AppScreen::Achievements { .. } => {
                            let count = blockgame_core::achievement::AchievementCategory::ALL.len();
                            state.achievements_category = if forward {
                                (state.achievements_category + 1) % count
                            } else {
                                (state.achievements_category + count - 1) % count
                            };
                            state.achievements_scroll = 0;
                        }
                        AppScreen::CharacterCreator { tab, .. } => {
                            use blockgame_render::CharacterCreatorTab;
                            let tabs = [
                                CharacterCreatorTab::Presets,
                                CharacterCreatorTab::Body,
                                CharacterCreatorTab::FaceHair,
                                CharacterCreatorTab::Clothing,
                            ];
                            let idx = tabs.iter().position(|t| t == tab).unwrap_or(0);
                            *tab = tabs[if forward {
                                (idx + 1) % tabs.len()
                            } else {
                                (idx + tabs.len() - 1) % tabs.len()
                            }];
                        }
                        AppScreen::InGame => {
                            if state.achievements_open {
                                let count =
                                    blockgame_core::achievement::AchievementCategory::ALL.len();
                                state.achievements_category = if forward {
                                    (state.achievements_category + 1) % count
                                } else {
                                    (state.achievements_category + count - 1) % count
                                };
                                state.achievements_scroll = 0;
                            } else if state.active_chest_pos.is_some() {
                                state.chest_page = if forward {
                                    state.chest_page + 1
                                } else {
                                    state.chest_page.saturating_sub(1)
                                };
                            } else if state.active_backpack_id.is_some() {
                                state.backpack_page = if forward {
                                    state.backpack_page + 1
                                } else {
                                    state.backpack_page.saturating_sub(1)
                                };
                            } else if state.active_storage_terminal_pos.is_some() {
                                state.storage_page = if forward {
                                    state.storage_page + 1
                                } else {
                                    state.storage_page.saturating_sub(1)
                                };
                            } else if state.inventory_open {
                                state.jei_page = if forward {
                                    state.jei_page + 1
                                } else {
                                    state.jei_page.saturating_sub(1)
                                };
                            }
                        }
                        _ => {}
                    }
                }
            }
            is_ctrl
        };

        if !in_controller_mode {
            return;
        }

        // Dispatch UI face/trigger button shortcuts via existing MouseInput & KeyboardInput handlers
        if out.ui_confirm_pressed {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticMouse {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                },
            );
        }
        if out.ui_confirm_released {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticMouse {
                    button: MouseButton::Left,
                    state: ElementState::Released,
                },
            );
        }
        if out.ui_west_pressed {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticMouse {
                    button: MouseButton::Right,
                    state: ElementState::Pressed,
                },
            );
        }
        if out.ui_north_pressed {
            if let Some(state) = self.state.as_mut() {
                let prev = state.shift_down;
                state.shift_down = true;
                self.dispatch_event(
                    event_loop,
                    InternalEvent::SyntheticMouse {
                        button: MouseButton::Left,
                        state: ElementState::Pressed,
                    },
                );
                if let Some(s) = self.state.as_mut() {
                    s.shift_down = prev;
                }
            }
        }
        if out.ui_lt_pressed {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticKey {
                    key: KeyCode::KeyR,
                    state: ElementState::Pressed,
                    text: None,
                },
            );
        }
        if out.ui_rt_pressed {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticKey {
                    key: KeyCode::KeyU,
                    state: ElementState::Pressed,
                    text: None,
                },
            );
        }
        if out.ui_cancel_pressed {
            self.dispatch_event(
                event_loop,
                InternalEvent::SyntheticKey {
                    key: KeyCode::Escape,
                    state: ElementState::Pressed,
                    text: None,
                },
            );
        }
    }

    /// Collects interactive widget/slot rectangles for D-pad Virtual Cursor snapping on the active screen.
    fn collect_dpad_snap_rects(state: &AppState, screen_w: f32, screen_h: f32) -> Vec<[f32; 4]> {
        let mut rects = Vec::new();
        match &state.screen {
            AppScreen::Title => {
                rects.extend_from_slice(&UiBuilder::title_screen_button_rects(screen_w, screen_h));
            }
            AppScreen::WorldSelect { .. } => {
                let (rows, btns) = UiBuilder::world_select_layout(
                    screen_w,
                    screen_h,
                    state.saved_worlds.len(),
                );
                rects.extend(rows);
                rects.extend_from_slice(&btns);
            }
            AppScreen::CreateWorld { .. } => {
                let (name_r, seed_r, create_r, cancel_r) =
                    UiBuilder::create_world_layout(screen_w, screen_h);
                rects.push(name_r);
                rects.push(seed_r);
                rects.push(create_r);
                rects.push(cancel_r);
            }
            AppScreen::MultiplayerLobby { .. } => {
                let (user_r, pass_r, addr_r, btns) =
                    UiBuilder::multiplayer_lobby_layout(screen_w, screen_h);
                rects.push(user_r);
                rects.push(pass_r);
                rects.push(addr_r);
                rects.extend_from_slice(&btns);
                let (_panel, refresh_r, rows) =
                    UiBuilder::multiplayer_browser_layout(screen_w, screen_h);
                rects.push(refresh_r);
                rects.extend_from_slice(&rows);
            }
            AppScreen::Guide { .. } => {
                rects.extend_from_slice(&UiBuilder::guide_screen_button_rects(screen_w, screen_h));
            }
            AppScreen::Achievements { .. } => {
                let (cat_rects, _list_rect, close_rect) =
                    UiBuilder::achievements_screen_rects(screen_w, screen_h);
                rects.extend_from_slice(&cat_rects);
                rects.push(close_rect);
            }
            AppScreen::Paused { .. } => {
                rects.extend_from_slice(&UiBuilder::pause_menu_button_rects(screen_w, screen_h));
            }
            AppScreen::Settings { .. } => {
                for t in 0..3 {
                    rects.push(UiBuilder::settings_tab_rect(screen_w, screen_h, t));
                }
                if state.settings_tab == 0 {
                    for i in 0..8 {
                        rects.push(UiBuilder::settings_slider_rect(screen_w, screen_h, i));
                        rects.push(UiBuilder::settings_toggle_rect(screen_w, screen_h, i));
                    }
                } else if state.settings_tab == 1 {
                    for i in 0..crate::settings::KeyBinds::COUNT {
                        rects.push(UiBuilder::settings_keybind_rect(screen_w, screen_h, i));
                    }
                } else {
                    for i in 0..3 {
                        rects.push(UiBuilder::settings_controller_slider_rect(
                            screen_w, screen_h, i,
                        ));
                    }
                    for i in 0..4 {
                        rects.push(UiBuilder::settings_controller_toggle_rect(
                            screen_w, screen_h, i,
                        ));
                    }
                    for i in 0..crate::controller::ControllerBindings::COUNT {
                        rects.push(UiBuilder::settings_controller_bind_rect(
                            screen_w, screen_h, i,
                        ));
                    }
                }
                rects.push(UiBuilder::settings_reset_button_rect(screen_w, screen_h));
                rects.push(UiBuilder::settings_done_button_rect(screen_w, screen_h));
            }
            AppScreen::CharacterCreator { .. } => {
                if let Some(creator_ui) = state.build_creator_ui_state() {
                    let layout =
                        UiBuilder::character_creator_layout(screen_w, screen_h, &creator_ui);
                    rects.extend_from_slice(&layout.tab_rects);
                    rects.push(layout.btn_zoom);
                    rects.push(layout.btn_walk);
                    rects.push(layout.btn_randomize);
                    rects.push(layout.btn_revert);
                    rects.push(layout.btn_done);
                    for (r, _, _) in &layout.option_buttons {
                        rects.push(*r);
                    }
                    for (r, _) in &layout.preset_rows {
                        rects.push(*r);
                    }
                    for (r, _) in &layout.swatch_rects {
                        rects.push(*r);
                    }
                }
            }
            AppScreen::InGame if state.player_stats.is_dead => {
                let btn_w = 240.0;
                let btn_h = 46.0;
                rects.push([(screen_w - btn_w) * 0.5, screen_h * 0.5 + 30.0, btn_w, btn_h]);
            }
            AppScreen::InGame if state.skills_open => {
                let (skill_rects, _detail, close_rect) =
                    UiBuilder::skills_screen_rects(screen_w, screen_h);
                rects.extend_from_slice(&skill_rects);
                rects.push(close_rect);
            }
            AppScreen::InGame if state.achievements_open => {
                let (cat_rects, _list_rect, close_rect) =
                    UiBuilder::achievements_screen_rects(screen_w, screen_h);
                rects.extend_from_slice(&cat_rects);
                rects.push(close_rect);
            }
            AppScreen::InGame if state.quick_menu_open => {
                for i in 0..4 {
                    rects.push(UiBuilder::quick_menu_item_rect(screen_w, screen_h, i));
                }
            }
            AppScreen::InGame if state.inventory_open => {
                // Main 36-slot player inventory + hotbar grid
                let inv_w = 9.0 * UiBuilder::SLOT_SIZE + 8.0 * UiBuilder::SLOT_GAP;
                let inv_x = (screen_w - inv_w) * 0.5;
                let base_y = screen_h * 0.5 + 10.0;
                for row in 0..4 {
                    let y = base_y
                        + (row as f32) * (UiBuilder::SLOT_SIZE + UiBuilder::SLOT_GAP)
                        + if row == 3 { 10.0 } else { 0.0 };
                    for col in 0..9 {
                        let x = inv_x + (col as f32) * (UiBuilder::SLOT_SIZE + UiBuilder::SLOT_GAP);
                        rects.push([x, y, UiBuilder::SLOT_SIZE, UiBuilder::SLOT_SIZE]);
                    }
                }
                // Upper container / crafting / armor / shop slots above player inventory
                let upper_y = base_y - 3.0 * (UiBuilder::SLOT_SIZE + UiBuilder::SLOT_GAP) - 24.0;
                for row in 0..3 {
                    let y = upper_y + (row as f32) * (UiBuilder::SLOT_SIZE + UiBuilder::SLOT_GAP);
                    for col in 0..9 {
                        let x = inv_x + (col as f32) * (UiBuilder::SLOT_SIZE + UiBuilder::SLOT_GAP);
                        rects.push([x, y, UiBuilder::SLOT_SIZE, UiBuilder::SLOT_SIZE]);
                    }
                }
                // JEI sidebar slots on the right edge
                let jei_w = (UiBuilder::JEI_COLS as f32) * UiBuilder::JEI_SLOT_SIZE
                    + ((UiBuilder::JEI_COLS - 1) as f32) * UiBuilder::JEI_SLOT_GAP;
                let jei_x = screen_w - jei_w - 20.0;
                let jei_y = 92.0;
                for row in 0..UiBuilder::JEI_ROWS {
                    for col in 0..UiBuilder::JEI_COLS {
                        let x = jei_x
                            + (col as f32) * (UiBuilder::JEI_SLOT_SIZE + UiBuilder::JEI_SLOT_GAP);
                        let y = jei_y
                            + (row as f32) * (UiBuilder::JEI_SLOT_SIZE + UiBuilder::JEI_SLOT_GAP);
                        rects.push([x, y, UiBuilder::JEI_SLOT_SIZE, UiBuilder::JEI_SLOT_SIZE]);
                    }
                }
            }
            _ => {}
        }
        rects
    }
}

fn handle_inventory_click(state: &mut AppState, is_right_click: bool) {
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

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
        if let Some(preview) = state.crafting_result {
            if state.shift_down {
                loop {
                    let upgrade_bp_id = if blockgame_core::storage::BackpackState::is_backpack_item(preview.item_id) {
                        state
                            .crafting_grid
                            .slots
                            .iter()
                            .find(|s| {
                                !s.is_empty()
                                    && blockgame_core::storage::BackpackState::is_backpack_item(s.item_id)
                                    && s.durability > 0
                            })
                            .map(|s| s.durability)
                    } else {
                        None
                    };
                    let Some(mut crafted) = state.crafting_manager.craft(&mut state.crafting_grid) else {
                        break;
                    };
                    let new_bp_tier =
                        blockgame_core::storage::BackpackState::tier_for_item(crafted.item_id);
                    if new_bp_tier > 0 {
                        if let Some(id) = upgrade_bp_id {
                            crafted.durability = id;
                            state.backpacks.ensure_bag(id, new_bp_tier);
                        } else {
                            state.backpacks.ensure_slot_id(&mut crafted);
                        }
                    }
                    let crafted_id = crafted.item_id.0;
                    if let Some(rem) = state.inventory.add_item(crafted, &state.item_registry) {
                        state.record_achievement_event(
                            blockgame_core::AchievementEvent::ItemCrafted {
                                item_id: crafted_id,
                            },
                        );
                        state.spawn_drop_synced(
                            state.player.position,
                            Vec3::new(0.0, 2.0, 0.0),
                            rem,
                        );
                        break;
                    }
                    state.record_achievement_event(
                        blockgame_core::AchievementEvent::ItemCrafted {
                            item_id: crafted_id,
                        },
                    );
                    if state
                        .crafting_manager
                        .find_match(&state.crafting_grid)
                        .is_none()
                    {
                        break;
                    }
                }
            } else {
                let max_stack = state.item_registry.max_stack_size(preview.item_id);
                let can_accept = state.cursor_held.is_empty()
                    || (max_stack > 1
                        && state.cursor_held.can_stack_with(&preview)
                        && state.cursor_held.count + preview.count <= max_stack);
                if can_accept {
                    let upgrade_bp_id = if blockgame_core::storage::BackpackState::is_backpack_item(preview.item_id) {
                        state
                            .crafting_grid
                            .slots
                            .iter()
                            .find(|s| {
                                !s.is_empty()
                                    && blockgame_core::storage::BackpackState::is_backpack_item(s.item_id)
                                    && s.durability > 0
                            })
                            .map(|s| s.durability)
                    } else {
                        None
                    };
                    if let Some(mut crafted) = state.crafting_manager.craft(&mut state.crafting_grid) {
                        state.record_achievement_event(
                            blockgame_core::AchievementEvent::ItemCrafted {
                                item_id: crafted.item_id.0,
                            },
                        );
                        let new_bp_tier =
                            blockgame_core::storage::BackpackState::tier_for_item(crafted.item_id);
                        if new_bp_tier > 0 {
                            if let Some(id) = upgrade_bp_id {
                                crafted.durability = id;
                                state.backpacks.ensure_bag(id, new_bp_tier);
                            } else {
                                state.backpacks.ensure_slot_id(&mut crafted);
                            }
                        }
                        if state.cursor_held.is_empty() {
                            state.cursor_held = crafted;
                        } else {
                            state.cursor_held.count += crafted.count;
                        }
                    }
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
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let chest_y = screen_h * 0.5 - 220.0;

    // 0. Pagination buttons (< / >) for Iron/Gold/Diamond Chests
    let total_slots = {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        world_guard.get_or_create_chest(chest_pos).len()
    };
    let total_pages = ((total_slots + 26) / 27).max(1);
    if total_pages > 1 {
        let btn_y = chest_y - 24.0;
        let next_x = start_x + inv_w - 26.0;
        let prev_x = next_x - 86.0;
        if cx >= prev_x && cx < prev_x + 24.0 && cy >= btn_y && cy < btn_y + 20.0 {
            if state.chest_page > 0 {
                state.chest_page -= 1;
            }
            return;
        }
        if cx >= next_x && cx < next_x + 24.0 && cy >= btn_y && cy < btn_y + 20.0 {
            if state.chest_page + 1 < total_pages {
                state.chest_page += 1;
            }
            return;
        }
    }
    let page = state.chest_page.min(total_pages - 1);
    let base_idx = page * 27;

    // 1. Chest Container Slots (3 rows x 9 columns per page)
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = base_idx + row * 9 + col;
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

    // 2. Player Inventory Grid (3 rows x 9 columns)
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

fn handle_storage_terminal_click(state: &mut AppState, is_right_click: bool) {
    let Some(term_pos) = state.active_storage_terminal_pos else {
        return;
    };
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let grid_y = screen_h * 0.5 - 205.0;
    let ctrl_y = grid_y - 24.0;

    // 0a. Search box focus
    let search_w = 210.0;
    if cx >= start_x && cx < start_x + search_w && cy >= ctrl_y && cy < ctrl_y + 20.0 {
        state.storage_search_focused = true;
        state.jei_search_focused = false;
        if is_right_click {
            state.storage_search_text.clear();
            state.storage_page = 0;
        }
        return;
    } else {
        state.storage_search_focused = false;
    }

    // 0b. Sort mode button
    let sort_x = start_x + search_w + 8.0;
    let sort_w = 104.0;
    if cx >= sort_x && cx < sort_x + sort_w && cy >= ctrl_y && cy < ctrl_y + 20.0 {
        state.storage_sort_mode = state.storage_sort_mode.next();
        state.storage_page = 0;
        return;
    }

    let (info, items) = {
        let world_guard = state.world_manager.world().lock().unwrap();
        let info = blockgame_core::storage::collect_storage_network(&world_guard, term_pos);
        let items = blockgame_core::storage::list_network_items(
            &world_guard,
            &info.chests,
            &state.storage_search_text,
            state.storage_sort_mode,
            &state.item_registry,
        );
        (info, items)
    };

    // 0c. Pagination buttons
    let total_pages = ((items.len() + 26) / 27).max(1);
    let next_x = start_x + inv_w - 24.0;
    let prev_x = next_x - 82.0;
    if cx >= prev_x && cx < prev_x + 24.0 && cy >= ctrl_y && cy < ctrl_y + 20.0 {
        if state.storage_page > 0 {
            state.storage_page -= 1;
        }
        return;
    }
    if cx >= next_x && cx < next_x + 24.0 && cy >= ctrl_y && cy < ctrl_y + 20.0 {
        if state.storage_page + 1 < total_pages {
            state.storage_page += 1;
        }
        return;
    }

    // 1. Unified 3x9 Storage Grid (requires power)
    let page = state.storage_page.min(total_pages - 1);
    let base_idx = page * 27;
    for row in 0..3 {
        for col in 0..9 {
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = grid_y + (row as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if !info.is_powered {
                    return;
                }
                if !state.cursor_held.is_empty() {
                    if blockgame_core::storage::BackpackState::is_backpack_item(state.cursor_held.item_id) {
                        state.backpacks.ensure_slot_id(&mut state.cursor_held);
                    }
                    let mut world_guard = state.world_manager.world().lock().unwrap();
                    if is_right_click {
                        let single = ItemSlot {
                            item_id: state.cursor_held.item_id,
                            count: 1,
                            durability: state.cursor_held.durability,
                            enhancement: state.cursor_held.enhancement,
                        };
                        if blockgame_core::storage::deposit_into_network(
                            &mut world_guard,
                            &info.chests,
                            single,
                            &state.item_registry,
                        )
                        .is_none()
                        {
                            state.cursor_held.count -= 1;
                            if state.cursor_held.count == 0 {
                                state.cursor_held = ItemSlot::EMPTY;
                            }
                        }
                    } else {
                        let held = std::mem::replace(&mut state.cursor_held, ItemSlot::EMPTY);
                        let rem = blockgame_core::storage::deposit_into_network(
                            &mut world_guard,
                            &info.chests,
                            held,
                            &state.item_registry,
                        );
                        state.cursor_held = rem.unwrap_or(ItemSlot::EMPTY);
                    }
                } else {
                    let idx = base_idx + row * 9 + col;
                    if let Some(entry) = items.get(idx) {
                        let mut world_guard = state.world_manager.world().lock().unwrap();
                        let mut pulled = if let Some((cpos, sidx)) = entry.unique_source {
                            blockgame_core::storage::withdraw_unique_from_network(
                                &mut world_guard,
                                cpos,
                                sidx,
                            )
                        } else {
                            let max_stack = state
                                .item_registry
                                .get(entry.item_id)
                                .map(|d| d.max_stack)
                                .unwrap_or(64);
                            let want = if is_right_click {
                                ((entry.total_count.min(max_stack as u32) + 1) / 2).max(1) as u16
                            } else {
                                max_stack
                            };
                            blockgame_core::storage::withdraw_from_network(
                                &mut world_guard,
                                &info.chests,
                                entry.item_id,
                                want,
                            )
                        };
                        if !pulled.is_empty() {
                            if blockgame_core::storage::BackpackState::is_backpack_item(pulled.item_id) {
                                state.backpacks.ensure_slot_id(&mut pulled);
                            }
                            if state.shift_down {
                                if let Some(rem) =
                                    state.inventory.add_item(pulled, &state.item_registry)
                                {
                                    let _ = blockgame_core::storage::deposit_into_network(
                                        &mut world_guard,
                                        &info.chests,
                                        rem,
                                        &state.item_registry,
                                    );
                                }
                            } else {
                                state.cursor_held = pulled;
                            }
                        }
                    }
                }
                return;
            }
        }
    }

    // 2. Player Inventory Grid (3 rows x 9 columns)
    let inv_y = screen_h * 0.5 - 220.0 + 3.0 * (slot_size + slot_gap) + 26.0;
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_y + (row as f32) * (slot_size + slot_gap);

            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if state.shift_down && info.is_powered {
                    let mut slot_val = state.inventory.slots[slot_idx];
                    if !slot_val.is_empty() {
                        if blockgame_core::storage::BackpackState::is_backpack_item(slot_val.item_id) {
                            state.backpacks.ensure_slot_id(&mut slot_val);
                        }
                        let mut world_guard = state.world_manager.world().lock().unwrap();
                        let rem = blockgame_core::storage::deposit_into_network(
                            &mut world_guard,
                            &info.chests,
                            slot_val,
                            &state.item_registry,
                        );
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
            if state.shift_down && info.is_powered {
                let mut slot_val = state.inventory.slots[i];
                if !slot_val.is_empty() {
                    if blockgame_core::storage::BackpackState::is_backpack_item(slot_val.item_id) {
                        state.backpacks.ensure_slot_id(&mut slot_val);
                    }
                    let mut world_guard = state.world_manager.world().lock().unwrap();
                    let rem = blockgame_core::storage::deposit_into_network(
                        &mut world_guard,
                        &info.chests,
                        slot_val,
                        &state.item_registry,
                    );
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

fn handle_backpack_click(state: &mut AppState, is_right_click: bool) {
    let Some(bp_id) = state.active_backpack_id else {
        return;
    };
    let slot_count = match state.backpacks.get(bp_id) {
        Some(bag) if !bag.slots.is_empty() => bag.slots.len(),
        _ => return,
    };
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let chest_y = screen_h * 0.5 - 220.0;

    let total_pages = ((slot_count + 26) / 27).max(1);
    if total_pages > 1 {
        let btn_y = chest_y - 24.0;
        let next_x = start_x + inv_w - 26.0;
        let prev_x = next_x - 86.0;
        if cx >= prev_x && cx < prev_x + 24.0 && cy >= btn_y && cy < btn_y + 20.0 {
            if state.backpack_page > 0 {
                state.backpack_page -= 1;
            }
            return;
        }
        if cx >= next_x && cx < next_x + 24.0 && cy >= btn_y && cy < btn_y + 20.0 {
            if state.backpack_page + 1 < total_pages {
                state.backpack_page += 1;
            }
            return;
        }
    }

    let page = state.backpack_page.min(total_pages - 1);
    let base_idx = page * 27;

    // 1. Backpack Container Slots (disallows placing backpacks inside backpacks)
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = base_idx + row * 9 + col;
            if slot_idx >= slot_count {
                continue;
            }
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = chest_y + (row as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if !state.cursor_held.is_empty()
                    && blockgame_core::storage::BackpackState::is_backpack_item(state.cursor_held.item_id)
                {
                    return;
                }
                if let Some(bag) = state.backpacks.get_mut(bp_id) {
                    if state.shift_down {
                        let item = bag.slots[slot_idx];
                        if !item.is_empty() {
                            if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                                bag.slots[slot_idx] = rem;
                            } else {
                                bag.slots[slot_idx] = ItemSlot::EMPTY;
                            }
                        }
                    } else {
                        click_slot_raw(
                            &mut bag.slots[slot_idx],
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

    // 2. Player Inventory Grid
    let inv_y = chest_y + 3.0 * (slot_size + slot_gap) + 26.0;
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_y + (row as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                if state.shift_down {
                    let slot_val = state.inventory.slots[slot_idx];
                    if !slot_val.is_empty()
                        && !blockgame_core::storage::BackpackState::is_backpack_item(slot_val.item_id)
                    {
                        if let Some(bag) = state.backpacks.get_mut(bp_id) {
                            let rem = add_to_chest_slots(
                                &mut bag.slots,
                                slot_val,
                                &state.item_registry,
                            );
                            state.inventory.slots[slot_idx] = rem.unwrap_or(ItemSlot::EMPTY);
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

    // 3. Hotbar Slots
    let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
    for i in 0..9 {
        let sx = start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;
        if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
            if state.shift_down {
                let slot_val = state.inventory.slots[i];
                if !slot_val.is_empty()
                    && !blockgame_core::storage::BackpackState::is_backpack_item(slot_val.item_id)
                {
                    if let Some(bag) = state.backpacks.get_mut(bp_id) {
                        let rem = add_to_chest_slots(
                            &mut bag.slots,
                            slot_val,
                            &state.item_registry,
                        );
                        state.inventory.slots[i] = rem.unwrap_or(ItemSlot::EMPTY);
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

fn handle_power_screen_inv_click(state: &mut AppState, is_right_click: bool) {
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();
    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let top_y = screen_h * 0.5 - 220.0;
    let inv_y = top_y + 10.0 + slot_size + 8.0 + 24.0 + 8.0 + slot_size + 24.0;
    let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;

    // Backpack (9..36)
    for row in 0..3 {
        for col in 0..9 {
            let slot_idx = 9 + row * 9 + col;
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            let sy = inv_y + (row as f32) * (slot_size + slot_gap);
            if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
                click_slot_raw(
                    &mut state.inventory.slots[slot_idx],
                    &mut state.cursor_held,
                    is_right_click,
                    &state.item_registry,
                );
                return;
            }
        }
    }
    // Hotbar (0..9)
    for i in 0..9 {
        let sx = start_x + (i as f32) * (slot_size + slot_gap);
        let sy = hotbar_y;
        if cx >= sx && cx < sx + slot_size && cy >= sy && cy < sy + slot_size {
            click_slot_raw(
                &mut state.inventory.slots[i],
                &mut state.cursor_held,
                is_right_click,
                &state.item_registry,
            );
            return;
        }
    }
}

fn handle_coal_generator_click(state: &mut AppState, is_right_click: bool) {
    let Some(gen_pos) = state.active_coal_gen_pos else {
        return;
    };
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();
    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let top_y = screen_h * 0.5 - 220.0;
    let fuel_x = start_x + 40.0;
    let fuel_y = top_y + 42.0;

    if cx >= fuel_x && cx < fuel_x + slot_size && cy >= fuel_y && cy < fuel_y + slot_size {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        let gen = world_guard.get_or_create_coal_generator(gen_pos);
        if state.shift_down {
            let item = gen.fuel;
            if !item.is_empty() {
                if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                    gen.fuel = rem;
                } else {
                    gen.fuel = ItemSlot::EMPTY;
                }
            }
        } else {
            click_slot_raw(
                &mut gen.fuel,
                &mut state.cursor_held,
                is_right_click,
                &state.item_registry,
            );
        }
        return;
    }

    handle_power_screen_inv_click(state, is_right_click);
}

fn handle_battery_click(state: &mut AppState, is_right_click: bool) {
    handle_power_screen_inv_click(state, is_right_click);
}

fn handle_electric_furnace_click(state: &mut AppState, is_right_click: bool) {
    let Some(ef_pos) = state.active_electric_furnace_pos else {
        return;
    };
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();
    let slot_size = UiBuilder::SLOT_SIZE;
    let slot_gap = UiBuilder::SLOT_GAP;
    let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
    let start_x = (screen_w - inv_w) * 0.5;
    let top_y = screen_h * 0.5 - 220.0;
    let input_x = start_x + (inv_w - slot_size) * 0.5 - 80.0;
    let slot_y = top_y + 34.0;
    let output_x = input_x + slot_size + 112.0;

    // Input slot
    if cx >= input_x && cx < input_x + slot_size && cy >= slot_y && cy < slot_y + slot_size {
        let mut world_guard = state.world_manager.world().lock().unwrap();
        let ef = world_guard.get_or_create_electric_furnace(ef_pos);
        if state.shift_down {
            let item = ef.input;
            if !item.is_empty() {
                if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                    ef.input = rem;
                } else {
                    ef.input = ItemSlot::EMPTY;
                }
            }
        } else {
            click_slot_raw(
                &mut ef.input,
                &mut state.cursor_held,
                is_right_click,
                &state.item_registry,
            );
        }
        return;
    }

    // Output slot (take only)
    if cx >= output_x && cx < output_x + slot_size && cy >= slot_y && cy < slot_y + slot_size {
        let taken_id = {
            let mut world_guard = state.world_manager.world().lock().unwrap();
            let ef = world_guard.get_or_create_electric_furnace(ef_pos);
            let out = ef.output;
            let mut took = None;
            if !out.is_empty() {
                if state.shift_down {
                    if let Some(rem) = state.inventory.add_item(out, &state.item_registry) {
                        if rem.count < out.count {
                            took = Some(out.item_id.0);
                        }
                        ef.output = rem;
                    } else {
                        took = Some(out.item_id.0);
                        ef.output = ItemSlot::EMPTY;
                    }
                } else if state.cursor_held.is_empty() {
                    took = Some(out.item_id.0);
                    state.cursor_held = out;
                    ef.output = ItemSlot::EMPTY;
                } else if state.cursor_held.item_id == out.item_id {
                    let max_s = state
                        .item_registry
                        .get(out.item_id)
                        .map(|d| d.max_stack)
                        .unwrap_or(64);
                    let space = max_s.saturating_sub(state.cursor_held.count);
                    let take = out.count.min(space);
                    if take > 0 {
                        took = Some(out.item_id.0);
                    }
                    state.cursor_held.count += take;
                    ef.output.count -= take;
                    if ef.output.count == 0 {
                        ef.output = ItemSlot::EMPTY;
                    }
                }
            }
            took
        };
        if let Some(item_id) = taken_id {
            state.record_achievement_event(blockgame_core::AchievementEvent::ItemSmelted {
                item_id,
                electric: true,
            });
        }
        return;
    }

    handle_power_screen_inv_click(state, is_right_click);
}

fn handle_furnace_click(state: &mut AppState, is_right_click: bool) {
    let Some(furnace_pos) = state.active_furnace_pos else {
        return;
    };
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

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
        let taken_id = {
            let mut world_guard = state.world_manager.world().lock().unwrap();
            let furnace = world_guard.get_or_create_furnace(furnace_pos);
            let mut took = None;
            if state.shift_down {
                let item = furnace.output;
                if !item.is_empty() {
                    if let Some(rem) = state.inventory.add_item(item, &state.item_registry) {
                        if rem.count < item.count {
                            took = Some(item.item_id.0);
                        }
                        furnace.output = rem;
                    } else {
                        took = Some(item.item_id.0);
                        furnace.output = ItemSlot::EMPTY;
                    }
                }
            } else if !furnace.output.is_empty() {
                // Can only TAKE from output slot, cannot place items into it
                if state.cursor_held.is_empty() {
                    took = Some(furnace.output.item_id.0);
                    state.cursor_held = furnace.output;
                    furnace.output = ItemSlot::EMPTY;
                } else if state.cursor_held.can_stack_with(&furnace.output) {
                    let max_stack = state.item_registry.max_stack_size(furnace.output.item_id);
                    let space = max_stack - state.cursor_held.count;
                    let take = space.min(furnace.output.count);
                    if take > 0 {
                        took = Some(furnace.output.item_id.0);
                    }
                    state.cursor_held.count += take;
                    furnace.output.count -= take;
                    if furnace.output.count == 0 {
                        furnace.output = ItemSlot::EMPTY;
                    }
                }
            }
            took
        };
        if let Some(item_id) = taken_id {
            state.record_achievement_event(blockgame_core::AchievementEvent::ItemSmelted {
                item_id,
                electric: false,
            });
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

    // 1. Try to merge into existing stacks (only for stackable non-unique items)
    if max_stack > 1 && item.durability == 0 {
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
    }

    // 2. Put remainder into first empty slot (preserving durability / unique backpack ID)
    for slot in chest_slots.iter_mut() {
        if slot.is_empty() {
            let transfer = max_stack.min(item.count);
            *slot = ItemSlot {
                item_id: item.item_id,
                count: transfer,
                durability: item.durability,
                enhancement: item.enhancement,
            };
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
            if max_stack <= 1 {
                if !split_half {
                    std::mem::swap(slot, held_cursor);
                }
            } else if slot.count < max_stack {
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
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

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
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

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
                    if state.player_stats.game_mode.is_creative() && !is_right_click {
                        let max_stack = state
                            .item_registry
                            .get(clicked_id)
                            .map(|d| d.max_stack)
                            .unwrap_or(64);
                        if state.cursor_held.is_empty() || state.cursor_held.item_id != clicked_id {
                            state.cursor_held = ItemSlot::new(clicked_id, max_stack);
                            if blockgame_core::storage::BackpackState::is_backpack_item(clicked_id) {
                                state.backpacks.ensure_slot_id(&mut state.cursor_held);
                            }
                        } else {
                            state.cursor_held = ItemSlot::EMPTY;
                        }
                    } else {
                        state.jei_recipe_modal = Some(JeiRecipeModalState {
                            target_item: clicked_id,
                            is_usage_mode: is_right_click,
                            recipe_index: 0,
                        });
                    }
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
    let (screen_w, screen_h, [cx, cy]) = state.ui_metrics();

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
    } else if let Some(term_pos) = state.active_storage_terminal_pos {
        let grid_y = screen_h * 0.5 - 205.0;
        let (info, items) = {
            let world_guard = state.world_manager.world().lock().unwrap();
            let info = blockgame_core::storage::collect_storage_network(&world_guard, term_pos);
            let items = blockgame_core::storage::list_network_items(
                &world_guard,
                &info.chests,
                &state.storage_search_text,
                state.storage_sort_mode,
                &state.item_registry,
            );
            (info, items)
        };
        if info.is_powered {
            let base_idx = state.storage_page * 27;
            for row in 0..3 {
                for col in 0..9 {
                    let sx = start_x + (col as f32) * (slot_size + slot_gap);
                    let sy = grid_y + (row as f32) * (slot_size + slot_gap);
                    if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                        if let Some(entry) = items.get(base_idx + row * 9 + col) {
                            return Some(entry.item_id);
                        }
                    }
                }
            }
        }
    } else if let Some(bp_id) = state.active_backpack_id {
        let chest_y = screen_h * 0.5 - 220.0;
        let base_idx = state.backpack_page * 27;
        if let Some(bag) = state.backpacks.get(bp_id) {
            for row in 0..3 {
                for col in 0..9 {
                    let slot_idx = base_idx + row * 9 + col;
                    let sx = start_x + (col as f32) * (slot_size + slot_gap);
                    let sy = chest_y + (row as f32) * (slot_size + slot_gap);
                    if cx >= sx && cx <= sx + slot_size && cy >= sy && cy <= sy + slot_size {
                        if let Some(slot) = bag.slots.get(slot_idx) {
                            if !slot.is_empty() {
                                return Some(slot.item_id);
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(chest_pos) = state.active_chest_pos {
        let chest_y = screen_h * 0.5 - 220.0;
        let base_idx = state.chest_page * 27;

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = base_idx + row * 9 + col;
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
    let inv_start_y = if state.active_furnace_pos.is_some()
        || state.active_coal_gen_pos.is_some()
        || state.active_battery_pos.is_some()
        || state.active_electric_furnace_pos.is_some()
    {
        let furnace_top_y = screen_h * 0.5 - 220.0;
        let input_y = furnace_top_y + 10.0;
        let flame_size = 24.0;
        let flame_y = input_y + slot_size + 8.0;
        let fuel_y = flame_y + flame_size + 8.0;
        fuel_y + slot_size + 24.0
    } else if state.active_chest_pos.is_some()
        || state.active_storage_terminal_pos.is_some()
        || state.active_backpack_id.is_some()
    {
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

fn handle_cooking_pot_click(state: &mut AppState, is_right_click: bool) {
    let Some(pot_pos) = state.active_cooking_pot_pos else {
        return;
    };
    let (screen_w, screen_h, [mx, my]) = state.ui_metrics();

    let Some((kind, idx)) = UiBuilder::hit_test_cooking_pot_screen(screen_w, screen_h, mx, my)
    else {
        return;
    };

    let mut cooked_meal_taken: Option<u16> = None;
    let mut swap_slot = |slot: &mut ItemSlot, output_only: bool| {
        if output_only {
            if !slot.is_empty() {
                if state.cursor_held.is_empty() {
                    cooked_meal_taken = Some(slot.item_id.0);
                    state.cursor_held = *slot;
                    *slot = ItemSlot::EMPTY;
                } else if state.cursor_held.item_id == slot.item_id {
                    let max_s = state
                        .item_registry
                        .get(slot.item_id)
                        .map(|d| d.max_stack)
                        .unwrap_or(16);
                    let space = max_s.saturating_sub(state.cursor_held.count);
                    let moved = slot.count.min(space);
                    if moved > 0 {
                        cooked_meal_taken = Some(slot.item_id.0);
                    }
                    state.cursor_held.count += moved;
                    slot.count -= moved;
                    if slot.count == 0 {
                        *slot = ItemSlot::EMPTY;
                    }
                }
            }
            return;
        }

        if is_right_click {
            if state.cursor_held.is_empty() {
                if !slot.is_empty() {
                    let take = (slot.count + 1) / 2;
                    state.cursor_held = ItemSlot {
                        item_id: slot.item_id,
                        count: take,
                        durability: slot.durability,
                        enhancement: slot.enhancement,
                    };
                    slot.count -= take;
                    if slot.count == 0 {
                        *slot = ItemSlot::EMPTY;
                    }
                }
            } else if slot.is_empty() {
                *slot = ItemSlot {
                    item_id: state.cursor_held.item_id,
                    count: 1,
                    durability: state.cursor_held.durability,
                    enhancement: state.cursor_held.enhancement,
                };
                state.cursor_held.count -= 1;
                if state.cursor_held.count == 0 {
                    state.cursor_held = ItemSlot::EMPTY;
                }
            } else if slot.item_id == state.cursor_held.item_id {
                let max_s = state
                    .item_registry
                    .get(slot.item_id)
                    .map(|d| d.max_stack)
                    .unwrap_or(64);
                if slot.count < max_s {
                    slot.count += 1;
                    state.cursor_held.count -= 1;
                    if state.cursor_held.count == 0 {
                        state.cursor_held = ItemSlot::EMPTY;
                    }
                }
            }
        } else if !state.cursor_held.is_empty() && slot.item_id == state.cursor_held.item_id {
            let max_s = state
                .item_registry
                .get(slot.item_id)
                .map(|d| d.max_stack)
                .unwrap_or(64);
            let space = max_s.saturating_sub(slot.count);
            let moved = state.cursor_held.count.min(space);
            slot.count += moved;
            state.cursor_held.count -= moved;
            if state.cursor_held.count == 0 {
                state.cursor_held = ItemSlot::EMPTY;
            }
        } else {
            std::mem::swap(slot, &mut state.cursor_held);
        }
    };

    match kind {
        0 => {
            let mut world_guard = state.world_manager.world().lock().unwrap();
            let pot = world_guard.get_or_create_cooking_pot(pot_pos);
            if idx < 4 {
                swap_slot(&mut pot.ingredients[idx], false);
            }
        }
        1 => {
            let mut world_guard = state.world_manager.world().lock().unwrap();
            let pot = world_guard.get_or_create_cooking_pot(pot_pos);
            swap_slot(&mut pot.fuel, false);
        }
        2 => {
            let mut world_guard = state.world_manager.world().lock().unwrap();
            let pot = world_guard.get_or_create_cooking_pot(pot_pos);
            swap_slot(&mut pot.output, true);
        }
        3 => {
            if idx < state.inventory.slots.len() {
                swap_slot(&mut state.inventory.slots[idx], false);
            }
        }
        _ => {}
    }
    drop(swap_slot);
    if let Some(item_id) = cooked_meal_taken {
        state.record_achievement_event(blockgame_core::AchievementEvent::MealCooked { item_id });
    }
}

