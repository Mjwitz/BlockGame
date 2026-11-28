use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::atlas::TextureAtlas;
use crate::controller_glyphs::{
    glyph_tile_id, ControllerFamily, GamepadButton, GlyphStyle, InputMode,
};
use blockgame_core::{
    crafting::CraftingGrid,
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
    world::FurnaceState,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ScreenUniform {
    pub screen_size: [f32; 2],
    pub padding: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UiRect {
    #[inline]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Action triggered when clicking an interactive element inside the in-game chat log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    Command(String),
    OpenUrl(String),
}

/// Clickable action hitbox inside the in-game chat log.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatActionHitbox {
    pub rect: [f32; 4],
    pub action: ChatAction,
}

/// Hitboxes for the clickable chat URL confirmation modal.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinkConfirmModalHitboxes {
    pub panel_rect: [f32; 4],
    pub open_rect: [f32; 4],
    pub copy_rect: [f32; 4],
    pub cancel_rect: [f32; 4],
}

/// Interactive action triggered by clicking inside the in-game Admin Panel.
#[derive(Debug, Clone, PartialEq)]
pub enum AdminUiClickAction {
    Close,
    Refresh,
    SelectTab(blockgame_core::admin_panel::AdminTab),
    SelectPlayer(String),
    SelectGroup(String),
    SelectPermPlayer(String),
    SetPermPlayersView(bool),
    ScrollNodes(i32),
    SendAction(blockgame_core::admin_panel::AdminPanelAction),
    /// Uses the current text input buffer to execute a parameterized action.
    SubmitInput(String),
}

/// Clickable hitbox inside the in-game Admin Panel.
#[derive(Debug, Clone, PartialEq)]
pub struct AdminUiHitbox {
    pub rect: [f32; 4],
    pub action: AdminUiClickAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatSegment<'a> {
    Text(&'a str),
    Button {
        label: &'a str,
        command: String,
        color: [f32; 4],
        hover_color: [f32; 4],
    },
    Link {
        label: &'a str,
        url: String,
    },
}

pub struct UiMesh {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
}

impl Default for UiMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl UiMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn add_quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4]) {
        let base_idx = self.vertices.len() as u32;

        let u_min = uv[0];
        let v_min = uv[1];
        let u_max = uv[2];
        let v_max = uv[3];

        self.vertices.push(UiVertex {
            position: [x, y],
            uv: [u_min, v_min],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x + w, y],
            uv: [u_max, v_min],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x + w, y + h],
            uv: [u_max, v_max],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x, y + h],
            uv: [u_min, v_max],
            color,
        });

        self.indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    pub fn add_quad_vertices(
        &mut self,
        positions: [[f32; 2]; 4],
        uvs: [[f32; 2]; 4],
        colors: [[f32; 4]; 4],
    ) {
        let base_idx = self.vertices.len() as u32;
        for i in 0..4 {
            self.vertices.push(UiVertex {
                position: positions[i],
                uv: uvs[i],
                color: colors[i],
            });
        }
        self.indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }
}

pub struct UiPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub screen_buffer: wgpu::Buffer,
    pub screen_bind_group: wgpu::BindGroup,
}

impl UiPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });

        let screen_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Uniform Buffer"),
            contents: bytemuck::cast_slice(&[ScreenUniform {
                screen_size: [1280.0, 720.0],
                padding: [0.0, 0.0],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let screen_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Screen Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let screen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Screen Bind Group"),
            layout: &screen_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&screen_bind_group_layout, texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[UiVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // 2D quads do not cull back faces
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // 2D UI draws on top of 3D scene without depth testing
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            screen_buffer,
            screen_bind_group,
        }
    }

    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        queue.write_buffer(
            &self.screen_buffer,
            0,
            bytemuck::cast_slice(&[ScreenUniform {
                screen_size: [width.max(1.0), height.max(1.0)],
                padding: [0.0, 0.0],
            }]),
        );
    }
}

#[derive(Debug, Clone)]
pub struct JeiParams<'a> {
    pub catalog_items: &'a [ItemId],
    pub page: usize,
    pub total_pages: usize,
    pub search_query: &'a str,
    pub search_focused: bool,
}

#[derive(Debug, Clone)]
pub struct RecipeModalParams<'a> {
    pub target_item: ItemId,
    pub recipe: &'a blockgame_core::crafting::DisplayRecipe,
    pub recipe_index: usize,
    pub total_recipes: usize,
    pub is_usage_mode: bool,
}

/// Snapshot of client settings passed to `UiBuilder::build_settings_screen`.
#[derive(Debug, Clone)]
pub struct SettingsUiState {
    pub render_distance: i32,
    pub fov_degrees: f32,
    pub mouse_sensitivity: f32,
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub creature_volume: f32,
    pub invert_y: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    pub graphics_quality_fast: bool,
    pub view_bobbing: bool,
    pub show_debug_hud: bool,
    pub ui_scale: f32,
    pub voice_mode: u8,
    pub voice_volume: f32,
    pub voice_input_device: String,
    pub controller_sensitivity: f32,
    pub controller_invert_y: bool,
    pub controller_deadzone: f32,
    pub controller_rumble_intensity: f32,
    pub controller_glyph_style: GlyphStyle,
    pub controller_swap_confirm_cancel: bool,
    pub controller_sneak_toggle: bool,
    pub active_controller_label: String,
    pub active_glyph_family: ControllerFamily,
}

pub struct UiBuilder;

impl UiBuilder {
    pub const CROSSHAIR_TILE: u16 = 21;
    pub const SLOT_BG_TILE: u16 = 22;
    pub const SLOT_HIGHLIGHT_TILE: u16 = 23;
    pub const WHITE_TILE: u16 = 24;
    pub const HEART_CONTAINER_TILE: u16 = 25;
    pub const HEART_FULL_TILE: u16 = 26;
    pub const HEART_HALF_TILE: u16 = 27;
    pub const HUNGER_CONTAINER_TILE: u16 = 28;
    pub const HUNGER_FULL_TILE: u16 = 29;
    pub const HUNGER_HALF_TILE: u16 = 30;
    pub const FLAME_CONTAINER_TILE: u16 = 183;
    pub const FLAME_FULL_TILE: u16 = 184;
    pub const BUBBLE_FULL_TILE: u16 = 201;
    pub const BUBBLE_POP_TILE: u16 = 202;
    pub const ARMOR_ICON_TILE: u16 = 372;

    pub const SLOT_SIZE: f32 = 48.0;
    pub const SLOT_GAP: f32 = 5.0;
    pub const JEI_SLOT_SIZE: f32 = 40.0;
    pub const JEI_SLOT_GAP: f32 = 4.0;
    pub const JEI_COLS: usize = 6;
    pub const JEI_ROWS: usize = 8;

    /// Returns the proportional width ratio (normalized around 1.0 for typical letters)
    /// based on authentic Minecraft glyph geometry in the Faithful 32x ascii font.
    pub fn get_char_width_ratio(c: u8) -> f32 {
        if c == b' ' {
            return 0.45;
        }
        if !(33..=126).contains(&c) {
            return 1.0;
        }
        const WIDTHS: [u8; 94] = [
            2, 6, 10, 10, 10, 10, 2, 6, 6, 6, 10, 2, 10, 2, 10, // 33..47
            10, 8, 10, 10, 10, 10, 10, 10, 10, 10, 2, 2, 8, 10, 8, 10, // 48..63
            12, 10, 10, 10, 10, 10, 10, 10, 10, 6, 10, 10, 10, 10, 10, 10, // 64..79
            10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 6, 10, 6, 10, 10, // 80..95
            4, 10, 10, 10, 10, 10, 8, 10, 10, 2, 10, 8, 4, 10, 10, 10, // 96..111
            10, 10, 10, 10, 6, 10, 10, 10, 10, 10, 10, 6, 2, 6, 12, // 112..126
        ];
        let raw_w = WIDTHS[(c - 33) as usize];
        (raw_w + 2) as f32 / 12.0
    }

    /// Computes the total rendered width of an ASCII string under the specified font scaling.
    pub fn string_width(text: &str, char_w: f32, spacing: f32) -> f32 {
        let mut total = 0.0;
        for b in text.bytes() {
            let cw = char_w * Self::get_char_width_ratio(b);
            total += cw + spacing;
        }
        total
    }

    /// Draws a single character glyph using the pixel font in the atlas.
    pub fn draw_char(mesh: &mut UiMesh, x: f32, y: f32, w: f32, h: f32, c: u8, color: [f32; 4]) {
        if c == b' ' {
            return;
        }
        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(c as u16);
        let ratio = Self::get_char_width_ratio(c).clamp(0.2, 1.2);
        let uv_span = (ratio * (12.0 / 16.0)).min(1.0);
        let char_u_max = u_min + (u_max - u_min) * uv_span;
        mesh.add_quad(x, y, w, h, [u_min, v_min, char_u_max, v_max], color);
    }

    /// Draws an ASCII string using the pixel font with proportional kerning.
    pub fn draw_string(
        mesh: &mut UiMesh,
        mut x: f32,
        y: f32,
        text: &str,
        char_w: f32,
        char_h: f32,
        spacing: f32,
        color: [f32; 4],
    ) {
        for b in text.bytes() {
            let cw = char_w * Self::get_char_width_ratio(b);
            Self::draw_char(mesh, x, y, cw, char_h, b, color);
            x += cw + spacing;
        }
    }

    /// Computes the rendered width of a string containing `&`-prefixed color codes.
    pub fn colored_string_width(text: &str, char_w: f32, spacing: f32) -> f32 {
        let stripped = blockgame_core::strip_color_codes(text);
        Self::string_width(&stripped, char_w, spacing)
    }

    /// Draws a string containing `&`-prefixed color codes (`&0`-`&f`, `&r`, `&&`).
    pub fn draw_colored_string(
        mesh: &mut UiMesh,
        mut x: f32,
        y: f32,
        text: &str,
        char_w: f32,
        char_h: f32,
        spacing: f32,
        default_color: [f32; 4],
    ) {
        for span in blockgame_core::parse_colored_spans(text, default_color) {
            Self::draw_string(mesh, x, y, &span.text, char_w, char_h, spacing, span.color);
            x += Self::string_width(&span.text, char_w, spacing);
        }
    }

    /// Draws a pixel-art controller button glyph from the UI texture atlas.
    pub fn draw_gamepad_glyph(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        size: f32,
        family: ControllerFamily,
        button: GamepadButton,
        tint: [f32; 4],
    ) {
        let uv = TextureAtlas::get_uv_bounds(glyph_tile_id(family, button));
        mesh.add_quad(x, y, size, size, uv, tint);
    }

    /// Draws the controller-driven Virtual Cursor pointer centered at `(x, y)`.
    pub fn draw_virtual_cursor(mesh: &mut UiMesh, x: f32, y: f32, scale: f32) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let s = scale.clamp(0.75, 1.5);
        let outer = 16.0 * s;
        let inner = 10.0 * s;
        let dot = 4.0 * s;
        // Drop shadow
        mesh.add_quad(
            x - outer * 0.5 + 1.5,
            y - outer * 0.5 + 1.5,
            outer,
            outer,
            white_uv,
            [0.02, 0.03, 0.05, 0.72],
        );
        // Gold outer frame
        mesh.add_quad(
            x - outer * 0.5,
            y - outer * 0.5,
            outer,
            outer,
            white_uv,
            [0.98, 0.82, 0.30, 0.96],
        );
        // Dark interior cutout
        mesh.add_quad(
            x - inner * 0.5,
            y - inner * 0.5,
            inner,
            inner,
            white_uv,
            [0.08, 0.10, 0.15, 0.92],
        );
        // Bright center precision dot
        mesh.add_quad(
            x - dot * 0.5,
            y - dot * 0.5,
            dot,
            dot,
            white_uv,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Draws a non-blocking top-center HUD toast (e.g. multiplayer controller disconnect notice).
    pub fn draw_hud_toast(mesh: &mut UiMesh, screen_w: f32, message: &str) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let tw = Self::string_width(message, 9.5, 1.0);
        let box_w = (tw + 32.0).max(220.0);
        let box_h = 32.0;
        let bx = (screen_w - box_w) * 0.5;
        let by = 18.0;
        mesh.add_quad(bx, by, box_w, box_h, white_uv, [0.95, 0.72, 0.25, 0.95]);
        mesh.add_quad(
            bx + 1.5,
            by + 1.5,
            box_w - 3.0,
            box_h - 3.0,
            white_uv,
            [0.10, 0.08, 0.06, 0.92],
        );
        Self::draw_string(
            mesh,
            bx + (box_w - tw) * 0.5,
            by + (box_h - 13.0) * 0.5,
            message,
            9.5,
            13.0,
            1.0,
            [1.0, 0.90, 0.55, 1.0],
        );
    }

    /// Returns `[x, y, w, h]` for Quick-Menu button `idx` (`0..4`: Skills, Achievements, Guide, Debug HUD).
    pub fn quick_menu_item_rect(screen_w: f32, screen_h: f32, idx: usize) -> [f32; 4] {
        let card_w = 320.0_f32.min(screen_w - 40.0);
        let card_h = 256.0;
        let card_x = (screen_w - card_w) * 0.5;
        let card_y = (screen_h - card_h) * 0.5;
        let btn_w = card_w - 36.0;
        let btn_h = 36.0;
        let gap = 8.0;
        [
            card_x + 18.0,
            card_y + 52.0 + (idx as f32) * (btn_h + gap),
            btn_w,
            btn_h,
        ]
    }

    /// Builds the lightweight in-game Quick-Menu overlay (Skills, Achievements, Guide, Debug HUD).
    pub fn build_quick_menu(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        debug_hud_on: bool,
        glyph_family: ControllerFamily,
        confirm_btn: GamepadButton,
        cancel_btn: GamepadButton,
        cursor_pos: [f32; 2],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let card_w = 320.0_f32.min(screen_w - 40.0);
        let card_h = 256.0;
        let card_x = (screen_w - card_w) * 0.5;
        let card_y = (screen_h - card_h) * 0.5;

        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.02, 0.03, 0.06, 0.42],
        );
        mesh.add_quad(
            card_x,
            card_y,
            card_w,
            card_h,
            white_uv,
            [0.92, 0.76, 0.30, 0.92],
        );
        mesh.add_quad(
            card_x + 1.5,
            card_y + 1.5,
            card_w - 3.0,
            card_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.14, 0.95],
        );

        let title = "QUICK MENU";
        let tw = Self::string_width(title, 12.0, 1.2);
        Self::draw_string(
            mesh,
            card_x + (card_w - tw) * 0.5,
            card_y + 18.0,
            title,
            12.0,
            15.0,
            1.2,
            [1.0, 0.90, 0.45, 1.0],
        );

        let labels = [
            "Skills & Milestones".to_string(),
            "Achievements".to_string(),
            "Expedition Guide".to_string(),
            format!("Debug HUD: {}", if debug_hud_on { "ON" } else { "OFF" }),
        ];
        for (idx, label) in labels.iter().enumerate() {
            let r = Self::quick_menu_item_rect(screen_w, screen_h, idx);
            Self::draw_menu_button(mesh, r[0], r[1], r[2], r[3], label, cursor_pos, true);
        }

        // Footer glyph legend
        let fy = card_y + card_h - 26.0;
        Self::draw_gamepad_glyph(
            mesh,
            card_x + 28.0,
            fy - 2.0,
            16.0,
            glyph_family,
            confirm_btn,
            [1.0; 4],
        );
        Self::draw_string(
            mesh,
            card_x + 48.0,
            fy + 1.0,
            "Select",
            8.0,
            11.0,
            0.8,
            [0.82, 0.86, 0.94, 0.95],
        );
        Self::draw_gamepad_glyph(
            mesh,
            card_x + card_w - 102.0,
            fy - 2.0,
            16.0,
            glyph_family,
            cancel_btn,
            [1.0; 4],
        );
        Self::draw_string(
            mesh,
            card_x + card_w - 82.0,
            fy + 1.0,
            "Close",
            8.0,
            11.0,
            0.8,
            [0.82, 0.86, 0.94, 0.95],
        );
    }

    /// Draws a stack count number in the bottom-right corner of an item slot.
    pub fn draw_stack_number(
        mesh: &mut UiMesh,
        slot_x: f32,
        slot_y: f32,
        slot_size: f32,
        count: u16,
    ) {
        if count <= 1 {
            return;
        }
        let text = count.to_string();
        let (char_w, char_h) = if count >= 1000 {
            (7.2, 10.5)
        } else if count >= 100 {
            (8.5, 12.0)
        } else {
            (10.0, 14.0)
        };
        let spacing = 0.5;
        let total_w = Self::string_width(&text, char_w, spacing);
        let x = slot_x + slot_size - total_w - 4.0;
        let y = slot_y + slot_size - char_h - 3.0;
        Self::draw_string(
            mesh,
            x,
            y,
            &text,
            char_w,
            char_h,
            spacing,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Draws a Minecraft-style durability bar beneath damaged tools in inventory/hotbar slots,
    /// plus a cyan-gold shimmer badge in the top-right corner for shop-enhanced items.
    pub fn draw_durability_bar(
        mesh: &mut UiMesh,
        slot_x: f32,
        slot_y: f32,
        slot_size: f32,
        item_slot: ItemSlot,
        registry: &ItemRegistry,
    ) {
        if item_slot.is_empty() {
            return;
        }
        if item_slot.is_enhanced() {
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
            let badge_s = 9.0;
            let bx = slot_x + slot_size - badge_s - 3.0;
            let by = slot_y + 3.0;
            mesh.add_quad(bx, by, badge_s, badge_s, white_uv, [0.22, 0.82, 0.96, 0.95]);
            mesh.add_quad(
                bx + 2.0,
                by + 2.0,
                badge_s - 4.0,
                badge_s - 4.0,
                white_uv,
                [1.0, 0.90, 0.35, 1.0],
            );
        }
        let Some(def) = registry.get(item_slot.item_id) else {
            return;
        };
        let Some(max_dur) = def.durability else {
            return;
        };
        if max_dur == 0 || !item_slot.is_damaged(max_dur) {
            return;
        }

        let fraction = item_slot.durability_fraction(max_dur).clamp(0.0, 1.0);
        let bar_pad_x = 6.0;
        let bar_w = (slot_size - bar_pad_x * 2.0).max(4.0);
        let bar_h = 3.0;
        let bar_x = slot_x + bar_pad_x;
        let bar_y = slot_y + slot_size - bar_h - 4.0;
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Black border / background
        mesh.add_quad(
            bar_x - 1.0,
            bar_y - 1.0,
            bar_w + 2.0,
            bar_h + 2.0,
            white_uv,
            [0.0, 0.0, 0.0, 0.95],
        );

        // Color interpolation based on fraction (green -> yellow -> red)
        let (r, g, b) = if fraction > 0.5 {
            let t = (fraction - 0.5) * 2.0;
            (1.0 - t, 1.0, 0.0)
        } else {
            let t = fraction * 2.0;
            (1.0, t, 0.0)
        };

        let fill_w = (bar_w * fraction).max(1.0);
        mesh.add_quad(bar_x, bar_y, fill_w, bar_h, white_uv, [r, g, b, 1.0]);
    }

    /// Draws a hover tooltip displaying an item's display name.
    pub fn draw_tooltip(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_x: f32,
        cursor_y: f32,
        text: &str,
    ) {
        let char_w = 9.0;
        let char_h = 13.0;
        let spacing = 1.0;
        let text_w = Self::string_width(text, char_w, spacing);
        let box_w = text_w + 16.0;
        let box_h = 24.0;

        let tx = (cursor_x + 14.0).clamp(4.0, (screen_w - box_w - 4.0).max(4.0));
        let ty = (cursor_y - 24.0).clamp(4.0, (screen_h - box_h - 4.0).max(4.0));

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Tooltip Border (dark purple)
        let border_pad = 2.0;
        mesh.add_quad(
            tx - border_pad,
            ty - border_pad,
            box_w + border_pad * 2.0,
            box_h + border_pad * 2.0,
            white_uv,
            [0.22, 0.10, 0.38, 0.98],
        );

        // Tooltip Background (deep dark violet)
        mesh.add_quad(tx, ty, box_w, box_h, white_uv, [0.08, 0.04, 0.16, 0.94]);

        // Tooltip Text
        Self::draw_string(
            mesh,
            tx + 7.0,
            ty + 5.0,
            text,
            char_w,
            char_h,
            spacing,
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    /// Renders first-person view: bare hand if empty, or the held item in the bottom right corner.
    /// Hand and items move only when walking and perform a punch animation on left click.
    pub fn draw_held_item(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        active_slot: ItemSlot,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
    ) {
        let default_app = blockgame_core::character::CharacterAppearance::default();
        Self::draw_held_item_with_appearance(
            mesh,
            screen_w,
            screen_h,
            active_slot,
            registry,
            walk_distance,
            punch_time,
            &default_app,
        );
    }

    /// Computes `(upper_top, upper_side, forearm_top, forearm_side, skin_light, optional_cuff_top, optional_cuff_side)`
    /// for first-person arm rendering so empty hand and held-item arms match the player's Character Appearance.
    fn first_person_arm_colors(
        appearance: &blockgame_core::character::CharacterAppearance,
    ) -> (
        [f32; 4],
        [f32; 4],
        [f32; 4],
        [f32; 4],
        [f32; 4],
        Option<([f32; 4], [f32; 4])>,
    ) {
        use blockgame_core::character::ShirtStyle;
        let app = appearance.sanitized();

        let skin_top = app.skin_tone.to_rgba_f32(1.0);
        let skin_side = app.skin_tone.scale(0.82).to_rgba_f32(1.0);
        let skin_light = app.skin_tone.scale(1.06).to_rgba_f32(1.0);

        let shirt_top = app.shirt_primary.to_rgba_f32(1.0);
        let shirt_side = app.shirt_primary.scale(0.78).to_rgba_f32(1.0);
        let shirt_dark_top = app.shirt_primary.scale(0.86).to_rgba_f32(1.0);
        let shirt_dark_side = app.shirt_primary.scale(0.68).to_rgba_f32(1.0);
        let cuff = Some((
            app.shirt_accent.to_rgba_f32(1.0),
            app.shirt_accent.scale(0.80).to_rgba_f32(1.0),
        ));

        match app.shirt_style {
            ShirtStyle::Vest => (skin_top, skin_side, skin_top, skin_side, skin_light, None),
            ShirtStyle::TShirt => (shirt_top, shirt_side, skin_top, skin_side, skin_light, None),
            ShirtStyle::LongSleeve | ShirtStyle::Hoodie | ShirtStyle::Jacket => {
                (shirt_top, shirt_side, shirt_top, shirt_side, skin_light, cuff)
            }
            ShirtStyle::Tunic => (
                shirt_top,
                shirt_side,
                shirt_dark_top,
                shirt_dark_side,
                skin_light,
                cuff,
            ),
        }
    }

    /// Renders first-person view using the player's `CharacterAppearance` for arm skin tone, sleeves, and cuffs.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_held_item_with_appearance(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        active_slot: ItemSlot,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
        appearance: &blockgame_core::character::CharacterAppearance,
    ) {
        let white_uv_bounds = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let white_uv = [
            [white_uv_bounds[0], white_uv_bounds[1]],
            [white_uv_bounds[2], white_uv_bounds[1]],
            [white_uv_bounds[2], white_uv_bounds[3]],
            [white_uv_bounds[0], white_uv_bounds[3]],
        ];

        let scale = (screen_h / 720.0).clamp(0.7, 1.5);
        // Bobbing & sway: occurs ONLY when walking (walk_distance increases while walking)
        let bob_x = (walk_distance * 3.5).sin() * 7.0 * scale;
        let bob_y = (walk_distance * 7.0).cos().abs() * 8.0 * scale;

        // Punch swing animation (0.0 -> 1.0 -> 0.0)
        let punch_swing = (punch_time * std::f32::consts::PI).sin();

        let (
            col_sleeve_top,
            col_sleeve_side,
            col_forearm_top,
            col_forearm_side,
            col_skin_light,
            cuff_colors,
        ) = Self::first_person_arm_colors(appearance);

        let def_opt = registry.get(active_slot.item_id);

        if active_slot.is_empty() || def_opt.is_none() {
            // =========================================================================
            // CASE 1: Empty hand -> See your hand and fist in the lower right
            // =========================================================================
            let punch_x = -45.0 * scale * punch_swing;
            let punch_y = -35.0 * scale * punch_swing;

            let p_arm_bot_r = [
                screen_w + 40.0 * scale + bob_x + punch_x * 0.3,
                screen_h + 30.0 * scale,
            ];
            let p_arm_bot_l = [
                screen_w - 95.0 * scale + bob_x + punch_x * 0.3,
                screen_h + 30.0 * scale,
            ];
            let p_arm_bot_m = [
                (p_arm_bot_l[0] + p_arm_bot_r[0]) * 0.5,
                (p_arm_bot_l[1] + p_arm_bot_r[1]) * 0.5,
            ];

            let p_sleeve_r = [
                screen_w - 20.0 * scale + bob_x + punch_x * 0.6,
                screen_h - 130.0 * scale + bob_y + punch_y * 0.6,
            ];
            let p_sleeve_l = [
                screen_w - 145.0 * scale + bob_x + punch_x * 0.6,
                screen_h - 85.0 * scale + bob_y + punch_y * 0.6,
            ];
            let p_sleeve_m = [
                (p_sleeve_l[0] + p_sleeve_r[0]) * 0.5,
                (p_sleeve_l[1] + p_sleeve_r[1]) * 0.5,
            ];

            let p_wrist_r = [
                screen_w - 75.0 * scale + bob_x + punch_x,
                screen_h - 195.0 * scale + bob_y + punch_y,
            ];
            let p_wrist_l = [
                screen_w - 180.0 * scale + bob_x + punch_x,
                screen_h - 150.0 * scale + bob_y + punch_y,
            ];
            let p_wrist_m = [
                (p_wrist_l[0] + p_wrist_r[0]) * 0.5,
                (p_wrist_l[1] + p_wrist_r[1]) * 0.5,
            ];

            let p_hand = [
                screen_w - 155.0 * scale + bob_x + punch_x * 1.1,
                screen_h - 200.0 * scale + bob_y + punch_y * 1.1,
            ];

            // 1. Sleeve quads (side facet and top facet)
            mesh.add_quad_vertices(
                [p_arm_bot_l, p_arm_bot_m, p_sleeve_m, p_sleeve_l],
                white_uv,
                [col_sleeve_side; 4],
            );
            mesh.add_quad_vertices(
                [p_arm_bot_m, p_arm_bot_r, p_sleeve_r, p_sleeve_m],
                white_uv,
                [col_sleeve_top; 4],
            );

            // 2. Forearm quads (side facet and top facet)
            mesh.add_quad_vertices(
                [p_sleeve_l, p_sleeve_m, p_wrist_m, p_wrist_l],
                white_uv,
                [col_forearm_side; 4],
            );
            mesh.add_quad_vertices(
                [p_sleeve_m, p_sleeve_r, p_wrist_r, p_wrist_m],
                white_uv,
                [col_forearm_top; 4],
            );

            // Optional sleeve cuff accent band near wrist for long-sleeved shirts
            if let Some((cuff_top, cuff_side)) = cuff_colors {
                let lerp2 = |a: [f32; 2], b: [f32; 2], t: f32| -> [f32; 2] {
                    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
                };
                let c_l = lerp2(p_sleeve_l, p_wrist_l, 0.78);
                let c_m = lerp2(p_sleeve_m, p_wrist_m, 0.78);
                let c_r = lerp2(p_sleeve_r, p_wrist_r, 0.78);
                mesh.add_quad_vertices([c_l, c_m, p_wrist_m, p_wrist_l], white_uv, [cuff_side; 4]);
                mesh.add_quad_vertices([c_m, c_r, p_wrist_r, p_wrist_m], white_uv, [cuff_top; 4]);
            }

            // 3. Clenched fist punching forward
            let p_fist_tip_l = [
                p_hand[0] - 25.0 * scale + punch_x * 0.2,
                p_hand[1] - 25.0 * scale + punch_y * 0.2,
            ];
            let p_fist_tip_r = [
                p_hand[0] + 40.0 * scale + punch_x * 0.2,
                p_hand[1] - 45.0 * scale + punch_y * 0.2,
            ];
            mesh.add_quad_vertices(
                [p_wrist_l, p_wrist_r, p_fist_tip_r, p_fist_tip_l],
                white_uv,
                [col_skin_light; 4],
            );
            return;
        }

        // =========================================================================
        // CASE 2: Holding an item -> See that item in the bottom right corner
        // =========================================================================
        // =========================================================================
        // CASE 2: Holding an item -> See that 3D item in the bottom right corner (no hand visible)
        // =========================================================================
        let def = def_opt.unwrap();
        let block_registry = blockgame_core::block::BlockRegistry::new();

        if let Some(block_id) = def.placeable_block {
            if block_id == blockgame_core::block::BlockId::TORCH {
                // -----------------------------------------------------------------
                // 3D HELD TORCH: 3D volumetric wooden shaft and 3D glowing flame head
                // -----------------------------------------------------------------
                let torch_punch_x = -45.0 * scale * punch_swing;
                let torch_punch_y = 35.0 * scale * punch_swing;

                let base = [
                    screen_w - 55.0 * scale + bob_x + torch_punch_x,
                    screen_h + 10.0 * scale + bob_y + torch_punch_y,
                ];
                let tip = [
                    screen_w - 180.0 * scale + bob_x + torch_punch_x * 1.4,
                    screen_h - 200.0 * scale + bob_y + torch_punch_y * 1.4,
                ];

                let dir_x = tip[0] - base[0];
                let dir_y = tip[1] - base[1];
                let len = (dir_x * dir_x + dir_y * dir_y).sqrt().max(0.001);
                let dir = [dir_x / len, dir_y / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                let p0 = [base[0] - norm[0] * half_w, base[1] - norm[1] * half_w];
                let p1 = [base[0] + norm[0] * half_w, base[1] + norm[1] * half_w];
                let p2 = [tip[0] + norm[0] * half_w, tip[1] + norm[1] * half_w];
                let p3 = [tip[0] - norm[0] * half_w, tip[1] - norm[1] * half_w];

                let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(13);
                let u_w = u_max - u_min;
                let v_h = v_max - v_min;
                let u_side_left = u_min + u_w * (7.0 / 16.0);
                let u_side_right = u_min + u_w * (9.0 / 16.0);
                let v_side_top = v_min + v_h * (6.0 / 16.0);
                let v_side_bottom = v_min + v_h * (16.0 / 16.0);

                let torch_uvs = [
                    [u_side_left, v_side_bottom],
                    [u_side_right, v_side_bottom],
                    [u_side_right, v_side_top],
                    [u_side_left, v_side_top],
                ];

                // 1. Shaft Front Face
                mesh.add_quad_vertices([p0, p1, p2, p3], torch_uvs, [[1.0, 1.0, 1.0, 1.0]; 4]);

                // 2. Shaft Side 3D Thickness Face (extruded back-left)
                let p3_b = [p3[0] + thick[0], p3[1] + thick[1]];
                let p0_b = [p0[0] + thick[0], p0[1] + thick[1]];
                mesh.add_quad_vertices(
                    [p0_b, p0, p3, p3_b],
                    white_uv,
                    [[0.46, 0.32, 0.18, 1.0]; 4],
                );

                // 3. Shaft Top Face (at tip)
                let p2_b = [p2[0] + thick[0], p2[1] + thick[1]];
                mesh.add_quad_vertices(
                    [p3, p2, p2_b, p3_b],
                    white_uv,
                    [[0.55, 0.40, 0.22, 1.0]; 4],
                );

                // 4. 3D Flame Head at the tip
                let flame_size = 36.0 * scale;
                let fx0 = tip[0] - flame_size * 0.5;
                let fx1 = tip[0] + flame_size * 0.5;
                let fy0 = tip[1] - flame_size * 0.5;
                let fy1 = tip[1] + flame_size * 0.5;

                let u_flame_left = u_min + u_w * (6.0 / 16.0);
                let u_flame_right = u_min + u_w * (10.0 / 16.0);
                let v_flame_top = v_min + v_h * (0.0 / 16.0);
                let v_flame_bottom = v_min + v_h * (5.0 / 16.0);
                let flame_uv = [
                    [u_flame_left, v_flame_bottom],
                    [u_flame_right, v_flame_bottom],
                    [u_flame_right, v_flame_top],
                    [u_flame_left, v_flame_top],
                ];

                // Flame Front Face
                mesh.add_quad_vertices(
                    [[fx0, fy1], [fx1, fy1], [fx1, fy0], [fx0, fy0]],
                    flame_uv,
                    [[1.0, 0.95, 0.65, 1.0]; 4],
                );

                // Flame Side 3D Thickness Face
                let fx0_b = fx0 + thick[0];
                let fy0_b = fy0 + thick[1];
                let fy1_b = fy1 + thick[1];
                mesh.add_quad_vertices(
                    [[fx0_b, fy1_b], [fx0, fy1], [fx0, fy0], [fx0_b, fy0_b]],
                    white_uv,
                    [[1.0, 0.55, 0.15, 1.0]; 4],
                );

                // Flame Top 3D Crown Face
                let fx1_b = fx1 + thick[0];
                mesh.add_quad_vertices(
                    [[fx0, fy0], [fx1, fy0], [fx1_b, fy0_b], [fx0_b, fy0_b]],
                    white_uv,
                    [[1.0, 1.0, 0.85, 1.0]; 4],
                );

                // Soft Outer Flame Glow
                mesh.add_quad(
                    tip[0] - 32.0 * scale,
                    tip[1] - 32.0 * scale,
                    64.0 * scale,
                    64.0 * scale,
                    TextureAtlas::get_uv_bounds(0),
                    [1.0, 0.85, 0.20, 0.22],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id.is_stairs() {
                // -----------------------------------------------------------------
                // 3D HELD STAIRS: 3D stepped staircase block (riser, lower/upper treads, sides)
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;

                let top_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosY);
                let side_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosZ);
                let right_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosX);

                let [u0_t, v0_t, u1_t, v1_t] = TextureAtlas::get_uv_bounds(top_tile);
                let [u0_s, v0_s, u1_s, v1_s] = TextureAtlas::get_uv_bounds(side_tile);
                let [u0_r, v0_r, u1_r, v1_r] = TextureAtlas::get_uv_bounds(right_tile);

                let h_face = bs * 1.10;
                let h_half = h_face * 0.50;

                // Diamond outer vertices and axis vectors
                let p_top_n = [bc[0], bc[1] - bs * 0.65];
                let u_vec = [bs * 1.05, bs * 0.51];
                let v_vec = [-bs * 1.05, bs * 0.51];
                let p_top_e = [p_top_n[0] + u_vec[0], p_top_n[1] + u_vec[1]];
                let p_top_w = [p_top_n[0] + v_vec[0], p_top_n[1] + v_vec[1]];
                let p_top_s = [p_top_w[0] + u_vec[0], p_top_w[1] + u_vec[1]];

                let d_full = [0.0, h_face];
                let d_half = [0.0, h_half];

                let uvs_top = [[u0_t, v0_t], [u1_t, v0_t], [u1_t, v1_t], [u0_t, v1_t]];
                let uvs_side = [[u0_s, v0_s], [u1_s, v0_s], [u1_s, v1_s], [u0_s, v1_s]];
                let uvs_right = [[u0_r, v0_r], [u1_r, v0_r], [u1_r, v1_r], [u0_r, v1_r]];

                // 1. Upper Step Top Tread Face (from u=0 to u=0.5, full width along v)
                let p_mid_n = [p_top_n[0] + 0.5 * u_vec[0], p_top_n[1] + 0.5 * u_vec[1]];
                let p_mid_w = [p_top_w[0] + 0.5 * u_vec[0], p_top_w[1] + 0.5 * u_vec[1]];
                mesh.add_quad_vertices(
                    [p_top_w, p_top_n, p_mid_n, p_mid_w],
                    uvs_top,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

                // 2. Vertical Riser between Upper and Lower Step (facing front-right at u=0.5)
                let r_bot_n = [p_mid_n[0] + d_half[0], p_mid_n[1] + d_half[1]];
                let r_bot_w = [p_mid_w[0] + d_half[0], p_mid_w[1] + d_half[1]];
                mesh.add_quad_vertices(
                    [p_mid_w, p_mid_n, r_bot_n, r_bot_w],
                    uvs_right,
                    [[0.75, 0.75, 0.78, 1.0]; 4],
                );

                // 3. Lower Step Top Tread Face (at d_half height, from u=0.5 to u=1.0)
                let t_e = [p_top_e[0] + d_half[0], p_top_e[1] + d_half[1]];
                let t_s = [p_top_s[0] + d_half[0], p_top_s[1] + d_half[1]];
                mesh.add_quad_vertices(
                    [r_bot_w, r_bot_n, t_e, t_s],
                    uvs_top,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

                // 4. Front Lower Step Riser Face (facing front-right at u=1.0)
                let f_e = [p_top_e[0] + d_full[0], p_top_e[1] + d_full[1]];
                let f_s = [p_top_s[0] + d_full[0], p_top_s[1] + d_full[1]];
                mesh.add_quad_vertices(
                    [t_s, t_e, f_e, f_s],
                    uvs_right,
                    [[0.68, 0.68, 0.70, 1.0]; 4],
                );

                // 5. Left Side Stepped L-Profile Faces (facing front-left along v=1)
                // 5a. Upper back block (full height)
                let p_bot_w = [p_top_w[0] + d_full[0], p_top_w[1] + d_full[1]];
                let p_bot_mid_w = [p_mid_w[0] + d_full[0], p_mid_w[1] + d_full[1]];
                mesh.add_quad_vertices(
                    [p_top_w, p_mid_w, p_bot_mid_w, p_bot_w],
                    uvs_side,
                    [[0.85, 0.85, 0.88, 1.0]; 4],
                );
                // 5b. Lower front block (from d_half down to d_full)
                mesh.add_quad_vertices(
                    [r_bot_w, t_s, f_s, p_bot_mid_w],
                    uvs_side,
                    [[0.82, 0.82, 0.85, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 20.0 * scale,
                        bc[1] + bs * 0.50,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id == blockgame_core::block::BlockId::OAK_FENCE {
                // -----------------------------------------------------------------
                // 3D HELD OAK FENCE: 3D wooden post with horizontal cross rails
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;

                let side_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosZ);
                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(side_tile);
                let plank_uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];

                let post_w = bs * 0.35;
                let post_h = bs * 1.15;

                // Center Post Top Diamond
                let pt_n = [bc[0], bc[1] - bs * 0.45];
                let pt_e = [bc[0] + post_w, bc[1] - bs * 0.25];
                let pt_s = [bc[0], bc[1] - bs * 0.05];
                let pt_w = [bc[0] - post_w, bc[1] - bs * 0.25];
                mesh.add_quad_vertices([pt_w, pt_n, pt_e, pt_s], plank_uv, [[1.0, 1.0, 1.0, 1.0]; 4]);

                // Center Post Left & Right Faces
                let pb_w = [pt_w[0], pt_w[1] + post_h];
                let pb_s = [pt_s[0], pt_s[1] + post_h];
                let pb_e = [pt_e[0], pt_e[1] + post_h];
                mesh.add_quad_vertices([pt_w, pt_s, pb_s, pb_w], plank_uv, [[0.82, 0.82, 0.82, 1.0]; 4]);
                mesh.add_quad_vertices([pt_s, pt_e, pb_e, pb_s], plank_uv, [[0.68, 0.68, 0.68, 1.0]; 4]);

                // Upper & Lower Horizontal Rails
                for &y_offset in &[0.20, 0.60] {
                    let ry = pt_s[1] + post_h * y_offset;
                    let rail_h = 14.0 * scale;
                    let rail_len = bs * 0.85;

                    // Left rail
                    let r_l_start = [pt_w[0], ry];
                    let r_l_end = [pt_w[0] - rail_len, ry - rail_len * 0.2];
                    let r0 = r_l_end;
                    let r1 = r_l_start;
                    let r2 = [r1[0], r1[1] + rail_h];
                    let r3 = [r0[0], r0[1] + rail_h];
                    mesh.add_quad_vertices([r0, r1, r2, r3], plank_uv, [[0.78, 0.78, 0.78, 1.0]; 4]);

                    // Right rail
                    let r_r_start = [pt_e[0], ry];
                    let r_r_end = [pt_e[0] + rail_len, ry - rail_len * 0.2];
                    let rr0 = r_r_start;
                    let rr1 = r_r_end;
                    let rr2 = [rr1[0], rr1[1] + rail_h];
                    let rr3 = [rr0[0], rr0[1] + rail_h];
                    mesh.add_quad_vertices([rr0, rr1, rr2, rr3], plank_uv, [[0.65, 0.65, 0.65, 1.0]; 4]);
                }

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 20.0 * scale,
                        bc[1] + bs * 0.50,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id == blockgame_core::block::BlockId::OAK_DOOR_CLOSED {
                // -----------------------------------------------------------------
                // 3D HELD OAK DOOR: 3D upright wooden door slab with thickness
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;

                let door_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosZ);
                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(door_tile);
                let door_uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];

                let door_w = bs * 0.75;
                let door_h = bs * 1.50;
                let thick = [-10.0 * scale, -8.0 * scale];

                // Front Face (tilted upright isometric door panel)
                let p0 = [bc[0] - door_w * 0.85, bc[1] + door_h * 0.45];
                let p1 = [bc[0] + door_w * 0.45, bc[1] + door_h * 0.25];
                let p2 = [bc[0] + door_w * 0.45, bc[1] - door_h * 0.55];
                let p3 = [bc[0] - door_w * 0.85, bc[1] - door_h * 0.35];
                mesh.add_quad_vertices([p0, p1, p2, p3], door_uv, [[1.0, 1.0, 1.0, 1.0]; 4]);

                // Side Thickness Face (left edge)
                let p0_b = [p0[0] + thick[0], p0[1] + thick[1]];
                let p3_b = [p3[0] + thick[0], p3[1] + thick[1]];
                mesh.add_quad_vertices([p0_b, p0, p3, p3_b], white_uv, [[0.46, 0.32, 0.18, 1.0]; 4]);

                // Top Thickness Face (top edge)
                let p2_b = [p2[0] + thick[0], p2[1] + thick[1]];
                mesh.add_quad_vertices([p3_b, p3, p2, p2_b], white_uv, [[0.62, 0.44, 0.25, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 15.0 * scale,
                        bc[1] + bs * 0.45,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id == blockgame_core::block::BlockId::BED {
                // -----------------------------------------------------------------
                // 3D HELD BED: 3D miniature bed (pillow, mattress/quilt, wood frame, legs)
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;
                let bed_h = bs * 0.38;

                // 2:1 Elongated Bed Isometric Proportions (Length along U, Width along V)
                let len_vec = [bs * 1.30, bs * 0.63];
                let wid_vec = [-bs * 0.65, bs * 0.32];

                let p_head_n = [bc[0] - bs * 0.15, bc[1] - bs * 0.50];
                let p_head_w = [p_head_n[0] + wid_vec[0], p_head_n[1] + wid_vec[1]];
                let p_foot_e = [p_head_n[0] + len_vec[0], p_head_n[1] + len_vec[1]];
                let p_foot_s = [p_head_w[0] + len_vec[0], p_head_w[1] + len_vec[1]];

                // Pillow line at 28% from headboard
                let p_pil_n = [p_head_n[0] + len_vec[0] * 0.28, p_head_n[1] + len_vec[1] * 0.28];
                let p_pil_w = [p_head_w[0] + len_vec[0] * 0.28, p_head_w[1] + len_vec[1] * 0.28];

                // 1. White Pillow Top Face
                mesh.add_quad_vertices([p_head_w, p_head_n, p_pil_n, p_pil_w], white_uv, [[0.94, 0.94, 0.96, 1.0]; 4]);

                // 2. Red Quilt / Blanket Top Face
                let [u0_wool, v0_wool, u1_wool, v1_wool] = TextureAtlas::get_uv_bounds(111); // red wool
                let wool_uv = [[u0_wool, v0_wool], [u1_wool, v0_wool], [u1_wool, v1_wool], [u0_wool, v1_wool]];
                mesh.add_quad_vertices([p_pil_w, p_pil_n, p_foot_e, p_foot_s], wool_uv, [[0.88, 0.22, 0.22, 1.0]; 4]);

                // 3. Left Long Side Face (facing front-left: blanket overhang + oak bed rail)
                let p_bot_head_w = [p_head_w[0], p_head_w[1] + bed_h];
                let p_bot_foot_s = [p_foot_s[0], p_foot_s[1] + bed_h];
                let p_mid_head_w = [p_head_w[0], p_head_w[1] + bed_h * 0.45];
                let p_mid_foot_s = [p_foot_s[0], p_foot_s[1] + bed_h * 0.45];
                mesh.add_quad_vertices([p_head_w, p_foot_s, p_mid_foot_s, p_mid_head_w], white_uv, [[0.72, 0.18, 0.18, 1.0]; 4]);
                mesh.add_quad_vertices([p_mid_head_w, p_mid_foot_s, p_bot_foot_s, p_bot_head_w], white_uv, [[0.46, 0.32, 0.18, 1.0]; 4]);

                // 4. Footboard End Face (facing front-right: blanket overhang + oak footboard)
                let p_bot_foot_e = [p_foot_e[0], p_foot_e[1] + bed_h];
                let p_mid_foot_e = [p_foot_e[0], p_foot_e[1] + bed_h * 0.45];
                mesh.add_quad_vertices([p_foot_s, p_foot_e, p_mid_foot_e, p_mid_foot_s], white_uv, [[0.58, 0.14, 0.14, 1.0]; 4]);
                mesh.add_quad_vertices([p_mid_foot_s, p_mid_foot_e, p_bot_foot_e, p_bot_foot_s], white_uv, [[0.36, 0.24, 0.13, 1.0]; 4]);

                // 5. Wooden Corner Legs
                let leg_w = 7.0 * scale;
                let leg_h = 14.0 * scale;
                for &pt in &[p_bot_head_w, p_bot_foot_s, p_bot_foot_e] {
                    mesh.add_quad_vertices(
                        [[pt[0] - leg_w * 0.5, pt[1]], [pt[0] + leg_w * 0.5, pt[1]], [pt[0] + leg_w * 0.5, pt[1] + leg_h], [pt[0] - leg_w * 0.5, pt[1] + leg_h]],
                        white_uv,
                        [[0.28, 0.18, 0.10, 1.0]; 4],
                    );
                }

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 20.0 * scale,
                        bc[1] + bs * 0.50,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id.is_flower()
                || block_id == blockgame_core::block::BlockId::TALL_GRASS
                || block_id == blockgame_core::block::BlockId::SEAGRASS
                || block_id == blockgame_core::block::BlockId::SUGAR_CANE
                || block_id == blockgame_core::block::BlockId::DRIPSTONE
                || block_id == blockgame_core::block::BlockId::GLOW_CRYSTAL
                || block_id.is_farmland_crop()
            {
                // -----------------------------------------------------------------
                // 3D HELD PLANT / FLOWER: Crossed-quad in-world plant geometry with soil clod
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];

                let plant_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosZ);
                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(plant_tile);
                let plant_uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];

                let ps = 95.0 * scale;
                let cx = bc[0];
                let cy = bc[1] + 20.0 * scale;

                // Little clod of earth at stem base
                let dw = 25.0 * scale;
                let dh = 12.0 * scale;
                let dirt_top = [
                    [cx - dw, cy + ps * 0.42],
                    [cx, cy + ps * 0.42 - dh],
                    [cx + dw, cy + ps * 0.42],
                    [cx, cy + ps * 0.42 + dh],
                ];
                mesh.add_quad_vertices(dirt_top, white_uv, [[0.44, 0.30, 0.18, 1.0]; 4]);

                // Quad 1: Diagonal plane A (back-left to front-right)
                let qa_bl = [cx - ps * 0.55, cy + ps * 0.42 - ps * 0.12];
                let qa_br = [cx + ps * 0.55, cy + ps * 0.42 + ps * 0.12];
                let qa_tr = [cx + ps * 0.55, cy - ps * 0.48 + ps * 0.12];
                let qa_tl = [cx - ps * 0.55, cy - ps * 0.48 - ps * 0.12];
                mesh.add_quad_vertices([qa_tl, qa_tr, qa_br, qa_bl], plant_uv, [[1.0, 1.0, 1.0, 1.0]; 4]);

                // Quad 2: Diagonal plane B (perpendicular cross: front-left to back-right)
                let qb_bl = [cx - ps * 0.55, cy + ps * 0.42 + ps * 0.12];
                let qb_br = [cx + ps * 0.55, cy + ps * 0.42 - ps * 0.12];
                let qb_tr = [cx + ps * 0.55, cy - ps * 0.48 - ps * 0.12];
                let qb_tl = [cx - ps * 0.55, cy - ps * 0.48 + ps * 0.12];
                mesh.add_quad_vertices([qb_tl, qb_tr, qb_br, qb_bl], plant_uv, [[0.86, 0.86, 0.86, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        cx + 20.0 * scale,
                        cy + ps * 0.35,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if block_id == blockgame_core::block::BlockId::LILY_PAD {
                // -----------------------------------------------------------------
                // 3D HELD LILY PAD: 3D floating disc with pad rim thickness
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;

                let lily_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosY);
                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(lily_tile);
                let lily_uv = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];

                let lw = bs * 1.0;
                let lh = bs * 0.55;

                let p_n = [bc[0], bc[1] - lh * 0.5];
                let p_e = [bc[0] + lw * 0.5, bc[1]];
                let p_s = [bc[0], bc[1] + lh * 0.5];
                let p_w = [bc[0] - lw * 0.5, bc[1]];
                mesh.add_quad_vertices([p_w, p_n, p_e, p_s], lily_uv, [[1.0, 1.0, 1.0, 1.0]; 4]);

                // Rim thickness underneath
                let rim_h = 7.0 * scale;
                let pb_w = [p_w[0], p_w[1] + rim_h];
                let pb_s = [p_s[0], p_s[1] + rim_h];
                let pb_e = [p_e[0], p_e[1] + rim_h];
                mesh.add_quad_vertices([p_w, p_s, pb_s, pb_w], white_uv, [[0.20, 0.48, 0.22, 1.0]; 4]);
                mesh.add_quad_vertices([p_s, p_e, pb_e, pb_s], white_uv, [[0.15, 0.38, 0.18, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 20.0 * scale,
                        bc[1] + bs * 0.40,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else {
                // -----------------------------------------------------------------
                // 3D HELD SOLID BLOCK: Prominent isometric 3D block with top and side shading
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 130.0 * scale + bob_y + item_punch_y,
                ];
                let bs = 90.0 * scale;

                let top_tile = block_registry
                    .get_texture(block_id, blockgame_core::direction::Direction::PosY);
                let side_tile = block_registry
                    .get_texture(block_id, blockgame_core::direction::Direction::PosZ);
                let right_tile = block_registry
                    .get_texture(block_id, blockgame_core::direction::Direction::PosX);

                let [u0_t, v0_t, u1_t, v1_t] = TextureAtlas::get_uv_bounds(top_tile);
                let [u0_s, v0_s, u1_s, v1_s] = TextureAtlas::get_uv_bounds(side_tile);
                let [u0_r, v0_r, u1_r, v1_r] = TextureAtlas::get_uv_bounds(right_tile);

                // Top Face (diamond)
                let p_top_n = [bc[0], bc[1] - bs * 0.65];
                let p_top_e = [bc[0] + bs * 1.05, bc[1] - bs * 0.16];
                let p_top_s = [bc[0], bc[1] + bs * 0.35];
                let p_top_w = [bc[0] - bs * 1.05, bc[1] - bs * 0.16];

                let uvs_top = [[u0_t, v0_t], [u1_t, v0_t], [u1_t, v1_t], [u0_t, v1_t]];
                mesh.add_quad_vertices(
                    [p_top_w, p_top_n, p_top_e, p_top_s],
                    uvs_top,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

                // Left Face (parallelogram facing front-left)
                let h_face = bs * 1.10;
                let p_left_bot_w = [p_top_w[0], p_top_w[1] + h_face];
                let p_left_bot_e = [p_top_s[0], p_top_s[1] + h_face];
                let uvs_left = [[u0_s, v0_s], [u1_s, v0_s], [u1_s, v1_s], [u0_s, v1_s]];
                mesh.add_quad_vertices(
                    [p_top_w, p_top_s, p_left_bot_e, p_left_bot_w],
                    uvs_left,
                    [[0.82, 0.82, 0.82, 1.0]; 4],
                );

                // Right Face (parallelogram facing front-right)
                let p_right_bot_w = p_left_bot_e;
                let p_right_bot_e = [p_top_e[0], p_top_e[1] + h_face];
                let uvs_right = [[u0_r, v0_r], [u1_r, v0_r], [u1_r, v1_r], [u0_r, v1_r]];
                mesh.add_quad_vertices(
                    [p_top_s, p_top_e, p_right_bot_e, p_right_bot_w],
                    uvs_right,
                    [[0.68, 0.68, 0.68, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 20.0 * scale,
                        bc[1] + bs * 0.50,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            }
        } else {
            // =====================================================================
            // CASE 2B: Non-Block 3D Items (Pickaxe, Stick, Iron Ingot, Coal, etc.)
            // =====================================================================
            let is_pickaxe = active_slot.item_id == ItemId::WOODEN_PICKAXE
                || active_slot.item_id == ItemId::STONE_PICKAXE
                || active_slot.item_id == ItemId::COPPER_PICKAXE
                || active_slot.item_id == ItemId::IRON_PICKAXE
                || active_slot.item_id == ItemId::DIAMOND_PICKAXE
                || active_slot.item_id == ItemId::GOLD_PICKAXE;

            let is_hoe = active_slot.item_id == ItemId::WOODEN_HOE
                || active_slot.item_id == ItemId::STONE_HOE
                || active_slot.item_id == ItemId::COPPER_HOE
                || active_slot.item_id == ItemId::IRON_HOE
                || active_slot.item_id == ItemId::DIAMOND_HOE
                || active_slot.item_id == ItemId::GOLD_HOE;

            let is_axe = active_slot.item_id == ItemId::WOODEN_AXE
                || active_slot.item_id == ItemId::STONE_AXE
                || active_slot.item_id == ItemId::COPPER_AXE
                || active_slot.item_id == ItemId::IRON_AXE
                || active_slot.item_id == ItemId::DIAMOND_AXE
                || active_slot.item_id == ItemId::GOLD_AXE;

            let is_shovel = active_slot.item_id == ItemId::WOODEN_SHOVEL
                || active_slot.item_id == ItemId::STONE_SHOVEL
                || active_slot.item_id == ItemId::COPPER_SHOVEL
                || active_slot.item_id == ItemId::IRON_SHOVEL
                || active_slot.item_id == ItemId::DIAMOND_SHOVEL
                || active_slot.item_id == ItemId::GOLD_SHOVEL;

            let is_sword = active_slot.item_id == ItemId::WOODEN_SWORD
                || active_slot.item_id == ItemId::STONE_SWORD
                || active_slot.item_id == ItemId::COPPER_SWORD
                || active_slot.item_id == ItemId::IRON_SWORD
                || active_slot.item_id == ItemId::DIAMOND_SWORD
                || active_slot.item_id == ItemId::GOLD_SWORD;

            let is_fishing_rod = active_slot.item_id == ItemId::WOODEN_FISHING_ROD
                || active_slot.item_id == ItemId::COPPER_FISHING_ROD
                || active_slot.item_id == ItemId::IRON_FISHING_ROD
                || active_slot.item_id == ItemId::DIAMOND_FISHING_ROD;

            let is_bow = active_slot.item_id == ItemId::BOW;
            let is_arrow = active_slot.item_id == ItemId::ARROW;

            let is_bucket = active_slot.item_id == ItemId::BUCKET
                || active_slot.item_id == ItemId::WATER_BUCKET
                || active_slot.item_id == ItemId::LAVA_BUCKET;

            let is_bread = active_slot.item_id == ItemId::BREAD;
            let is_apple = active_slot.item_id == ItemId::APPLE;

            if is_pickaxe {
                // -----------------------------------------------------------------
                // 3D HELD PICKAXE: 3D wooden handle and 3D arched crosshead blade
                // -----------------------------------------------------------------
                let pick_punch_x = -55.0 * scale * punch_swing;
                let pick_punch_y = 70.0 * scale * punch_swing;

                let handle_base = [
                    screen_w - 55.0 * scale + bob_x + pick_punch_x,
                    screen_h + 15.0 * scale + bob_y + pick_punch_y,
                ];
                let handle_tip = [
                    screen_w - 200.0 * scale + bob_x + pick_punch_x * 1.4,
                    screen_h - 220.0 * scale + bob_y + pick_punch_y * 1.4,
                ];

                let dx = handle_tip[0] - handle_base[0];
                let dy = handle_tip[1] - handle_base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                // 1. 3D Handle Front Face
                let h0 = [
                    handle_base[0] - norm[0] * half_w,
                    handle_base[1] - norm[1] * half_w,
                ];
                let h1 = [
                    handle_base[0] + norm[0] * half_w,
                    handle_base[1] + norm[1] * half_w,
                ];
                let h2 = [
                    handle_tip[0] + norm[0] * half_w,
                    handle_tip[1] + norm[1] * half_w,
                ];
                let h3 = [
                    handle_tip[0] - norm[0] * half_w,
                    handle_tip[1] - norm[1] * half_w,
                ];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);

                // 2. 3D Handle Side Thickness Face
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices(
                    [h0_b, h0, h3, h3_b],
                    white_uv,
                    [[0.38, 0.25, 0.13, 1.0]; 4],
                );

                // 3. 3D Pickaxe Head (Arched crosshead)
                let (col_head_main, col_head_top, col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_PICKAXE => (
                        [0.68, 0.48, 0.28, 1.0],
                        [0.85, 0.65, 0.42, 1.0],
                        [0.45, 0.30, 0.16, 1.0],
                    ),
                    ItemId::STONE_PICKAXE => (
                        [0.55, 0.55, 0.58, 1.0],
                        [0.75, 0.75, 0.78, 1.0],
                        [0.35, 0.35, 0.38, 1.0],
                    ),
                    ItemId::COPPER_PICKAXE => (
                        [0.85, 0.50, 0.32, 1.0],
                        [0.96, 0.68, 0.48, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_PICKAXE => (
                        [0.95, 0.80, 0.20, 1.0],
                        [1.00, 0.95, 0.45, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND_PICKAXE => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => (
                        // Iron Pickaxe
                        [0.82, 0.85, 0.90, 1.0],
                        [0.96, 0.98, 1.00, 1.0],
                        [0.55, 0.58, 0.65, 1.0],
                    ),
                };

                let head_center = handle_tip;
                let blade_len = 70.0 * scale;
                let blade_w = 14.0 * scale;

                let tip_l = [
                    head_center[0] - norm[0] * blade_len + dir[0] * 22.0 * scale,
                    head_center[1] - norm[1] * blade_len + dir[1] * 22.0 * scale,
                ];
                let tip_r = [
                    head_center[0] + norm[0] * blade_len - dir[0] * 22.0 * scale,
                    head_center[1] + norm[1] * blade_len - dir[1] * 22.0 * scale,
                ];
                let head_top = [
                    head_center[0] + dir[0] * blade_w,
                    head_center[1] + dir[1] * blade_w,
                ];
                let head_bot = [
                    head_center[0] - dir[0] * blade_w,
                    head_center[1] - dir[1] * blade_w,
                ];

                // Left Blade Quad
                let tip_l_in = [
                    tip_l[0] - dir[0] * 8.0 * scale,
                    tip_l[1] - dir[1] * 8.0 * scale,
                ];
                mesh.add_quad_vertices(
                    [head_bot, tip_l_in, tip_l, head_top],
                    white_uv,
                    [col_head_main; 4],
                );

                // Right Blade Quad
                let tip_r_in = [
                    tip_r[0] - dir[0] * 8.0 * scale,
                    tip_r[1] - dir[1] * 8.0 * scale,
                ];
                mesh.add_quad_vertices(
                    [head_bot, head_top, tip_r, tip_r_in],
                    white_uv,
                    [col_head_main; 4],
                );

                // 3D Top Spine Bevel (Top Highlight)
                let head_top_b = [head_top[0] + thick[0], head_top[1] + thick[1]];
                let tip_l_b = [tip_l[0] + thick[0], tip_l[1] + thick[1]];
                let tip_r_b = [tip_r[0] + thick[0], tip_r[1] + thick[1]];
                mesh.add_quad_vertices(
                    [tip_l, head_top, head_top_b, tip_l_b],
                    white_uv,
                    [col_head_top; 4],
                );
                mesh.add_quad_vertices(
                    [head_top, tip_r, tip_r_b, head_top_b],
                    white_uv,
                    [col_head_top; 4],
                );

                // 3D Underside Bevel (Side Shadow)
                let head_bot_b = [head_bot[0] + thick[0], head_bot[1] + thick[1]];
                let tip_l_in_b = [tip_l_in[0] + thick[0], tip_l_in[1] + thick[1]];
                mesh.add_quad_vertices(
                    [tip_l_in_b, head_bot_b, head_bot, tip_l_in],
                    white_uv,
                    [col_head_side; 4],
                );

                // Center Socket Collar (Dark steel/band)
                let c0 = [
                    head_center[0] - norm[0] * 14.0 * scale - dir[0] * 12.0 * scale,
                    head_center[1] - norm[1] * 14.0 * scale - dir[1] * 12.0 * scale,
                ];
                let c1 = [
                    head_center[0] + norm[0] * 14.0 * scale - dir[0] * 12.0 * scale,
                    head_center[1] + norm[0] * 14.0 * scale - dir[1] * 12.0 * scale,
                ];
                let c2 = [
                    head_center[0] + norm[0] * 14.0 * scale + dir[0] * 12.0 * scale,
                    head_center[1] + norm[0] * 14.0 * scale + dir[1] * 12.0 * scale,
                ];
                let c3 = [
                    head_center[0] - norm[0] * 14.0 * scale + dir[0] * 12.0 * scale,
                    head_center[1] - norm[0] * 14.0 * scale + dir[1] * 12.0 * scale,
                ];
                mesh.add_quad_vertices([c0, c1, c2, c3], white_uv, [[0.30, 0.30, 0.34, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_hoe {
                // -----------------------------------------------------------------
                // 3D HELD HOE: 3D wooden handle and angled hoe blade
                // -----------------------------------------------------------------
                let hoe_punch_x = -55.0 * scale * punch_swing;
                let hoe_punch_y = 70.0 * scale * punch_swing;

                let handle_base = [
                    screen_w - 55.0 * scale + bob_x + hoe_punch_x,
                    screen_h + 15.0 * scale + bob_y + hoe_punch_y,
                ];
                let handle_tip = [
                    screen_w - 200.0 * scale + bob_x + hoe_punch_x * 1.4,
                    screen_h - 220.0 * scale + bob_y + hoe_punch_y * 1.4,
                ];

                let dx = handle_tip[0] - handle_base[0];
                let dy = handle_tip[1] - handle_base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                // 1. 3D Handle Front Face
                let h0 = [
                    handle_base[0] - norm[0] * half_w,
                    handle_base[1] - norm[1] * half_w,
                ];
                let h1 = [
                    handle_base[0] + norm[0] * half_w,
                    handle_base[1] + norm[1] * half_w,
                ];
                let h2 = [
                    handle_tip[0] + norm[0] * half_w,
                    handle_tip[1] + norm[1] * half_w,
                ];
                let h3 = [
                    handle_tip[0] - norm[0] * half_w,
                    handle_tip[1] - norm[1] * half_w,
                ];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);

                // 2. 3D Handle Side Thickness Face
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices(
                    [h0_b, h0, h3, h3_b],
                    white_uv,
                    [[0.38, 0.25, 0.13, 1.0]; 4],
                );

                // 3. 3D Hoe Blade (angled forward)
                let (col_head_main, col_head_top, col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_HOE => (
                        [0.68, 0.48, 0.28, 1.0],
                        [0.85, 0.65, 0.42, 1.0],
                        [0.45, 0.30, 0.16, 1.0],
                    ),
                    ItemId::STONE_HOE => (
                        [0.55, 0.55, 0.58, 1.0],
                        [0.75, 0.75, 0.78, 1.0],
                        [0.35, 0.35, 0.38, 1.0],
                    ),
                    ItemId::COPPER_HOE => (
                        [0.85, 0.50, 0.32, 1.0],
                        [0.96, 0.68, 0.48, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_HOE => (
                        [0.95, 0.80, 0.20, 1.0],
                        [1.00, 0.95, 0.45, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND_HOE => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => (
                        // Iron Hoe
                        [0.82, 0.85, 0.90, 1.0],
                        [0.96, 0.98, 1.00, 1.0],
                        [0.55, 0.58, 0.65, 1.0],
                    ),
                };

                let head_center = handle_tip;
                let blade_len = 55.0 * scale;
                let blade_w = 15.0 * scale;

                let tip_l = [
                    head_center[0] - norm[0] * blade_len + dir[0] * 8.0 * scale,
                    head_center[1] - norm[1] * blade_len + dir[1] * 8.0 * scale,
                ];
                let head_top = [
                    head_center[0] + dir[0] * blade_w,
                    head_center[1] + dir[1] * blade_w,
                ];
                let head_bot = [
                    head_center[0] - dir[0] * blade_w,
                    head_center[1] - dir[1] * blade_w,
                ];
                let tip_l_in = [
                    tip_l[0] - dir[0] * 8.0 * scale,
                    tip_l[1] - dir[1] * 8.0 * scale,
                ];

                mesh.add_quad_vertices(
                    [head_bot, tip_l_in, tip_l, head_top],
                    white_uv,
                    [col_head_main; 4],
                );

                let head_top_b = [head_top[0] + thick[0], head_top[1] + thick[1]];
                let tip_l_b = [tip_l[0] + thick[0], tip_l[1] + thick[1]];
                mesh.add_quad_vertices(
                    [tip_l, head_top, head_top_b, tip_l_b],
                    white_uv,
                    [col_head_top; 4],
                );

                let head_bot_b = [head_bot[0] + thick[0], head_bot[1] + thick[1]];
                let tip_l_in_b = [tip_l_in[0] + thick[0], tip_l_in[1] + thick[1]];
                mesh.add_quad_vertices(
                    [tip_l_in_b, head_bot_b, head_bot, tip_l_in],
                    white_uv,
                    [col_head_side; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_axe {
                // -----------------------------------------------------------------
                // 3D HELD AXE: 3D wooden handle and broad axe head
                // -----------------------------------------------------------------
                let axe_punch_x = -55.0 * scale * punch_swing;
                let axe_punch_y = 70.0 * scale * punch_swing;

                let handle_base = [
                    screen_w - 55.0 * scale + bob_x + axe_punch_x,
                    screen_h + 15.0 * scale + bob_y + axe_punch_y,
                ];
                let handle_tip = [
                    screen_w - 200.0 * scale + bob_x + axe_punch_x * 1.4,
                    screen_h - 220.0 * scale + bob_y + axe_punch_y * 1.4,
                ];

                let dx = handle_tip[0] - handle_base[0];
                let dy = handle_tip[1] - handle_base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                let h0 = [
                    handle_base[0] - norm[0] * half_w,
                    handle_base[1] - norm[1] * half_w,
                ];
                let h1 = [
                    handle_base[0] + norm[0] * half_w,
                    handle_base[1] + norm[1] * half_w,
                ];
                let h2 = [
                    handle_tip[0] + norm[0] * half_w,
                    handle_tip[1] + norm[1] * half_w,
                ];
                let h3 = [
                    handle_tip[0] - norm[0] * half_w,
                    handle_tip[1] - norm[1] * half_w,
                ];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices(
                    [h0_b, h0, h3, h3_b],
                    white_uv,
                    [[0.38, 0.25, 0.13, 1.0]; 4],
                );

                let (col_head_main, col_head_top, _col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_AXE => (
                        [0.68, 0.48, 0.28, 1.0],
                        [0.85, 0.65, 0.42, 1.0],
                        [0.45, 0.30, 0.16, 1.0],
                    ),
                    ItemId::STONE_AXE => (
                        [0.55, 0.55, 0.58, 1.0],
                        [0.75, 0.75, 0.78, 1.0],
                        [0.35, 0.35, 0.38, 1.0],
                    ),
                    ItemId::COPPER_AXE => (
                        [0.85, 0.50, 0.32, 1.0],
                        [0.96, 0.68, 0.48, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_AXE => (
                        [0.95, 0.80, 0.20, 1.0],
                        [1.00, 0.95, 0.45, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND_AXE => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => (
                        [0.82, 0.85, 0.90, 1.0],
                        [0.96, 0.98, 1.00, 1.0],
                        [0.55, 0.58, 0.65, 1.0],
                    ),
                };

                let head_center = handle_tip;
                let blade_len = 50.0 * scale;
                let p_top_tip = [
                    head_center[0] - norm[0] * blade_len + dir[0] * 20.0 * scale,
                    head_center[1] - norm[1] * blade_len + dir[1] * 20.0 * scale,
                ];
                let p_bot_tip = [
                    head_center[0] - norm[0] * (blade_len - 10.0 * scale) - dir[0] * 35.0 * scale,
                    head_center[1] - norm[1] * (blade_len - 10.0 * scale) - dir[1] * 35.0 * scale,
                ];
                let p_top_back = [
                    head_center[0] + norm[0] * 15.0 * scale + dir[0] * 12.0 * scale,
                    head_center[1] + norm[0] * 15.0 * scale + dir[1] * 12.0 * scale,
                ];
                let p_bot_back = [
                    head_center[0] + norm[0] * 15.0 * scale - dir[0] * 20.0 * scale,
                    head_center[1] + norm[0] * 15.0 * scale - dir[1] * 20.0 * scale,
                ];

                mesh.add_quad_vertices(
                    [p_bot_tip, p_bot_back, p_top_back, p_top_tip],
                    white_uv,
                    [col_head_main; 4],
                );
                let p_top_tip_b = [p_top_tip[0] + thick[0], p_top_tip[1] + thick[1]];
                let p_top_back_b = [p_top_back[0] + thick[0], p_top_back[1] + thick[1]];
                mesh.add_quad_vertices(
                    [p_top_tip, p_top_back, p_top_back_b, p_top_tip_b],
                    white_uv,
                    [col_head_top; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_shovel {
                // -----------------------------------------------------------------
                // 3D HELD SHOVEL: 3D wooden handle and spade blade
                // -----------------------------------------------------------------
                let shovel_punch_x = -55.0 * scale * punch_swing;
                let shovel_punch_y = 70.0 * scale * punch_swing;

                let handle_base = [
                    screen_w - 55.0 * scale + bob_x + shovel_punch_x,
                    screen_h + 15.0 * scale + bob_y + shovel_punch_y,
                ];
                let handle_tip = [
                    screen_w - 200.0 * scale + bob_x + shovel_punch_x * 1.4,
                    screen_h - 220.0 * scale + bob_y + shovel_punch_y * 1.4,
                ];

                let dx = handle_tip[0] - handle_base[0];
                let dy = handle_tip[1] - handle_base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                let h0 = [
                    handle_base[0] - norm[0] * half_w,
                    handle_base[1] - norm[1] * half_w,
                ];
                let h1 = [
                    handle_base[0] + norm[0] * half_w,
                    handle_base[1] + norm[1] * half_w,
                ];
                let h2 = [
                    handle_tip[0] + norm[0] * half_w,
                    handle_tip[1] + norm[1] * half_w,
                ];
                let h3 = [
                    handle_tip[0] - norm[0] * half_w,
                    handle_tip[1] - norm[1] * half_w,
                ];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices(
                    [h0_b, h0, h3, h3_b],
                    white_uv,
                    [[0.38, 0.25, 0.13, 1.0]; 4],
                );

                let (col_head_main, col_head_top, _col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_SHOVEL => (
                        [0.68, 0.48, 0.28, 1.0],
                        [0.85, 0.65, 0.42, 1.0],
                        [0.45, 0.30, 0.16, 1.0],
                    ),
                    ItemId::STONE_SHOVEL => (
                        [0.55, 0.55, 0.58, 1.0],
                        [0.75, 0.75, 0.78, 1.0],
                        [0.35, 0.35, 0.38, 1.0],
                    ),
                    ItemId::COPPER_SHOVEL => (
                        [0.85, 0.50, 0.32, 1.0],
                        [0.96, 0.68, 0.48, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_SHOVEL => (
                        [0.95, 0.80, 0.20, 1.0],
                        [1.00, 0.95, 0.45, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND_SHOVEL => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => (
                        [0.82, 0.85, 0.90, 1.0],
                        [0.96, 0.98, 1.00, 1.0],
                        [0.55, 0.58, 0.65, 1.0],
                    ),
                };

                let blade_w = 20.0 * scale;
                let blade_len = 50.0 * scale;
                let b_base_l = [
                    handle_tip[0] - norm[0] * blade_w,
                    handle_tip[1] - norm[1] * blade_w,
                ];
                let b_base_r = [
                    handle_tip[0] + norm[0] * blade_w,
                    handle_tip[1] + norm[1] * blade_w,
                ];
                let b_tip_l = [
                    b_base_l[0] + dir[0] * blade_len,
                    b_base_l[1] + dir[1] * blade_len,
                ];
                let b_tip_r = [
                    b_base_r[0] + dir[0] * blade_len,
                    b_base_r[1] + dir[1] * blade_len,
                ];

                mesh.add_quad_vertices(
                    [b_base_l, b_base_r, b_tip_r, b_tip_l],
                    white_uv,
                    [col_head_main; 4],
                );
                let b_tip_l_b = [b_tip_l[0] + thick[0], b_tip_l[1] + thick[1]];
                let b_tip_r_b = [b_tip_r[0] + thick[0], b_tip_r[1] + thick[1]];
                mesh.add_quad_vertices(
                    [b_tip_l, b_tip_r, b_tip_r_b, b_tip_l_b],
                    white_uv,
                    [col_head_top; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_sword {
                // -----------------------------------------------------------------
                // 3D HELD SWORD: 3D hilt, crossguard, and double-edged blade
                // -----------------------------------------------------------------
                let sword_punch_x = -55.0 * scale * punch_swing;
                let sword_punch_y = 70.0 * scale * punch_swing;

                let handle_base = [
                    screen_w - 55.0 * scale + bob_x + sword_punch_x,
                    screen_h + 15.0 * scale + bob_y + sword_punch_y,
                ];
                let handle_tip = [
                    screen_w - 145.0 * scale + bob_x + sword_punch_x * 1.3,
                    screen_h - 120.0 * scale + bob_y + sword_punch_y * 1.3,
                ];

                let dx = handle_tip[0] - handle_base[0];
                let dy = handle_tip[1] - handle_base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 7.0 * scale;
                let thick = [-8.0 * scale, -6.0 * scale];

                // 1. Grip / Handle
                let h0 = [handle_base[0] - norm[0] * half_w, handle_base[1] - norm[1] * half_w];
                let h1 = [handle_base[0] + norm[0] * half_w, handle_base[1] + norm[1] * half_w];
                let h2 = [handle_tip[0] + norm[0] * half_w, handle_tip[1] + norm[1] * half_w];
                let h3 = [handle_tip[0] - norm[0] * half_w, handle_tip[1] - norm[1] * half_w];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.45, 0.30, 0.16, 1.0]; 4]);
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices([h0_b, h0, h3, h3_b], white_uv, [[0.28, 0.18, 0.10, 1.0]; 4]);

                let (col_blade_main, col_blade_top, col_blade_side) = match active_slot.item_id {
                    ItemId::WOODEN_SWORD => (
                        [0.68, 0.48, 0.28, 1.0],
                        [0.85, 0.65, 0.42, 1.0],
                        [0.45, 0.30, 0.16, 1.0],
                    ),
                    ItemId::STONE_SWORD => (
                        [0.55, 0.55, 0.58, 1.0],
                        [0.75, 0.75, 0.78, 1.0],
                        [0.35, 0.35, 0.38, 1.0],
                    ),
                    ItemId::COPPER_SWORD => (
                        [0.85, 0.50, 0.32, 1.0],
                        [0.96, 0.68, 0.48, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_SWORD => (
                        [0.95, 0.80, 0.20, 1.0],
                        [1.00, 0.95, 0.45, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND_SWORD => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => (
                        // Iron Sword
                        [0.82, 0.85, 0.90, 1.0],
                        [0.96, 0.98, 1.00, 1.0],
                        [0.55, 0.58, 0.65, 1.0],
                    ),
                };

                // 2. Crossguard
                let guard_w = 28.0 * scale;
                let guard_h = 8.0 * scale;
                let guard_c = handle_tip;
                let gl = [guard_c[0] - norm[0] * guard_w, guard_c[1] - norm[1] * guard_w];
                let gr = [guard_c[0] + norm[0] * guard_w, guard_c[1] + norm[1] * guard_w];
                let g0 = [gl[0] - dir[0] * guard_h, gl[1] - dir[1] * guard_h];
                let g1 = [gr[0] - dir[0] * guard_h, gr[1] - dir[1] * guard_h];
                let g2 = [gr[0] + dir[0] * guard_h, gr[1] + dir[1] * guard_h];
                let g3 = [gl[0] + dir[0] * guard_h, gl[1] + dir[1] * guard_h];
                mesh.add_quad_vertices([g0, g1, g2, g3], white_uv, [col_blade_top; 4]);
                let g2_b = [g2[0] + thick[0], g2[1] + thick[1]];
                let g3_b = [g3[0] + thick[0], g3[1] + thick[1]];
                mesh.add_quad_vertices([g3, g2, g2_b, g3_b], white_uv, [col_blade_side; 4]);

                // 3. Blade & Point
                let b_base = [guard_c[0] + dir[0] * guard_h, guard_c[1] + dir[1] * guard_h];
                let blade_len = 160.0 * scale;
                let blade_w = 13.0 * scale;
                let b_tip = [b_base[0] + dir[0] * blade_len, b_base[1] + dir[1] * blade_len];
                let b_taper = [
                    b_base[0] + dir[0] * (blade_len - 30.0 * scale),
                    b_base[1] + dir[1] * (blade_len - 30.0 * scale),
                ];

                let bl0 = [b_base[0] - norm[0] * blade_w, b_base[1] - norm[1] * blade_w];
                let br0 = [b_base[0] + norm[0] * blade_w, b_base[1] + norm[1] * blade_w];
                let bl1 = [b_taper[0] - norm[0] * blade_w, b_taper[1] - norm[1] * blade_w];
                let br1 = [b_taper[0] + norm[0] * blade_w, b_taper[1] + norm[1] * blade_w];

                // Main Blade Quad
                mesh.add_quad_vertices([bl0, br0, br1, bl1], white_uv, [col_blade_main; 4]);
                // Point Triangle
                mesh.add_quad_vertices([bl1, br1, b_tip, b_tip], white_uv, [col_blade_top; 4]);

                // 3D Side Thickness
                let bl0_b = [bl0[0] + thick[0], bl0[1] + thick[1]];
                let bl1_b = [bl1[0] + thick[0], bl1[1] + thick[1]];
                let b_tip_b = [b_tip[0] + thick[0], b_tip[1] + thick[1]];
                mesh.add_quad_vertices([bl0_b, bl0, bl1, bl1_b], white_uv, [col_blade_side; 4]);
                mesh.add_quad_vertices([bl1_b, bl1, b_tip, b_tip_b], white_uv, [col_blade_side; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if active_slot.item_id == ItemId::STICK {
                // -----------------------------------------------------------------
                // 3D HELD STICK: 3D carved wooden shaft with top and side depth
                // -----------------------------------------------------------------
                let stick_punch_x = -50.0 * scale * punch_swing;
                let stick_punch_y = 60.0 * scale * punch_swing;

                let base = [
                    screen_w - 55.0 * scale + bob_x + stick_punch_x,
                    screen_h + 15.0 * scale + bob_y + stick_punch_y,
                ];
                let tip = [
                    screen_w - 200.0 * scale + bob_x + stick_punch_x * 1.4,
                    screen_h - 220.0 * scale + bob_y + stick_punch_y * 1.4,
                ];

                let dx = tip[0] - base[0];
                let dy = tip[1] - base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let half_w = 9.0 * scale;
                let thick = [-9.0 * scale, -7.0 * scale];

                let s0 = [base[0] - norm[0] * half_w, base[1] - norm[1] * half_w];
                let s1 = [base[0] + norm[0] * half_w, base[1] + norm[1] * half_w];
                let s2 = [tip[0] + norm[0] * half_w, tip[1] + norm[1] * half_w];
                let s3 = [tip[0] - norm[0] * half_w, tip[1] - norm[1] * half_w];

                // 1. Stick Front Face
                mesh.add_quad_vertices([s0, s1, s2, s3], white_uv, [[0.62, 0.44, 0.25, 1.0]; 4]);

                // 2. Stick Side 3D Thickness Face
                let s0_b = [s0[0] + thick[0], s0[1] + thick[1]];
                let s3_b = [s3[0] + thick[0], s3[1] + thick[1]];
                mesh.add_quad_vertices(
                    [s0_b, s0, s3, s3_b],
                    white_uv,
                    [[0.40, 0.26, 0.14, 1.0]; 4],
                );

                // 3. Stick Top Face (cut end cap)
                let s2_b = [s2[0] + thick[0], s2[1] + thick[1]];
                mesh.add_quad_vertices(
                    [s3, s2, s2_b, s3_b],
                    white_uv,
                    [[0.75, 0.55, 0.34, 1.0]; 4],
                );

                // 4. Carved Notches along the shaft
                for i in 1..=3 {
                    let t = i as f32 * 0.25;
                    let nx = base[0] + dir[0] * len * t;
                    let ny = base[1] + dir[1] * len * t;
                    let n0 = [nx - norm[0] * half_w, ny - norm[1] * half_w];
                    let n1 = [nx + norm[0] * half_w, ny + norm[1] * half_w];
                    let n2 = [n1[0] + dir[0] * 4.0 * scale, n1[1] + dir[1] * 4.0 * scale];
                    let n3 = [n0[0] + dir[0] * 4.0 * scale, n0[1] + dir[1] * 4.0 * scale];
                    mesh.add_quad_vertices(
                        [n0, n1, n2, n3],
                        white_uv,
                        [[0.45, 0.30, 0.16, 1.0]; 4],
                    );
                }

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if active_slot.item_id == ItemId::IRON_INGOT
                || active_slot.item_id == ItemId::COPPER_INGOT
                || active_slot.item_id == ItemId::GOLD_INGOT
            {
                // -----------------------------------------------------------------
                // 3D HELD INGOT: 3D metallic ingot bar with specular top face
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let ic = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 120.0 * scale + bob_y + item_punch_y,
                ];
                let iw = 65.0 * scale;
                let ih = 35.0 * scale;
                let id = 45.0 * scale;

                let (col_top, col_front, col_side) = match active_slot.item_id {
                    ItemId::COPPER_INGOT => (
                        [0.96, 0.68, 0.48, 1.0],
                        [0.85, 0.50, 0.32, 1.0],
                        [0.65, 0.35, 0.20, 1.0],
                    ),
                    ItemId::GOLD_INGOT => (
                        [1.00, 0.95, 0.45, 1.0],
                        [0.95, 0.80, 0.20, 1.0],
                        [0.72, 0.58, 0.12, 1.0],
                    ),
                    _ => (
                        // Iron Ingot
                        [0.94, 0.96, 1.00, 1.0],
                        [0.78, 0.81, 0.88, 1.0],
                        [0.60, 0.62, 0.70, 1.0],
                    ),
                };

                // Top Face (trapezoid with brilliant metallic sheen)
                let t_nw = [ic[0] - iw * 0.70, ic[1] - id * 0.60];
                let t_ne = [ic[0] + iw * 0.70, ic[1] - id * 0.60];
                let t_se = [ic[0] + iw, ic[1] + id * 0.20];
                let t_sw = [ic[0] - iw, ic[1] + id * 0.20];
                mesh.add_quad_vertices([t_sw, t_nw, t_ne, t_se], white_uv, [col_top; 4]);

                // Front Face
                let f_bl = [t_sw[0], t_sw[1] + ih];
                let f_br = [t_se[0], t_se[1] + ih];
                mesh.add_quad_vertices([t_sw, t_se, f_br, f_bl], white_uv, [col_front; 4]);

                // Right Face
                let r_br = [t_ne[0], t_ne[1] + ih];
                mesh.add_quad_vertices([t_se, t_ne, r_br, f_br], white_uv, [col_side; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        ic[0] + 15.0 * scale,
                        ic[1] + ih * 0.5,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if active_slot.item_id == ItemId::COAL
                || active_slot.item_id == ItemId::RAW_IRON
                || active_slot.item_id == ItemId::RAW_COPPER
                || active_slot.item_id == ItemId::RAW_GOLD
                || active_slot.item_id == ItemId::DIAMOND
            {
                // -----------------------------------------------------------------
                // 3D HELD MINERAL / CHUNK: 3D faceted mineral chunk with specular highlights
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let cc = [
                    screen_w - 145.0 * scale + bob_x + item_punch_x,
                    screen_h - 120.0 * scale + bob_y + item_punch_y,
                ];
                let cs = 45.0 * scale;

                let (col_f1, col_f2, col_f3) = match active_slot.item_id {
                    ItemId::RAW_IRON => (
                        [0.80, 0.72, 0.64, 1.0],
                        [0.68, 0.58, 0.48, 1.0],
                        [0.55, 0.45, 0.35, 1.0],
                    ),
                    ItemId::RAW_COPPER => (
                        [0.75, 0.55, 0.42, 1.0],
                        [0.62, 0.42, 0.30, 1.0],
                        [0.45, 0.55, 0.50, 1.0],
                    ),
                    ItemId::RAW_GOLD => (
                        [0.98, 0.88, 0.40, 1.0],
                        [0.88, 0.72, 0.20, 1.0],
                        [0.68, 0.52, 0.12, 1.0],
                    ),
                    ItemId::DIAMOND => (
                        [0.65, 0.98, 1.00, 1.0],
                        [0.32, 0.88, 0.92, 1.0],
                        [0.18, 0.65, 0.72, 1.0],
                    ),
                    _ => (
                        // Coal
                        [0.34, 0.34, 0.38, 1.0],
                        [0.22, 0.22, 0.26, 1.0],
                        [0.16, 0.16, 0.19, 1.0],
                    ),
                };

                // Facet 1: Top facet (specular highlight)
                let c_top = [cc[0], cc[1] - cs * 0.90];
                let c_tl = [cc[0] - cs * 0.75, cc[1] - cs * 0.30];
                let c_tr = [cc[0] + cs * 0.75, cc[1] - cs * 0.30];
                let c_mid = [cc[0] + cs * 0.10, cc[1] + cs * 0.10];
                mesh.add_quad_vertices([c_tl, c_top, c_tr, c_mid], white_uv, [col_f1; 4]);

                // Facet 2: Front-left facet
                let c_bl = [cc[0] - cs * 0.60, cc[1] + cs * 0.75];
                let c_bm = [cc[0], cc[1] + cs * 0.95];
                mesh.add_quad_vertices([c_tl, c_mid, c_bm, c_bl], white_uv, [col_f2; 4]);

                // Facet 3: Front-right facet
                let c_br = [cc[0] + cs * 0.70, cc[1] + cs * 0.65];
                mesh.add_quad_vertices([c_mid, c_tr, c_br, c_bm], white_uv, [col_f3; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        cc[0] + 15.0 * scale,
                        cc[1] + cs * 0.40,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_fishing_rod {
                // -----------------------------------------------------------------
                // 3D HELD FISHING ROD: 3D tapered pole, grip, reel drum, guides, line, & bobber
                // -----------------------------------------------------------------
                let rod_punch_x = -60.0 * scale * punch_swing;
                let rod_punch_y = 75.0 * scale * punch_swing;

                let base = [
                    screen_w - 45.0 * scale + bob_x + rod_punch_x,
                    screen_h + 20.0 * scale + bob_y + rod_punch_y,
                ];
                let tip = [
                    screen_w - 225.0 * scale + bob_x + rod_punch_x * 1.5,
                    screen_h - 250.0 * scale + bob_y + rod_punch_y * 1.5,
                ];

                let dx = tip[0] - base[0];
                let dy = tip[1] - base[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let base_w = 8.5 * scale;
                let tip_w = 4.0 * scale;
                let thick = [-8.0 * scale, -6.0 * scale];

                let (col_rod, col_side) = match active_slot.item_id {
                    ItemId::COPPER_FISHING_ROD => (
                        [0.85, 0.55, 0.38, 1.0],
                        [0.60, 0.38, 0.24, 1.0],
                    ),
                    ItemId::IRON_FISHING_ROD => (
                        [0.85, 0.88, 0.92, 1.0],
                        [0.60, 0.63, 0.68, 1.0],
                    ),
                    ItemId::DIAMOND_FISHING_ROD => (
                        [0.45, 0.92, 0.96, 1.0],
                        [0.22, 0.65, 0.72, 1.0],
                    ),
                    _ => (
                        // Wooden Fishing Rod
                        [0.65, 0.46, 0.28, 1.0],
                        [0.42, 0.28, 0.16, 1.0],
                    ),
                };

                // 1. Tapered Rod Pole Front Face
                let p0 = [base[0] - norm[0] * base_w, base[1] - norm[1] * base_w];
                let p1 = [base[0] + norm[0] * base_w, base[1] + norm[1] * base_w];
                let p2 = [tip[0] + norm[0] * tip_w, tip[1] + norm[1] * tip_w];
                let p3 = [tip[0] - norm[0] * tip_w, tip[1] - norm[1] * tip_w];
                mesh.add_quad_vertices([p0, p1, p2, p3], white_uv, [col_rod; 4]);

                // 2. Rod Side Thickness Face
                let p0_b = [p0[0] + thick[0], p0[1] + thick[1]];
                let p3_b = [p3[0] + thick[0], p3[1] + thick[1]];
                mesh.add_quad_vertices([p0_b, p0, p3, p3_b], white_uv, [col_side; 4]);

                // 3. Cork / Leather Grip at handle base
                let grip_len = len * 0.28;
                let g_tip = [base[0] + dir[0] * grip_len, base[1] + dir[1] * grip_len];
                let g_half_w = base_w + 3.0 * scale;
                let g0 = [base[0] - norm[0] * g_half_w, base[1] - norm[1] * g_half_w];
                let g1 = [base[0] + norm[0] * g_half_w, base[1] + norm[1] * g_half_w];
                let g2 = [g_tip[0] + norm[0] * g_half_w, g_tip[1] + norm[1] * g_half_w];
                let g3 = [g_tip[0] - norm[0] * g_half_w, g_tip[1] - norm[1] * g_half_w];
                mesh.add_quad_vertices([g0, g1, g2, g3], white_uv, [[0.36, 0.24, 0.14, 1.0]; 4]);

                // 4. Reel Drum
                let reel_c = [
                    base[0] + dir[0] * (grip_len + 8.0 * scale) - norm[0] * 12.0 * scale,
                    base[1] + dir[1] * (grip_len + 8.0 * scale) - norm[1] * 12.0 * scale,
                ];
                let rw = 12.0 * scale;
                mesh.add_quad_vertices(
                    [
                        [reel_c[0] - rw, reel_c[1] - rw],
                        [reel_c[0] + rw, reel_c[1] - rw],
                        [reel_c[0] + rw, reel_c[1] + rw],
                        [reel_c[0] - rw, reel_c[1] + rw],
                    ],
                    white_uv,
                    [[0.25, 0.25, 0.28, 1.0]; 4],
                );

                // 5. Fishing Line (from rod tip hanging down)
                let line_end = [tip[0] + 35.0 * scale, tip[1] + 120.0 * scale];
                let lw = 1.8 * scale;
                let l_dir_x = line_end[0] - tip[0];
                let l_dir_y = line_end[1] - tip[1];
                let l_len = (l_dir_x * l_dir_x + l_dir_y * l_dir_y).sqrt().max(0.001);
                let l_norm = [-l_dir_y / l_len * lw, l_dir_x / l_len * lw];
                mesh.add_quad_vertices(
                    [
                        [tip[0] - l_norm[0], tip[1] - l_norm[1]],
                        [tip[0] + l_norm[0], tip[1] + l_norm[1]],
                        [line_end[0] + l_norm[0], line_end[1] + l_norm[1]],
                        [line_end[0] - l_norm[0], line_end[1] - l_norm[1]],
                    ],
                    white_uv,
                    [[0.92, 0.94, 0.98, 0.90]; 4],
                );

                // 6. 3D Bobber (Red top, White bottom)
                let bs_bob = 13.0 * scale;
                let bob_c = line_end;
                // Red top half
                mesh.add_quad_vertices(
                    [
                        [bob_c[0] - bs_bob, bob_c[1]],
                        [bob_c[0] + bs_bob, bob_c[1]],
                        [bob_c[0] + bs_bob * 0.7, bob_c[1] - bs_bob],
                        [bob_c[0] - bs_bob * 0.7, bob_c[1] - bs_bob],
                    ],
                    white_uv,
                    [[0.90, 0.18, 0.18, 1.0]; 4],
                );
                // White bottom half
                mesh.add_quad_vertices(
                    [
                        [bob_c[0] - bs_bob * 0.7, bob_c[1] + bs_bob],
                        [bob_c[0] + bs_bob * 0.7, bob_c[1] + bs_bob],
                        [bob_c[0] + bs_bob, bob_c[1]],
                        [bob_c[0] - bs_bob, bob_c[1]],
                    ],
                    white_uv,
                    [[0.95, 0.95, 0.98, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_bow {
                // -----------------------------------------------------------------
                // 3D HELD BOW: 3D curved wooden stave, grip wrap, and taut bowstring
                // -----------------------------------------------------------------
                let bow_punch_x = -45.0 * scale * punch_swing;
                let bow_punch_y = 45.0 * scale * punch_swing;

                let grip = [
                    screen_w - 110.0 * scale + bob_x + bow_punch_x,
                    screen_h - 110.0 * scale + bob_y + bow_punch_y,
                ];
                let nock_top = [grip[0] - 85.0 * scale, grip[1] - 120.0 * scale];
                let nock_bot = [grip[0] + 45.0 * scale, grip[1] + 115.0 * scale];

                let stave_w = 8.0 * scale;
                let thick = [-7.0 * scale, -5.0 * scale];

                // Upper curved limb (two segments)
                let mid_top = [(grip[0] + nock_top[0]) * 0.5 - 20.0 * scale, (grip[1] + nock_top[1]) * 0.5 - 10.0 * scale];
                mesh.add_quad_vertices(
                    [[grip[0] - stave_w, grip[1]], [grip[0] + stave_w, grip[1]], [mid_top[0] + stave_w, mid_top[1]], [mid_top[0] - stave_w, mid_top[1]]],
                    white_uv,
                    [[0.60, 0.42, 0.24, 1.0]; 4],
                );
                mesh.add_quad_vertices(
                    [[mid_top[0] - stave_w, mid_top[1]], [mid_top[0] + stave_w, mid_top[1]], [nock_top[0] + stave_w * 0.6, nock_top[1]], [nock_top[0] - stave_w * 0.6, nock_top[1]]],
                    white_uv,
                    [[0.55, 0.38, 0.20, 1.0]; 4],
                );

                // Lower curved limb (two segments)
                let mid_bot = [(grip[0] + nock_bot[0]) * 0.5 - 15.0 * scale, (grip[1] + nock_bot[1]) * 0.5 + 10.0 * scale];
                mesh.add_quad_vertices(
                    [[grip[0] - stave_w, grip[1]], [grip[0] + stave_w, grip[1]], [mid_bot[0] + stave_w, mid_bot[1]], [mid_bot[0] - stave_w, mid_bot[1]]],
                    white_uv,
                    [[0.60, 0.42, 0.24, 1.0]; 4],
                );
                mesh.add_quad_vertices(
                    [[mid_bot[0] - stave_w, mid_bot[1]], [mid_bot[0] + stave_w, mid_bot[1]], [nock_bot[0] + stave_w * 0.6, nock_bot[0]], [nock_bot[0] - stave_w * 0.6, nock_bot[1]]],
                    white_uv,
                    [[0.55, 0.38, 0.20, 1.0]; 4],
                );

                // Stave 3D Thickness along grip
                mesh.add_quad_vertices(
                    [[grip[0] - stave_w + thick[0], grip[1] + thick[1]], [grip[0] - stave_w, grip[1]], [mid_top[0] - stave_w, mid_top[1]], [mid_top[0] - stave_w + thick[0], mid_top[1] + thick[1]]],
                    white_uv,
                    [[0.38, 0.25, 0.14, 1.0]; 4],
                );

                // Center Grip Wrap
                mesh.add_quad_vertices(
                    [[grip[0] - stave_w * 1.3, grip[1] - 15.0 * scale], [grip[0] + stave_w * 1.3, grip[1] - 15.0 * scale], [grip[0] + stave_w * 1.3, grip[1] + 15.0 * scale], [grip[0] - stave_w * 1.3, grip[1] + 15.0 * scale]],
                    white_uv,
                    [[0.30, 0.20, 0.12, 1.0]; 4],
                );

                // Bowstring
                let str_w = 2.0 * scale;
                mesh.add_quad_vertices(
                    [[nock_top[0] - str_w, nock_top[1]], [nock_top[0] + str_w, nock_top[1]], [nock_bot[0] + str_w, nock_bot[1]], [nock_bot[0] - str_w, nock_bot[1]]],
                    white_uv,
                    [[0.95, 0.95, 0.95, 0.92]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_arrow {
                // -----------------------------------------------------------------
                // 3D HELD ARROW: 3D shaft, pointed arrowhead, and feather fletching
                // -----------------------------------------------------------------
                let arrow_punch_x = -45.0 * scale * punch_swing;
                let arrow_punch_y = 55.0 * scale * punch_swing;

                let nock = [
                    screen_w - 55.0 * scale + bob_x + arrow_punch_x,
                    screen_h + 15.0 * scale + bob_y + arrow_punch_y,
                ];
                let head = [
                    screen_w - 180.0 * scale + bob_x + arrow_punch_x * 1.4,
                    screen_h - 200.0 * scale + bob_y + arrow_punch_y * 1.4,
                ];

                let dx = head[0] - nock[0];
                let dy = head[1] - nock[1];
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let dir = [dx / len, dy / len];
                let norm = [-dir[1], dir[0]];

                let shaft_w = 5.0 * scale;
                let thick = [-6.0 * scale, -5.0 * scale];

                // Shaft
                let a0 = [nock[0] - norm[0] * shaft_w, nock[1] - norm[1] * shaft_w];
                let a1 = [nock[0] + norm[0] * shaft_w, nock[1] + norm[1] * shaft_w];
                let a2 = [head[0] + norm[0] * shaft_w, head[1] + norm[1] * shaft_w];
                let a3 = [head[0] - norm[0] * shaft_w, head[1] - norm[1] * shaft_w];
                mesh.add_quad_vertices([a0, a1, a2, a3], white_uv, [[0.65, 0.48, 0.30, 1.0]; 4]);
                let a0_b = [a0[0] + thick[0], a0[1] + thick[1]];
                let a3_b = [a3[0] + thick[0], a3[1] + thick[1]];
                mesh.add_quad_vertices([a0_b, a0, a3, a3_b], white_uv, [[0.40, 0.28, 0.16, 1.0]; 4]);

                // Flint / Iron Arrowhead (triangular tip)
                let tip_pt = [head[0] + dir[0] * 32.0 * scale, head[1] + dir[1] * 32.0 * scale];
                let hw = 16.0 * scale;
                let h_l = [head[0] - norm[0] * hw, head[1] - norm[1] * hw];
                let h_r = [head[0] + norm[0] * hw, head[1] + norm[1] * hw];
                mesh.add_quad_vertices([h_l, h_r, tip_pt, h_l], white_uv, [[0.68, 0.70, 0.74, 1.0]; 4]);

                // Feather Fletching Vanes
                let fw = 14.0 * scale;
                let flen = 35.0 * scale;
                let f_base = [nock[0] + dir[0] * 10.0 * scale, nock[1] + dir[1] * 10.0 * scale];
                let f_end = [f_base[0] + dir[0] * flen, f_base[1] + dir[1] * flen];
                mesh.add_quad_vertices(
                    [
                        [f_base[0] - norm[0] * fw, f_base[1] - norm[1] * fw],
                        [f_base[0] + norm[0] * fw, f_base[1] + norm[1] * fw],
                        [f_end[0] + norm[0] * shaft_w, f_end[1] + norm[1] * shaft_w],
                        [f_end[0] - norm[0] * shaft_w, f_end[1] - norm[1] * shaft_w],
                    ],
                    white_uv,
                    [[0.90, 0.90, 0.92, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        screen_w - 75.0 * scale,
                        screen_h - 45.0 * scale,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_bucket {
                // -----------------------------------------------------------------
                // 3D HELD BUCKET: 3D metallic iron bucket with arched bail & water surface
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 125.0 * scale + bob_x + item_punch_x,
                    screen_h - 115.0 * scale + bob_y + item_punch_y,
                ];
                let bw_top = 58.0 * scale;
                let bw_bot = 42.0 * scale;
                let bh = 65.0 * scale;

                // Rim Top Lip (diamond / ellipse)
                let r_n = [bc[0], bc[1] - bh * 0.5 - 14.0 * scale];
                let r_e = [bc[0] + bw_top, bc[1] - bh * 0.5];
                let r_s = [bc[0], bc[1] - bh * 0.5 + 14.0 * scale];
                let r_w = [bc[0] - bw_top, bc[1] - bh * 0.5];

                // Liquid interior surface if Water Bucket or Lava Bucket
                if active_slot.item_id == ItemId::WATER_BUCKET {
                    let w_uv = TextureAtlas::get_uv_bounds(38); // water tile
                    let water_uvs = [[w_uv[0], w_uv[1]], [w_uv[2], w_uv[1]], [w_uv[2], w_uv[3]], [w_uv[0], w_uv[3]]];
                    mesh.add_quad_vertices([r_w, r_n, r_e, r_s], water_uvs, [[0.22, 0.55, 0.92, 0.95]; 4]);
                } else if active_slot.item_id == ItemId::LAVA_BUCKET {
                    let l_uv = TextureAtlas::get_uv_bounds(579); // lava tile
                    let lava_uvs = [[l_uv[0], l_uv[1]], [l_uv[2], l_uv[1]], [l_uv[2], l_uv[3]], [l_uv[0], l_uv[3]]];
                    mesh.add_quad_vertices([r_w, r_n, r_e, r_s], lava_uvs, [[1.0, 0.72, 0.22, 1.0]; 4]);
                } else {
                    // Empty bucket dark interior
                    mesh.add_quad_vertices([r_w, r_n, r_e, r_s], white_uv, [[0.30, 0.32, 0.36, 1.0]; 4]);
                }

                // Bucket Outer Body: Left Facet & Right Facet (tapered)
                let b_bl = [bc[0] - bw_bot, bc[1] + bh * 0.5];
                let b_bm = [bc[0], bc[1] + bh * 0.5 + 10.0 * scale];
                let b_br = [bc[0] + bw_bot, bc[1] + bh * 0.5];

                // Left body facet
                mesh.add_quad_vertices([r_w, r_s, b_bm, b_bl], white_uv, [[0.76, 0.78, 0.82, 1.0]; 4]);
                // Right body facet
                mesh.add_quad_vertices([r_s, r_e, b_br, b_bm], white_uv, [[0.58, 0.60, 0.65, 1.0]; 4]);

                // Metallic Rim Highlight
                let rim_w = 4.0 * scale;
                mesh.add_quad_vertices(
                    [[r_w[0], r_w[1] - rim_w], [r_s[0], r_s[1] - rim_w], r_s, r_w],
                    white_uv,
                    [[0.90, 0.92, 0.96, 1.0]; 4],
                );
                mesh.add_quad_vertices(
                    [r_s, [r_s[0], r_s[1] - rim_w], [r_e[0], r_e[1] - rim_w], r_e],
                    white_uv,
                    [[0.72, 0.74, 0.78, 1.0]; 4],
                );

                // Arched Wire Handle / Bail
                let h_top = [bc[0] - 8.0 * scale, bc[1] - bh * 0.5 - 32.0 * scale];
                let hw = 3.0 * scale;
                mesh.add_quad_vertices(
                    [[r_w[0] - hw, r_w[1]], [r_w[0] + hw, r_w[1]], [h_top[0] + hw, h_top[1]], [h_top[0] - hw, h_top[1]]],
                    white_uv,
                    [[0.50, 0.52, 0.56, 1.0]; 4],
                );
                mesh.add_quad_vertices(
                    [[h_top[0] - hw, h_top[1]], [h_top[0] + hw, h_top[1]], [r_e[0] + hw, r_e[1]], [r_e[0] - hw, r_e[1]]],
                    white_uv,
                    [[0.40, 0.42, 0.46, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 15.0 * scale,
                        bc[1] + bh * 0.40,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_bread {
                // -----------------------------------------------------------------
                // 3D HELD BREAD: 3D golden loaf with scoring cuts & crust bevels
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let bc = [
                    screen_w - 130.0 * scale + bob_x + item_punch_x,
                    screen_h - 110.0 * scale + bob_y + item_punch_y,
                ];
                let lw = 55.0 * scale;
                let lh = 30.0 * scale;
                let ld = 40.0 * scale;

                // Loaf Top Rounded Crust
                let t_nw = [bc[0] - lw * 0.75, bc[1] - ld * 0.6];
                let t_ne = [bc[0] + lw * 0.75, bc[1] - ld * 0.6];
                let t_se = [bc[0] + lw, bc[1] + ld * 0.2];
                let t_sw = [bc[0] - lw, bc[1] + ld * 0.2];
                mesh.add_quad_vertices([t_sw, t_nw, t_ne, t_se], white_uv, [[0.86, 0.62, 0.30, 1.0]; 4]);

                // 3 Diagonal Scoring Slashes
                for &offset in &[-0.4, 0.0, 0.4] {
                    let sx = bc[0] + lw * offset;
                    let sy = bc[1] - ld * 0.2;
                    let sw = 5.0 * scale;
                    let slen = 18.0 * scale;
                    mesh.add_quad_vertices(
                        [[sx - sw, sy + slen], [sx + sw, sy + slen], [sx + sw + 8.0 * scale, sy - slen], [sx - sw + 8.0 * scale, sy - slen]],
                        white_uv,
                        [[0.96, 0.82, 0.55, 1.0]; 4],
                    );
                }

                // Front & Side Crust
                let f_bl = [t_sw[0], t_sw[1] + lh];
                let f_br = [t_se[0], t_se[1] + lh];
                mesh.add_quad_vertices([t_sw, t_se, f_br, f_bl], white_uv, [[0.72, 0.48, 0.22, 1.0]; 4]);

                let r_br = [t_ne[0], t_ne[1] + lh];
                mesh.add_quad_vertices([t_se, t_ne, r_br, f_br], white_uv, [[0.55, 0.35, 0.16, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        bc[0] + 15.0 * scale,
                        bc[1] + lh * 0.40,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else if is_apple {
                // -----------------------------------------------------------------
                // 3D HELD APPLE: 3D sculpted crimson fruit with stem and leaf
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let ac = [
                    screen_w - 130.0 * scale + bob_x + item_punch_x,
                    screen_h - 110.0 * scale + bob_y + item_punch_y,
                ];
                let asize = 42.0 * scale;

                // Apple Body: Faceted rounded isometric sphere
                // Top Highlight Facet
                let a_top = [ac[0], ac[1] - asize * 0.85];
                let a_tl = [ac[0] - asize * 0.70, ac[1] - asize * 0.25];
                let a_tr = [ac[0] + asize * 0.70, ac[1] - asize * 0.25];
                let a_mid = [ac[0] + asize * 0.05, ac[1] + asize * 0.10];
                mesh.add_quad_vertices([a_tl, a_top, a_tr, a_mid], white_uv, [[0.95, 0.26, 0.22, 1.0]; 4]);

                // Lower-Left Facet
                let a_bl = [ac[0] - asize * 0.55, ac[1] + asize * 0.75];
                let a_bm = [ac[0], ac[1] + asize * 0.90];
                mesh.add_quad_vertices([a_tl, a_mid, a_bm, a_bl], white_uv, [[0.80, 0.15, 0.15, 1.0]; 4]);

                // Lower-Right Facet (shaded)
                let a_br = [ac[0] + asize * 0.65, ac[1] + asize * 0.65];
                mesh.add_quad_vertices([a_mid, a_tr, a_br, a_bm], white_uv, [[0.60, 0.08, 0.08, 1.0]; 4]);

                // Stem sticking out from top indentation
                let s_base = [a_top[0], a_top[1] + 5.0 * scale];
                let s_tip = [s_base[0] - 6.0 * scale, s_base[1] - 18.0 * scale];
                let sw = 2.5 * scale;
                mesh.add_quad_vertices(
                    [[s_base[0] - sw, s_base[1]], [s_base[0] + sw, s_base[1]], [s_tip[0] + sw, s_tip[1]], [s_tip[0] - sw, s_tip[1]]],
                    white_uv,
                    [[0.38, 0.22, 0.10, 1.0]; 4],
                );

                // Little green leaf on the stem
                let l_tip = [s_tip[0] + 14.0 * scale, s_tip[1] - 2.0 * scale];
                mesh.add_quad_vertices(
                    [s_tip, [s_tip[0] + 7.0 * scale, s_tip[1] + 5.0 * scale], l_tip, [s_tip[0] + 7.0 * scale, s_tip[1] - 5.0 * scale]],
                    white_uv,
                    [[0.22, 0.75, 0.18, 1.0]; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        ac[0] + 15.0 * scale,
                        ac[1] + asize * 0.40,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            } else {
                // -----------------------------------------------------------------
                // HELD ITEM SPRITE WITH FIRST-PERSON HAND & ARM
                // -----------------------------------------------------------------
                let punch_x = -45.0 * scale * punch_swing;
                let punch_y = -35.0 * scale * punch_swing;
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                // 1. Arm & Sleeve coming in from bottom-right
                let p_arm_bot_r = [
                    screen_w + 35.0 * scale + bob_x + punch_x * 0.3,
                    screen_h + 30.0 * scale,
                ];
                let p_arm_bot_l = [
                    screen_w - 90.0 * scale + bob_x + punch_x * 0.3,
                    screen_h + 30.0 * scale,
                ];
                let p_arm_bot_m = [
                    (p_arm_bot_l[0] + p_arm_bot_r[0]) * 0.5,
                    (p_arm_bot_l[1] + p_arm_bot_r[1]) * 0.5,
                ];

                let p_sleeve_r = [
                    screen_w - 25.0 * scale + bob_x + punch_x * 0.6,
                    screen_h - 110.0 * scale + bob_y + punch_y * 0.6,
                ];
                let p_sleeve_l = [
                    screen_w - 135.0 * scale + bob_x + punch_x * 0.6,
                    screen_h - 70.0 * scale + bob_y + punch_y * 0.6,
                ];
                let p_sleeve_m = [
                    (p_sleeve_l[0] + p_sleeve_r[0]) * 0.5,
                    (p_sleeve_l[1] + p_sleeve_r[1]) * 0.5,
                ];

                let p_wrist_r = [
                    screen_w - 75.0 * scale + bob_x + punch_x,
                    screen_h - 165.0 * scale + bob_y + punch_y,
                ];
                let p_wrist_l = [
                    screen_w - 165.0 * scale + bob_x + punch_x,
                    screen_h - 125.0 * scale + bob_y + punch_y,
                ];
                let p_wrist_m = [
                    (p_wrist_l[0] + p_wrist_r[0]) * 0.5,
                    (p_wrist_l[1] + p_wrist_r[1]) * 0.5,
                ];

                // Sleeve quads
                mesh.add_quad_vertices(
                    [p_arm_bot_l, p_arm_bot_m, p_sleeve_m, p_sleeve_l],
                    white_uv,
                    [col_sleeve_side; 4],
                );
                mesh.add_quad_vertices(
                    [p_arm_bot_m, p_arm_bot_r, p_sleeve_r, p_sleeve_m],
                    white_uv,
                    [col_sleeve_top; 4],
                );

                // Forearm quads
                mesh.add_quad_vertices(
                    [p_sleeve_l, p_sleeve_m, p_wrist_m, p_wrist_l],
                    white_uv,
                    [col_forearm_side; 4],
                );
                mesh.add_quad_vertices(
                    [p_sleeve_m, p_sleeve_r, p_wrist_r, p_wrist_m],
                    white_uv,
                    [col_forearm_top; 4],
                );

                // Optional sleeve cuff accent band near wrist
                if let Some((cuff_top, cuff_side)) = cuff_colors {
                    let lerp2 = |a: [f32; 2], b: [f32; 2], t: f32| -> [f32; 2] {
                        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
                    };
                    let c_l = lerp2(p_sleeve_l, p_wrist_l, 0.78);
                    let c_m = lerp2(p_sleeve_m, p_wrist_m, 0.78);
                    let c_r = lerp2(p_sleeve_r, p_wrist_r, 0.78);
                    mesh.add_quad_vertices([c_l, c_m, p_wrist_m, p_wrist_l], white_uv, [cuff_side; 4]);
                    mesh.add_quad_vertices([c_m, c_r, p_wrist_r, p_wrist_m], white_uv, [cuff_top; 4]);
                }

                // 2. The Clean Item Sprite (crisp upright 2D quad held by hand)
                let item_size = 96.0 * scale;
                let ix = screen_w - 180.0 * scale + bob_x + item_punch_x;
                let iy = screen_h - 220.0 * scale + bob_y + item_punch_y;
                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let item_uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
                mesh.add_quad_vertices(
                    [
                        [ix, iy],
                        [ix + item_size, iy],
                        [ix + item_size, iy + item_size],
                        [ix, iy + item_size],
                    ],
                    item_uvs,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

                // 3. Thumb / Knuckles gripping the item
                let p_thumb = [ix + item_size * 0.45, iy + item_size * 0.70];
                let p_fist_w = 26.0 * scale;
                let p_fist_h = 24.0 * scale;
                mesh.add_quad_vertices(
                    [
                        [p_thumb[0], p_thumb[1]],
                        [p_thumb[0] + p_fist_w, p_thumb[1]],
                        [p_thumb[0] + p_fist_w, p_thumb[1] + p_fist_h],
                        [p_thumb[0], p_thumb[1] + p_fist_h],
                    ],
                    white_uv,
                    [col_skin_light; 4],
                );

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        ix + item_size * 0.45,
                        iy + item_size * 0.65,
                        30.0 * scale,
                        active_slot.count,
                    );
                }
            }
        }
    }

    /// Builds the standard gameplay HUD: center crosshair, held item, bottom hotbar,
    /// player health hearts, hunger drumsticks, damage flash, and death screen overlay.
    pub fn build_hud(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_slot: usize,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
        stats: &blockgame_core::player::PlayerStats,
    ) {
        Self::build_hud_with_armor(
            mesh,
            screen_w,
            screen_h,
            inventory,
            active_slot,
            registry,
            walk_distance,
            punch_time,
            stats,
            0,
        );
    }

    pub fn build_hud_with_armor(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_slot: usize,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
        stats: &blockgame_core::player::PlayerStats,
        armor_points: u8,
    ) {
        Self::build_hud_with_armor_and_tuck(
            mesh,
            screen_w,
            screen_h,
            inventory,
            active_slot,
            registry,
            walk_distance,
            punch_time,
            stats,
            armor_points,
            0.0,
        );
    }

    /// Draws a sleek, stylized segmented status gauge (Roots & Ridges style)
    /// replacing legacy heart/drumstick sprites.
    pub fn draw_segmented_gauge(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        label_color: [f32; 4],
        current_val: f32,
        max_val: f32,
        active_color: [f32; 4],
        highlight_color: [f32; 4],
        empty_color: [f32; 4],
        border_color: [f32; 4],
        num_segments: usize,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // 1. Dark outer container / frame
        // Drop shadow
        mesh.add_quad(x + 1.5, y + 1.5, w, h, white_uv, [0.02, 0.03, 0.05, 0.60]);
        // Border
        mesh.add_quad(x, y, w, h, white_uv, border_color);
        // Slate background
        mesh.add_quad(x + 1.0, y + 1.0, w - 2.0, h - 2.0, white_uv, [0.06, 0.08, 0.12, 0.88]);

        // 2. Left label / icon tag
        let label_w = Self::string_width(label, 7.5, 0.5);
        Self::draw_string(mesh, x + 5.0, y + (h - 9.0) * 0.5, label, 7.5, 9.0, 0.5, label_color);

        // 3. Segmented track
        let track_start_x = x + 8.0 + label_w;
        let num_str = format!("{:.0}", current_val.max(0.0));
        let num_w = Self::string_width(&num_str, 7.0, 0.5);
        let track_end_x = x + w - num_w - 7.0;
        let track_w = (track_end_x - track_start_x).max(10.0);

        let seg_gap = 2.0;
        let total_gaps = (num_segments.saturating_sub(1)) as f32 * seg_gap;
        let seg_w = ((track_w - total_gaps) / num_segments as f32).max(2.0);
        let seg_h = h - 4.0;
        let seg_y = y + 2.0;

        let val_per_seg = max_val / num_segments as f32;

        for i in 0..num_segments {
            let sx = track_start_x + (i as f32) * (seg_w + seg_gap);
            let threshold = (i as f32 + 1.0) * val_per_seg;
            let half_threshold = threshold - (val_per_seg * 0.5);

            // Empty groove
            mesh.add_quad(sx, seg_y, seg_w, seg_h, white_uv, empty_color);

            if current_val >= threshold {
                // Full segment
                mesh.add_quad(sx, seg_y, seg_w, seg_h, white_uv, active_color);
                // Top gloss highlight
                mesh.add_quad(sx, seg_y, seg_w, (seg_h * 0.35).max(1.0), white_uv, highlight_color);
            } else if current_val >= half_threshold {
                // Half segment
                let half_w = seg_w * 0.5;
                mesh.add_quad(sx, seg_y, half_w, seg_h, white_uv, active_color);
                mesh.add_quad(sx, seg_y, half_w, (seg_h * 0.35).max(1.0), white_uv, highlight_color);
            }
        }

        // 4. Numeric readout on the right
        Self::draw_string(mesh, x + w - num_w - 4.0, y + (h - 9.0) * 0.5, &num_str, 7.0, 9.0, 0.5, [0.92, 0.94, 0.98, 0.95]);
    }

    /// Draws a segmented gauge with a two-tone fill:
    /// `primary_val` (`0..=secondary_val`, e.g. Saturation) renders in `primary_color`/`primary_highlight`,
    /// and the remainder up to `secondary_val` (e.g. Hunger) renders in `secondary_color`/`secondary_highlight`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_two_tone_segmented_gauge(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        label_color: [f32; 4],
        primary_val: f32,
        secondary_val: f32,
        max_val: f32,
        primary_color: [f32; 4],
        primary_highlight: [f32; 4],
        secondary_color: [f32; 4],
        secondary_highlight: [f32; 4],
        empty_color: [f32; 4],
        border_color: [f32; 4],
        num_segments: usize,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        mesh.add_quad(x + 1.5, y + 1.5, w, h, white_uv, [0.02, 0.03, 0.05, 0.60]);
        mesh.add_quad(x, y, w, h, white_uv, border_color);
        mesh.add_quad(x + 1.0, y + 1.0, w - 2.0, h - 2.0, white_uv, [0.06, 0.08, 0.12, 0.88]);

        let label_w = Self::string_width(label, 7.5, 0.5);
        Self::draw_string(mesh, x + 5.0, y + (h - 9.0) * 0.5, label, 7.5, 9.0, 0.5, label_color);

        let track_start_x = x + 8.0 + label_w;
        let num_str = format!("{:.0}", secondary_val.max(0.0));
        let num_w = Self::string_width(&num_str, 7.0, 0.5);
        let track_end_x = x + w - num_w - 7.0;
        let track_w = (track_end_x - track_start_x).max(10.0);

        let seg_gap = 2.0;
        let total_gaps = (num_segments.saturating_sub(1)) as f32 * seg_gap;
        let seg_w = ((track_w - total_gaps) / num_segments as f32).max(2.0);
        let seg_h = h - 4.0;
        let seg_y = y + 2.0;
        let val_per_seg = max_val / num_segments as f32;

        let clamped_sec = secondary_val.clamp(0.0, max_val);
        let clamped_pri = primary_val.clamp(0.0, clamped_sec);

        for i in 0..num_segments {
            let sx = track_start_x + (i as f32) * (seg_w + seg_gap);
            let seg_min = (i as f32) * val_per_seg;

            // Empty groove
            mesh.add_quad(sx, seg_y, seg_w, seg_h, white_uv, empty_color);

            // Secondary fill (Hunger)
            let sec_frac = ((clamped_sec - seg_min) / val_per_seg).clamp(0.0, 1.0);
            if sec_frac > 0.01 {
                let sw = seg_w * sec_frac;
                mesh.add_quad(sx, seg_y, sw, seg_h, white_uv, secondary_color);
                mesh.add_quad(sx, seg_y, sw, (seg_h * 0.35).max(1.0), white_uv, secondary_highlight);
            }

            // Primary fill (Saturation in bright gold)
            let pri_frac = ((clamped_pri - seg_min) / val_per_seg).clamp(0.0, 1.0);
            if pri_frac > 0.01 {
                let pw = seg_w * pri_frac;
                mesh.add_quad(sx, seg_y, pw, seg_h, white_uv, primary_color);
                mesh.add_quad(sx, seg_y, pw, (seg_h * 0.35).max(1.0), white_uv, primary_highlight);
            }
        }

        Self::draw_string(mesh, x + w - num_w - 4.0, y + (h - 9.0) * 0.5, &num_str, 7.0, 9.0, 0.5, [0.92, 0.94, 0.98, 0.95]);
    }

    /// Draws a compact eating/drinking progress bar below the center crosshair.
    pub fn draw_eating_progress_bar(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        progress: f32,
    ) {
        let p = progress.clamp(0.0, 1.0);
        if p <= 0.001 {
            return;
        }
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let bar_w = 72.0;
        let bar_h = 6.0;
        let bx = (screen_w - bar_w) * 0.5;
        let by = screen_h * 0.5 + 22.0;
        mesh.add_quad(bx - 1.0, by - 1.0, bar_w + 2.0, bar_h + 2.0, white_uv, [0.05, 0.06, 0.08, 0.85]);
        mesh.add_quad(bx, by, bar_w, bar_h, white_uv, [0.18, 0.16, 0.12, 0.80]);
        mesh.add_quad(bx, by, bar_w * p, bar_h, white_uv, [1.0, 0.82, 0.25, 0.96]);
    }

    /// Builds the standard Roots & Ridges gameplay HUD: center crosshair, held item,
    /// dynamic auto-tucking hotbar, segmented Vitality/Stamina gauges, damage flash, and death screen.
    pub fn build_hud_with_armor_and_tuck(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_slot: usize,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
        stats: &blockgame_core::player::PlayerStats,
        armor_points: u8,
        hotbar_tuck_progress: f32,
    ) {
        let default_app = blockgame_core::character::CharacterAppearance::default();
        Self::build_hud_with_appearance(
            mesh,
            screen_w,
            screen_h,
            inventory,
            active_slot,
            registry,
            walk_distance,
            punch_time,
            stats,
            armor_points,
            hotbar_tuck_progress,
            &default_app,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_hud_with_appearance(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_slot: usize,
        registry: &ItemRegistry,
        walk_distance: f32,
        punch_time: f32,
        stats: &blockgame_core::player::PlayerStats,
        armor_points: u8,
        hotbar_tuck_progress: f32,
        appearance: &blockgame_core::character::CharacterAppearance,
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // 0. Full-screen Underwater Screen Overlay (When Player is Submerged)
        if stats.is_submerged && !stats.is_dead {
            mesh.add_quad(
                0.0,
                0.0,
                screen_w,
                screen_h,
                white_uv,
                [0.03, 0.22, 0.48, 0.28],
            );
        }

        // 1. Center Crosshair (24x24 px) - only when alive
        if !stats.is_dead {
            let ch_size = 24.0;
            let ch_x = (screen_w - ch_size) * 0.5;
            let ch_y = (screen_h - ch_size) * 0.5;
            let ch_uv = TextureAtlas::get_uv_bounds(Self::CROSSHAIR_TILE);
            mesh.add_quad(ch_x, ch_y, ch_size, ch_size, ch_uv, [1.0, 1.0, 1.0, 1.0]);
        }

        // 2. First-Person Hand or Held Item in lower-right corner - only when alive
        if !stats.is_dead {
            let held_slot = inventory.get(active_slot);
            Self::draw_held_item_with_appearance(
                mesh,
                screen_w,
                screen_h,
                held_slot,
                registry,
                walk_distance,
                punch_time,
                appearance,
            );
        }

        // 3. Hotbar Slots (9 slots, each 50x50 px with 5px gap) with smooth tuck animation
        let slot_size = 50.0;
        let slot_gap = 5.0;
        let total_w = 9.0 * slot_size + 8.0 * slot_gap;
        let start_x = (screen_w - total_w) * 0.5;
        let base_start_y = screen_h - slot_size - 16.0;
        let tuck_offset = (slot_size - 18.0) * hotbar_tuck_progress.clamp(0.0, 1.0);
        let hotbar_y = base_start_y + tuck_offset;

        // Elegant dock backdrop behind the slots
        let dock_pad_x = 10.0;
        let dock_pad_y = 6.0;
        mesh.add_quad(
            start_x - dock_pad_x,
            hotbar_y - dock_pad_y,
            total_w + dock_pad_x * 2.0,
            slot_size + dock_pad_y * 2.0 + 8.0,
            white_uv,
            [0.04, 0.06, 0.09, 0.75],
        );
        mesh.add_quad(
            start_x - dock_pad_x,
            hotbar_y - dock_pad_y,
            total_w + dock_pad_x * 2.0,
            1.5,
            white_uv,
            [0.35, 0.40, 0.48, 0.85],
        );

        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        for i in 0..9 {
            let x = start_x + (i as f32) * (slot_size + slot_gap);
            let y = hotbar_y;

            // Slot Background
            mesh.add_quad(x, y, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

            // Selected Slot Highlight Box
            if i == active_slot && !stats.is_dead {
                let hl_pad = 3.0;
                mesh.add_quad(
                    x - hl_pad,
                    y - hl_pad,
                    slot_size + hl_pad * 2.0,
                    slot_size + hl_pad * 2.0,
                    highlight_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );

                // When tucked, draw a prominent illuminated gold tab at the visible top edge
                if hotbar_tuck_progress > 0.15 {
                    mesh.add_quad(
                        x + (slot_size - 18.0) * 0.5,
                        y - 5.0,
                        18.0,
                        5.0,
                        white_uv,
                        [1.0, 0.82, 0.28, 0.95],
                    );
                }
            }

            // Item Icon & Stack Count
            let item_slot = inventory.get(i);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 8.0;
                    let icon_size = slot_size - icon_pad * 2.0;
                    mesh.add_quad(
                        x + icon_pad,
                        y + icon_pad,
                        icon_size,
                        icon_size,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, x, y, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, x, y, slot_size, item_slot, registry);
                }
            }
        }

        // 4. Roots & Ridges Vitality & Stamina Segmented Gauges (Survival only)
        let bars_y = hotbar_y - 20.0;

        if stats.game_mode.is_creative() {
            let mode_label = "Creative Mode";
            let label_w = Self::string_width(mode_label, 9.0, 1.0);
            Self::draw_string(
                mesh,
                (screen_w - label_w) * 0.5,
                hotbar_y - 18.0,
                mode_label,
                9.0,
                12.0,
                1.0,
                [0.78, 0.92, 1.0, 0.92],
            );
        } else {
            let gauge_w = 188.0;
            let gauge_h = 13.0;

            // 4a. Vitality Gauge (Health) on the left above hotbar (tinted green when Poisoned)
            let is_poisoned = stats.has_status_effect(blockgame_core::item::FoodEffectKind::Poison);
            let (hp_label_col, hp_fill, hp_hi, hp_border) = if is_poisoned {
                (
                    [0.45, 0.96, 0.42, 1.0],
                    [0.24, 0.72, 0.24, 1.0],
                    [0.52, 0.96, 0.48, 1.0],
                    [0.22, 0.56, 0.22, 0.92],
                )
            } else {
                (
                    [1.0, 0.35, 0.40, 1.0],
                    [0.90, 0.18, 0.22, 1.0],
                    [1.0, 0.45, 0.48, 1.0],
                    [0.48, 0.22, 0.26, 0.90],
                )
            };
            Self::draw_segmented_gauge(
                mesh,
                start_x,
                bars_y,
                gauge_w,
                gauge_h,
                "HP",
                hp_label_col,
                stats.health,
                stats.max_health,
                hp_fill,
                hp_hi,
                [0.16, 0.18, 0.22, 0.65],
                hp_border,
                10,
            );

            // 4b. Armor Points Gauge (above Vitality gauge when armor > 0)
            if armor_points > 0 {
                let armor_y = bars_y - 15.0;
                Self::draw_segmented_gauge(
                    mesh,
                    start_x,
                    armor_y,
                    gauge_w,
                    11.0,
                    "AP",
                    [0.65, 0.82, 1.0, 1.0],
                    armor_points as f32,
                    20.0,
                    [0.55, 0.75, 0.95, 1.0],
                    [0.80, 0.90, 1.0, 1.0],
                    [0.14, 0.18, 0.24, 0.65],
                    [0.32, 0.45, 0.60, 0.85],
                    10,
                );
            }

            // 5. Stamina / Hunger Gauge with Two-Tone Saturation Fill (Q23 Option C)
            // Bright gold from 0..saturation, warm orange from saturation..hunger (sickly green on Food Poisoning)
            let stamina_x = start_x + total_w - gauge_w;
            let has_food_poisoning =
                stats.has_status_effect(blockgame_core::item::FoodEffectKind::FoodPoisoning);
            let (st_label_col, sat_fill, sat_hi, hun_fill, hun_hi, st_border) = if has_food_poisoning {
                (
                    [0.68, 0.95, 0.34, 1.0],
                    [0.66, 0.88, 0.26, 1.0],
                    [0.82, 0.98, 0.50, 1.0],
                    [0.42, 0.64, 0.16, 1.0],
                    [0.58, 0.80, 0.26, 1.0],
                    [0.34, 0.52, 0.16, 0.92],
                )
            } else {
                (
                    [1.0, 0.84, 0.28, 1.0],
                    [1.0, 0.85, 0.20, 1.0],
                    [1.0, 0.96, 0.60, 1.0],
                    [0.92, 0.50, 0.10, 1.0],
                    [1.0, 0.70, 0.26, 1.0],
                    [0.58, 0.44, 0.18, 0.92],
                )
            };
            Self::draw_two_tone_segmented_gauge(
                mesh,
                stamina_x,
                bars_y,
                gauge_w,
                gauge_h,
                "ST",
                st_label_col,
                stats.saturation,
                stats.hunger,
                stats.max_hunger,
                sat_fill,
                sat_hi,
                hun_fill,
                hun_hi,
                [0.20, 0.18, 0.14, 0.65],
                st_border,
                10,
            );

            // 5b. Active Food Status Effect Badges (Food Poisoning / Poison)
            if !stats.status_effects.is_empty() && !stats.is_dead {
                let mut badge_y = bars_y - 32.0;
                for eff in &stats.status_effects {
                    if eff.remaining_secs <= 0.0 {
                        continue;
                    }
                    let (text, col) = match eff.kind {
                        blockgame_core::item::FoodEffectKind::FoodPoisoning => (
                            format!("FOOD POISONING {:.0}s", eff.remaining_secs.ceil()),
                            [0.72, 0.95, 0.36, 0.98],
                        ),
                        blockgame_core::item::FoodEffectKind::Poison => (
                            format!("POISON {:.0}s", eff.remaining_secs.ceil()),
                            [0.42, 0.96, 0.46, 0.98],
                        ),
                        blockgame_core::item::FoodEffectKind::Speed => (
                            format!("SPEED {:.0}s", eff.remaining_secs.ceil()),
                            [0.45, 0.88, 1.00, 0.98],
                        ),
                        blockgame_core::item::FoodEffectKind::Regeneration => (
                            format!("REGENERATION {:.0}s", eff.remaining_secs.ceil()),
                            [1.00, 0.56, 0.72, 0.98],
                        ),
                        blockgame_core::item::FoodEffectKind::MiningHaste => (
                            format!("HASTE {:.0}s", eff.remaining_secs.ceil()),
                            [1.00, 0.86, 0.36, 0.98],
                        ),
                        blockgame_core::item::FoodEffectKind::Burning => (
                            format!("BURNING {:.0}s", eff.remaining_secs.ceil()),
                            [1.00, 0.45, 0.14, 0.98],
                        ),
                    };
                    let tw = Self::string_width(&text, 7.5, 0.5);
                    let bx = (screen_w - tw) * 0.5;
                    mesh.add_quad(
                        bx - 6.0,
                        badge_y - 2.0,
                        tw + 12.0,
                        13.0,
                        TextureAtlas::get_uv_bounds(Self::WHITE_TILE),
                        [0.05, 0.10, 0.05, 0.78],
                    );
                    Self::draw_string(mesh, bx, badge_y, &text, 7.5, 9.0, 0.5, col);
                    badge_y -= 16.0;
                }
            }

            // 6. Oxygen / Breath Depth Gauge (above Stamina gauge when submerged or recovering breath)
            if (stats.is_submerged || stats.breath < stats.max_breath) && !stats.is_dead {
                let breath_y = bars_y - 15.0;
                let cyan_active = if stats.drown_timer > 0.0 && ((stats.drown_timer * 8.0) as i32 % 2 == 0) {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.22, 0.85, 0.95, 1.0]
                };
                Self::draw_segmented_gauge(
                    mesh,
                    stamina_x,
                    breath_y,
                    gauge_w,
                    11.0,
                    "O2",
                    [0.30, 0.90, 1.0, 1.0],
                    stats.breath,
                    stats.max_breath,
                    cyan_active,
                    [0.65, 0.95, 1.0, 1.0],
                    [0.10, 0.18, 0.22, 0.65],
                    [0.18, 0.50, 0.60, 0.85],
                    10,
                );
            }
        }

        // 7. Full-screen Red Damage Flash
        if stats.damage_flash > 0.0 {
            let flash_alpha = ((stats.damage_flash / 0.5) * 0.40).clamp(0.0, 0.40);
            mesh.add_quad(
                0.0,
                0.0,
                screen_w,
                screen_h,
                white_uv,
                [0.85, 0.08, 0.08, flash_alpha],
            );
        }

        // 7. Full-screen Death Screen Overlay (When Player is Dead)
        if stats.is_dead {
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
            // Deep red backdrop
            mesh.add_quad(
                0.0,
                0.0,
                screen_w,
                screen_h,
                white_uv,
                [0.55, 0.06, 0.06, 0.76],
            );

            // Big "You Died!" Title
            let title = "You Died!";
            let title_w = title.len() as f32 * 20.0;
            let title_x = (screen_w - title_w) * 0.5;
            let title_y = screen_h * 0.32;
            Self::draw_string(
                mesh,
                title_x,
                title_y,
                title,
                16.0,
                22.0,
                4.0,
                [1.0, 0.95, 0.95, 1.0],
            );

            // Subtitle
            let subtitle = "Score: 0";
            let sub_w = subtitle.len() as f32 * 10.0;
            let sub_x = (screen_w - sub_w) * 0.5;
            Self::draw_string(
                mesh,
                sub_x,
                title_y + 36.0,
                subtitle,
                9.0,
                13.0,
                1.0,
                [0.85, 0.85, 0.85, 0.95],
            );

            // "Respawn" Button
            let btn_w = 200.0;
            let btn_h = 44.0;
            let btn_x = (screen_w - btn_w) * 0.5;
            let btn_y = screen_h * 0.50;

            let border_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);
            mesh.add_quad(
                btn_x - 2.0,
                btn_y - 2.0,
                btn_w + 4.0,
                btn_h + 4.0,
                border_uv,
                [1.0, 1.0, 1.0, 1.0],
            );
            mesh.add_quad(
                btn_x,
                btn_y,
                btn_w,
                btn_h,
                white_uv,
                [0.22, 0.22, 0.24, 0.95],
            );

            let respawn_text = "Respawn";
            let respawn_w = respawn_text.len() as f32 * 11.0;
            let text_x = btn_x + (btn_w - respawn_w) * 0.5;
            let text_y = btn_y + 14.0;
            Self::draw_string(
                mesh,
                text_x,
                text_y,
                respawn_text,
                10.0,
                15.0,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }
    }

    /// Returns `[x, y, size, size]` for armor slot `idx` (0=Helmet, 1=Chestplate, 2=Leggings, 3=Boots).
    pub fn armor_slot_rect(screen_w: f32, screen_h: f32, grid_h: usize, idx: usize) -> [f32; 4] {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });
        let inv_w = 9.0 * slot_size + 8.0 * slot_gap;
        let inv_start_x = (screen_w - inv_w) * 0.5;
        let inv_start_y = craft_y + (grid_h as f32) * (slot_size + slot_gap) + 24.0;
        let ax = inv_start_x - slot_size - 16.0;
        let ay = inv_start_y + (idx as f32) * (slot_size + slot_gap)
            + if idx == 3 { 16.0 } else { 0.0 };
        [ax, ay, slot_size, slot_size]
    }

    /// Bounding rectangle for the player inventory (armor, crafting grid, backpack, and hotbar).
    pub fn inventory_panel_bounds(screen_w: f32, screen_h: f32, grid_h: usize) -> UiRect {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let inv_start_x = (screen_w - inv_w) * 0.5;
        let armor_left_x = inv_start_x - slot_size - 16.0;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });
        let inv_start_y = craft_y + (grid_h as f32) * (slot_size + slot_gap) + 24.0;
        let hotbar_y = inv_start_y + 3.0 * (slot_size + slot_gap) + 16.0;

        let pad = 12.0;
        let x = armor_left_x - pad;
        let y = craft_y - 24.0;
        let right = inv_start_x + inv_w + pad;
        let bottom = hotbar_y + slot_size + pad;
        UiRect {
            x,
            y,
            w: right - x,
            h: bottom - y,
        }
    }

    /// Bounding rectangle for standard 9-column container screens (chest, furnace, backpack, etc.).
    pub fn container_panel_bounds(screen_w: f32, top_y: f32, hotbar_y: f32) -> UiRect {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let pad = 12.0;
        let x = start_x - pad;
        let y = top_y - 28.0;
        UiRect {
            x,
            y,
            w: inv_w + pad * 2.0,
            h: (hotbar_y + slot_size + pad) - y,
        }
    }

    /// Bounding rectangle for the cooking pot screen.
    pub fn cooking_pot_panel_bounds(screen_w: f32, screen_h: f32) -> UiRect {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let top_y = screen_h * 0.5 - 220.0;
        let inv_y = top_y + 12.0 + 2.0 * (slot_size + slot_gap) + 28.0;
        let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
        Self::container_panel_bounds(screen_w, top_y, hotbar_y)
    }

    /// Builds the full Inventory & 2x2 Crafting Screen Overlay.
    pub fn build_inventory_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        crafting_grid: &CraftingGrid,
        crafting_result: Option<ItemSlot>,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        Self::build_inventory_screen_with_armor(
            mesh,
            screen_w,
            screen_h,
            inventory,
            active_hotbar_slot,
            crafting_grid,
            crafting_result,
            cursor_held,
            cursor_pos,
            registry,
            jei_params,
            recipe_modal,
            &[ItemSlot::EMPTY; 4],
        );
    }

    /// Builds the full Inventory & Crafting Screen Overlay with 4 Armor equipment slots.
    pub fn build_inventory_screen_with_armor(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        crafting_grid: &CraftingGrid,
        crafting_result: Option<ItemSlot>,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
        armor: &[ItemSlot; 4],
    ) {
        mesh.clear();

        let mut hovered_item_name: Option<String> = None;

        // 1. Semi-transparent backdrop overlay
        let backdrop_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            backdrop_uv,
            [0.0, 0.0, 0.0, 0.65],
        );

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        // 2. Dynamic Crafting Grid (2x2 Player Crafting or 3x3 Crafting Table) + Result Slot
        let grid_w = crafting_grid.width;
        let grid_h = crafting_grid.height;

        let craft_x = (screen_w - ((grid_w as f32 + 2.0) * (slot_size + slot_gap) + 24.0)) * 0.5;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 210.0 } else { 180.0 });

        // Header text above crafting grid
        let title = if grid_w == 3 {
            "Crafting Table (3x3)"
        } else {
            "Crafting (2x2)"
        };
        Self::draw_string(
            mesh,
            craft_x,
            craft_y - 18.0,
            title,
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        for cy in 0..grid_h {
            for cx in 0..grid_w {
                let sx = craft_x + (cx as f32) * (slot_size + slot_gap);
                let sy = craft_y + (cy as f32) * (slot_size + slot_gap);
                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 1.0]);

                let slot = crafting_grid.get(cx, cy);
                if !slot.is_empty() {
                    if let Some(def) = registry.get(slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let icon_pad = 6.0;
                        mesh.add_quad(
                            sx + icon_pad,
                            sy + icon_pad,
                            slot_size - icon_pad * 2.0,
                            slot_size - icon_pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        Self::draw_stack_number(mesh, sx, sy, slot_size, slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, slot, registry);

                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            hovered_item_name = slot.tooltip_label(registry);
                        }
                    }
                }
            }
        }

        // Crafting Result Slot (vertically centered next to grid)
        let result_x = craft_x + (grid_w as f32) * (slot_size + slot_gap) + 32.0;
        let result_y = craft_y + ((grid_h as f32 - 1.0) * (slot_size + slot_gap) * 0.5);
        let r_size = slot_size + 8.0;
        mesh.add_quad(
            result_x,
            result_y,
            r_size,
            r_size,
            slot_uv,
            [1.0, 0.9, 0.6, 1.0],
        );
        if let Some(res) = crafting_result {
            if let Some(def) = registry.get(res.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                mesh.add_quad(
                    result_x + icon_pad,
                    result_y + icon_pad,
                    r_size - icon_pad * 2.0,
                    r_size - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, result_x, result_y, r_size, res.count);
                Self::draw_durability_bar(mesh, result_x, result_y, r_size, res, registry);

                if cursor_pos[0] >= result_x
                    && cursor_pos[0] <= result_x + r_size
                    && cursor_pos[1] >= result_y
                    && cursor_pos[1] <= result_y + r_size
                {
                    hovered_item_name = res.tooltip_label(registry);
                }
            }
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

                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

                let item_slot = inventory.get(slot_idx);
                if !item_slot.is_empty() {
                    if let Some(def) = registry.get(item_slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let icon_pad = 6.0;
                        mesh.add_quad(
                            sx + icon_pad,
                            sy + icon_pad,
                            slot_size - icon_pad * 2.0,
                            slot_size - icon_pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            hovered_item_name = item_slot.tooltip_label(registry);
                        }
                    }
                }
            }
        }

        // 4. Hotbar Slots at bottom of inventory screen
        let hotbar_y = inv_start_y + 3.0 * (slot_size + slot_gap) + 16.0;
        for i in 0..9 {
            let sx = inv_start_x + (i as f32) * (slot_size + slot_gap);
            let sy = hotbar_y;

            mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
            if i == active_hotbar_slot {
                let hl_pad = 2.0;
                mesh.add_quad(
                    sx - hl_pad,
                    sy - hl_pad,
                    slot_size + hl_pad * 2.0,
                    slot_size + hl_pad * 2.0,
                    highlight_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }

            let item_slot = inventory.get(i);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        sx + icon_pad,
                        sy + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                    if cursor_pos[0] >= sx
                        && cursor_pos[0] <= sx + slot_size
                        && cursor_pos[1] >= sy
                        && cursor_pos[1] <= sy + slot_size
                    {
                        hovered_item_name = item_slot.tooltip_label(registry);
                    }
                }
            }
        }

        // 4b. Armor Equipment Slots (Helmet, Chestplate, Leggings, Boots) to the left of inventory
        let armor_labels = ["H", "C", "L", "B"];
        for (idx, item_slot) in armor.iter().enumerate() {
            let rect = Self::armor_slot_rect(screen_w, screen_h, grid_h, idx);
            let ax = rect[0];
            let ay = rect[1];
            mesh.add_quad(ax, ay, slot_size, slot_size, slot_uv, [0.92, 0.92, 1.0, 0.95]);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        ax + icon_pad,
                        ay + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_durability_bar(mesh, ax, ay, slot_size, *item_slot, registry);

                    if cursor_pos[0] >= ax
                        && cursor_pos[0] <= ax + slot_size
                        && cursor_pos[1] >= ay
                        && cursor_pos[1] <= ay + slot_size
                    {
                        hovered_item_name = item_slot.tooltip_label(registry);
                    }
                }
            } else {
                Self::draw_string(
                    mesh,
                    ax + slot_size * 0.5 - 5.0,
                    ay + slot_size * 0.5 - 6.0,
                    armor_labels[idx],
                    10.0,
                    12.0,
                    1.0,
                    [0.55, 0.58, 0.65, 0.75],
                );
            }
        }

        // 5. JEI Catalog Panel on right side
        let mut jei_hover: Option<&str> = None;
        if let Some(jei) = jei_params {
            Self::draw_jei_panel(
                mesh,
                screen_w,
                screen_h,
                jei,
                cursor_pos,
                registry,
                &mut jei_hover,
            );
        }

        // 6. Recipe Inspector Modal (if open, drawn on top)
        if let Some(modal) = recipe_modal {
            Self::draw_recipe_modal(
                mesh,
                screen_w,
                screen_h,
                modal,
                cursor_pos,
                registry,
                &mut jei_hover,
            );
        }
        if let Some(j) = jei_hover {
            hovered_item_name = Some(j.to_string());
        }

        // 7. Cursor Held Item Stack (follows mouse position)
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                let hx = cursor_pos[0] - hold_size * 0.5;
                let hy = cursor_pos[1] - hold_size * 0.5;
                mesh.add_quad(hx, hy, hold_size, hold_size, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, hx, hy, hold_size, cursor_held.count);
                Self::draw_durability_bar(mesh, hx, hy, hold_size, *cursor_held, registry);
            }
        }

        // 6. Hover Tooltip (drawn on top of all slots and cursor items)
        if let Some(name) = hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], &name);
        }
    }

    /// Builds the Chest Container Screen: 27 Chest Slots (3x9), 27 Backpack Slots (3x9),
    /// 9 Hotbar Slots (1x9), Cursor Held Item, and Item Tooltips.
    pub fn build_chest_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        chest_slots: &[ItemSlot],
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();

        let mut hovered_item_name: Option<&str> = None;

        // 1. Semi-transparent backdrop overlay
        let backdrop_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            backdrop_uv,
            [0.0, 0.0, 0.0, 0.65],
        );

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;

        // 2. Chest Container Slots (3 rows of 9 slots = 27 slots)
        let chest_y = screen_h * 0.5 - 220.0;
        Self::draw_string(
            mesh,
            start_x,
            chest_y - 18.0,
            "Chest (27 Slots)",
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = row * 9 + col;
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = chest_y + (row as f32) * (slot_size + slot_gap);

                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

                if slot_idx < chest_slots.len() {
                    let slot = chest_slots[slot_idx];
                    if !slot.is_empty() {
                        if let Some(def) = registry.get(slot.item_id) {
                            let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                            let icon_pad = 6.0;
                            mesh.add_quad(
                                sx + icon_pad,
                                sy + icon_pad,
                                slot_size - icon_pad * 2.0,
                                slot_size - icon_pad * 2.0,
                                icon_uv,
                                [1.0, 1.0, 1.0, 1.0],
                            );
                            Self::draw_stack_number(mesh, sx, sy, slot_size, slot.count);
                            Self::draw_durability_bar(mesh, sx, sy, slot_size, slot, registry);

                            if cursor_pos[0] >= sx
                                && cursor_pos[0] <= sx + slot_size
                                && cursor_pos[1] >= sy
                                && cursor_pos[1] <= sy + slot_size
                            {
                                hovered_item_name = Some(def.display_name);
                            }
                        }
                    }
                }
            }
        }

        // 3. Player Backpack Storage Grid (3 rows x 9 columns)
        let inv_y = chest_y + 3.0 * (slot_size + slot_gap) + 26.0;
        Self::draw_string(
            mesh,
            start_x,
            inv_y - 18.0,
            "Inventory",
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = 9 + row * 9 + col;
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = inv_y + (row as f32) * (slot_size + slot_gap);

                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

                let item_slot = inventory.get(slot_idx);
                if !item_slot.is_empty() {
                    if let Some(def) = registry.get(item_slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let icon_pad = 6.0;
                        mesh.add_quad(
                            sx + icon_pad,
                            sy + icon_pad,
                            slot_size - icon_pad * 2.0,
                            slot_size - icon_pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            hovered_item_name = Some(def.display_name);
                        }
                    }
                }
            }
        }

        // 4. Hotbar Slots at bottom of chest screen
        let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
        for i in 0..9 {
            let sx = start_x + (i as f32) * (slot_size + slot_gap);
            let sy = hotbar_y;

            mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
            if i == active_hotbar_slot {
                let hl_pad = 2.0;
                mesh.add_quad(
                    sx - hl_pad,
                    sy - hl_pad,
                    slot_size + hl_pad * 2.0,
                    slot_size + hl_pad * 2.0,
                    highlight_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }

            let item_slot = inventory.get(i);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        sx + icon_pad,
                        sy + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                    if cursor_pos[0] >= sx
                        && cursor_pos[0] <= sx + slot_size
                        && cursor_pos[1] >= sy
                        && cursor_pos[1] <= sy + slot_size
                    {
                        hovered_item_name = Some(def.display_name);
                    }
                }
            }
        }

        // 5. JEI Catalog Panel on right side
        if let Some(jei) = jei_params {
            Self::draw_jei_panel(
                mesh,
                screen_w,
                screen_h,
                jei,
                cursor_pos,
                registry,
                &mut hovered_item_name,
            );
        }

        // 6. Recipe Inspector Modal (if open, drawn on top)
        if let Some(modal) = recipe_modal {
            Self::draw_recipe_modal(
                mesh,
                screen_w,
                screen_h,
                modal,
                cursor_pos,
                registry,
                &mut hovered_item_name,
            );
        }

        // 7. Cursor Held Item Stack (follows mouse position)
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                let hx = cursor_pos[0] - hold_size * 0.5;
                let hy = cursor_pos[1] - hold_size * 0.5;
                mesh.add_quad(hx, hy, hold_size, hold_size, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, hx, hy, hold_size, cursor_held.count);
                Self::draw_durability_bar(mesh, hx, hy, hold_size, *cursor_held, registry);
            }
        }

        // 8. Hover Tooltip
        if let Some(name) = hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], name);
        }
    }

    /// Builds the Furnace Smelting / Cooking Screen:
    /// - Input slot at top
    /// - Animated burn flame indicator in the center
    /// - Fuel slot below the flame
    /// - Cook progress indicator and larger output slot to the right
    /// - 27 Backpack slots (3x9) and 9 Hotbar slots (1x9) below
    /// - JEI panel, Recipe modal, cursor held item, and tooltips
    pub fn build_furnace_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        furnace: &FurnaceState,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();

        let mut hovered_item_name: Option<&str> = None;

        // 1. Semi-transparent backdrop overlay
        let backdrop_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            backdrop_uv,
            [0.0, 0.0, 0.0, 0.65],
        );

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;

        // 2. Furnace Chamber (Upper Section)
        let furnace_top_y = screen_h * 0.5 - 220.0;
        Self::draw_string(
            mesh,
            start_x,
            furnace_top_y - 20.0,
            "Furnace",
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        // Center column for Input & Fuel slots
        let center_x = start_x + (inv_w - slot_size) * 0.5 - 55.0;
        let input_y = furnace_top_y + 10.0;
        let flame_size = 24.0;
        let flame_x = center_x + (slot_size - flame_size) * 0.5;
        let flame_y = input_y + slot_size + 8.0;
        let fuel_y = flame_y + flame_size + 8.0;

        // Input Slot
        mesh.add_quad(
            center_x,
            input_y,
            slot_size,
            slot_size,
            slot_uv,
            [1.0, 1.0, 1.0, 0.95],
        );
        let in_slot = furnace.input;
        if !in_slot.is_empty() {
            if let Some(def) = registry.get(in_slot.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                mesh.add_quad(
                    center_x + icon_pad,
                    input_y + icon_pad,
                    slot_size - icon_pad * 2.0,
                    slot_size - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, center_x, input_y, slot_size, in_slot.count);
                Self::draw_durability_bar(mesh, center_x, input_y, slot_size, in_slot, registry);

                if cursor_pos[0] >= center_x
                    && cursor_pos[0] <= center_x + slot_size
                    && cursor_pos[1] >= input_y
                    && cursor_pos[1] <= input_y + slot_size
                {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // Animated Flame (between Input and Fuel)
        // Background / Empty flame icon
        let flame_empty_uv = TextureAtlas::get_uv_bounds(Self::FLAME_CONTAINER_TILE);
        mesh.add_quad(
            flame_x,
            flame_y,
            flame_size,
            flame_size,
            flame_empty_uv,
            [0.35, 0.35, 0.35, 0.85],
        );

        if furnace.burn_time_remaining > 0.0 && furnace.max_burn_time > 0.0 {
            let burn_ratio = (furnace.burn_time_remaining / furnace.max_burn_time).clamp(0.0, 1.0);
            let active_h = flame_size * burn_ratio;
            let active_y = flame_y + (flame_size - active_h);
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(Self::FLAME_FULL_TILE);
            let v_active_top = v_max - (v_max - v_min) * burn_ratio;
            mesh.add_quad(
                flame_x,
                active_y,
                flame_size,
                active_h,
                [u_min, v_active_top, u_max, v_max],
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        // Fuel Slot
        mesh.add_quad(
            center_x,
            fuel_y,
            slot_size,
            slot_size,
            slot_uv,
            [1.0, 1.0, 1.0, 0.95],
        );
        let fuel_slot = furnace.fuel;
        if !fuel_slot.is_empty() {
            if let Some(def) = registry.get(fuel_slot.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                mesh.add_quad(
                    center_x + icon_pad,
                    fuel_y + icon_pad,
                    slot_size - icon_pad * 2.0,
                    slot_size - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, center_x, fuel_y, slot_size, fuel_slot.count);
                Self::draw_durability_bar(mesh, center_x, fuel_y, slot_size, fuel_slot, registry);

                if cursor_pos[0] >= center_x
                    && cursor_pos[0] <= center_x + slot_size
                    && cursor_pos[1] >= fuel_y
                    && cursor_pos[1] <= fuel_y + slot_size
                {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // Cook Progress Arrow & Output Slot to the right
        let arrow_x = center_x + slot_size + 24.0;
        let arrow_y = input_y + (fuel_y + slot_size - input_y) * 0.5 - 7.0;
        Self::draw_string(
            mesh,
            arrow_x,
            arrow_y,
            "===>",
            10.0,
            14.0,
            1.0,
            [0.45, 0.45, 0.50, 1.0],
        );

        // Highlight cooking arrow progress
        let cook_ratio = (furnace.cook_progress / FurnaceState::FURNACE_COOK_TIME).clamp(0.0, 1.0);
        if cook_ratio > 0.0 {
            let progress_chars = (cook_ratio * 4.0).ceil() as usize;
            let full_arrow = "===>";
            let active_part = &full_arrow[..progress_chars.min(4)];
            Self::draw_string(
                mesh,
                arrow_x,
                arrow_y,
                active_part,
                10.0,
                14.0,
                1.0,
                [1.0, 0.85, 0.20, 1.0],
            );
        }

        // Output Slot (enlarged 56x56, warm golden tinted background)
        let out_x = arrow_x + 64.0;
        let out_y = input_y + (fuel_y + slot_size - input_y) * 0.5 - 28.0;
        let out_size = 56.0;
        mesh.add_quad(
            out_x,
            out_y,
            out_size,
            out_size,
            slot_uv,
            [1.0, 0.95, 0.70, 1.0],
        );
        let out_slot = furnace.output;
        if !out_slot.is_empty() {
            if let Some(def) = registry.get(out_slot.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                mesh.add_quad(
                    out_x + icon_pad,
                    out_y + icon_pad,
                    out_size - icon_pad * 2.0,
                    out_size - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, out_x, out_y, out_size, out_slot.count);
                Self::draw_durability_bar(mesh, out_x, out_y, out_size, out_slot, registry);

                if cursor_pos[0] >= out_x
                    && cursor_pos[0] <= out_x + out_size
                    && cursor_pos[1] >= out_y
                    && cursor_pos[1] <= out_y + out_size
                {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // 3. Player Backpack Storage Grid (3 rows x 9 columns)
        let inv_y = fuel_y + slot_size + 24.0;
        Self::draw_string(
            mesh,
            start_x,
            inv_y - 18.0,
            "Inventory",
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = 9 + row * 9 + col;
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = inv_y + (row as f32) * (slot_size + slot_gap);

                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

                let item_slot = inventory.get(slot_idx);
                if !item_slot.is_empty() {
                    if let Some(def) = registry.get(item_slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let icon_pad = 6.0;
                        mesh.add_quad(
                            sx + icon_pad,
                            sy + icon_pad,
                            slot_size - icon_pad * 2.0,
                            slot_size - icon_pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            hovered_item_name = Some(def.display_name);
                        }
                    }
                }
            }
        }

        // 4. Hotbar Slots at bottom of furnace screen
        let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
        for i in 0..9 {
            let sx = start_x + (i as f32) * (slot_size + slot_gap);
            let sy = hotbar_y;

            mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
            if i == active_hotbar_slot {
                let hl_pad = 2.0;
                mesh.add_quad(
                    sx - hl_pad,
                    sy - hl_pad,
                    slot_size + hl_pad * 2.0,
                    slot_size + hl_pad * 2.0,
                    highlight_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }

            let item_slot = inventory.get(i);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        sx + icon_pad,
                        sy + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);

                    if cursor_pos[0] >= sx
                        && cursor_pos[0] <= sx + slot_size
                        && cursor_pos[1] >= sy
                        && cursor_pos[1] <= sy + slot_size
                    {
                        hovered_item_name = Some(def.display_name);
                    }
                }
            }
        }

        // 5. JEI Catalog Panel on right side
        if let Some(jei) = jei_params {
            Self::draw_jei_panel(
                mesh,
                screen_w,
                screen_h,
                jei,
                cursor_pos,
                registry,
                &mut hovered_item_name,
            );
        }

        // 6. Recipe Inspector Modal (if open, drawn on top)
        if let Some(modal) = recipe_modal {
            Self::draw_recipe_modal(
                mesh,
                screen_w,
                screen_h,
                modal,
                cursor_pos,
                registry,
                &mut hovered_item_name,
            );
        }

        // 7. Cursor Held Item Stack (follows mouse position)
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                let hx = cursor_pos[0] - hold_size * 0.5;
                let hy = cursor_pos[1] - hold_size * 0.5;
                mesh.add_quad(hx, hy, hold_size, hold_size, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, hx, hy, hold_size, cursor_held.count);
                Self::draw_durability_bar(mesh, hx, hy, hold_size, *cursor_held, registry);
            }
        }

        // 8. Hover Tooltip
        if let Some(name) = hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], name);
        }
    }

    /// Hit-test helper for the Cooking Pot screen.
    /// Returns `(kind, index)` where:
    /// - `0` = Ingredient slot (`0..4`)
    /// - `1` = Fuel slot (`0`)
    /// - `2` = Output slot (`0`)
    /// - `3` = Player inventory / hotbar slot (`0..36`)
    pub fn hit_test_cooking_pot_screen(
        screen_w: f32,
        screen_h: f32,
        mx: f32,
        my: f32,
    ) -> Option<(u8, usize)> {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let top_y = screen_h * 0.5 - 220.0;

        let ing_x0 = start_x + 48.0;
        let ing_y0 = top_y + 12.0;
        for i in 0..4 {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let sx = ing_x0 + col * (slot_size + slot_gap);
            let sy = ing_y0 + row * (slot_size + slot_gap);
            if mx >= sx && mx <= sx + slot_size && my >= sy && my <= sy + slot_size {
                return Some((0, i));
            }
        }

        let fuel_x = ing_x0 + 2.0 * (slot_size + slot_gap) + 28.0;
        let fuel_y = ing_y0 + (slot_size + slot_gap);
        if mx >= fuel_x && mx <= fuel_x + slot_size && my >= fuel_y && my <= fuel_y + slot_size {
            return Some((1, 0));
        }

        let out_x = fuel_x + slot_size + 52.0;
        let out_y = ing_y0 + (slot_size + slot_gap) * 0.5;
        if mx >= out_x && mx <= out_x + slot_size && my >= out_y && my <= out_y + slot_size {
            return Some((2, 0));
        }

        let inv_y = ing_y0 + 2.0 * (slot_size + slot_gap) + 32.0;
        for row in 0..3 {
            for col in 0..9 {
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = inv_y + (row as f32) * (slot_size + slot_gap);
                if mx >= sx && mx <= sx + slot_size && my >= sy && my <= sy + slot_size {
                    return Some((3, 9 + row * 9 + col));
                }
            }
        }
        let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 14.0;
        for col in 0..9 {
            let sx = start_x + (col as f32) * (slot_size + slot_gap);
            if mx >= sx && mx <= sx + slot_size && my >= hotbar_y && my <= hotbar_y + slot_size {
                return Some((3, col));
            }
        }

        None
    }

    /// Builds the Cooking Pot UI screen (4-slot shapeless ingredient tray, fuel slot, flame,
    /// cook progress bar, output slot, plus player backpack, hotbar, JEI, and tooltips).
    #[allow(clippy::too_many_arguments)]
    pub fn build_cooking_pot_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        pot: &blockgame_core::CookingPotState,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let top_y = screen_h * 0.5 - 220.0;

        Self::draw_string(
            mesh,
            start_x,
            top_y - 20.0,
            "Cooking Pot",
            8.0,
            10.0,
            1.0,
            [1.0, 0.88, 0.62, 1.0],
        );

        let draw_slot_item = |mesh: &mut UiMesh,
                              sx: f32,
                              sy: f32,
                              item_slot: ItemSlot,
                              hovered: &mut Option<&str>| {
            mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        sx + icon_pad,
                        sy + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);
                    if cursor_pos[0] >= sx
                        && cursor_pos[0] <= sx + slot_size
                        && cursor_pos[1] >= sy
                        && cursor_pos[1] <= sy + slot_size
                    {
                        *hovered = Some(def.display_name);
                    }
                }
            }
        };

        // 4-slot shapeless ingredient tray (2x2)
        let ing_x0 = start_x + 48.0;
        let ing_y0 = top_y + 12.0;
        for i in 0..4 {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            let sx = ing_x0 + col * (slot_size + slot_gap);
            let sy = ing_y0 + row * (slot_size + slot_gap);
            draw_slot_item(mesh, sx, sy, pot.ingredients[i], &mut hovered_item_name);
        }

        // Fuel slot & flame meter
        let fuel_x = ing_x0 + 2.0 * (slot_size + slot_gap) + 28.0;
        let fuel_y = ing_y0 + (slot_size + slot_gap);
        draw_slot_item(mesh, fuel_x, fuel_y, pot.fuel, &mut hovered_item_name);

        let flame_w = 24.0;
        let flame_h = 24.0;
        let flame_x = fuel_x + (slot_size - flame_w) * 0.5;
        let flame_y = ing_y0 + 12.0;
        mesh.add_quad(flame_x, flame_y, flame_w, flame_h, white_uv, [0.18, 0.14, 0.12, 0.85]);
        if pot.is_burning() && pot.max_burn_time > 0.0 {
            let frac = (pot.burn_time_remaining / pot.max_burn_time).clamp(0.0, 1.0);
            let fh = flame_h * frac;
            mesh.add_quad(
                flame_x + 2.0,
                flame_y + (flame_h - fh),
                flame_w - 4.0,
                fh,
                white_uv,
                [1.0, 0.55, 0.12, 0.95],
            );
        }

        // Cook progress bar between fuel/flame and output
        let prog_x = fuel_x + slot_size + 10.0;
        let prog_y = ing_y0 + (slot_size + slot_gap) * 0.5 + 16.0;
        let prog_w = 34.0;
        let prog_h = 12.0;
        mesh.add_quad(prog_x, prog_y, prog_w, prog_h, white_uv, [0.16, 0.16, 0.18, 0.85]);
        let cook_frac = (pot.cook_progress / blockgame_core::world::COOKING_POT_COOK_TIME).clamp(0.0, 1.0);
        if cook_frac > 0.001 {
            mesh.add_quad(
                prog_x + 1.0,
                prog_y + 1.0,
                (prog_w - 2.0) * cook_frac,
                prog_h - 2.0,
                white_uv,
                [1.0, 0.82, 0.24, 0.95],
            );
        }

        // Output slot
        let out_x = fuel_x + slot_size + 52.0;
        let out_y = ing_y0 + (slot_size + slot_gap) * 0.5;
        draw_slot_item(mesh, out_x, out_y, pot.output, &mut hovered_item_name);

        // Shared player inventory, hotbar, JEI, recipe modal, cursor, and tooltips
        let inv_y = ing_y0 + 2.0 * (slot_size + slot_gap) + 32.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh,
            screen_w,
            screen_h,
            start_x,
            inv_y,
            inventory,
            active_hotbar_slot,
            cursor_held,
            cursor_pos,
            registry,
            jei_params,
            recipe_modal,
            &mut hovered_item_name,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_power_screen_inv_and_chrome<'a>(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        start_x: f32,
        inv_y: f32,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &'a ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
        hovered_item_name: &mut Option<&'a str>,
    ) {
        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        Self::draw_string(
            mesh,
            start_x,
            inv_y - 18.0,
            "Inventory",
            8.0,
            10.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        for row in 0..3 {
            for col in 0..9 {
                let slot_idx = 9 + row * 9 + col;
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = inv_y + (row as f32) * (slot_size + slot_gap);
                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
                let item_slot = inventory.get(slot_idx);
                if !item_slot.is_empty() {
                    if let Some(def) = registry.get(item_slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let icon_pad = 6.0;
                        mesh.add_quad(
                            sx + icon_pad,
                            sy + icon_pad,
                            slot_size - icon_pad * 2.0,
                            slot_size - icon_pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);
                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            *hovered_item_name = Some(def.display_name);
                        }
                    }
                }
            }
        }

        let hotbar_y = inv_y + 3.0 * (slot_size + slot_gap) + 16.0;
        for i in 0..9 {
            let sx = start_x + (i as f32) * (slot_size + slot_gap);
            let sy = hotbar_y;
            mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
            if i == active_hotbar_slot {
                let hl_pad = 2.0;
                mesh.add_quad(
                    sx - hl_pad,
                    sy - hl_pad,
                    slot_size + hl_pad * 2.0,
                    slot_size + hl_pad * 2.0,
                    highlight_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            let item_slot = inventory.get(i);
            if !item_slot.is_empty() {
                if let Some(def) = registry.get(item_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let icon_pad = 6.0;
                    mesh.add_quad(
                        sx + icon_pad,
                        sy + icon_pad,
                        slot_size - icon_pad * 2.0,
                        slot_size - icon_pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                    Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);
                    if cursor_pos[0] >= sx
                        && cursor_pos[0] <= sx + slot_size
                        && cursor_pos[1] >= sy
                        && cursor_pos[1] <= sy + slot_size
                    {
                        *hovered_item_name = Some(def.display_name);
                    }
                }
            }
        }

        if let Some(jei) = jei_params {
            Self::draw_jei_panel(
                mesh,
                screen_w,
                screen_h,
                jei,
                cursor_pos,
                registry,
                hovered_item_name,
            );
        }
        if let Some(modal) = recipe_modal {
            Self::draw_recipe_modal(
                mesh,
                screen_w,
                screen_h,
                modal,
                cursor_pos,
                registry,
                hovered_item_name,
            );
        }
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                let hx = cursor_pos[0] - hold_size * 0.5;
                let hy = cursor_pos[1] - hold_size * 0.5;
                mesh.add_quad(hx, hy, hold_size, hold_size, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, hx, hy, hold_size, cursor_held.count);
                Self::draw_durability_bar(mesh, hx, hy, hold_size, *cursor_held, registry);
            }
        }
        if let Some(name) = *hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], name);
        }
    }

    /// Builds the Coal Generator UI screen (Fuel slot, flame bar, live output & grid stats).
    #[allow(clippy::too_many_arguments)]
    pub fn build_coal_generator_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        gen: &blockgame_core::power::CoalGeneratorState,
        summary: &blockgame_core::power::PowerNetworkSummary,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let top_y = screen_h * 0.5 - 220.0;

        Self::draw_string(
            mesh,
            start_x,
            top_y - 20.0,
            "Coal Generator",
            8.0,
            10.0,
            1.0,
            [0.98, 0.88, 0.65, 1.0],
        );

        // Fuel slot centered-left
        let fuel_x = start_x + 40.0;
        let fuel_y = top_y + 42.0;
        Self::draw_string(mesh, fuel_x, fuel_y - 16.0, "Fuel", 7.0, 9.0, 1.0, [0.85, 0.85, 0.85, 1.0]);
        mesh.add_quad(fuel_x, fuel_y, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
        if !gen.fuel.is_empty() {
            if let Some(def) = registry.get(gen.fuel.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let pad = 6.0;
                mesh.add_quad(fuel_x + pad, fuel_y + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, fuel_x, fuel_y, slot_size, gen.fuel.count);
                if cursor_pos[0] >= fuel_x && cursor_pos[0] <= fuel_x + slot_size && cursor_pos[1] >= fuel_y && cursor_pos[1] <= fuel_y + slot_size {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // Burn flame meter next to fuel slot
        let flame_x = fuel_x + slot_size + 22.0;
        let flame_y = fuel_y + 10.0;
        let flame_size = 28.0;
        mesh.add_quad(flame_x, flame_y, flame_size, flame_size, TextureAtlas::get_uv_bounds(Self::FLAME_CONTAINER_TILE), [0.35, 0.35, 0.35, 0.85]);
        let ratio = gen.burn_progress();
        if ratio > 0.0 {
            let active_h = flame_size * ratio;
            let active_y = flame_y + (flame_size - active_h);
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(Self::FLAME_FULL_TILE);
            let v_top = v_max - (v_max - v_min) * ratio;
            mesh.add_quad(flame_x, active_y, flame_size, active_h, [u_min, v_top, u_max, v_max], [1.0, 1.0, 1.0, 1.0]);
        }

        // Live power readout panel on the right
        let info_x = flame_x + flame_size + 28.0;
        let status_txt = if gen.current_output_eu_s > 0.0 {
            format!("Provides: +{:.0} EU/s (Active)", gen.current_output_eu_s)
        } else {
            "Provides: 0 / +40 EU/s (Standby)".to_string()
        };
        let status_col = if gen.current_output_eu_s > 0.0 { [0.40, 0.95, 0.55, 1.0] } else { [0.75, 0.78, 0.82, 1.0] };
        Self::draw_string(mesh, info_x, top_y + 34.0, &status_txt, 7.5, 9.5, 1.0, status_col);
        let grid_txt = format!("Grid: +{:.0} EU/s Gen / -{:.0} EU/s Use", summary.total_gen_eu_s, summary.total_demand_eu_s);
        Self::draw_string(mesh, info_x, top_y + 56.0, &grid_txt, 7.0, 9.0, 1.0, [0.85, 0.90, 0.95, 1.0]);
        let bat_txt = format!("Stored: {:.0} / {:.0} EU", summary.total_stored_eu, summary.total_capacity_eu);
        Self::draw_string(mesh, info_x, top_y + 76.0, &bat_txt, 7.0, 9.0, 1.0, [0.65, 0.92, 0.78, 1.0]);

        let inv_y = top_y + 10.0 + slot_size + 8.0 + 24.0 + 8.0 + slot_size + 24.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh, screen_w, screen_h, start_x, inv_y, inventory, active_hotbar_slot,
            cursor_held, cursor_pos, registry, jei_params, recipe_modal, &mut hovered_item_name,
        );
    }

    /// Builds the Battery Box UI screen (large charge bar, net EU/s flow, and grid summary).
    #[allow(clippy::too_many_arguments)]
    pub fn build_battery_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        bat: &blockgame_core::power::BatteryState,
        summary: &blockgame_core::power::PowerNetworkSummary,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let top_y = screen_h * 0.5 - 220.0;

        let (title_txt, title_col, bar_col) = if bat.capacity_eu >= 400_000.0 {
            (
                "Quantum Battery Box (Max +/-1,000 EU/s)",
                [0.84, 0.56, 0.99, 1.0],
                [0.74, 0.36, 0.96, 1.0],
            )
        } else if bat.capacity_eu >= 80_000.0 {
            (
                "Advanced Battery Box (Max +/-300 EU/s)",
                [0.45, 0.92, 0.99, 1.0],
                [0.20, 0.80, 0.96, 1.0],
            )
        } else {
            (
                "Battery Box (Max +/-100 EU/s)",
                [0.55, 0.98, 0.72, 1.0],
                [0.22, 0.86, 0.48, 1.0],
            )
        };

        Self::draw_string(
            mesh,
            start_x,
            top_y - 20.0,
            title_txt,
            8.0,
            10.0,
            1.0,
            title_col,
        );

        // Large visual energy bar
        let bar_x = start_x + 20.0;
        let bar_y = top_y + 24.0;
        let bar_w = inv_w - 40.0;
        let bar_h = 30.0;
        mesh.add_quad(bar_x - 2.0, bar_y - 2.0, bar_w + 4.0, bar_h + 4.0, white_uv, [0.18, 0.22, 0.26, 0.95]);
        mesh.add_quad(bar_x, bar_y, bar_w, bar_h, white_uv, [0.08, 0.10, 0.12, 0.95]);
        let ratio = bat.charge_ratio();
        if ratio > 0.0 {
            mesh.add_quad(bar_x, bar_y, bar_w * ratio, bar_h, white_uv, bar_col);
        }

        let pct_txt = format!("{:.0} / {:.0} EU ({:.0}%)", bat.stored_eu, bat.capacity_eu, ratio * 100.0);
        Self::draw_string(mesh, bar_x + 8.0, bar_y + 9.0, &pct_txt, 7.5, 9.5, 1.0, [0.98, 0.98, 0.98, 1.0]);

        let flow_txt = if bat.last_net_eu_s > 0.5 {
            format!("Status: Charging (+{:.0} EU/s)", bat.last_net_eu_s)
        } else if bat.last_net_eu_s < -0.5 {
            format!("Status: Discharging ({:.0} EU/s)", bat.last_net_eu_s)
        } else {
            "Status: Standby (0 EU/s)".to_string()
        };
        let flow_col = if bat.last_net_eu_s > 0.5 {
            [0.35, 0.95, 0.55, 1.0]
        } else if bat.last_net_eu_s < -0.5 {
            [0.98, 0.72, 0.32, 1.0]
        } else {
            [0.78, 0.82, 0.86, 1.0]
        };
        Self::draw_string(mesh, bar_x, bar_y + 44.0, &flow_txt, 7.5, 9.5, 1.0, flow_col);

        let grid_txt = format!(
            "Network: +{:.0} EU/s Gen | -{:.0} EU/s Use | {} Nodes",
            summary.total_gen_eu_s, summary.total_demand_eu_s, summary.node_count
        );
        Self::draw_string(mesh, bar_x, bar_y + 66.0, &grid_txt, 7.0, 9.0, 1.0, [0.85, 0.90, 0.95, 1.0]);

        let inv_y = top_y + 10.0 + slot_size + 8.0 + 24.0 + 8.0 + slot_size + 24.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh, screen_w, screen_h, start_x, inv_y, inventory, active_hotbar_slot,
            cursor_held, cursor_pos, registry, jei_params, recipe_modal, &mut hovered_item_name,
        );
    }

    /// Builds the Electric Furnace UI screen (Input, fast cook bar, Output, EU/s consumption).
    #[allow(clippy::too_many_arguments)]
    pub fn build_electric_furnace_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        ef: &blockgame_core::power::ElectricFurnaceState,
        summary: &blockgame_core::power::PowerNetworkSummary,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let top_y = screen_h * 0.5 - 220.0;

        Self::draw_string(
            mesh,
            start_x,
            top_y - 20.0,
            "Electric Furnace (2.5x Speed | -30 EU/s)",
            8.0,
            10.0,
            1.0,
            [0.45, 0.92, 0.98, 1.0],
        );

        let input_x = start_x + (inv_w - slot_size) * 0.5 - 80.0;
        let slot_y = top_y + 34.0;
        let output_x = input_x + slot_size + 112.0;

        // Input Slot
        mesh.add_quad(input_x, slot_y, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
        if !ef.input.is_empty() {
            if let Some(def) = registry.get(ef.input.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let pad = 6.0;
                mesh.add_quad(input_x + pad, slot_y + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, input_x, slot_y, slot_size, ef.input.count);
                if cursor_pos[0] >= input_x && cursor_pos[0] <= input_x + slot_size && cursor_pos[1] >= slot_y && cursor_pos[1] <= slot_y + slot_size {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // Progress bar between Input and Output
        let bar_x = input_x + slot_size + 18.0;
        let bar_y = slot_y + (slot_size - 14.0) * 0.5;
        let bar_w = 76.0;
        let bar_h = 14.0;
        mesh.add_quad(bar_x, bar_y, bar_w, bar_h, white_uv, [0.16, 0.20, 0.26, 0.95]);
        let prog = ef.cook_progress();
        if prog > 0.0 {
            mesh.add_quad(bar_x, bar_y, bar_w * prog, bar_h, white_uv, [0.25, 0.85, 0.98, 1.0]);
        }

        // Output Slot
        mesh.add_quad(output_x, slot_y, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
        if !ef.output.is_empty() {
            if let Some(def) = registry.get(ef.output.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let pad = 6.0;
                mesh.add_quad(output_x + pad, slot_y + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                Self::draw_stack_number(mesh, output_x, slot_y, slot_size, ef.output.count);
                if cursor_pos[0] >= output_x && cursor_pos[0] <= output_x + slot_size && cursor_pos[1] >= slot_y && cursor_pos[1] <= slot_y + slot_size {
                    hovered_item_name = Some(def.display_name);
                }
            }
        }

        // Power status line below slots
        let pwr_txt = if ef.is_powered {
            format!("Consuming: -30 EU/s | Grid: +{:.0} EU/s", summary.total_gen_eu_s)
        } else if ef.can_smelt() {
            "Insufficient Power (Needs 30 EU/s)".to_string()
        } else {
            format!("Grid: +{:.0} EU/s Gen / -{:.0} EU/s Use | {:.0} EU", summary.total_gen_eu_s, summary.total_demand_eu_s, summary.total_stored_eu)
        };
        let pwr_col = if ef.is_powered {
            [0.35, 0.95, 0.98, 1.0]
        } else if ef.can_smelt() {
            [0.98, 0.42, 0.35, 1.0]
        } else {
            [0.82, 0.86, 0.92, 1.0]
        };
        Self::draw_string(mesh, start_x + 20.0, slot_y + slot_size + 18.0, &pwr_txt, 7.0, 9.0, 1.0, pwr_col);

        let inv_y = top_y + 10.0 + slot_size + 8.0 + 24.0 + 8.0 + slot_size + 24.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh, screen_w, screen_h, start_x, inv_y, inventory, active_hotbar_slot,
            cursor_held, cursor_pos, registry, jei_params, recipe_modal, &mut hovered_item_name,
        );
    }

    /// Builds a paginated Chest UI screen supporting Wooden (27), Iron (45), Gold (63), and Diamond (81) chests.
    #[allow(clippy::too_many_arguments)]
    pub fn build_chest_screen_paged(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        title: &str,
        chest_slots: &[ItemSlot],
        page: usize,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let chest_y = screen_h * 0.5 - 220.0;

        Self::draw_string(mesh, start_x, chest_y - 20.0, title, 8.0, 10.0, 1.0, [0.95, 0.95, 0.95, 1.0]);

        let total_pages = (chest_slots.len() + 26) / 27;
        let page = page.min(total_pages.saturating_sub(1));
        if total_pages > 1 {
            let btn_y = chest_y - 24.0;
            let next_x = start_x + inv_w - 26.0;
            let prev_x = next_x - 86.0;
            mesh.add_quad(prev_x, btn_y, 24.0, 20.0, white_uv, [0.22, 0.26, 0.32, 0.95]);
            Self::draw_string(mesh, prev_x + 8.0, btn_y + 5.0, "<", 7.0, 9.0, 1.0, [0.95, 0.95, 0.95, 1.0]);
            let ptxt = format!("{}/{}", page + 1, total_pages);
            Self::draw_string(mesh, prev_x + 30.0, btn_y + 5.0, &ptxt, 7.0, 9.0, 1.0, [0.90, 0.94, 0.98, 1.0]);
            mesh.add_quad(next_x, btn_y, 24.0, 20.0, white_uv, [0.22, 0.26, 0.32, 0.95]);
            Self::draw_string(mesh, next_x + 8.0, btn_y + 5.0, ">", 7.0, 9.0, 1.0, [0.95, 0.95, 0.95, 1.0]);
        }

        let base_idx = page * 27;
        for row in 0..3 {
            for col in 0..9 {
                let idx = base_idx + row * 9 + col;
                if idx >= chest_slots.len() {
                    continue;
                }
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = chest_y + (row as f32) * (slot_size + slot_gap);
                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);
                let item_slot = chest_slots[idx];
                if !item_slot.is_empty() {
                    if let Some(def) = registry.get(item_slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let pad = 6.0;
                        mesh.add_quad(sx + pad, sy + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                        Self::draw_stack_number(mesh, sx, sy, slot_size, item_slot.count);
                        Self::draw_durability_bar(mesh, sx, sy, slot_size, item_slot, registry);
                        if cursor_pos[0] >= sx && cursor_pos[0] <= sx + slot_size && cursor_pos[1] >= sy && cursor_pos[1] <= sy + slot_size {
                            hovered_item_name = Some(def.display_name);
                        }
                    }
                }
            }
        }

        let inv_y = chest_y + 3.0 * (slot_size + slot_gap) + 26.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh, screen_w, screen_h, start_x, inv_y, inventory, active_hotbar_slot,
            cursor_held, cursor_pos, registry, jei_params, recipe_modal, &mut hovered_item_name,
        );
    }

    /// Builds the Powered Storage Terminal UI screen (linked chests, search, sort, power cost).
    #[allow(clippy::too_many_arguments)]
    pub fn build_storage_terminal_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        info: &blockgame_core::storage::StorageNetworkInfo,
        items: &[blockgame_core::storage::AggregatedStorageItem],
        search_text: &str,
        search_focused: bool,
        sort_mode: blockgame_core::storage::StorageSortMode,
        page: usize,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        mesh.clear();
        let mut hovered_item_name: Option<&str> = None;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = Self::SLOT_SIZE;
        let slot_gap = Self::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let grid_y = screen_h * 0.5 - 205.0;

        // Header line: Title + Online/Offline power badge
        let header_y = grid_y - 52.0;
        let power_badge = if info.is_powered {
            format!("Storage Terminal [ONLINE -{:.0} EU/s]", info.power_draw_eu_s)
        } else {
            format!("Storage Terminal [OFFLINE - Needs {:.0} EU/s]", info.power_draw_eu_s)
        };
        let badge_col = if info.is_powered {
            [0.35, 0.95, 0.98, 1.0]
        } else {
            [0.98, 0.38, 0.35, 1.0]
        };
        Self::draw_string(mesh, start_x, header_y, &power_badge, 7.5, 9.5, 1.0, badge_col);

        let stats_txt = format!(
            "Terminals: {} | Chests: {} | Slots: {}/{}",
            info.terminals.len(),
            info.chests.len(),
            info.used_slots,
            info.total_slots
        );
        Self::draw_string(mesh, start_x, header_y + 15.0, &stats_txt, 6.5, 8.5, 1.0, [0.80, 0.86, 0.92, 1.0]);

        // Controls row: Search box + Sort button + Page buttons
        let ctrl_y = grid_y - 24.0;
        let search_w = 210.0;
        let search_col = if search_focused { [0.16, 0.28, 0.36, 0.98] } else { [0.12, 0.16, 0.22, 0.95] };
        mesh.add_quad(start_x, ctrl_y, search_w, 20.0, white_uv, search_col);
        let search_display = if search_text.is_empty() {
            if search_focused { "|" } else { "Search items..." }
        } else {
            search_text
        };
        let search_txt_col = if search_text.is_empty() && !search_focused {
            [0.55, 0.60, 0.68, 1.0]
        } else {
            [0.95, 0.98, 1.0, 1.0]
        };
        Self::draw_string(mesh, start_x + 6.0, ctrl_y + 5.0, search_display, 6.5, 8.5, 1.0, search_txt_col);

        let sort_x = start_x + search_w + 8.0;
        let sort_w = 104.0;
        mesh.add_quad(sort_x, ctrl_y, sort_w, 20.0, white_uv, [0.20, 0.28, 0.36, 0.95]);
        Self::draw_string(mesh, sort_x + 6.0, ctrl_y + 5.0, sort_mode.label(), 6.5, 8.5, 1.0, [0.92, 0.96, 1.0, 1.0]);

        let total_pages = ((items.len() + 26) / 27).max(1);
        let page = page.min(total_pages - 1);
        let next_x = start_x + inv_w - 24.0;
        let prev_x = next_x - 82.0;
        mesh.add_quad(prev_x, ctrl_y, 24.0, 20.0, white_uv, [0.22, 0.26, 0.32, 0.95]);
        Self::draw_string(mesh, prev_x + 8.0, ctrl_y + 5.0, "<", 7.0, 9.0, 1.0, [0.95, 0.95, 0.95, 1.0]);
        let ptxt = format!("{}/{}", page + 1, total_pages);
        Self::draw_string(mesh, prev_x + 28.0, ctrl_y + 5.0, &ptxt, 6.5, 8.5, 1.0, [0.90, 0.94, 0.98, 1.0]);
        mesh.add_quad(next_x, ctrl_y, 24.0, 20.0, white_uv, [0.22, 0.26, 0.32, 0.95]);
        Self::draw_string(mesh, next_x + 8.0, ctrl_y + 5.0, ">", 7.0, 9.0, 1.0, [0.95, 0.95, 0.95, 1.0]);

        // 3x9 unified storage grid
        let base_idx = page * 27;
        for row in 0..3 {
            for col in 0..9 {
                let sx = start_x + (col as f32) * (slot_size + slot_gap);
                let sy = grid_y + (row as f32) * (slot_size + slot_gap);
                let tint = if info.is_powered { [1.0, 1.0, 1.0, 0.95] } else { [0.55, 0.45, 0.45, 0.85] };
                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, tint);

                if info.is_powered {
                    let idx = base_idx + row * 9 + col;
                    if let Some(entry) = items.get(idx) {
                        if let Some(def) = registry.get(entry.item_id) {
                            let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                            let pad = 6.0;
                            mesh.add_quad(sx + pad, sy + pad, slot_size - pad * 2.0, slot_size - pad * 2.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                            if entry.unique_source.is_some()
                                && entry.durability > 0
                                && blockgame_core::storage::BackpackState::is_backpack_item(entry.item_id)
                            {
                                let tag = format!("#{}", entry.durability);
                                Self::draw_string(mesh, sx + 3.0, sy + 3.0, &tag, 5.5, 7.0, 1.0, [0.98, 0.92, 0.45, 1.0]);
                            } else {
                                Self::draw_stack_number(mesh, sx, sy, slot_size, entry.total_count.min(9999) as u16);
                            }
                            if cursor_pos[0] >= sx && cursor_pos[0] <= sx + slot_size && cursor_pos[1] >= sy && cursor_pos[1] <= sy + slot_size {
                                hovered_item_name = Some(def.display_name);
                            }
                        }
                    }
                }
            }
        }

        let inv_y = screen_h * 0.5 - 220.0 + 3.0 * (slot_size + slot_gap) + 26.0;
        Self::draw_power_screen_inv_and_chrome(
            mesh, screen_w, screen_h, start_x, inv_y, inventory, active_hotbar_slot,
            cursor_held, cursor_pos, registry, jei_params, recipe_modal, &mut hovered_item_name,
        );
    }

    /// Builds the portable wearable Backpack UI screen (18 / 27 / 36 slots).
    #[allow(clippy::too_many_arguments)]
    pub fn build_backpack_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        backpack_id: u16,
        backpack: &blockgame_core::storage::BackpackState,
        page: usize,
        inventory: &Inventory,
        active_hotbar_slot: usize,
        cursor_held: &ItemSlot,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        jei_params: Option<&JeiParams>,
        recipe_modal: Option<&RecipeModalParams>,
    ) {
        let title = backpack.title_with_id(backpack_id);
        Self::build_chest_screen_paged(
            mesh,
            screen_w,
            screen_h,
            &title,
            &backpack.slots,
            page,
            inventory,
            active_hotbar_slot,
            cursor_held,
            cursor_pos,
            registry,
            jei_params,
            recipe_modal,
        );
    }

    /// Draws the JEI (Just Enough Items) Item Index Grid, Pagination, and Search Bar on the right side.
    pub fn draw_jei_panel(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        params: &JeiParams,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        hovered_item_name: &mut Option<&str>,
    ) {
        let slot_size = Self::JEI_SLOT_SIZE;
        let slot_gap = Self::JEI_SLOT_GAP;
        let cols = Self::JEI_COLS;
        let rows = Self::JEI_ROWS;
        let panel_w = (cols as f32) * (slot_size + slot_gap) - slot_gap;
        let start_x = screen_w - panel_w - 20.0;
        let start_y = screen_h * 0.5 - 210.0;

        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // 1. Pagination Controls Header
        let page_y = start_y - 28.0;
        let btn_w = 26.0;
        let btn_h = 20.0;

        // Prev Button [<]
        let prev_x = start_x;
        let prev_hovered = cursor_pos[0] >= prev_x
            && cursor_pos[0] <= prev_x + btn_w
            && cursor_pos[1] >= page_y
            && cursor_pos[1] <= page_y + btn_h;
        let prev_color = if prev_hovered {
            [0.35, 0.35, 0.40, 1.0]
        } else {
            [0.20, 0.20, 0.22, 1.0]
        };
        mesh.add_quad(prev_x, page_y, btn_w, btn_h, white_uv, prev_color);
        Self::draw_char(
            mesh,
            prev_x + 9.0,
            page_y + 4.0,
            8.0,
            12.0,
            b'<',
            [0.95, 0.95, 0.95, 1.0],
        );

        // Page Indicator: "1 / 2"
        let page_text = format!("{} / {}", params.page + 1, params.total_pages.max(1));
        let text_w = page_text.len() as f32 * 9.0;
        let text_x = start_x + (panel_w - text_w) * 0.5;
        Self::draw_string(
            mesh,
            text_x,
            page_y + 4.0,
            &page_text,
            8.0,
            12.0,
            1.0,
            [0.85, 0.85, 0.90, 1.0],
        );

        // Next Button [>]
        let next_x = start_x + panel_w - btn_w;
        let next_hovered = cursor_pos[0] >= next_x
            && cursor_pos[0] <= next_x + btn_w
            && cursor_pos[1] >= page_y
            && cursor_pos[1] <= page_y + btn_h;
        let next_color = if next_hovered {
            [0.35, 0.35, 0.40, 1.0]
        } else {
            [0.20, 0.20, 0.22, 1.0]
        };
        mesh.add_quad(next_x, page_y, btn_w, btn_h, white_uv, next_color);
        Self::draw_char(
            mesh,
            next_x + 9.0,
            page_y + 4.0,
            8.0,
            12.0,
            b'>',
            [0.95, 0.95, 0.95, 1.0],
        );

        // 2. 6x8 Item Catalog Grid (48 items per page)
        let items_per_page = cols * rows;
        let page_offset = params.page * items_per_page;

        for r in 0..rows {
            for c in 0..cols {
                let slot_idx = page_offset + r * cols + c;
                let sx = start_x + (c as f32) * (slot_size + slot_gap);
                let sy = start_y + (r as f32) * (slot_size + slot_gap);

                mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.9]);

                if slot_idx < params.catalog_items.len() {
                    let item_id = params.catalog_items[slot_idx];
                    if let Some(def) = registry.get(item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        let pad = 5.0;
                        mesh.add_quad(
                            sx + pad,
                            sy + pad,
                            slot_size - pad * 2.0,
                            slot_size - pad * 2.0,
                            icon_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );

                        if cursor_pos[0] >= sx
                            && cursor_pos[0] <= sx + slot_size
                            && cursor_pos[1] >= sy
                            && cursor_pos[1] <= sy + slot_size
                        {
                            mesh.add_quad(
                                sx - 2.0,
                                sy - 2.0,
                                slot_size + 4.0,
                                slot_size + 4.0,
                                highlight_uv,
                                [1.0, 1.0, 1.0, 1.0],
                            );
                            *hovered_item_name = Some(def.display_name);
                        }
                    }
                }
            }
        }

        // 3. Search Bar
        let search_y = start_y + (rows as f32) * (slot_size + slot_gap) + 8.0;
        let search_h = 24.0;
        let border_color = if params.search_focused {
            [0.95, 0.85, 0.30, 1.0]
        } else {
            [0.40, 0.40, 0.45, 1.0]
        };
        // Outer border
        mesh.add_quad(
            start_x - 1.0,
            search_y - 1.0,
            panel_w + 2.0,
            search_h + 2.0,
            white_uv,
            border_color,
        );
        // Inner box
        mesh.add_quad(
            start_x,
            search_y,
            panel_w,
            search_h,
            white_uv,
            [0.08, 0.08, 0.10, 0.96],
        );

        if params.search_query.is_empty() && !params.search_focused {
            Self::draw_string(
                mesh,
                start_x + 6.0,
                search_y + 6.0,
                "Search...",
                8.0,
                12.0,
                0.0,
                [0.55, 0.55, 0.60, 1.0],
            );
        } else {
            let display_query = if params.search_focused {
                format!("{}_", params.search_query)
            } else {
                params.search_query.to_string()
            };
            Self::draw_string(
                mesh,
                start_x + 6.0,
                search_y + 6.0,
                &display_query,
                8.0,
                12.0,
                0.0,
                [1.0, 1.0, 0.95, 1.0],
            );
        }
    }

    /// Draws the JEI Recipe Inspector Modal dialog showing recipes, 3x3 pattern or smelting, and output.
    pub fn draw_recipe_modal(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        params: &RecipeModalParams,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
        hovered_item_name: &mut Option<&str>,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        // 1. Dark overlay behind modal
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.0, 0.0, 0.0, 0.72],
        );

        // 2. Modal Window Frame
        let modal_w = 460.0;
        let modal_h = 340.0;
        let mx = (screen_w - modal_w) * 0.5;
        let my = (screen_h - modal_h) * 0.5;

        // Shadow & outer border
        mesh.add_quad(
            mx - 3.0,
            my - 3.0,
            modal_w + 6.0,
            modal_h + 6.0,
            white_uv,
            [0.04, 0.04, 0.05, 0.98],
        );
        // Main window background
        mesh.add_quad(mx, my, modal_w, modal_h, white_uv, [0.18, 0.18, 0.20, 0.98]);
        // Top header banner
        mesh.add_quad(mx, my, modal_w, 36.0, white_uv, [0.12, 0.12, 0.14, 1.0]);

        // Close Button [X]
        let close_size = 24.0;
        let close_x = mx + modal_w - close_size - 6.0;
        let close_y = my + 6.0;
        let close_hovered = cursor_pos[0] >= close_x
            && cursor_pos[0] <= close_x + close_size
            && cursor_pos[1] >= close_y
            && cursor_pos[1] <= close_y + close_size;
        let close_col = if close_hovered {
            [0.85, 0.25, 0.25, 1.0]
        } else {
            [0.65, 0.18, 0.18, 1.0]
        };
        mesh.add_quad(
            close_x, close_y, close_size, close_size, white_uv, close_col,
        );
        Self::draw_char(
            mesh,
            close_x + 8.0,
            close_y + 6.0,
            8.0,
            12.0,
            b'X',
            [1.0, 1.0, 1.0, 1.0],
        );

        // Header Title
        let target_def = registry.get(params.target_item);
        let target_name = target_def.map(|d| d.display_name).unwrap_or("Item");
        let header_prefix = if params.is_usage_mode {
            "Uses: "
        } else {
            "Recipe: "
        };
        let full_title = format!("{}{}", header_prefix, target_name);
        Self::draw_string(
            mesh,
            mx + 12.0,
            my + 12.0,
            &full_title,
            8.0,
            12.0,
            1.0,
            [0.95, 0.95, 0.95, 1.0],
        );

        // Subheader: Recipe Cycle buttons & Category
        let sub_y = my + 44.0;
        if params.total_recipes > 1 {
            let prev_rx = mx + 20.0;
            let next_rx = mx + 50.0;
            let btn_w = 22.0;
            let btn_h = 20.0;

            let prev_hov = cursor_pos[0] >= prev_rx
                && cursor_pos[0] <= prev_rx + btn_w
                && cursor_pos[1] >= sub_y
                && cursor_pos[1] <= sub_y + btn_h;
            let prev_col = if prev_hov {
                [0.35, 0.35, 0.40, 1.0]
            } else {
                [0.22, 0.22, 0.25, 1.0]
            };
            mesh.add_quad(prev_rx, sub_y, btn_w, btn_h, white_uv, prev_col);
            Self::draw_char(
                mesh,
                prev_rx + 7.0,
                sub_y + 4.0,
                8.0,
                12.0,
                b'<',
                [0.95, 0.95, 0.95, 1.0],
            );

            let next_hov = cursor_pos[0] >= next_rx
                && cursor_pos[0] <= next_rx + btn_w
                && cursor_pos[1] >= sub_y
                && cursor_pos[1] <= sub_y + btn_h;
            let next_col = if next_hov {
                [0.35, 0.35, 0.40, 1.0]
            } else {
                [0.22, 0.22, 0.25, 1.0]
            };
            mesh.add_quad(next_rx, sub_y, btn_w, btn_h, white_uv, next_col);
            Self::draw_char(
                mesh,
                next_rx + 7.0,
                sub_y + 4.0,
                8.0,
                12.0,
                b'>',
                [0.95, 0.95, 0.95, 1.0],
            );

            let count_text = format!("({} of {})", params.recipe_index + 1, params.total_recipes);
            Self::draw_string(
                mesh,
                next_rx + 30.0,
                sub_y + 4.0,
                &count_text,
                8.0,
                12.0,
                1.0,
                [0.75, 0.75, 0.80, 1.0],
            );
        }

        // Category Badge
        let cat_str = match params.recipe.category {
            blockgame_core::crafting::RecipeCategory::Crafting3x3 => "[ 3x3 Crafting Table ]",
            blockgame_core::crafting::RecipeCategory::Crafting2x2 => "[ 2x2 Crafting Grid ]",
            blockgame_core::crafting::RecipeCategory::Smelting => "[ Smelting / Cooking ]",
            blockgame_core::crafting::RecipeCategory::CookingPot => "[ Cooking Pot ]",
            blockgame_core::crafting::RecipeCategory::WorldGen => "[ World Generation / Natural Source ]",
        };
        let cat_col = if params.recipe.category == blockgame_core::crafting::RecipeCategory::WorldGen {
            [0.35, 0.95, 0.65, 1.0]
        } else {
            [0.35, 0.85, 0.95, 1.0]
        };
        let cat_x = mx + modal_w - (cat_str.len() as f32 * 9.0) - 20.0;
        Self::draw_string(
            mesh,
            cat_x,
            sub_y + 4.0,
            cat_str,
            8.0,
            12.0,
            1.0,
            cat_col,
        );

        // 3. Recipe Content Box
        let box_x = mx + 20.0;
        let box_y = my + 76.0;
        let box_w = modal_w - 40.0;
        let box_h = modal_h - 96.0;
        mesh.add_quad(
            box_x,
            box_y,
            box_w,
            box_h,
            white_uv,
            [0.12, 0.12, 0.14, 0.95],
        );

        if params.recipe.category == blockgame_core::crafting::RecipeCategory::WorldGen {
            let info = blockgame_core::crafting::get_item_source_info(params.target_item);

            // Left: Display Pedestal & Slot
            let ped_x = box_x + 36.0;
            let ped_y = box_y + 36.0;
            let ped_size = 72.0;

            mesh.add_quad(ped_x, ped_y, ped_size, ped_size, slot_uv, [1.0, 1.0, 1.0, 1.0]);
            mesh.add_quad(ped_x - 8.0, ped_y + ped_size + 4.0, ped_size + 16.0, 6.0, white_uv, [0.3, 0.3, 0.35, 1.0]);
            mesh.add_quad(ped_x - 14.0, ped_y + ped_size + 10.0, ped_size + 28.0, 6.0, white_uv, [0.2, 0.2, 0.25, 1.0]);

            if let Some(def) = registry.get(params.target_item) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let pad = 8.0;
                mesh.add_quad(
                    ped_x + pad,
                    ped_y + pad,
                    ped_size - pad * 2.0,
                    ped_size - pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );

                if cursor_pos[0] >= ped_x
                    && cursor_pos[0] <= ped_x + ped_size
                    && cursor_pos[1] >= ped_y
                    && cursor_pos[1] <= ped_y + ped_size
                {
                    mesh.add_quad(
                        ped_x - 2.0,
                        ped_y - 2.0,
                        ped_size + 4.0,
                        ped_size + 4.0,
                        highlight_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    *hovered_item_name = Some(def.display_name);
                }
            }

            // Right: Information Text
            let text_x = ped_x + ped_size + 32.0;
            let mut line_y = box_y + 24.0;

            Self::draw_string(
                mesh,
                text_x,
                line_y,
                "Natural Occurrence",
                9.0,
                14.0,
                1.0,
                [1.0, 0.85, 0.30, 1.0],
            );
            line_y += 20.0;

            Self::draw_string(
                mesh,
                text_x,
                line_y,
                "No crafting recipe required.",
                8.0,
                12.0,
                1.0,
                [0.70, 0.70, 0.75, 1.0],
            );
            line_y += 18.0;

            mesh.add_quad(text_x, line_y, 220.0, 1.5, white_uv, [0.3, 0.3, 0.35, 1.0]);
            line_y += 10.0;

            if let Some(src) = info {
                Self::draw_string(
                    mesh,
                    text_x,
                    line_y,
                    "Found In:",
                    8.0,
                    12.0,
                    1.0,
                    [0.95, 0.95, 0.95, 1.0],
                );
                line_y += 14.0;
                Self::draw_string(
                    mesh,
                    text_x + 8.0,
                    line_y,
                    src.location,
                    8.0,
                    12.0,
                    0.5,
                    [0.45, 0.95, 0.55, 1.0],
                );
                line_y += 20.0;

                Self::draw_string(
                    mesh,
                    text_x,
                    line_y,
                    "Harvest Tool:",
                    8.0,
                    12.0,
                    1.0,
                    [0.95, 0.95, 0.95, 1.0],
                );
                line_y += 14.0;
                Self::draw_string(
                    mesh,
                    text_x + 8.0,
                    line_y,
                    src.tool_required,
                    8.0,
                    12.0,
                    0.5,
                    [0.45, 0.80, 1.0, 1.0],
                );
                line_y += 20.0;

                Self::draw_string(
                    mesh,
                    text_x,
                    line_y,
                    "Notes:",
                    8.0,
                    12.0,
                    1.0,
                    [0.95, 0.95, 0.95, 1.0],
                );
                line_y += 14.0;
                Self::draw_string(
                    mesh,
                    text_x + 8.0,
                    line_y,
                    src.notes,
                    8.0,
                    12.0,
                    0.5,
                    [0.80, 0.80, 0.85, 1.0],
                );
            } else {
                Self::draw_string(
                    mesh,
                    text_x,
                    line_y,
                    "Obtained via exploration in world biomes.",
                    8.0,
                    12.0,
                    0.5,
                    [0.45, 0.95, 0.55, 1.0],
                );
            }

            Self::draw_string(
                mesh,
                box_x + 20.0,
                box_y + box_h - 20.0,
                "Press U on this item to view crafting recipes using it",
                8.0,
                12.0,
                0.5,
                [0.55, 0.70, 0.85, 1.0],
            );
        } else {
            let slot_size = Self::SLOT_SIZE;
            let slot_gap = Self::SLOT_GAP;
            let grid_start_x = box_x + 28.0;
            let grid_start_y = box_y + 24.0;

            // 3x3 Input Grid
            for gy in 0..3 {
                for gx in 0..3 {
                    let idx = gy * 3 + gx;
                    let sx = grid_start_x + (gx as f32) * (slot_size + slot_gap);
                    let sy = grid_start_y + (gy as f32) * (slot_size + slot_gap);

                    mesh.add_quad(sx, sy, slot_size, slot_size, slot_uv, [1.0, 1.0, 1.0, 0.95]);

                    if let Some(ing_id) = params.recipe.inputs[idx] {
                        if let Some(def) = registry.get(ing_id) {
                            let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                            let pad = 6.0;
                            mesh.add_quad(
                                sx + pad,
                                sy + pad,
                                slot_size - pad * 2.0,
                                slot_size - pad * 2.0,
                                icon_uv,
                                [1.0, 1.0, 1.0, 1.0],
                            );

                            if cursor_pos[0] >= sx
                                && cursor_pos[0] <= sx + slot_size
                                && cursor_pos[1] >= sy
                                && cursor_pos[1] <= sy + slot_size
                            {
                                mesh.add_quad(
                                    sx - 2.0,
                                    sy - 2.0,
                                    slot_size + 4.0,
                                    slot_size + 4.0,
                                    highlight_uv,
                                    [1.0, 1.0, 1.0, 1.0],
                                );
                                *hovered_item_name = Some(def.display_name);
                            }
                        }
                    }
                }
            }

            // Arrow ===>
            let arrow_x = grid_start_x + 3.0 * (slot_size + slot_gap) + 24.0;
            let arrow_y = grid_start_y + (slot_size + slot_gap) * 1.5 - 6.0;
            Self::draw_string(
                mesh,
                arrow_x,
                arrow_y,
                "===>",
                10.0,
                14.0,
                1.0,
                [0.85, 0.85, 0.90, 1.0],
            );

            // Output Slot
            let out_x = arrow_x + 64.0;
            let out_y = grid_start_y + (slot_size + slot_gap) * 1.5 - 28.0;
            let out_size = 56.0;
            mesh.add_quad(
                out_x,
                out_y,
                out_size,
                out_size,
                slot_uv,
                [1.0, 1.0, 1.0, 1.0],
            );

            let out_slot = params.recipe.output;
            if !out_slot.is_empty() {
                if let Some(def) = registry.get(out_slot.item_id) {
                    let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                    let pad = 6.0;
                    mesh.add_quad(
                        out_x + pad,
                        out_y + pad,
                        out_size - pad * 2.0,
                        out_size - pad * 2.0,
                        icon_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    Self::draw_stack_number(mesh, out_x, out_y, out_size, out_slot.count);

                    if cursor_pos[0] >= out_x
                        && cursor_pos[0] <= out_x + out_size
                        && cursor_pos[1] >= out_y
                        && cursor_pos[1] <= out_y + out_size
                    {
                        mesh.add_quad(
                            out_x - 2.0,
                            out_y - 2.0,
                            out_size + 4.0,
                            out_size + 4.0,
                            highlight_uv,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        *hovered_item_name = Some(def.display_name);
                    }
                }
            }

            // Recipe Name at bottom of box
            Self::draw_string(
                mesh,
                box_x + 20.0,
                box_y + box_h - 22.0,
                params.recipe.name,
                8.0,
                12.0,
                0.5,
                [0.70, 0.70, 0.75, 1.0],
            );
        }
    }

    // =========================================================================
    // Phase 2: Title Screen, World Select, Create World, Pause & Settings Menus
    // =========================================================================

    /// Checks whether `cursor_pos` is inside the rectangle `(x, y, w, h)`.
    #[inline]
    pub fn is_rect_hovered(cursor_pos: [f32; 2], x: f32, y: f32, w: f32, h: f32) -> bool {
        cursor_pos[0] >= x
            && cursor_pos[0] <= x + w
            && cursor_pos[1] >= y
            && cursor_pos[1] <= y + h
    }

    /// Draws a sleek Roots & Ridges modal button with drop shadow, slate plate fill,
    /// illuminated gold hover border, active indicator notch, and responsive typography/badges.
    pub fn draw_menu_button(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        cursor_pos: [f32; 2],
        enabled: bool,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let hovered = enabled && Self::is_rect_hovered(cursor_pos, x, y, w, h);

        // 1. Drop shadow
        mesh.add_quad(
            x + 2.0,
            y + 2.0,
            w,
            h,
            white_uv,
            [0.02, 0.03, 0.05, 0.50],
        );

        let (bg, border, text_color, glyph_color, tab_color) = if !enabled {
            (
                [0.10, 0.12, 0.16, 0.60],
                [0.22, 0.25, 0.32, 0.60],
                [0.45, 0.48, 0.55, 0.60],
                [0.45, 0.48, 0.55, 0.60],
                [0.25, 0.28, 0.35, 0.0],
            )
        } else if hovered {
            (
                [0.16, 0.18, 0.24, 0.95],
                [0.96, 0.78, 0.28, 1.0], // illuminated gold border
                [1.0, 0.98, 0.95, 1.0],  // bright text
                [1.0, 0.85, 0.35, 1.0],  // glowing gold glyph
                [1.0, 0.80, 0.25, 1.0],  // prominent left active indicator notch
            )
        } else {
            (
                [0.08, 0.10, 0.15, 0.88],
                [0.28, 0.32, 0.42, 0.85],
                [0.90, 0.92, 0.96, 0.95],
                [0.85, 0.70, 0.30, 0.90],
                [0.85, 0.70, 0.30, 0.0],
            )
        };

        // 2. Outer border
        mesh.add_quad(x, y, w, h, white_uv, border);
        // 3. Slate plate fill
        mesh.add_quad(x + 1.0, y + 1.0, w - 2.0, h - 2.0, white_uv, bg);

        // 4. Left active indicator notch (illuminates when hovered)
        if hovered {
            mesh.add_quad(x, y, 4.0, h, white_uv, tab_color);
        }

        // 5. Intelligent Content Parsing & Typography
        if label.contains(": ") {
            // Key-Value toggle or setting
            let parts: Vec<&str> = label.splitn(2, ": ").collect();
            let key_label = parts[0];
            let val_label = parts[1];

            let (char_w, char_h) = if h >= 36.0 { (9.0, 12.5) } else { (8.0, 11.0) };
            let ty = y + (h - char_h) * 0.5;
            let tx = if key_label == "Skin" { x + 44.0 } else { x + 12.0 };

            // Draw key label on left
            Self::draw_string(
                mesh,
                tx + 1.0,
                ty + 1.0,
                key_label,
                char_w,
                char_h,
                0.8,
                [0.03, 0.04, 0.06, 0.90],
            );
            Self::draw_string(mesh, tx, ty, key_label, char_w, char_h, 0.8, text_color);

            // Draw value pill badge on right
            let badge_pad_x = 8.0;
            let badge_char_w = 8.5;
            let badge_char_h = 12.0;
            let val_w = Self::string_width(val_label, badge_char_w, 0.8);
            let badge_w = val_w + badge_pad_x * 2.0;
            let badge_h = (h - 10.0).max(18.0);
            let badge_x = x + w - badge_w - 8.0;
            let badge_y = y + (h - badge_h) * 0.5;

            let (badge_bg, badge_border, badge_text_col) = if val_label == "ON" {
                (
                    [0.08, 0.22, 0.12, 0.95],
                    [0.35, 0.95, 0.55, 0.95],
                    [0.55, 1.0, 0.70, 1.0],
                )
            } else if val_label == "OFF" {
                (
                    [0.11, 0.13, 0.17, 0.95],
                    [0.32, 0.36, 0.44, 0.85],
                    [0.60, 0.65, 0.75, 0.85],
                )
            } else {
                (
                    [0.14, 0.13, 0.10, 0.95],
                    [0.85, 0.70, 0.30, 0.90],
                    [1.0, 0.88, 0.40, 1.0],
                )
            };

            mesh.add_quad(badge_x, badge_y, badge_w, badge_h, white_uv, badge_border);
            mesh.add_quad(
                badge_x + 1.0,
                badge_y + 1.0,
                badge_w - 2.0,
                badge_h - 2.0,
                white_uv,
                badge_bg,
            );
            Self::draw_string(
                mesh,
                badge_x + (badge_w - val_w) * 0.5,
                badge_y + (badge_h - badge_char_h) * 0.5,
                val_label,
                badge_char_w,
                badge_char_h,
                0.8,
                badge_text_col,
            );
        } else if label.len() >= 2
            && label.chars().nth(1) == Some(' ')
            && matches!(
                label.chars().next().unwrap(),
                '>' | '+' | '<' | 'X' | '-' | '*' | '#' | '@' | '?' | '•'
            )
        {
            let glyph_char = label.chars().next().unwrap();
            let mut glyph_buf = [0u8; 4];
            let glyph_str = glyph_char.encode_utf8(&mut glyph_buf);
            let sub_text = &label[2..];

            let (char_w, char_h) = if h >= 38.0 { (10.0, 13.5) } else { (8.5, 12.0) };
            let glyph_char_w = char_w * 0.95;
            let glyph_char_h = char_h;
            let spacing = 1.0;

            let text_w = Self::string_width(sub_text, char_w, spacing);
            let glyph_w = Self::string_width(glyph_str, glyph_char_w, spacing);

            if w >= 220.0 {
                let gx = x + 14.0;
                let gy = y + (h - glyph_char_h) * 0.5;
                let tx = x + 34.0;
                let ty = y + (h - char_h) * 0.5;

                Self::draw_string(
                    mesh,
                    gx + 1.0,
                    gy + 1.0,
                    glyph_str,
                    glyph_char_w,
                    glyph_char_h,
                    spacing,
                    [0.03, 0.04, 0.06, 0.90],
                );
                Self::draw_string(
                    mesh,
                    gx,
                    gy,
                    glyph_str,
                    glyph_char_w,
                    glyph_char_h,
                    spacing,
                    glyph_color,
                );

                Self::draw_string(
                    mesh,
                    tx + 1.0,
                    ty + 1.0,
                    sub_text,
                    char_w,
                    char_h,
                    spacing,
                    [0.03, 0.04, 0.06, 0.90],
                );
                Self::draw_string(mesh, tx, ty, sub_text, char_w, char_h, spacing, text_color);
            } else {
                let total_w = glyph_w + 6.0 + text_w;
                let start_x = x + (w - total_w) * 0.5;
                let gy = y + (h - glyph_char_h) * 0.5;
                let ty = y + (h - char_h) * 0.5;

                Self::draw_string(
                    mesh,
                    start_x + 1.0,
                    gy + 1.0,
                    glyph_str,
                    glyph_char_w,
                    glyph_char_h,
                    spacing,
                    [0.03, 0.04, 0.06, 0.90],
                );
                Self::draw_string(
                    mesh,
                    start_x,
                    gy,
                    glyph_str,
                    glyph_char_w,
                    glyph_char_h,
                    spacing,
                    glyph_color,
                );

                let tx = start_x + glyph_w + 6.0;
                Self::draw_string(
                    mesh,
                    tx + 1.0,
                    ty + 1.0,
                    sub_text,
                    char_w,
                    char_h,
                    spacing,
                    [0.03, 0.04, 0.06, 0.90],
                );
                Self::draw_string(mesh, tx, ty, sub_text, char_w, char_h, spacing, text_color);
            }
        } else if label.ends_with(" >") {
            let sub_text = &label[..label.len() - 2];
            let (char_w, char_h) = if h >= 38.0 { (10.0, 13.5) } else { (8.5, 12.0) };
            let glyph_char_w = char_w * 0.95;
            let glyph_char_h = char_h;
            let spacing = 1.0;
            let text_w = Self::string_width(sub_text, char_w, spacing);
            let glyph_w = Self::string_width(">", glyph_char_w, spacing);
            let total_w = text_w + 6.0 + glyph_w;
            let start_x = x + (w - total_w) * 0.5;
            let ty = y + (h - char_h) * 0.5;
            let gy = y + (h - glyph_char_h) * 0.5;

            Self::draw_string(
                mesh,
                start_x + 1.0,
                ty + 1.0,
                sub_text,
                char_w,
                char_h,
                spacing,
                [0.03, 0.04, 0.06, 0.90],
            );
            Self::draw_string(mesh, start_x, ty, sub_text, char_w, char_h, spacing, text_color);

            let gx = start_x + text_w + 6.0;
            Self::draw_string(
                mesh,
                gx + 1.0,
                gy + 1.0,
                ">",
                glyph_char_w,
                glyph_char_h,
                spacing,
                [0.03, 0.04, 0.06, 0.90],
            );
            Self::draw_string(mesh, gx, gy, ">", glyph_char_w, glyph_char_h, spacing, glyph_color);
        } else {
            let (mut char_w, mut char_h) = if h >= 38.0 { (10.0, 13.5) } else { (8.5, 12.0) };
            let spacing = 1.0;
            let max_w = (w - 20.0).max(10.0);
            let nat_w = Self::string_width(label, char_w, spacing);
            if nat_w > max_w {
                let scale = max_w / nat_w;
                char_w *= scale;
                char_h *= scale;
            }
            let text_w = Self::string_width(label, char_w, spacing);
            let tx = x + (w - text_w) * 0.5;
            let ty = y + (h - char_h) * 0.5;

            Self::draw_string(
                mesh,
                tx + 1.0,
                ty + 1.0,
                label,
                char_w,
                char_h,
                spacing,
                [0.03, 0.04, 0.06, 0.90],
            );
            Self::draw_string(mesh, tx, ty, label, char_w, char_h, spacing, text_color);
        }
    }

    /// Draws a labeled text input box with blinking caret when focused.
    pub fn draw_text_input_box(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        value: &str,
        placeholder: &str,
        focused: bool,
        cursor_pos: [f32; 2],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let hovered = Self::is_rect_hovered(cursor_pos, x, y, w, h);

        // Label above input box
        Self::draw_string(
            mesh,
            x,
            y - 20.0,
            label,
            9.5,
            13.0,
            1.0,
            [0.82, 0.84, 0.88, 1.0],
        );

        let border_color = if focused {
            [0.95, 0.85, 0.35, 1.0]
        } else if hovered {
            [0.65, 0.68, 0.78, 1.0]
        } else {
            [0.42, 0.44, 0.50, 1.0]
        };

        // Border
        mesh.add_quad(x - 2.0, y - 2.0, w + 4.0, h + 4.0, white_uv, border_color);
        // Dark inset background
        mesh.add_quad(x, y, w, h, white_uv, [0.06, 0.07, 0.09, 0.96]);

        let char_w = 10.5;
        let char_h = 15.0;
        let spacing = 1.0;
        let ty = y + (h - char_h) * 0.5;

        if value.is_empty() && !focused {
            Self::draw_string(
                mesh,
                x + 10.0,
                ty,
                placeholder,
                char_w,
                char_h,
                spacing,
                [0.45, 0.46, 0.52, 0.90],
            );
        } else {
            let display = if focused {
                format!("{}_", value)
            } else {
                value.to_string()
            };
            Self::draw_string(
                mesh,
                x + 10.0,
                ty,
                &display,
                char_w,
                char_h,
                spacing,
                [0.98, 0.98, 1.0, 1.0],
            );
        }
    }

    pub const TITLE_BTN_SINGLEPLAYER: usize = 0;
    pub const TITLE_BTN_NEW_WORLD: usize = 1;
    pub const TITLE_BTN_MULTIPLAYER: usize = 2;
    pub const TITLE_BTN_CHARACTER_CREATOR: usize = 3;
    pub const TITLE_BTN_SETTINGS: usize = 4;
    pub const TITLE_BTN_GUIDE: usize = 5;
    pub const TITLE_BTN_ACHIEVEMENTS: usize = 6;
    pub const TITLE_BTN_QUIT: usize = 7;
    pub const TITLE_BTN_QUICK_RESUME: usize = 8;
    pub const TITLE_BUTTON_COUNT: usize = 9;

    /// Returns the 9 interactive button rectangles on the Roots & Ridges Title Screen:
    /// `[0: Singleplayer, 1: New Expedition, 2: Multiplayer, 3: Character Creator, 4: Settings, 5: Guide, 6: Achievements, 7: Quit, 8: Quick Resume]`.
    pub fn title_screen_button_rects(screen_w: f32, screen_h: f32) -> [[f32; 4]; Self::TITLE_BUTTON_COUNT] {
        let dock_w = 340.0_f32.min(screen_w * 0.35);
        let pad_x = 28.0;
        let btn_w = dock_w - pad_x * 2.0;
        let btn_h = 36.0;
        let gap = 7.0;
        let start_y = 136.0_f32.min(screen_h * 0.19);

        let card_w = 340.0_f32.min(screen_w * 0.34);
        let card_h = 160.0;
        let card_x = screen_w - card_w - 32.0;
        let card_y = 44.0;
        let res_btn_h = 36.0;
        let res_btn_w = card_w - 32.0;
        let res_btn_x = card_x + 16.0;
        let res_btn_y = card_y + card_h - res_btn_h - 14.0;

        [
            [pad_x, start_y, btn_w, btn_h],                              // 0: Continue Expedition / Singleplayer
            [pad_x, start_y + (btn_h + gap), btn_w, btn_h],              // 1: New Expedition
            [pad_x, start_y + (btn_h + gap) * 2.0, btn_w, btn_h],        // 2: Multiplayer & Servers
            [pad_x, start_y + (btn_h + gap) * 3.0, btn_w, btn_h],        // 3: Character Creator
            [pad_x, start_y + (btn_h + gap) * 4.0, btn_w, btn_h],        // 4: Game Settings
            [pad_x, start_y + (btn_h + gap) * 5.0, btn_w, btn_h],        // 5: How to Play & Guide
            [pad_x, start_y + (btn_h + gap) * 6.0, btn_w, btn_h],        // 6: Achievements
            [pad_x, start_y + (btn_h + gap) * 7.0 + 6.0, btn_w, btn_h],  // 7: Exit to Desktop
            [res_btn_x, res_btn_y, res_btn_w, res_btn_h],                // 8: Quick Resume / First World
        ]
    }

    /// Draws a sleek, modern sidebar dock button for Roots & Ridges.
    pub fn draw_sidebar_button(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        glyph: &str,
        label: &str,
        cursor_pos: [f32; 2],
        enabled: bool,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let hovered = enabled && Self::is_rect_hovered(cursor_pos, x, y, w, h);

        // Drop shadow
        mesh.add_quad(x + 2.0, y + 2.0, w, h, white_uv, [0.02, 0.03, 0.05, 0.50]);

        let (bg, border, glyph_color, text_color, tab_color) = if !enabled {
            (
                [0.10, 0.12, 0.16, 0.60],
                [0.22, 0.25, 0.32, 0.60],
                [0.45, 0.48, 0.55, 0.60],
                [0.45, 0.48, 0.55, 0.60],
                [0.25, 0.28, 0.35, 0.0],
            )
        } else if hovered {
            (
                [0.16, 0.18, 0.24, 0.95],
                [0.96, 0.78, 0.28, 1.0], // illuminated gold border
                [1.0, 0.85, 0.35, 1.0],  // glowing gold glyph
                [1.0, 0.98, 0.95, 1.0],  // bright text
                [1.0, 0.80, 0.25, 1.0],  // prominent left active indicator notch
            )
        } else {
            (
                [0.08, 0.10, 0.15, 0.82],
                [0.28, 0.32, 0.42, 0.85],
                [0.85, 0.70, 0.30, 0.90],
                [0.90, 0.92, 0.96, 0.95],
                [0.85, 0.70, 0.30, 0.0],
            )
        };

        // Outer border
        mesh.add_quad(x, y, w, h, white_uv, border);
        // Slate plate fill
        mesh.add_quad(x + 1.0, y + 1.0, w - 2.0, h - 2.0, white_uv, bg);

        // Left active indicator notch (slides out when hovered)
        if hovered {
            mesh.add_quad(x, y, 4.0, h, white_uv, tab_color);
        }

        // Glyph tag box on left
        let glyph_w = 26.0;
        let glyph_char_w = 9.0;
        let glyph_char_h = 13.0;
        let gx = x + 12.0;
        let gy = y + (h - glyph_char_h) * 0.5;
        Self::draw_string(mesh, gx, gy, glyph, glyph_char_w, glyph_char_h, 1.0, glyph_color);

        // Button label
        let char_w = 10.5;
        let char_h = 14.0;
        let spacing = 1.0;
        let tx = x + 12.0 + glyph_w;
        let ty = y + (h - char_h) * 0.5;

        // Shadow & label
        Self::draw_string(mesh, tx + 1.0, ty + 1.0, label, char_w, char_h, spacing, [0.03, 0.04, 0.06, 0.90]);
        Self::draw_string(mesh, tx, ty, label, char_w, char_h, spacing, text_color);
    }

    /// Draws the Recent Expedition quick-resume card on the Title Screen.
    pub fn draw_quick_resume_card(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        recent: Option<&blockgame_core::save::WorldMetadata>,
        cursor_pos: [f32; 2],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Card drop shadow
        mesh.add_quad(x + 3.0, y + 3.0, w, h, white_uv, [0.01, 0.02, 0.04, 0.60]);
        // Outer border
        mesh.add_quad(x, y, w, h, white_uv, [0.38, 0.42, 0.52, 0.85]);
        // Slate background
        mesh.add_quad(x + 1.5, y + 1.5, w - 3.0, h - 3.0, white_uv, [0.06, 0.08, 0.12, 0.85]);

        // Top decorative accent bar
        mesh.add_quad(x + 1.5, y + 1.5, w - 3.0, 3.0, white_uv, [0.85, 0.68, 0.28, 0.95]);

        // Header label
        Self::draw_string(
            mesh,
            x + 16.0,
            y + 14.0,
            "RECENT EXPEDITION",
            8.0,
            11.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        let btn_h = 36.0;
        let btn_w = w - 32.0;
        let btn_x = x + 16.0;
        let btn_y = y + h - btn_h - 14.0;

        if let Some(world) = recent {
            // World name
            let name_display = if world.name.len() > 22 {
                format!("{}...", &world.name[..20])
            } else {
                world.name.clone()
            };
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 34.0,
                &name_display,
                11.0,
                15.0,
                1.0,
                [0.98, 0.98, 1.0, 1.0],
            );

            // World stats: Mode & Seed
            let mode_str = match world.game_mode {
                blockgame_core::GameMode::Creative => "Creative Mode",
                blockgame_core::GameMode::Survival => "Survival Mode",
            };
            let stats_str = format!("{}  *  Seed: {}", mode_str, world.seed);
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 58.0,
                &stats_str,
                7.5,
                10.5,
                0.8,
                [0.72, 0.76, 0.84, 0.90],
            );

            // Playtime
            let hours = world.total_time / 3600.0;
            let time_str = if hours >= 1.0 {
                format!("Playtime: {:.1} hrs", hours)
            } else {
                let mins = (world.total_time / 60.0).max(1.0);
                format!("Playtime: {:.0} mins", mins)
            };
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 76.0,
                &time_str,
                7.0,
                10.0,
                0.8,
                [0.55, 0.60, 0.70, 0.85],
            );

            // Resume button
            let btn_hovered = Self::is_rect_hovered(cursor_pos, btn_x, btn_y, btn_w, btn_h);
            let (bg, border, text_col) = if btn_hovered {
                (
                    [0.22, 0.48, 0.28, 0.98],
                    [0.45, 0.95, 0.55, 1.0],
                    [1.0, 1.0, 1.0, 1.0],
                )
            } else {
                (
                    [0.15, 0.35, 0.20, 0.90],
                    [0.32, 0.70, 0.40, 0.90],
                    [0.90, 0.98, 0.92, 0.95],
                )
            };
            mesh.add_quad(btn_x, btn_y, btn_w, btn_h, white_uv, border);
            mesh.add_quad(btn_x + 1.0, btn_y + 1.0, btn_w - 2.0, btn_h - 2.0, white_uv, bg);
            let btn_text = "> RESUME EXPEDITION";
            let tw = Self::string_width(btn_text, 9.5, 1.0);
            Self::draw_string(
                mesh,
                btn_x + (btn_w - tw) * 0.5,
                btn_y + (btn_h - 13.0) * 0.5,
                btn_text,
                9.5,
                13.0,
                1.0,
                text_col,
            );
        } else {
            // No world saved yet
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 36.0,
                "Ready for Discovery",
                11.0,
                15.0,
                1.0,
                [0.95, 0.95, 0.98, 1.0],
            );
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 60.0,
                "No saved expeditions yet.",
                8.0,
                11.0,
                0.8,
                [0.72, 0.75, 0.82, 0.90],
            );
            Self::draw_string(
                mesh,
                x + 16.0,
                y + 78.0,
                "Generate your first infinite realm.",
                7.0,
                10.0,
                0.8,
                [0.55, 0.60, 0.70, 0.85],
            );

            // Create button
            let btn_hovered = Self::is_rect_hovered(cursor_pos, btn_x, btn_y, btn_w, btn_h);
            let (bg, border, text_col) = if btn_hovered {
                (
                    [0.24, 0.38, 0.55, 0.98],
                    [0.50, 0.80, 1.0, 1.0],
                    [1.0, 1.0, 1.0, 1.0],
                )
            } else {
                (
                    [0.16, 0.26, 0.38, 0.90],
                    [0.35, 0.55, 0.75, 0.90],
                    [0.90, 0.95, 1.0, 0.95],
                )
            };
            mesh.add_quad(btn_x, btn_y, btn_w, btn_h, white_uv, border);
            mesh.add_quad(btn_x + 1.0, btn_y + 1.0, btn_w - 2.0, btn_h - 2.0, white_uv, bg);
            let btn_text = "+ CREATE FIRST WORLD";
            let tw = Self::string_width(btn_text, 9.5, 1.0);
            Self::draw_string(
                mesh,
                btn_x + (btn_w - tw) * 0.5,
                btn_y + (btn_h - 13.0) * 0.5,
                btn_text,
                9.5,
                13.0,
                1.0,
                text_col,
            );
        }
    }

    /// Builds the Title Screen & Main Menu overlay (default without recent world info).
    pub fn build_title_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
    ) {
        Self::build_title_screen_with_recent_world(mesh, screen_w, screen_h, cursor_pos, None);
    }

    /// Builds the Roots & Ridges Main Menu overlay with left cinematic sidebar and recent world card.
    pub fn build_title_screen_with_recent_world(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
        recent_world: Option<&blockgame_core::save::WorldMetadata>,
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // 1. Subtle cinematic vignette darkening over the live 3D background panorama
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.28],
        );

        let dock_w = 340.0_f32.min(screen_w * 0.35);

        // 2. Left Translucent Dark-Slate Dock Panel
        mesh.add_quad(
            0.0,
            0.0,
            dock_w,
            screen_h,
            white_uv,
            [0.05, 0.07, 0.10, 0.88],
        );
        // Right dock border accent (warm amber / gold edge trim)
        mesh.add_quad(
            dock_w - 2.0,
            0.0,
            2.0,
            screen_h,
            white_uv,
            [0.85, 0.68, 0.28, 0.85],
        );

        // 3. Title Emblem & Typography
        let pad_x = 28.0;
        let title_y = 36.0;

        // Top decorative gold pip
        mesh.add_quad(
            pad_x,
            title_y - 8.0,
            32.0,
            2.5,
            white_uv,
            [0.95, 0.78, 0.30, 0.95],
        );

        // Title: ROOTS & RIDGES
        let title_text = "ROOTS & RIDGES";
        let title_char_w = 19.0;
        let title_char_h = 26.0;
        let title_spacing = 2.5;

        // 3D block shadow for title
        Self::draw_string(
            mesh,
            pad_x + 2.5,
            title_y + 2.5,
            title_text,
            title_char_w,
            title_char_h,
            title_spacing,
            [0.02, 0.03, 0.05, 0.95],
        );
        Self::draw_string(
            mesh,
            pad_x,
            title_y,
            title_text,
            title_char_w,
            title_char_h,
            title_spacing,
            [0.98, 0.97, 0.94, 1.0],
        );

        // Subtitle / Tagline
        let sub_text = "A WILDERNESS OF ENDLESS HORIZONS";
        let sub_y = title_y + title_char_h + 8.0;
        Self::draw_string(
            mesh,
            pad_x + 1.0,
            sub_y + 1.0,
            sub_text,
            7.5,
            10.5,
            0.8,
            [0.02, 0.03, 0.05, 0.90],
        );
        Self::draw_string(
            mesh,
            pad_x,
            sub_y,
            sub_text,
            7.5,
            10.5,
            0.8,
            [0.95, 0.78, 0.30, 0.95],
        );

        // Thin divider line
        mesh.add_quad(
            pad_x,
            sub_y + 18.0,
            dock_w - pad_x * 2.0,
            1.5,
            white_uv,
            [0.32, 0.36, 0.45, 0.65],
        );

        // 4. Navigation Buttons inside Left Dock
        let rects = Self::title_screen_button_rects(screen_w, screen_h);
        let button_defs = [
            (">", "Continue Expedition"),
            ("+", "New Expedition"),
            ("@", "Multiplayer & Servers"),
            ("&", "Character Creator"),
            ("*", "Game Settings"),
            ("?", "How to Play & Guide"),
            ("!", "Achievements"),
            ("X", "Exit to Desktop"),
        ];

        for (i, (glyph, label)) in button_defs.iter().enumerate() {
            let rect = rects[i];
            Self::draw_sidebar_button(
                mesh,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                glyph,
                label,
                cursor_pos,
                true,
            );
        }

        // 5. Recent Expedition Quick-Resume Card (Floating on Upper Right)
        let card_w = 340.0_f32.min(screen_w * 0.34);
        let card_h = 160.0;
        let card_x = screen_w - card_w - 32.0;
        let card_y = 44.0;
        Self::draw_quick_resume_card(mesh, card_x, card_y, card_w, card_h, recent_world, cursor_pos);

        // 6. Bottom footer version text in left dock
        let ver_text = concat!("Roots & Ridges v", env!("CARGO_PKG_VERSION"));
        Self::draw_string(
            mesh,
            pad_x,
            screen_h - 32.0,
            ver_text,
            8.0,
            11.0,
            0.8,
            [0.72, 0.76, 0.84, 0.85],
        );
        Self::draw_string(
            mesh,
            pad_x,
            screen_h - 18.0,
            "Voxel Engine  *  32x Retro",
            7.0,
            9.5,
            0.5,
            [0.48, 0.52, 0.60, 0.75],
        );
    }

    /// Returns the 3 button rectangles for the Guide screen `[Previous, Next, Back to Title]`.
    pub fn guide_screen_button_rects(screen_w: f32, screen_h: f32) -> [[f32; 4]; 3] {
        let btn_w = 160.0;
        let btn_h = 40.0;
        let back_x = (screen_w - btn_w) * 0.5;
        let y = screen_h - 58.0;
        [
            [back_x - 170.0, y, 140.0, btn_h], // 0: Previous Page
            [back_x + 190.0, y, 140.0, btn_h], // 1: Next Page
            [back_x, y, btn_w, btn_h],         // 2: Back to Title
        ]
    }

    /// Builds the Adventurer's Field Guide screen with game controls, mechanics, and MMO skill info.
    pub fn build_guide_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        page: usize,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Background backdrop
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.04, 0.06, 0.09, 0.90]);

        // Main modal container
        let modal_w = 720.0_f32.min(screen_w - 60.0);
        let modal_h = 520.0_f32.min(screen_h - 90.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = 36.0;

        mesh.add_quad(modal_x + 3.0, modal_y + 3.0, modal_w, modal_h, white_uv, [0.01, 0.02, 0.04, 0.60]);
        mesh.add_quad(modal_x, modal_y, modal_w, modal_h, white_uv, [0.38, 0.42, 0.52, 0.85]);
        mesh.add_quad(modal_x + 1.5, modal_y + 1.5, modal_w - 3.0, modal_h - 3.0, white_uv, [0.07, 0.09, 0.13, 0.96]);

        // Top gold accent bar
        mesh.add_quad(modal_x + 1.5, modal_y + 1.5, modal_w - 3.0, 3.0, white_uv, [0.85, 0.68, 0.28, 0.95]);

        // Title
        let header_text = "ADVENTURER'S FIELD GUIDE";
        let hw = Self::string_width(header_text, 14.0, 1.5);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - hw) * 0.5,
            modal_y + 18.0,
            header_text,
            14.0,
            18.0,
            1.5,
            [0.98, 0.98, 1.0, 1.0],
        );

        let page_title = match page % 3 {
            0 => "Section 1: Controls & Exploration",
            1 => "Section 2: Survival, Crafting & Fishing",
            _ => "Section 3: MMO Skills & Progression",
        };
        let ptw = Self::string_width(page_title, 9.5, 1.0);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - ptw) * 0.5,
            modal_y + 42.0,
            page_title,
            9.5,
            13.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        // Content cards based on page
        let content_y = modal_y + 70.0;
        let pad = 24.0;
        let card_w = modal_w - pad * 2.0;

        match page % 3 {
            0 => {
                let controls = [
                    ("W / A / S / D", "Move Forward, Strafe Left, Backward, Right"),
                    ("Space", "Jump / Ascend (Hold in water to swim up)"),
                    ("Left Ctrl", "Sprint (Consumes stamina over time)"),
                    ("E", "Open Inventory, 2x2 Crafting & Equipment"),
                    ("1 - 9 / Scroll", "Select Active Hotbar Slot (Auto-tucks after 4s)"),
                    ("Left Click", "Attack Entity / Mine targeted block"),
                    ("Right Click", "Place Block / Use Item / Eat Food / Open Container"),
                    ("K", "Open MMO Skills & Milestone Perks menu"),
                    ("F", "Toggle Flight Mode (Creative Mode only)"),
                    ("T", "Open In-Game Chat / Slash Commands"),
                    ("F3 / F5", "Toggle Debug HUD Overlay / Cycle 3rd Person View"),
                ];
                for (i, (key, desc)) in controls.iter().enumerate() {
                    let y = content_y + (i as f32) * 29.0;
                    if y + 25.0 > modal_y + modal_h - 10.0 { break; }
                    mesh.add_quad(modal_x + pad, y, 140.0, 22.0, white_uv, [0.12, 0.15, 0.20, 0.85]);
                    Self::draw_string(mesh, modal_x + pad + 8.0, y + 5.0, key, 8.5, 12.0, 0.8, [1.0, 0.82, 0.35, 1.0]);
                    Self::draw_string(mesh, modal_x + pad + 155.0, y + 5.0, desc, 8.5, 12.0, 0.8, [0.90, 0.92, 0.96, 0.95]);
                }
            }
            1 => {
                let survival_tips = [
                    ("1. Crafting Tools", "Chop Wood -> 4 Planks -> Crafting Table (3x3 grid) for Wooden & Stone Pickaxes."),
                    ("2. Vitals & Food", "Vitals (HP) and Stamina (ST) are tracked at the bottom. Eat Bread or Cooked Meat to restore."),
                    ("3. Night & Monsters", "Hostile Zombies, Skeletons, and Spiders hunt in darkness. Place Torches and sleep in Beds."),
                    ("4. Mining & Smelting", "Smelt Raw Iron, Copper, and Gold in Furnaces using Coal or Wood for advanced equipment."),
                    ("5. Fishing & Tackle", "Craft Fishing Rods with Lines, Lures, and Bait. Hook fish in rivers/lakes with timed reels."),
                    ("6. Armor & Bows", "Equip Leather, Iron, or Diamond Armor for defense points (AP). Craft Bows & Arrows for ranged combat."),
                ];
                for (i, (title, text)) in survival_tips.iter().enumerate() {
                    let y = content_y + (i as f32) * 58.0;
                    if y + 50.0 > modal_y + modal_h - 10.0 { break; }
                    mesh.add_quad(modal_x + pad, y, card_w, 48.0, white_uv, [0.10, 0.12, 0.17, 0.75]);
                    mesh.add_quad(modal_x + pad, y, 3.0, 48.0, white_uv, [0.35, 0.65, 0.85, 0.95]);
                    Self::draw_string(mesh, modal_x + pad + 12.0, y + 7.0, title, 9.5, 13.0, 1.0, [1.0, 0.85, 0.35, 1.0]);
                    Self::draw_string(mesh, modal_x + pad + 12.0, y + 26.0, text, 8.0, 11.0, 0.8, [0.85, 0.88, 0.94, 0.95]);
                }
            }
            _ => {
                let skills_info = [
                    ("Combat", "Level up by defeating monsters. Unlocks critical strike chance, sweep attacks, and bonus damage."),
                    ("Woodcutting", "Level up by chopping trees. Unlocks tree feller chances and bonus log drops."),
                    ("Mining", "Level up by mining stone & ores. Unlocks double ore drops, auto-smelting, and vein mining."),
                    ("Fishing", "Level up by catching fish. Unlocks rare fish species, treasure lures, and master angler perks."),
                    ("Digging", "Level up by excavating dirt, sand, and gravel. Unlocks excavation speed and buried treasures."),
                    ("Farming", "Level up by harvesting crops and breeding animals. Unlocks instant regrowth and twin breeding."),
                    ("Agility", "Level up by running and jumping. Unlocks increased sprint speed, reduced fall damage, and leap."),
                ];
                for (i, (skill, desc)) in skills_info.iter().enumerate() {
                    let y = content_y + (i as f32) * 52.0;
                    if y + 46.0 > modal_y + modal_h - 10.0 { break; }
                    mesh.add_quad(modal_x + pad, y, card_w, 44.0, white_uv, [0.10, 0.12, 0.17, 0.75]);
                    mesh.add_quad(modal_x + pad, y, 3.0, 44.0, white_uv, [0.85, 0.68, 0.28, 0.95]);
                    Self::draw_string(mesh, modal_x + pad + 12.0, y + 6.0, skill, 9.5, 13.0, 1.0, [0.95, 0.78, 0.30, 1.0]);
                    Self::draw_string(mesh, modal_x + pad + 12.0, y + 23.0, desc, 8.0, 11.0, 0.8, [0.85, 0.88, 0.94, 0.95]);
                }
            }
        }

        // Bottom navigation buttons
        let btns = Self::guide_screen_button_rects(screen_w, screen_h);
        Self::draw_menu_button(mesh, btns[0][0], btns[0][1], btns[0][2], btns[0][3], "< Previous", cursor_pos, true);
        Self::draw_menu_button(mesh, btns[1][0], btns[1][1], btns[1][2], btns[1][3], "Next >", cursor_pos, true);
        Self::draw_menu_button(mesh, btns[2][0], btns[2][1], btns[2][2], btns[2][3], "Back to Title", cursor_pos, true);
    }

    /// Builds the Expedition Guide with controller pixel-art button glyphs on page 0 when in Controller Input Mode.
    pub fn build_guide_screen_with_input_mode(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        page: usize,
        cursor_pos: [f32; 2],
        input_mode: InputMode,
        glyph_family: ControllerFamily,
    ) {
        Self::build_guide_screen(mesh, screen_w, screen_h, page, cursor_pos);
        if input_mode != InputMode::Controller || page % 3 != 0 {
            return;
        }
        // Overdraw Page 0 content area with controller glyph controls table
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let modal_w = 720.0_f32.min(screen_w - 40.0);
        let modal_h = 460.0_f32.min(screen_h - 100.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5 - 24.0).max(20.0);
        let content_y = modal_y + 70.0;
        let pad = 24.0;
        mesh.add_quad(
            modal_x + 4.0,
            content_y - 4.0,
            modal_w - 8.0,
            modal_h - 74.0,
            white_uv,
            [0.06, 0.08, 0.12, 0.96],
        );

        let pad_controls: [(Option<GamepadButton>, &str, &str); 11] = [
            (None, "L-Stick / R-Stick", "Analog Movement & Camera Look"),
            (Some(GamepadButton::South), "Jump / Fly Up", "Jump, Swim Up, Double-Tap to Fly"),
            (Some(GamepadButton::East), "Sneak / Fly Down", "Sneak (Toggle/Hold) & Descend in Flight"),
            (Some(GamepadButton::LeftStick), "Sprint", "Toggle Sprint (Clears when stopped)"),
            (Some(GamepadButton::RightTrigger), "Mine / Attack", "Break Blocks & Attack Entities"),
            (Some(GamepadButton::LeftTrigger), "Place / Use", "Place Block, Use Item, Eat, Fish"),
            (Some(GamepadButton::RightBumper), "Hotbar Cycle", "Cycle Hotbar Slots / Switch UI Tabs"),
            (Some(GamepadButton::North), "Inventory", "Open Inventory & Quick-Transfer in UI"),
            (Some(GamepadButton::West), "Backpack", "Open Portable Backpack / Half-Stack in UI"),
            (Some(GamepadButton::DPadUp), "Quick Menu", "Skills, Achievements, Guide & Debug HUD"),
            (Some(GamepadButton::Start), "Pause Menu", "Pause Game / Close Overlay"),
        ];
        for (i, (glyph_opt, label, desc)) in pad_controls.iter().enumerate() {
            let y = content_y + (i as f32) * 29.0;
            if y + 25.0 > modal_y + modal_h - 10.0 {
                break;
            }
            mesh.add_quad(modal_x + pad, y, 162.0, 22.0, white_uv, [0.12, 0.15, 0.20, 0.88]);
            let text_x = if let Some(btn) = glyph_opt {
                Self::draw_gamepad_glyph(
                    mesh,
                    modal_x + pad + 4.0,
                    y + 2.0,
                    18.0,
                    glyph_family,
                    *btn,
                    [1.0; 4],
                );
                modal_x + pad + 26.0
            } else {
                modal_x + pad + 8.0
            };
            Self::draw_string(mesh, text_x, y + 5.0, label, 8.0, 11.5, 0.8, [1.0, 0.82, 0.35, 1.0]);
            Self::draw_string(
                mesh,
                modal_x + pad + 174.0,
                y + 5.0,
                desc,
                8.5,
                12.0,
                0.8,
                [0.90, 0.92, 0.96, 0.95],
            );
        }
    }

    /// Draws a compact controller glyph legend bar for JEI Recipe (`LT`) / Usage (`RT`) and Quick-Transfer (`North`) in inventory screens.
    pub fn draw_jei_controller_footer(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        glyph_family: ControllerFamily,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let bar_w = 340.0;
        let bar_h = 26.0;
        let bx = (screen_w - bar_w) * 0.5;
        let by = screen_h - bar_h - 8.0;
        mesh.add_quad(bx, by, bar_w, bar_h, white_uv, [0.06, 0.08, 0.13, 0.86]);

        let items = [
            (GamepadButton::LeftTrigger, "Recipe"),
            (GamepadButton::RightTrigger, "Uses"),
            (GamepadButton::North, "Quick Move"),
            (GamepadButton::West, "Half"),
        ];
        let mut x = bx + 10.0;
        for (btn, label) in items {
            Self::draw_gamepad_glyph(mesh, x, by + 4.0, 18.0, glyph_family, btn, [1.0; 4]);
            x += 21.0;
            Self::draw_string(mesh, x, by + 7.5, label, 7.5, 10.5, 0.7, [0.86, 0.90, 0.96, 0.95]);
            x += Self::string_width(label, 7.5, 0.7) + 12.0;
        }
    }

    /// Returns the world list row rectangles (up to 5 visible slots) and the 4 bottom action buttons
    /// `[Play Selected World, Create New World, Delete World, Back]` for the World Select screen.
    pub fn world_select_layout(
        screen_w: f32,
        screen_h: f32,
        world_count: usize,
    ) -> (Vec<[f32; 4]>, [[f32; 4]; 4]) {
        let modal_w = 700.0_f32.min(screen_w - 40.0);
        let modal_h = 540.0_f32.min(screen_h - 60.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(30.0);

        let list_w = modal_w - 48.0;
        let list_x = modal_x + 24.0;
        let list_top = modal_y + 78.0;
        let row_h = 56.0;
        let row_gap = 8.0;
        let max_rows = world_count.min(5);

        let mut rows = Vec::with_capacity(max_rows);
        for i in 0..max_rows {
            let ry = list_top + (i as f32) * (row_h + row_gap);
            rows.push([list_x, ry, list_w, row_h]);
        }

        let btn_w = (list_w - 16.0) * 0.5;
        let btn_h = 38.0;
        let col_gap = 16.0;
        let row_btn_gap = 8.0;
        let left_x = list_x;
        let right_x = left_x + btn_w + col_gap;
        let bot_btn_y = modal_y + modal_h - 48.0;
        let top_btn_y = bot_btn_y - btn_h - row_btn_gap;

        let buttons = [
            [left_x, top_btn_y, btn_w, btn_h],  // Play Selected World
            [right_x, top_btn_y, btn_w, btn_h], // Create New World
            [left_x, bot_btn_y, btn_w, btn_h],  // Delete World
            [right_x, bot_btn_y, btn_w, btn_h], // Back
        ];
        (rows, buttons)
    }

    /// Builds the Singleplayer World Selection screen.
    pub fn build_world_select_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        worlds: &[blockgame_core::save::WorldMetadata],
        selected_index: Option<usize>,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Subtle vignette over 3D world
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.35],
        );

        let modal_w = 700.0_f32.min(screen_w - 40.0);
        let modal_h = 540.0_f32.min(screen_h - 60.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(30.0);

        // Drop shadow
        mesh.add_quad(
            modal_x + 3.0,
            modal_y + 3.0,
            modal_w,
            modal_h,
            white_uv,
            [0.01, 0.02, 0.04, 0.65],
        );
        // Beveled slate border
        mesh.add_quad(
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            [0.38, 0.42, 0.52, 0.85],
        );
        // Interior card background
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            modal_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.13, 0.94],
        );

        // Top gold accent bar
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            3.0,
            white_uv,
            [0.85, 0.68, 0.28, 0.95],
        );

        // Screen Header
        let header = "EXPEDITION ARCHIVE";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - header_w) * 0.5,
            modal_y + 16.0,
            header,
            14.0,
            18.0,
            1.5,
            [0.98, 0.98, 1.0, 1.0],
        );

        let sub = "Select a recorded frontier journey or embark on a new expedition";
        let sub_w = Self::string_width(sub, 8.5, 1.0);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - sub_w) * 0.5,
            modal_y + 40.0,
            sub,
            8.5,
            12.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        let (row_rects, btn_rects) = Self::world_select_layout(screen_w, screen_h, worlds.len());

        let list_w = modal_w - 48.0;
        let list_x = modal_x + 24.0;
        let list_top = modal_y + 68.0;
        let list_h = 328.0;

        // Inset list container backing
        mesh.add_quad(
            list_x - 4.0,
            list_top,
            list_w + 8.0,
            list_h,
            white_uv,
            [0.04, 0.05, 0.08, 0.60],
        );

        if worlds.is_empty() {
            let empty_msg = "No saved expeditions found. Click 'New Expedition' below to forge your first world!";
            let ew = Self::string_width(empty_msg, 9.0, 1.0);
            Self::draw_string(
                mesh,
                modal_x + (modal_w - ew) * 0.5,
                list_top + list_h * 0.45,
                empty_msg,
                9.0,
                13.0,
                1.0,
                [0.72, 0.76, 0.85, 0.95],
            );
        } else {
            for (i, rect) in row_rects.iter().enumerate() {
                let world = &worlds[i];
                let is_selected = selected_index == Some(i);
                let is_hovered =
                    Self::is_rect_hovered(cursor_pos, rect[0], rect[1], rect[2], rect[3]);

                let border_color = if is_selected {
                    [0.95, 0.78, 0.30, 1.0]
                } else if is_hovered {
                    [0.48, 0.54, 0.68, 0.90]
                } else {
                    [0.20, 0.22, 0.28, 0.80]
                };

                let fill_color = if is_selected {
                    [0.15, 0.19, 0.28, 0.95]
                } else if is_hovered {
                    [0.12, 0.15, 0.22, 0.92]
                } else {
                    [0.09, 0.11, 0.16, 0.88]
                };

                let ind_color = if is_selected {
                    [0.95, 0.78, 0.30, 1.0]
                } else if is_hovered {
                    [0.65, 0.55, 0.30, 0.80]
                } else {
                    [0.22, 0.25, 0.32, 0.70]
                };

                // Card shadow
                mesh.add_quad(rect[0] + 2.0, rect[1] + 2.0, rect[2], rect[3], white_uv, [0.01, 0.02, 0.04, 0.40]);
                // Border & fill
                mesh.add_quad(
                    rect[0] - 1.5,
                    rect[1] - 1.5,
                    rect[2] + 3.0,
                    rect[3] + 3.0,
                    white_uv,
                    border_color,
                );
                mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, fill_color);
                // Left indicator bar
                mesh.add_quad(rect[0], rect[1], 4.0, rect[3], white_uv, ind_color);

                // World Name
                Self::draw_string(
                    mesh,
                    rect[0] + 16.0,
                    rect[1] + 9.0,
                    &world.name,
                    11.0,
                    15.0,
                    1.0,
                    [0.98, 0.98, 1.0, 1.0],
                );

                // Game Mode badge
                let is_creative = world.game_mode.is_creative();
                let mode_badge = if is_creative { "[ CREATIVE ]" } else { "[ SURVIVAL ]" };
                let mode_color = if is_creative { [0.40, 0.85, 1.0, 1.0] } else { [1.0, 0.85, 0.35, 1.0] };
                let mw = Self::string_width(mode_badge, 8.5, 0.8);
                Self::draw_string(
                    mesh,
                    rect[0] + rect[2] - mw - 14.0,
                    rect[1] + 10.0,
                    mode_badge,
                    8.5,
                    12.0,
                    0.8,
                    mode_color,
                );

                // Folder & Seed details
                let play_mins = (world.total_time / 60.0) as u32;
                let play_secs = (world.total_time as u32) % 60;
                let details = format!(
                    "Folder: {}  |  Seed: {}  |  Time: {}m {}s",
                    world.folder_name, world.seed, play_mins, play_secs
                );
                Self::draw_string(
                    mesh,
                    rect[0] + 16.0,
                    rect[1] + 32.0,
                    &details,
                    8.5,
                    12.0,
                    0.8,
                    [0.72, 0.76, 0.85, 0.95],
                );
            }
        }

        let has_selection = selected_index.map(|idx| idx < worlds.len()).unwrap_or(false);
        Self::draw_menu_button(
            mesh,
            btn_rects[0][0],
            btn_rects[0][1],
            btn_rects[0][2],
            btn_rects[0][3],
            "> Resume Expedition",
            cursor_pos,
            has_selection,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[1][0],
            btn_rects[1][1],
            btn_rects[1][2],
            btn_rects[1][3],
            "+ New Expedition",
            cursor_pos,
            true,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[2][0],
            btn_rects[2][1],
            btn_rects[2][2],
            btn_rects[2][3],
            "X Delete Expedition",
            cursor_pos,
            has_selection,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[3][0],
            btn_rects[3][1],
            btn_rects[3][2],
            btn_rects[3][3],
            "< Back to Title",
            cursor_pos,
            true,
        );
    }

    /// Returns `(name_input_rect, seed_input_rect, create_btn_rect, cancel_btn_rect)`
    /// for the Create New World screen.
    pub fn create_world_layout(
        screen_w: f32,
        screen_h: f32,
    ) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
        let modal_w = 640.0_f32.min(screen_w - 40.0);
        let modal_h = 510.0_f32.min(screen_h - 60.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(30.0);

        let box_w = modal_w - 60.0;
        let box_h = 40.0;
        let box_x = modal_x + 30.0;
        let name_y = modal_y + 88.0;
        let seed_y = name_y + 84.0;

        let btn_w = (box_w - 16.0) * 0.5;
        let btn_h = 42.0;
        let gap = 16.0;
        let btn_y = modal_y + modal_h - 58.0;
        let create_x = box_x;
        let cancel_x = create_x + btn_w + gap;

        (
            [box_x, name_y, box_w, box_h],
            [box_x, seed_y, box_w, box_h],
            [create_x, btn_y, btn_w, btn_h],
            [cancel_x, btn_y, btn_w, btn_h],
        )
    }

    /// Returns the Game Mode toggle button rectangle on the Create New World screen.
    pub fn create_world_gamemode_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let modal_w = 640.0_f32.min(screen_w - 40.0);
        let modal_h = 510.0_f32.min(screen_h - 60.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(30.0);

        let box_w = modal_w - 60.0;
        let box_x = modal_x + 30.0;
        let name_y = modal_y + 88.0;
        let seed_y = name_y + 84.0;
        let mode_y = seed_y + 82.0;
        [box_x, mode_y, box_w, 62.0]
    }

    /// Builds the Create New World screen.
    /// `focused_field`: `0` = World Name input, `1` = World Seed input.
    pub fn build_create_world_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        name_input: &str,
        seed_input: &str,
        focused_field: usize,
        game_mode: blockgame_core::GameMode,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Subtle vignette over 3D background
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.35],
        );

        let modal_w = 640.0_f32.min(screen_w - 40.0);
        let modal_h = 510.0_f32.min(screen_h - 60.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(30.0);

        // Modal shadow
        mesh.add_quad(
            modal_x + 3.0,
            modal_y + 3.0,
            modal_w,
            modal_h,
            white_uv,
            [0.01, 0.02, 0.04, 0.65],
        );
        // Beveled slate border
        mesh.add_quad(
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            [0.38, 0.42, 0.52, 0.85],
        );
        // Interior card background
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            modal_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.13, 0.94],
        );

        // Top gold accent bar
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            3.0,
            white_uv,
            [0.85, 0.68, 0.28, 0.95],
        );

        let header = "FORGE NEW EXPEDITION";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - header_w) * 0.5,
            modal_y + 18.0,
            header,
            14.0,
            18.0,
            1.5,
            [0.98, 0.98, 1.0, 1.0],
        );

        let sub = "Chart uncharted mountain ridges, deep underground caverns, and dense woodlands";
        let sub_w = Self::string_width(sub, 8.5, 1.0);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - sub_w) * 0.5,
            modal_y + 42.0,
            sub,
            8.5,
            12.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        let (name_rect, seed_rect, create_rect, cancel_rect) =
            Self::create_world_layout(screen_w, screen_h);
        let gamemode_rect = Self::create_world_gamemode_rect(screen_w, screen_h);

        Self::draw_text_input_box(
            mesh,
            name_rect[0],
            name_rect[1],
            name_rect[2],
            name_rect[3],
            "Expedition Name:",
            name_input,
            "New World",
            focused_field == 0,
            cursor_pos,
        );

        let folder_preview = format!(
            "Directory: saves/{}",
            blockgame_core::save::sanitize_folder_name(if name_input.trim().is_empty() {
                "New World"
            } else {
                name_input
            })
        );
        Self::draw_string(
            mesh,
            name_rect[0],
            name_rect[1] + name_rect[3] + 6.0,
            &folder_preview,
            8.5,
            12.0,
            0.8,
            [0.60, 0.75, 0.90, 0.95],
        );

        Self::draw_text_input_box(
            mesh,
            seed_rect[0],
            seed_rect[1],
            seed_rect[2],
            seed_rect[3],
            "Expedition Seed (optional):",
            seed_input,
            "Leave blank for random procedural seed",
            focused_field == 1,
            cursor_pos,
        );

        // Interactive Game Mode Selector Card
        let mode_hovered = Self::is_rect_hovered(cursor_pos, gamemode_rect[0], gamemode_rect[1], gamemode_rect[2], gamemode_rect[3]);
        let mode_border = if mode_hovered { [0.95, 0.78, 0.30, 0.95] } else { [0.32, 0.36, 0.48, 0.85] };
        let mode_fill = if mode_hovered { [0.14, 0.18, 0.26, 0.96] } else { [0.10, 0.12, 0.18, 0.92] };
        let is_creative = game_mode.is_creative();
        let mode_ind = if is_creative { [0.40, 0.85, 1.0, 1.0] } else { [1.0, 0.85, 0.35, 1.0] };

        mesh.add_quad(gamemode_rect[0] - 1.5, gamemode_rect[1] - 1.5, gamemode_rect[2] + 3.0, gamemode_rect[3] + 3.0, white_uv, mode_border);
        mesh.add_quad(gamemode_rect[0], gamemode_rect[1], gamemode_rect[2], gamemode_rect[3], white_uv, mode_fill);
        mesh.add_quad(gamemode_rect[0], gamemode_rect[1], 4.0, gamemode_rect[3], white_uv, mode_ind);

        let mode_label = format!("Game Mode: {} (Click to toggle)", game_mode.display_name());
        Self::draw_string(
            mesh,
            gamemode_rect[0] + 16.0,
            gamemode_rect[1] + 12.0,
            &mode_label,
            10.5,
            14.0,
            1.0,
            if is_creative { [0.40, 0.85, 1.0, 1.0] } else { [1.0, 0.85, 0.35, 1.0] },
        );

        let mode_desc = if is_creative {
            "Unlimited building blocks, instant block breaking, and free flight (F key)."
        } else {
            "Vitality & stamina gauges, hunger, hostile monsters, and crafting progression."
        };
        Self::draw_string(
            mesh,
            gamemode_rect[0] + 16.0,
            gamemode_rect[1] + 36.0,
            mode_desc,
            8.5,
            12.0,
            0.8,
            [0.72, 0.76, 0.85, 0.95],
        );

        Self::draw_menu_button(
            mesh,
            create_rect[0],
            create_rect[1],
            create_rect[2],
            create_rect[3],
            "> Embark on Journey",
            cursor_pos,
            !name_input.trim().is_empty(),
        );
        Self::draw_menu_button(
            mesh,
            cancel_rect[0],
            cancel_rect[1],
            cancel_rect[2],
            cancel_rect[3],
            "< Return to Archive",
            cursor_pos,
            true,
        );
    }

    /// Returns the 5 Pause Menu button rectangles:
    /// `[Resume Journey, Save Expedition, Achievements, Expedition Settings, Save & Quit to Title]`.
    pub fn pause_menu_button_rects(screen_w: f32, screen_h: f32) -> [[f32; 4]; 5] {
        let card_w = 380.0_f32.min(screen_w - 40.0);
        let card_h = 420.0_f32.min(screen_h - 40.0);
        let card_y = (screen_h - card_h) * 0.5;
        let btn_w = 320.0_f32.min(card_w - 40.0);
        let btn_h = 40.0;
        let gap = 11.0;
        let btn_x = (screen_w - btn_w) * 0.5;
        let start_y = card_y + 82.0;
        [
            [btn_x, start_y, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap), btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 2.0, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 3.0, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 4.0 + 6.0, btn_w, btn_h],
        ]
    }

    /// Builds the Roots & Ridges In-Game Pause Menu overlay with centered modal docket.
    pub fn build_pause_menu(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
        status_message: Option<&str>,
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // 1. Cinematic vignette darkening over live 3D game world
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.72],
        );

        let card_w = 380.0_f32.min(screen_w - 40.0);
        let card_h = 420.0_f32.min(screen_h - 40.0);
        let card_x = (screen_w - card_w) * 0.5;
        let card_y = (screen_h - card_h) * 0.5;

        // 2. Centered Modal Card Drop Shadow
        mesh.add_quad(
            card_x + 3.0,
            card_y + 3.0,
            card_w,
            card_h,
            white_uv,
            [0.01, 0.02, 0.03, 0.55],
        );

        // 3. Modal Slate Border & Dark Interior Plate
        mesh.add_quad(
            card_x,
            card_y,
            card_w,
            card_h,
            white_uv,
            [0.38, 0.42, 0.52, 0.85],
        );
        mesh.add_quad(
            card_x + 1.5,
            card_y + 1.5,
            card_w - 3.0,
            card_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.13, 0.95],
        );

        // 4. Top Warm Amber / Gold Trim
        mesh.add_quad(
            card_x,
            card_y,
            card_w,
            2.5,
            white_uv,
            [0.85, 0.68, 0.28, 0.95],
        );

        // 5. Header Emblem & Typography
        mesh.add_quad(
            card_x + (card_w - 36.0) * 0.5,
            card_y + 14.0,
            36.0,
            2.0,
            white_uv,
            [0.95, 0.78, 0.30, 0.95],
        );

        let title = "EXPEDITION PAUSED";
        let title_w = Self::string_width(title, 13.0, 1.5);
        Self::draw_string(
            mesh,
            card_x + (card_w - title_w) * 0.5 + 1.5,
            card_y + 24.0 + 1.5,
            title,
            13.0,
            17.0,
            1.5,
            [0.02, 0.03, 0.05, 0.95],
        );
        Self::draw_string(
            mesh,
            card_x + (card_w - title_w) * 0.5,
            card_y + 24.0,
            title,
            13.0,
            17.0,
            1.5,
            [0.98, 0.97, 0.94, 1.0],
        );

        let sub_text = "ROOTS & RIDGES";
        let sub_w = Self::string_width(sub_text, 7.5, 1.0);
        Self::draw_string(
            mesh,
            card_x + (card_w - sub_w) * 0.5,
            card_y + 46.0,
            sub_text,
            7.5,
            10.5,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        // Thin divider line
        mesh.add_quad(
            card_x + 24.0,
            card_y + 66.0,
            card_w - 48.0,
            1.2,
            white_uv,
            [0.32, 0.36, 0.45, 0.65],
        );

        // 6. Navigation Action Buttons
        let rects = Self::pause_menu_button_rects(screen_w, screen_h);
        let labels = [
            "> Resume Journey",
            "* Save Expedition",
            "! Achievements",
            "# Expedition Settings",
            "< Save & Quit to Title",
        ];
        for (rect, label) in rects.iter().zip(labels.iter()) {
            Self::draw_menu_button(
                mesh,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                label,
                cursor_pos,
                true,
            );
        }

        // 7. Toast / Status Message or ESC Reminder
        if let Some(msg) = status_message {
            let toast_h = 24.0;
            let toast_w = card_w - 48.0;
            let toast_x = card_x + 24.0;
            let toast_y = card_y + card_h - 38.0;

            mesh.add_quad(
                toast_x,
                toast_y,
                toast_w,
                toast_h,
                white_uv,
                [0.30, 0.80, 0.45, 0.80],
            );
            mesh.add_quad(
                toast_x + 1.0,
                toast_y + 1.0,
                toast_w - 2.0,
                toast_h - 2.0,
                white_uv,
                [0.05, 0.12, 0.08, 0.95],
            );

            let mw = Self::string_width(msg, 8.5, 0.8);
            Self::draw_string(
                mesh,
                toast_x + (toast_w - mw) * 0.5,
                toast_y + (toast_h - 11.5) * 0.5,
                msg,
                8.5,
                11.5,
                0.8,
                [0.45, 0.95, 0.55, 1.0],
            );
        } else {
            let esc_tip = "Press ESC to return to expedition";
            let tw = Self::string_width(esc_tip, 7.5, 0.8);
            Self::draw_string(
                mesh,
                card_x + (card_w - tw) * 0.5,
                card_y + card_h - 30.0,
                esc_tip,
                7.5,
                10.0,
                0.8,
                [0.48, 0.52, 0.62, 0.75],
            );
        }
    }

    /// Builds the Pause Menu with controller-aware footer prompt (renders pixel-art button glyph in Controller Input Mode).
    pub fn build_pause_menu_with_input_mode(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
        status_toast: Option<&str>,
        input_mode: InputMode,
        glyph_family: ControllerFamily,
    ) {
        if input_mode == InputMode::KeyboardMouse || status_toast.is_some() {
            Self::build_pause_menu(mesh, screen_w, screen_h, cursor_pos, status_toast);
            return;
        }
        Self::build_pause_menu(mesh, screen_w, screen_h, cursor_pos, Some(" "));
        // Overdraw the bottom strip with a clean controller glyph hint
        let card_w = 340.0_f32.min(screen_w - 40.0);
        let card_h = 440.0_f32.min(screen_h - 40.0);
        let card_x = (screen_w - card_w) * 0.5;
        let card_y = (screen_h - card_h) * 0.5;
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(
            card_x + 12.0,
            card_y + card_h - 48.0,
            card_w - 24.0,
            34.0,
            white_uv,
            [0.06, 0.08, 0.12, 0.96],
        );
        let tip = "to return to expedition";
        let tw = Self::string_width(tip, 7.5, 0.8);
        let total_w = 16.0 + 6.0 + tw;
        let start_x = card_x + (card_w - total_w) * 0.5;
        let fy = card_y + card_h - 35.0;
        Self::draw_gamepad_glyph(
            mesh,
            start_x,
            fy - 2.0,
            15.0,
            glyph_family,
            GamepadButton::Start,
            [1.0; 4],
        );
        Self::draw_string(
            mesh,
            start_x + 21.0,
            fy + 1.0,
            tip,
            7.5,
            10.0,
            0.8,
            [0.65, 0.72, 0.85, 0.90],
        );
    }

    /// Returns the Back button rectangle for modal/notice/settings screens.
    pub fn notice_back_button_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let btn_w = 240.0;
        let btn_h = 42.0;
        [(screen_w - btn_w) * 0.5, screen_h * 0.66, btn_w, btn_h]
    }

    /// Builds the Settings screen or Multiplayer notice screen.
    pub fn build_notice_or_settings_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        title: &str,
        lines: &[&str],
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.05, 0.06, 0.09, 0.78],
        );

        let tw = Self::string_width(title, 14.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - tw) * 0.5,
            screen_h * 0.22,
            title,
            14.0,
            19.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        let mut y = screen_h * 0.36;
        for line in lines {
            let lw = Self::string_width(line, 10.5, 1.0);
            Self::draw_string(
                mesh,
                (screen_w - lw) * 0.5,
                y,
                line,
                10.5,
                15.0,
                1.0,
                [0.82, 0.85, 0.90, 0.96],
            );
            y += 28.0;
        }

        let back_rect = Self::notice_back_button_rect(screen_w, screen_h);
        Self::draw_menu_button(
            mesh,
            back_rect[0],
            back_rect[1],
            back_rect[2],
            back_rect[3],
            "< Return to Title",
            cursor_pos,
            true,
        );
    }

    pub fn settings_modal_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let modal_w = 760.0_f32.min(screen_w - 40.0);
        let modal_h = 580.0_f32.min(screen_h - 40.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(20.0);
        [modal_x, modal_y, modal_w, modal_h]
    }

    /// Returns the tab button rectangle `[x, y, w, h]` for tab `0` (General), `1` (Controls), or `2` (Controller).
    pub fn settings_tab_rect(screen_w: f32, screen_h: f32, tab_idx: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let gap_x = 12.0;
        let tab_w = (m[2] - 48.0 - gap_x * 2.0) / 3.0;
        let start_x = m[0] + 24.0;
        let y = m[1] + 68.0;
        [start_x + (tab_idx as f32) * (tab_w + gap_x), y, tab_w, 34.0]
    }

    /// Returns `[x, y, w, h]` for Controller tab slider `index` (`0` = Sensitivity, `1` = Deadzone, `2` = Rumble).
    pub fn settings_controller_slider_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let gap_x = 12.0;
        let col_w = (m[2] - 48.0 - gap_x * 2.0) / 3.0;
        let start_x = m[0] + 24.0;
        let y = m[1] + 132.0;
        [start_x + (index as f32) * (col_w + gap_x), y, col_w, 26.0]
    }

    /// Returns `[x, y, w, h]` for Controller tab toggle `index` (`0` = Invert Y, `1` = Glyph Style, `2` = Swap Confirm/Cancel, `3` = Sneak Mode).
    pub fn settings_controller_toggle_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let gap_x = 10.0;
        let col_w = (m[2] - 48.0 - gap_x * 3.0) / 4.0;
        let start_x = m[0] + 24.0;
        let y = m[1] + 164.0;
        [start_x + (index as f32) * (col_w + gap_x), y, col_w, 26.0]
    }

    /// Returns `[x, y, w, h]` for Controller tab button binding `index` (`0..16`, 2 columns × 8 rows).
    pub fn settings_controller_bind_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let gap_x = 16.0;
        let start_x = m[0] + 24.0;
        let start_y = m[1] + 198.0;
        let row_h = 26.0;
        let gap_y = 4.0;
        let col = (index / 8) as f32;
        let row = (index % 8) as f32;
        [
            start_x + col * (col_w + gap_x),
            start_y + row * (row_h + gap_y),
            col_w,
            row_h,
        ]
    }

    /// Returns the slider rectangle `[x, y, w, h]` for slider index `0..7`.
    pub fn settings_slider_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let gap_x = 16.0;
        let start_x = m[0] + 24.0;
        let start_y = m[1] + 118.0;
        let row_h = 30.0;
        let gap_y = 10.0;

        let col = (index / 4) as f32;
        let row = (index % 4) as f32;
        [
            start_x + col * (col_w + gap_x),
            start_y + row * (row_h + gap_y),
            col_w,
            row_h,
        ]
    }

    /// Returns the toggle button rectangle `[x, y, w, h]` for toggle index `0..7`.
    pub fn settings_toggle_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let gap_x = 16.0;
        let start_x = m[0] + 24.0;
        let base_y = m[1] + 118.0 + 4.0 * (30.0 + 10.0) + 12.0;
        let row_h = 30.0;
        let gap_y = 8.0;
        let col = (index % 2) as f32;
        let row = (index / 2) as f32;
        [
            start_x + col * (col_w + gap_x),
            base_y + row * (row_h + gap_y),
            col_w,
            row_h,
        ]
    }

    /// Returns the keybind button rectangle `[x, y, w, h]` for keybind index `0..14`.
    pub fn settings_keybind_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let gap_x = 16.0;
        let start_x = m[0] + 24.0;
        let start_y = m[1] + 118.0;
        let row_h = 28.0;
        let gap_y = 6.0;
        let col = (index / 8) as f32;
        let row = (index % 8) as f32;
        [
            start_x + col * (col_w + gap_x),
            start_y + row * (row_h + gap_y),
            col_w,
            row_h,
        ]
    }

    /// Returns `[x, y, w, h]` for the `Reset to Defaults` button on the Settings screen.
    pub fn settings_reset_button_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let start_x = m[0] + 24.0;
        [start_x, m[1] + m[3] - 52.0, col_w, 40.0]
    }

    /// Returns `[x, y, w, h]` for the `Done` button on the Settings screen.
    pub fn settings_done_button_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let m = Self::settings_modal_rect(screen_w, screen_h);
        let col_w = (m[2] - 64.0) * 0.5;
        let gap_x = 16.0;
        let start_x = m[0] + 24.0;
        [start_x + col_w + gap_x, m[1] + m[3] - 52.0, col_w, 40.0]
    }

    /// Builds the interactive Settings screen with General, Controls, and Controller tabs.
    #[allow(clippy::too_many_arguments)]
    pub fn build_settings_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        settings: SettingsUiState,
        settings_tab: usize,
        active_slider: Option<usize>,
        listening_keybind: Option<usize>,
        keybinds: &[(&str, &str)],
        listening_controller_bind: Option<usize>,
        controller_binds: &[(&str, GamepadButton)],
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Subtle vignette over 3D world
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.35],
        );

        let m = Self::settings_modal_rect(screen_w, screen_h);
        let modal_x = m[0];
        let modal_y = m[1];
        let modal_w = m[2];
        let modal_h = m[3];

        // Drop shadow
        mesh.add_quad(
            modal_x + 3.0,
            modal_y + 3.0,
            modal_w,
            modal_h,
            white_uv,
            [0.01, 0.02, 0.04, 0.65],
        );
        // Beveled slate border
        mesh.add_quad(
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            [0.38, 0.42, 0.52, 0.85],
        );
        // Interior card background
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            modal_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.13, 0.94],
        );

        // Top gold accent bar
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            3.0,
            white_uv,
            [0.85, 0.68, 0.28, 0.95],
        );

        let title = "EXPEDITION SETTINGS";
        let tw = Self::string_width(title, 14.0, 1.5);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - tw) * 0.5,
            modal_y + 16.0,
            title,
            14.0,
            18.0,
            1.5,
            [0.98, 0.98, 1.0, 1.0],
        );

        let sub = "Tune visual fidelity, soundscapes, audio devices, and control bindings";
        let sub_w = Self::string_width(sub, 8.5, 1.0);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - sub_w) * 0.5,
            modal_y + 40.0,
            sub,
            8.5,
            12.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        for (tab_idx, tab_label) in ["General & Audio", "Controls & Keybinds", "Controller"]
            .iter()
            .enumerate()
        {
            let r = Self::settings_tab_rect(screen_w, screen_h, tab_idx);
            let is_active = settings_tab == tab_idx;
            let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3]);
            let border = if is_active {
                [0.95, 0.78, 0.30, 1.0]
            } else if hovered {
                [0.55, 0.60, 0.75, 0.95]
            } else {
                [0.25, 0.28, 0.36, 0.85]
            };
            let bg = if is_active {
                [0.16, 0.20, 0.30, 0.96]
            } else {
                [0.10, 0.12, 0.16, 0.90]
            };
            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
            mesh.add_quad(r[0] + 1.5, r[1] + 1.5, r[2] - 3.0, r[3] - 3.0, white_uv, bg);
            if is_active {
                // Bottom gold active indicator
                mesh.add_quad(r[0] + 1.5, r[1] + r[3] - 3.0, r[2] - 3.0, 3.0, white_uv, [1.0, 0.85, 0.35, 1.0]);
            }
            let lw = Self::string_width(tab_label, 10.5, 1.0);
            Self::draw_string(
                mesh,
                r[0] + (r[2] - lw) * 0.5,
                r[1] + (r[3] - 14.0) * 0.5,
                tab_label,
                10.5,
                14.0,
                1.0,
                if is_active { [1.0, 0.88, 0.40, 1.0] } else { [0.75, 0.78, 0.86, 0.95] },
            );
        }

        if settings_tab == 0 {
            let sliders: [(String, f32); 8] = [
                (
                    format!("Render Distance: {} chunks", settings.render_distance),
                    ((settings.render_distance - 2) as f32 / 10.0).clamp(0.0, 1.0),
                ),
                (
                    format!("FOV: {:.0} deg", settings.fov_degrees),
                    ((settings.fov_degrees - 50.0) / 60.0).clamp(0.0, 1.0),
                ),
                (
                    format!("Sensitivity: {:.0}%", settings.mouse_sensitivity * 100.0),
                    ((settings.mouse_sensitivity - 0.2) / 2.3).clamp(0.0, 1.0),
                ),
                (
                    format!("Master Volume: {:.0}%", settings.master_volume * 100.0),
                    settings.master_volume.clamp(0.0, 1.0),
                ),
                (
                    format!("SFX Volume: {:.0}%", settings.sfx_volume * 100.0),
                    settings.sfx_volume.clamp(0.0, 1.0),
                ),
                (
                    format!("Music Volume: {:.0}%", settings.music_volume * 100.0),
                    settings.music_volume.clamp(0.0, 1.0),
                ),
                (
                    format!("Voice Volume: {:.0}%", settings.voice_volume * 100.0),
                    settings.voice_volume.clamp(0.0, 1.0),
                ),
                (
                    format!("Creature Volume: {:.0}%", settings.creature_volume * 100.0),
                    settings.creature_volume.clamp(0.0, 1.0),
                ),
            ];

            for (idx, (label, norm)) in sliders.iter().enumerate() {
                let r = Self::settings_slider_rect(screen_w, screen_h, idx);
                let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3])
                    || active_slider == Some(idx);

                let border = if hovered {
                    [0.95, 0.78, 0.30, 0.95]
                } else {
                    [0.26, 0.30, 0.40, 0.85]
                };
                mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
                mesh.add_quad(
                    r[0] + 1.5,
                    r[1] + 1.5,
                    r[2] - 3.0,
                    r[3] - 3.0,
                    white_uv,
                    [0.08, 0.10, 0.14, 0.96],
                );

                let fill_w = ((r[2] - 3.0) * *norm).max(0.0);
                if fill_w > 0.0 {
                    mesh.add_quad(
                        r[0] + 1.5,
                        r[1] + 1.5,
                        fill_w,
                        r[3] - 3.0,
                        white_uv,
                        if hovered {
                            [0.85, 0.68, 0.28, 0.85]
                        } else {
                            [0.65, 0.52, 0.22, 0.80]
                        },
                    );
                }

                let thumb_w = 8.0;
                let thumb_x = (r[0] + 1.5 + (r[2] - 3.0 - thumb_w) * *norm)
                    .clamp(r[0] + 1.5, r[0] + r[2] - 1.5 - thumb_w);
                mesh.add_quad(
                    thumb_x,
                    r[1] + 1.5,
                    thumb_w,
                    r[3] - 3.0,
                    white_uv,
                    if hovered {
                        [1.0, 0.90, 0.45, 1.0]
                    } else {
                        [0.85, 0.75, 0.35, 0.95]
                    },
                );

                let lw = Self::string_width(label, 9.0, 1.0);
                Self::draw_string(
                    mesh,
                    r[0] + (r[2] - lw) * 0.5,
                    r[1] + (r[3] - 13.0) * 0.5,
                    label,
                    9.0,
                    13.0,
                    1.0,
                    [0.98, 0.98, 1.0, 1.0],
                );
            }

            let ui_scale_str = if (settings.ui_scale - 0.75).abs() < 0.01 {
                "0.75x"
            } else if (settings.ui_scale - 1.25).abs() < 0.01 {
                "1.25x"
            } else if (settings.ui_scale - 1.5).abs() < 0.01 {
                "1.5x"
            } else {
                "1.0x"
            };
            let voice_mode_str = match settings.voice_mode {
                0 => "OFF",
                2 => "Voice Act",
                _ => "Push-to-Talk",
            };
            let mic_display = if settings.voice_input_device.is_empty() {
                "Default".to_string()
            } else {
                let chars: Vec<char> = settings.voice_input_device.chars().collect();
                if chars.len() > 19 {
                    let prefix: String = chars.into_iter().take(16).collect();
                    format!("{}...", prefix.trim_end())
                } else {
                    settings.voice_input_device.clone()
                }
            };
            let toggles = [
                format!(
                    "Invert Y-Axis: {}",
                    if settings.invert_y { "ON" } else { "OFF" }
                ),
                format!(
                    "Fullscreen: {}",
                    if settings.fullscreen { "ON" } else { "OFF" }
                ),
                format!("VSync: {}", if settings.vsync { "ON" } else { "OFF" }),
                format!(
                    "View Bobbing: {}",
                    if settings.view_bobbing { "ON" } else { "OFF" }
                ),
                format!(
                    "Debug HUD (F3): {}",
                    if settings.show_debug_hud { "ON" } else { "OFF" }
                ),
                format!("UI Scale: {}", ui_scale_str),
                format!("Voice Chat: {}", voice_mode_str),
                format!("Mic: {}", mic_display),
                format!(
                    "Graphics Quality: {}",
                    if settings.graphics_quality_fast {
                        "Fast"
                    } else {
                        "Fancy"
                    }
                ),
            ];

            for (idx, label) in toggles.iter().enumerate() {
                let r = Self::settings_toggle_rect(screen_w, screen_h, idx);
                Self::draw_menu_button(mesh, r[0], r[1], r[2], r[3], label, cursor_pos, true);
            }
        } else if settings_tab == 1 {
            for (idx, (action, key_name)) in keybinds.iter().enumerate() {
                let r = Self::settings_keybind_rect(screen_w, screen_h, idx);
                let is_listening = listening_keybind == Some(idx);
                let label = if is_listening {
                    format!("{}: [Press Key]", action)
                } else {
                    format!("{}: {}", action, key_name)
                };
                if is_listening {
                    mesh.add_quad(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        white_uv,
                        [0.95, 0.78, 0.30, 1.0],
                    );
                    mesh.add_quad(
                        r[0] + 1.5,
                        r[1] + 1.5,
                        r[2] - 3.0,
                        r[3] - 3.0,
                        white_uv,
                        [0.20, 0.16, 0.08, 0.96],
                    );
                    let lw = Self::string_width(&label, 9.0, 1.0);
                    Self::draw_string(
                        mesh,
                        r[0] + (r[2] - lw) * 0.5,
                        r[1] + (r[3] - 13.0) * 0.5,
                        &label,
                        9.0,
                        13.0,
                        1.0,
                        [1.0, 0.90, 0.40, 1.0],
                    );
                } else {
                    Self::draw_menu_button(
                        mesh, r[0], r[1], r[2], r[3], &label, cursor_pos, true,
                    );
                }
            }
        } else {
            // Tab 2: Controller tuning, Glyph Style, and per-action button remapping
            let status_txt = format!("Active Controller: {}", settings.active_controller_label);
            let sw = Self::string_width(&status_txt, 8.5, 0.9);
            Self::draw_string(
                mesh,
                modal_x + (modal_w - sw) * 0.5,
                modal_y + 111.0,
                &status_txt,
                8.5,
                12.0,
                0.9,
                [0.72, 0.88, 1.0, 0.96],
            );

            let pad_sliders: [(String, f32); 3] = [
                (
                    format!("Look Sens: {:.0}%", settings.controller_sensitivity * 100.0),
                    ((settings.controller_sensitivity - 0.2) / 2.3).clamp(0.0, 1.0),
                ),
                (
                    format!("Deadzone: {:.0}%", settings.controller_deadzone * 100.0),
                    ((settings.controller_deadzone - 0.05) / 0.40).clamp(0.0, 1.0),
                ),
                (
                    if settings.controller_rumble_intensity <= 0.001 {
                        "Rumble: OFF".to_string()
                    } else {
                        format!("Rumble: {:.0}%", settings.controller_rumble_intensity * 100.0)
                    },
                    settings.controller_rumble_intensity.clamp(0.0, 1.0),
                ),
            ];

            for (idx, (label, norm)) in pad_sliders.iter().enumerate() {
                let r = Self::settings_controller_slider_rect(screen_w, screen_h, idx);
                let slider_id = 100 + idx;
                let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3])
                    || active_slider == Some(slider_id);
                let border = if hovered {
                    [0.95, 0.78, 0.30, 0.95]
                } else {
                    [0.26, 0.30, 0.40, 0.85]
                };
                mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
                mesh.add_quad(
                    r[0] + 1.5,
                    r[1] + 1.5,
                    r[2] - 3.0,
                    r[3] - 3.0,
                    white_uv,
                    [0.08, 0.10, 0.14, 0.96],
                );
                let fill_w = ((r[2] - 3.0) * *norm).max(0.0);
                if fill_w > 0.0 {
                    mesh.add_quad(
                        r[0] + 1.5,
                        r[1] + 1.5,
                        fill_w,
                        r[3] - 3.0,
                        white_uv,
                        if hovered {
                            [0.45, 0.62, 0.92, 0.75]
                        } else {
                            [0.28, 0.42, 0.68, 0.65]
                        },
                    );
                }
                let lw = Self::string_width(label, 8.0, 0.8);
                Self::draw_string(
                    mesh,
                    r[0] + (r[2] - lw) * 0.5,
                    r[1] + (r[3] - 11.5) * 0.5,
                    label,
                    8.0,
                    11.5,
                    0.8,
                    [0.96, 0.97, 1.0, 1.0],
                );
            }

            let pad_toggles = [
                format!(
                    "Invert Y: {}",
                    if settings.controller_invert_y { "ON" } else { "OFF" }
                ),
                format!("Glyphs: {}", settings.controller_glyph_style.display_name()),
                format!(
                    "Swap A/B: {}",
                    if settings.controller_swap_confirm_cancel { "ON" } else { "OFF" }
                ),
                format!(
                    "Sneak: {}",
                    if settings.controller_sneak_toggle { "Toggle" } else { "Hold" }
                ),
            ];
            for (idx, label) in pad_toggles.iter().enumerate() {
                let r = Self::settings_controller_toggle_rect(screen_w, screen_h, idx);
                Self::draw_menu_button(mesh, r[0], r[1], r[2], r[3], label, cursor_pos, true);
            }

            let glyph_family = settings.active_glyph_family;
            for (idx, (action, button)) in controller_binds.iter().enumerate() {
                let r = Self::settings_controller_bind_rect(screen_w, screen_h, idx);
                let is_listening = listening_controller_bind == Some(idx);
                let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3]);
                let border = if is_listening {
                    [0.95, 0.78, 0.30, 1.0]
                } else if hovered {
                    [0.70, 0.76, 0.90, 0.95]
                } else {
                    [0.24, 0.28, 0.36, 0.85]
                };
                let bg = if is_listening {
                    [0.20, 0.16, 0.08, 0.96]
                } else if hovered {
                    [0.15, 0.19, 0.28, 0.95]
                } else {
                    [0.09, 0.11, 0.16, 0.94]
                };
                mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
                mesh.add_quad(r[0] + 1.5, r[1] + 1.5, r[2] - 3.0, r[3] - 3.0, white_uv, bg);

                let text = if is_listening {
                    format!("{}: [Press Button]", action)
                } else {
                    format!("{}: {}", action, button.short_label(glyph_family))
                };
                Self::draw_string(
                    mesh,
                    r[0] + 10.0,
                    r[1] + (r[3] - 11.5) * 0.5,
                    &text,
                    8.0,
                    11.5,
                    0.8,
                    if is_listening {
                        [1.0, 0.90, 0.40, 1.0]
                    } else {
                        [0.92, 0.94, 0.98, 1.0]
                    },
                );
                if !is_listening {
                    Self::draw_gamepad_glyph(
                        mesh,
                        r[0] + r[2] - 24.0,
                        r[1] + (r[3] - 18.0) * 0.5,
                        18.0,
                        glyph_family,
                        *button,
                        [1.0; 4],
                    );
                }
            }
        }

        let reset_r = Self::settings_reset_button_rect(screen_w, screen_h);
        let reset_label = if settings_tab == 2 {
            "* Reset Controller Defaults"
        } else {
            "* Reset to Defaults"
        };
        Self::draw_menu_button(
            mesh,
            reset_r[0],
            reset_r[1],
            reset_r[2],
            reset_r[3],
            reset_label,
            cursor_pos,
            true,
        );

        let done_r = Self::settings_done_button_rect(screen_w, screen_h);
        Self::draw_menu_button(
            mesh,
            done_r[0],
            done_r[1],
            done_r[2],
            done_r[3],
            "> Done & Save",
            cursor_pos,
            true,
        );
    }

    /// Draws the top-left F3 debug telemetry overlay (FPS, XYZ, chunk, facing, time).
    pub fn draw_debug_overlay(
        mesh: &mut UiMesh,
        fps: u32,
        pos: [f32; 3],
        chunk: [i32; 3],
        facing: &str,
        time_ticks: f32,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let lines = [
            format!("BlockGame  |  FPS: {}", fps),
            format!("XYZ: {:.1} / {:.1} / {:.1}", pos[0], pos[1], pos[2]),
            format!(
                "Chunk: ({}, {}, {})  |  Facing: {}",
                chunk[0], chunk[1], chunk[2], facing
            ),
            format!("Time: {:.0} ticks", time_ticks),
        ];
        let mut max_w = 0.0_f32;
        for line in &lines {
            max_w = max_w.max(Self::string_width(line, 8.5, 1.0));
        }
        let box_x = 10.0;
        let box_y = 10.0;
        let box_w = max_w + 16.0;
        let box_h = lines.len() as f32 * 16.0 + 12.0;

        mesh.add_quad(
            box_x,
            box_y,
            box_w,
            box_h,
            white_uv,
            [0.04, 0.05, 0.08, 0.65],
        );

        for (i, line) in lines.iter().enumerate() {
            Self::draw_string(
                mesh,
                box_x + 8.0,
                box_y + 7.0 + i as f32 * 16.0,
                line,
                8.5,
                12.0,
                1.0,
                [0.92, 0.95, 1.0, 0.96],
            );
        }
    }

    /// Returns layout rectangles for the Multiplayer Lobby left-hand account/direct-connect column:
    /// `(username_box, password_box, address_box, [skin_btn, join_btn, host_btn, back_btn])`.
    pub fn multiplayer_lobby_layout(
        screen_w: f32,
        screen_h: f32,
    ) -> ([f32; 4], [f32; 4], [f32; 4], [[f32; 4]; 4]) {
        let modal_w = 840.0_f32.min(screen_w - 40.0);
        let modal_h = 560.0_f32.min(screen_h - 50.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(25.0);

        let left_w = 340.0;
        let fx = modal_x + 24.0;
        let field_h = 34.0;

        let content_y = modal_y + 80.0;
        let user_rect = [fx, content_y + 2.0, left_w, field_h];
        let pass_rect = [fx, content_y + 58.0, left_w, field_h];
        let addr_rect = [fx, content_y + 114.0, left_w, field_h];

        let skin_btn = [fx, content_y + 172.0, left_w, 36.0];
        let half_w = (left_w - 12.0) * 0.5;
        let join_btn = [fx, content_y + 224.0, half_w, 40.0];
        let host_btn = [fx + half_w + 12.0, content_y + 224.0, half_w, 40.0];
        let back_btn = [fx, content_y + 278.0, left_w, 38.0];

        (
            user_rect,
            pass_rect,
            addr_rect,
            [skin_btn, join_btn, host_btn, back_btn],
        )
    }

    /// Returns layout rectangles for the Discoverable Servers browser panel on the Multiplayer Lobby screen:
    /// `(panel_rect, refresh_btn_rect, row_rects[0..6])`.
    pub fn multiplayer_browser_layout(
        screen_w: f32,
        screen_h: f32,
    ) -> ([f32; 4], [f32; 4], [[f32; 4]; 6]) {
        let modal_w = 840.0_f32.min(screen_w - 40.0);
        let modal_h = 560.0_f32.min(screen_h - 50.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(25.0);

        let left_w = 340.0;
        let gap = 24.0;
        let right_x = modal_x + 24.0 + left_w + gap;
        let right_w = modal_w - (right_x - modal_x) - 24.0;

        let panel_y = modal_y + 80.0;
        let panel_h = 360.0;
        let panel_rect = [right_x, panel_y, right_w, panel_h];
        let refresh_btn = [right_x + right_w - 96.0, panel_y + 8.0, 86.0, 26.0];

        let mut rows = [[0.0; 4]; 6];
        let row_x = right_x + 10.0;
        let row_w = right_w - 20.0;
        let row_h = 46.0;
        let row_gap = 6.0;
        let rows_start_y = panel_y + 40.0;
        for (i, row) in rows.iter_mut().enumerate() {
            *row = [row_x, rows_start_y + (i as f32) * (row_h + row_gap), row_w, row_h];
        }

        (panel_rect, refresh_btn, rows)
    }

    /// Builds the interactive Multiplayer Lobby Screen (Account, Direct Connect, and Discoverable Servers browser).
    #[allow(clippy::too_many_arguments)]
    pub fn build_multiplayer_lobby_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        username_input: &str,
        password_input: &str,
        address_input: &str,
        appearance: &blockgame_core::character::CharacterAppearance,
        preset_label: &str,
        focused_field: usize,
        status_message: Option<(&str, bool)>,
        directory_servers: &[blockgame_core::directory::DirectoryServerEntry],
        selected_server_idx: Option<usize>,
        directory_loading: bool,
        directory_error: Option<&str>,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Subtle vignette over 3D world
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.03, 0.05, 0.08, 0.35],
        );

        let modal_w = 840.0_f32.min(screen_w - 40.0);
        let modal_h = 560.0_f32.min(screen_h - 50.0);
        let modal_x = (screen_w - modal_w) * 0.5;
        let modal_y = ((screen_h - modal_h) * 0.5).max(25.0);

        // Modal drop shadow
        mesh.add_quad(
            modal_x + 3.0,
            modal_y + 3.0,
            modal_w,
            modal_h,
            white_uv,
            [0.01, 0.02, 0.04, 0.65],
        );
        // Beveled slate border
        mesh.add_quad(
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            white_uv,
            [0.38, 0.42, 0.52, 0.85],
        );
        // Interior card background
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            modal_h - 3.0,
            white_uv,
            [0.07, 0.09, 0.13, 0.94],
        );

        // Top gold accent bar
        mesh.add_quad(
            modal_x + 1.5,
            modal_y + 1.5,
            modal_w - 3.0,
            3.0,
            white_uv,
            [0.85, 0.68, 0.28, 0.95],
        );

        let header = "MULTIPLAYER REALMS";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - header_w) * 0.5,
            modal_y + 16.0,
            header,
            14.0,
            18.0,
            1.5,
            [0.98, 0.98, 1.0, 1.0],
        );

        let sub = "Connect to dedicated servers, explore player frontiers, or host a local LAN realm";
        let sub_w = Self::string_width(sub, 8.5, 1.0);
        Self::draw_string(
            mesh,
            modal_x + (modal_w - sub_w) * 0.5,
            modal_y + 40.0,
            sub,
            8.5,
            12.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        let (user_r, pass_r, addr_r, btns) = Self::multiplayer_lobby_layout(screen_w, screen_h);

        Self::draw_text_input_box(
            mesh,
            user_r[0],
            user_r[1],
            user_r[2],
            user_r[3],
            "Username (3-16 chars):",
            username_input,
            "Adventurer",
            focused_field == 0,
            cursor_pos,
        );

        let masked_pass: String = "*".repeat(password_input.len());
        Self::draw_text_input_box(
            mesh,
            pass_r[0],
            pass_r[1],
            pass_r[2],
            pass_r[3],
            "Account Password (optional):",
            &masked_pass,
            "Enter world password",
            focused_field == 1,
            cursor_pos,
        );

        Self::draw_text_input_box(
            mesh,
            addr_r[0],
            addr_r[1],
            addr_r[2],
            addr_r[3],
            "Server Address (IP:Port):",
            address_input,
            "127.0.0.1:25565",
            focused_field == 2,
            cursor_pos,
        );

        let char_label = format!("Character: {}", preset_label);
        Self::draw_menu_button(
            mesh,
            btns[0][0],
            btns[0][1],
            btns[0][2],
            btns[0][3],
            &char_label,
            cursor_pos,
            true,
        );

        // Procedural 2D Character Portrait on the left side of the Character button
        Self::draw_character_portrait(
            mesh,
            btns[0][0] + 8.0,
            btns[0][1] + 4.0,
            28.0,
            appearance,
        );

        let can_connect =
            username_input.trim().len() >= 3 && !address_input.trim().is_empty();

        Self::draw_menu_button(
            mesh,
            btns[1][0],
            btns[1][1],
            btns[1][2],
            btns[1][3],
            "> Join Realm",
            cursor_pos,
            can_connect,
        );
        Self::draw_menu_button(
            mesh,
            btns[2][0],
            btns[2][1],
            btns[2][2],
            btns[2][3],
            "+ Host Realm",
            cursor_pos,
            username_input.trim().len() >= 3,
        );
        Self::draw_menu_button(
            mesh,
            btns[3][0],
            btns[3][1],
            btns[3][2],
            btns[3][3],
            "< Back to Title",
            cursor_pos,
            true,
        );

        // Right-hand Discoverable Servers panel
        let (panel_r, refresh_r, row_rects) =
            Self::multiplayer_browser_layout(screen_w, screen_h);
        
        // Panel shadow & container
        mesh.add_quad(
            panel_r[0] + 2.0,
            panel_r[1] + 2.0,
            panel_r[2],
            panel_r[3],
            white_uv,
            [0.01, 0.02, 0.04, 0.50],
        );
        mesh.add_quad(
            panel_r[0],
            panel_r[1],
            panel_r[2],
            panel_r[3],
            white_uv,
            [0.32, 0.36, 0.46, 0.85],
        );
        mesh.add_quad(
            panel_r[0] + 1.5,
            panel_r[1] + 1.5,
            panel_r[2] - 3.0,
            panel_r[3] - 3.0,
            white_uv,
            [0.06, 0.08, 0.12, 0.94],
        );

        Self::draw_string(
            mesh,
            panel_r[0] + 14.0,
            panel_r[1] + 13.0,
            "DISCOVERABLE REALMS",
            10.0,
            14.0,
            1.0,
            [0.95, 0.78, 0.30, 0.95],
        );

        Self::draw_menu_button(
            mesh,
            refresh_r[0],
            refresh_r[1],
            refresh_r[2],
            refresh_r[3],
            if directory_loading { "..." } else { "* Refresh" },
            cursor_pos,
            !directory_loading,
        );

        if directory_loading && directory_servers.is_empty() {
            Self::draw_string(
                mesh,
                panel_r[0] + 16.0,
                panel_r[1] + 64.0,
                "Scanning public realm directory...",
                9.0,
                13.0,
                1.0,
                [0.72, 0.78, 0.88, 0.90],
            );
        } else if let Some(err) = directory_error.filter(|_| directory_servers.is_empty()) {
            let short_err: String = err.chars().take(48).collect();
            Self::draw_string(
                mesh,
                panel_r[0] + 16.0,
                panel_r[1] + 64.0,
                "Directory unreachable:",
                9.0,
                13.0,
                1.0,
                [1.0, 0.45, 0.45, 1.0],
            );
            Self::draw_string(
                mesh,
                panel_r[0] + 16.0,
                panel_r[1] + 84.0,
                &short_err,
                8.0,
                11.5,
                1.0,
                [0.85, 0.65, 0.65, 0.95],
            );
        } else if directory_servers.is_empty() {
            Self::draw_string(
                mesh,
                panel_r[0] + 16.0,
                panel_r[1] + 64.0,
                "No public servers found. Direct connect via IP or host above.",
                8.5,
                12.0,
                0.8,
                [0.70, 0.75, 0.84, 0.85],
            );
        } else {
            for (idx, srv) in directory_servers.iter().take(row_rects.len()).enumerate() {
                let r = row_rects[idx];
                let selected = selected_server_idx == Some(idx);
                let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3]);
                let border = if selected {
                    [0.95, 0.78, 0.30, 1.0]
                } else if hovered {
                    [0.55, 0.62, 0.78, 0.95]
                } else {
                    [0.22, 0.25, 0.32, 0.85]
                };
                let bg = if selected {
                    [0.15, 0.20, 0.28, 0.96]
                } else if hovered {
                    [0.12, 0.15, 0.22, 0.94]
                } else {
                    [0.09, 0.11, 0.16, 0.90]
                };
                let ind = if selected {
                    [0.95, 0.78, 0.30, 1.0]
                } else if hovered {
                    [0.65, 0.55, 0.30, 0.80]
                } else {
                    [0.22, 0.25, 0.32, 0.70]
                };

                mesh.add_quad(r[0] + 1.5, r[1] + 1.5, r[2], r[3], white_uv, [0.01, 0.02, 0.04, 0.35]);
                mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
                mesh.add_quad(
                    r[0] + 1.5,
                    r[1] + 1.5,
                    r[2] - 3.0,
                    r[3] - 3.0,
                    white_uv,
                    bg,
                );
                mesh.add_quad(r[0] + 1.5, r[1] + 1.5, 3.0, r[3] - 3.0, white_uv, ind);

                let name_short: String = srv.name.chars().take(24).collect();
                Self::draw_string(
                    mesh,
                    r[0] + 10.0,
                    r[1] + 6.0,
                    &name_short,
                    9.5,
                    13.0,
                    1.0,
                    [0.98, 0.98, 1.0, 1.0],
                );

                let right_meta = if srv.voice_enabled {
                    format!("{}/{} [Voice]", srv.players, srv.max_players)
                } else {
                    format!("{}/{}", srv.players, srv.max_players)
                };
                let meta_w = Self::string_width(&right_meta, 8.5, 1.0);
                Self::draw_string(
                    mesh,
                    r[0] + r[2] - meta_w - 8.0,
                    r[1] + 7.0,
                    &right_meta,
                    8.5,
                    12.0,
                    1.0,
                    [0.45, 0.92, 0.65, 1.0],
                );

                let motd_short: String = srv.motd.chars().take(34).collect();
                Self::draw_colored_string(
                    mesh,
                    r[0] + 10.0,
                    r[1] + 24.0,
                    &motd_short,
                    8.0,
                    11.5,
                    0.8,
                    [0.78, 0.82, 0.90, 0.95],
                );

                let addr_str = format!("{}:{}", srv.host, srv.port);
                let addr_w = Self::string_width(&addr_str, 7.5, 0.8);
                Self::draw_string(
                    mesh,
                    r[0] + r[2] - addr_w - 8.0,
                    r[1] + 25.0,
                    &addr_str,
                    7.5,
                    11.0,
                    0.8,
                    [0.55, 0.65, 0.80, 0.85],
                );
            }
        }

        if let Some((msg, is_err)) = status_message {
            let mw = Self::string_width(msg, 9.0, 1.0);
            let color = if is_err {
                [1.0, 0.42, 0.42, 1.0]
            } else {
                [0.45, 0.95, 0.58, 1.0]
            };
            let sx = modal_x + (modal_w - mw) * 0.5;
            let sy = modal_y + modal_h - 40.0;
            mesh.add_quad(sx - 12.0, sy - 4.0, mw + 24.0, 22.0, white_uv, [0.08, 0.10, 0.14, 0.90]);
            Self::draw_string(
                mesh,
                sx,
                sy,
                msg,
                9.0,
                13.0,
                1.0,
                color,
            );
        }
    }

    /// Projects remote (and 3rd-person local) 3D player head positions to 2D screen coordinates
    /// and draws floating username nameplates + health bars above their heads.
    pub fn draw_player_nametags(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        view_proj: glam::Mat4,
        camera_pos: glam::Vec3,
        players: &[blockgame_core::net::PlayerModelState],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        for player in players {
            if player.health <= 0.0 || player.username.is_empty() {
                continue;
            }
            let dist = player.position.distance(camera_pos);
            if dist > 64.0 {
                continue;
            }

            let tag_offset_y = if player.is_sneaking { 1.92 } else { 2.08 };
            let world_pt = player.position + glam::Vec3::new(0.0, tag_offset_y, 0.0);
            let clip = view_proj * glam::Vec4::new(world_pt.x, world_pt.y, world_pt.z, 1.0);
            if clip.w <= 0.1 {
                continue;
            }

            let ndc_x = clip.x / clip.w;
            let ndc_y = clip.y / clip.w;
            if ndc_x < -1.1 || ndc_x > 1.1 || ndc_y < -1.1 || ndc_y > 1.1 {
                continue;
            }

            let sx = (ndc_x * 0.5 + 0.5) * screen_w;
            let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * screen_h;

            let char_w = 8.5;
            let char_h = 12.0;
            let text_w = Self::string_width(&player.username, char_w, 1.0);
            let pad_x = 6.0;
            let pad_y = 4.0;
            let box_w = text_w + pad_x * 2.0;
            let box_h = char_h + pad_y * 2.0;
            let bx = sx - box_w * 0.5;
            let by = sy - box_h;

            let alpha = if player.is_sneaking { 0.42 } else { 0.68 };
            mesh.add_quad(
                bx,
                by,
                box_w,
                box_h,
                white_uv,
                [0.04, 0.05, 0.08, alpha],
            );

            Self::draw_string(
                mesh,
                bx + pad_x,
                by + pad_y,
                &player.username,
                char_w,
                char_h,
                1.0,
                [1.0, 1.0, 1.0, if player.is_sneaking { 0.65 } else { 0.98 }],
            );

            // Health bar underneath nameplate if damaged
            if player.health < player.max_health {
                let bar_w = box_w.max(44.0);
                let bar_h = 4.0;
                let bar_x = sx - bar_w * 0.5;
                let bar_y = by + box_h + 2.0;
                let ratio = (player.health / player.max_health.max(1.0)).clamp(0.0, 1.0);
                mesh.add_quad(
                    bar_x,
                    bar_y,
                    bar_w,
                    bar_h,
                    white_uv,
                    [0.15, 0.04, 0.04, 0.80],
                );
                mesh.add_quad(
                    bar_x,
                    bar_y,
                    bar_w * ratio,
                    bar_h,
                    white_uv,
                    [0.25, 0.92, 0.38, 0.92],
                );
            }
        }
    }

    /// Splits a plain/color-coded text slice into `ChatSegment::Text` and `ChatSegment::Link`
    /// segments for any valid `http://` or `https://` URLs. Non-HTTP(S) schemes are left as text.
    fn split_text_with_urls<'a>(text: &'a str, out: &mut Vec<ChatSegment<'a>>) {
        let mut cursor = 0;
        let bytes = text.as_bytes();
        while cursor < text.len() {
            let rest = &text[cursor..];
            let next_http = rest.find("http://");
            let next_https = rest.find("https://");
            let rel_start = match (next_http, next_https) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break,
            };

            let abs_start = cursor + rel_start;
            let scheme_len = if text[abs_start..].starts_with("https://") {
                8
            } else {
                7
            };

            // Reject if immediately preceded by an alphanumeric/scheme character (e.g., `foohttp://`)
            if abs_start > 0 {
                let prev = text[..abs_start].chars().next_back().unwrap_or(' ');
                if prev.is_ascii_alphanumeric() || prev == '_' || prev == '+' || prev == '-' {
                    let advance = abs_start + scheme_len;
                    out.push(ChatSegment::Text(&text[cursor..advance]));
                    cursor = advance;
                    continue;
                }
            }

            // Scan until whitespace or control character
            let mut abs_end = abs_start + scheme_len;
            while abs_end < bytes.len() {
                let ch = text[abs_end..].chars().next().unwrap();
                if ch.is_whitespace() || ch.is_control() || ch == '<' || ch == '>' {
                    break;
                }
                abs_end += ch.len_utf8();
            }

            // Trim trailing sentence punctuation (preserving balanced parentheses in URLs)
            while abs_end > abs_start + scheme_len {
                let last = text[..abs_end].chars().next_back().unwrap();
                let should_trim = match last {
                    '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | ']' => true,
                    ')' => {
                        let slice = &text[abs_start..abs_end];
                        let opens = slice.chars().filter(|&c| c == '(').count();
                        let closes = slice.chars().filter(|&c| c == ')').count();
                        closes > opens
                    }
                    _ => false,
                };
                if should_trim {
                    abs_end -= last.len_utf8();
                } else {
                    break;
                }
            }

            let candidate = &text[abs_start..abs_end];
            let host_part = &candidate[scheme_len..];
            let valid_host = !host_part.is_empty()
                && host_part
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '[');

            if valid_host {
                if abs_start > cursor {
                    out.push(ChatSegment::Text(&text[cursor..abs_start]));
                }
                out.push(ChatSegment::Link {
                    label: candidate,
                    url: candidate.to_string(),
                });
                cursor = abs_end;
            } else {
                let advance = abs_start + scheme_len;
                out.push(ChatSegment::Text(&text[cursor..advance]));
                cursor = advance;
            }
        }

        if cursor < text.len() {
            out.push(ChatSegment::Text(&text[cursor..]));
        }
    }

    pub fn parse_chat_segments<'a>(line: &'a str, alpha: f32) -> Vec<ChatSegment<'a>> {
        let mut segments = Vec::new();
        let mut cursor = 0;

        while let Some(start_rel) = line[cursor..].find('[') {
            let abs_start = cursor + start_rel;
            if let Some(end_rel) = line[abs_start..].find(']') {
                let abs_end = abs_start + end_rel;
                let inner = &line[abs_start + 1..abs_end];
                let lower = inner.trim().to_ascii_lowercase();

                let button = if lower == "accept" {
                    Some((
                        "Accept",
                        "/tpaccept".to_string(),
                        [0.15, 0.58, 0.25, alpha],
                        [0.25, 0.82, 0.38, alpha.max(0.95)],
                    ))
                } else if lower == "deny" {
                    Some((
                        "Deny",
                        "/tpdeny".to_string(),
                        [0.65, 0.20, 0.20, alpha],
                        [0.88, 0.28, 0.28, alpha.max(0.95)],
                    ))
                } else if lower == "spawn" {
                    Some((
                        "Spawn",
                        "/spawn".to_string(),
                        [0.20, 0.45, 0.75, alpha],
                        [0.30, 0.62, 0.95, alpha.max(0.95)],
                    ))
                } else if inner.starts_with('/') {
                    Some((
                        inner,
                        inner.to_string(),
                        [0.20, 0.48, 0.62, alpha],
                        [0.30, 0.65, 0.85, alpha.max(0.95)],
                    ))
                } else if lower.starts_with("home:") || lower.starts_with("home ") {
                    let hname = inner[5..].trim();
                    Some((
                        hname,
                        format!("/home {}", hname),
                        [0.20, 0.48, 0.62, alpha],
                        [0.30, 0.65, 0.85, alpha.max(0.95)],
                    ))
                } else if lower.starts_with("click:") {
                    let cmd = inner[6..].trim();
                    Some((
                        cmd,
                        cmd.to_string(),
                        [0.72, 0.58, 0.18, alpha],
                        [0.92, 0.75, 0.25, alpha.max(0.95)],
                    ))
                } else {
                    None
                };

                if let Some((label, cmd, col, hover_col)) = button {
                    if abs_start > cursor {
                        Self::split_text_with_urls(&line[cursor..abs_start], &mut segments);
                    }
                    segments.push(ChatSegment::Button {
                        label,
                        command: cmd,
                        color: col,
                        hover_color: hover_col,
                    });
                    cursor = abs_end + 1;
                    continue;
                }
            }
            let next_start = abs_start + 1;
            if next_start <= line.len() {
                let next_bracket = line[next_start..]
                    .find('[')
                    .map(|i| next_start + i)
                    .unwrap_or(line.len());
                Self::split_text_with_urls(&line[cursor..next_bracket], &mut segments);
                cursor = next_bracket;
            } else {
                break;
            }
        }

        if cursor < line.len() {
            Self::split_text_with_urls(&line[cursor..], &mut segments);
        }

        segments
    }

    /// Draws the in-game Multiplayer Chat Box (bottom-left) and the Tab Online Player List overlay.
    pub fn draw_multiplayer_hud_overlays(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        chat_history: &[blockgame_core::net::ChatEntry],
        chat_open: bool,
        chat_input: &str,
        chat_scroll: usize,
        cursor_pos: [f32; 2],
        tab_held: bool,
        online_players: &[blockgame_core::net::PlayerModelState],
        local_username: &str,
        is_hosting: bool,
        server_addr: &str,
    ) -> Vec<ChatActionHitbox> {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let mut hitboxes = Vec::new();

        // 1. Top-left small multiplayer status badge
        let badge = if is_hosting {
            format!("Hosting MP ({}) - {} Online [Tab: List | T: Chat | F5: Camera]", server_addr, online_players.len().max(1))
        } else if !server_addr.is_empty() {
            format!("Connected ({}) - {} Online [Tab: List | T: Chat | F5: Camera]", server_addr, online_players.len().max(1))
        } else {
            String::new()
        };
        if !badge.is_empty() {
            let bw = Self::string_width(&badge, 7.5, 1.0) + 12.0;
            mesh.add_quad(8.0, 8.0, bw, 20.0, white_uv, [0.04, 0.05, 0.08, 0.55]);
            Self::draw_string(
                mesh,
                14.0,
                12.0,
                &badge,
                7.5,
                11.0,
                1.0,
                [0.82, 0.90, 1.0, 0.92],
            );
        }

        // 2. Bottom-left Chat Log & Input Box
        let visible_count = 10;
        let visible_messages: Vec<&blockgame_core::net::ChatEntry> = if chat_open {
            let total = chat_history.len();
            let max_scroll = total.saturating_sub(visible_count);
            let actual_scroll = chat_scroll.min(max_scroll);
            chat_history
                .iter()
                .rev()
                .skip(actual_scroll)
                .take(visible_count)
                .collect()
        } else {
            chat_history
                .iter()
                .rev()
                .filter(|m| m.age < 9.0)
                .take(8)
                .collect()
        };

        let chat_x = 14.0;
        let chat_w = 480.0_f32.min(screen_w * 0.55);
        let mut msg_y = screen_h - 140.0;

        // Subtle scrollbar on right edge when chat is open and exceeds visible count
        if chat_open && chat_history.len() > visible_count {
            let total = chat_history.len();
            let max_scroll = total.saturating_sub(visible_count);
            let actual_scroll = chat_scroll.min(max_scroll);
            let sb_x = chat_x + chat_w - 4.0;
            let sb_top = screen_h - 140.0 - (visible_count as f32 - 1.0) * 22.0;
            let sb_height = (visible_count as f32) * 22.0;
            mesh.add_quad(sb_x, sb_top, 3.0, sb_height, white_uv, [0.10, 0.12, 0.18, 0.50]);
            let thumb_h = (sb_height * (visible_count as f32 / total as f32)).max(16.0);
            let thumb_ratio = if max_scroll > 0 { actual_scroll as f32 / max_scroll as f32 } else { 0.0 };
            let thumb_y = (sb_top + sb_height - thumb_h) - thumb_ratio * (sb_height - thumb_h);
            mesh.add_quad(sb_x, thumb_y, 3.0, thumb_h, white_uv, [0.45, 0.70, 0.95, 0.85]);

            if actual_scroll > 0 {
                let note = format!("▲ Scrolled up (+{})", actual_scroll);
                Self::draw_string(mesh, chat_x + 6.0, sb_top - 16.0, &note, 7.0, 10.0, 1.0, [0.75, 0.85, 1.0, 0.85]);
            }
        }

        for entry in visible_messages {
            let alpha = if chat_open {
                0.88
            } else {
                ((9.0 - entry.age) / 2.0).clamp(0.0, 0.82)
            };
            if alpha <= 0.02 {
                continue;
            }
            mesh.add_quad(
                chat_x,
                msg_y,
                chat_w,
                20.0,
                white_uv,
                [0.03, 0.04, 0.06, alpha * 0.72],
            );
            let line = if entry.is_system {
                format!("* {}", entry.message)
            } else if entry.prefix.is_empty() {
                format!("<{}> {}", entry.sender, entry.message)
            } else {
                format!("{}&r<{}> &f{}", entry.prefix, entry.sender, entry.message)
            };
            let text_col = if entry.is_system {
                [0.98, 0.86, 0.35, alpha]
            } else {
                [0.95, 0.97, 1.0, alpha]
            };

            let segments = Self::parse_chat_segments(&line, alpha);
            let mut seg_x = chat_x + 6.0;
            for seg in segments {
                match seg {
                    ChatSegment::Text(t) => {
                        Self::draw_colored_string(
                            mesh,
                            seg_x,
                            msg_y + 4.0,
                            t,
                            8.0,
                            11.5,
                            1.0,
                            text_col,
                        );
                        seg_x += Self::colored_string_width(t, 8.0, 1.0);
                    }
                    ChatSegment::Button { label, command, color, hover_color } => {
                        let btn_w = Self::string_width(label, 7.5, 1.0) + 10.0;
                        let btn_h = 16.0;
                        let btn_y = msg_y + 2.0;
                        let is_hovered = chat_open && Self::is_rect_hovered(cursor_pos, seg_x, btn_y, btn_w, btn_h);
                        let bg = if is_hovered { hover_color } else { color };
                        mesh.add_quad(seg_x, btn_y, btn_w, btn_h, white_uv, bg);
                        if is_hovered {
                            mesh.add_quad(seg_x - 1.0, btn_y - 1.0, btn_w + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.9]);
                            mesh.add_quad(seg_x - 1.0, btn_y + btn_h, btn_w + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.9]);
                            mesh.add_quad(seg_x - 1.0, btn_y, 1.0, btn_h, white_uv, [1.0, 1.0, 1.0, 0.9]);
                            mesh.add_quad(seg_x + btn_w, btn_y, 1.0, btn_h, white_uv, [1.0, 1.0, 1.0, 0.9]);
                        }
                        Self::draw_string(
                            mesh,
                            seg_x + 5.0,
                            btn_y + 3.0,
                            label,
                            7.5,
                            10.5,
                            1.0,
                            [1.0, 1.0, 1.0, alpha.max(0.9)],
                        );
                        if chat_open {
                            hitboxes.push(ChatActionHitbox {
                                rect: [seg_x, btn_y, btn_w, btn_h],
                                action: ChatAction::Command(command),
                            });
                        }
                        seg_x += btn_w + 5.0;
                    }
                    ChatSegment::Link { label, url } => {
                        let link_w = Self::string_width(label, 8.0, 1.0);
                        let link_h = 16.0;
                        let link_y = msg_y + 2.0;
                        let is_hovered =
                            chat_open && Self::is_rect_hovered(cursor_pos, seg_x, link_y, link_w, link_h);
                        let link_col = if is_hovered {
                            [0.65, 0.90, 1.0, alpha.max(0.95)]
                        } else {
                            [0.42, 0.78, 1.0, alpha.max(0.90)]
                        };
                        Self::draw_string(
                            mesh,
                            seg_x,
                            msg_y + 4.0,
                            label,
                            8.0,
                            11.5,
                            1.0,
                            link_col,
                        );
                        mesh.add_quad(seg_x, msg_y + 16.0, link_w, 1.2, white_uv, link_col);
                        if chat_open {
                            hitboxes.push(ChatActionHitbox {
                                rect: [seg_x, link_y, link_w, link_h],
                                action: ChatAction::OpenUrl(url),
                            });
                        }
                        seg_x += link_w;
                    }
                }
            }
            msg_y -= 22.0;
        }

        if chat_open {
            let input_y = screen_h - 112.0;
            mesh.add_quad(
                chat_x - 2.0,
                input_y - 2.0,
                chat_w + 4.0,
                28.0,
                white_uv,
                [0.45, 0.75, 1.0, 0.90],
            );
            mesh.add_quad(
                chat_x,
                input_y,
                chat_w,
                24.0,
                white_uv,
                [0.05, 0.06, 0.09, 0.94],
            );
            let prompt = format!("> {}_", chat_input);
            Self::draw_string(
                mesh,
                chat_x + 6.0,
                input_y + 6.0,
                &prompt,
                8.5,
                12.0,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        // 3. Tab Online Player List Overlay
        if tab_held {
            let panel_w = 340.0;
            let row_h = 26.0;
            let count = online_players.len().max(1);
            let panel_h = 38.0 + (count as f32) * row_h + 10.0;
            let px = (screen_w - panel_w) * 0.5;
            let py = 46.0;

            mesh.add_quad(
                px - 2.0,
                py - 2.0,
                panel_w + 4.0,
                panel_h + 4.0,
                white_uv,
                [0.25, 0.32, 0.45, 0.85],
            );
            mesh.add_quad(
                px,
                py,
                panel_w,
                panel_h,
                white_uv,
                [0.05, 0.07, 0.11, 0.88],
            );

            let title = format!("Online Players ({})", count);
            let tw = Self::string_width(&title, 9.5, 1.2);
            Self::draw_string(
                mesh,
                px + (panel_w - tw) * 0.5,
                py + 10.0,
                &title,
                9.5,
                13.5,
                1.2,
                [0.95, 0.85, 0.38, 1.0],
            );

            for (i, p) in online_players.iter().enumerate() {
                let ry = py + 34.0 + (i as f32) * row_h;
                mesh.add_quad(
                    px + 8.0,
                    ry,
                    panel_w - 16.0,
                    row_h - 3.0,
                    white_uv,
                    [0.12, 0.15, 0.22, 0.78],
                );
                Self::draw_character_portrait(
                    mesh,
                    px + 14.0,
                    ry + 3.0,
                    17.0,
                    &p.appearance,
                );

                let tag = if p.username.eq_ignore_ascii_case(local_username) {
                    format!("{}&r{} (You)", p.prefix, p.username)
                } else {
                    format!("{}&r{}", p.prefix, p.username)
                };
                Self::draw_colored_string(
                    mesh,
                    px + 38.0,
                    ry + 6.0,
                    &tag,
                    8.5,
                    12.0,
                    1.0,
                    [1.0, 1.0, 1.0, 0.98],
                );

                let hp_str = format!("{:.0}/20 HP", p.health.ceil());
                let hw = Self::string_width(&hp_str, 8.0, 1.0);
                Self::draw_string(
                    mesh,
                    px + panel_w - 16.0 - hw,
                    ry + 6.0,
                    &hp_str,
                    8.0,
                    11.0,
                    1.0,
                    [0.45, 0.95, 0.55, 0.95],
                );
            }
        }

        hitboxes
    }

    /// Draws the confirmation modal shown when a player clicks an `http://` or `https://` URL in chat.
    /// Returns hitboxes for the Open in Browser, Copy URL, and Cancel buttons.
    pub fn draw_link_confirm_modal(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        url: &str,
        copied_feedback: bool,
        cursor_pos: [f32; 2],
    ) -> LinkConfirmModalHitboxes {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Dim backdrop
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.02, 0.03, 0.05, 0.68]);

        let panel_w = 540.0_f32.min(screen_w - 40.0).max(320.0);
        let panel_h = 196.0;
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Outer border + dark panel
        mesh.add_quad(px - 2.0, py - 2.0, panel_w + 4.0, panel_h + 4.0, white_uv, [0.42, 0.72, 0.98, 0.90]);
        mesh.add_quad(px, py, panel_w, panel_h, white_uv, [0.07, 0.09, 0.13, 0.96]);

        // Title
        let title = "Open External Link?";
        let tw = Self::string_width(title, 11.0, 1.2);
        Self::draw_string(
            mesh,
            px + (panel_w - tw) * 0.5,
            py + 16.0,
            title,
            11.0,
            15.0,
            1.2,
            [0.95, 0.97, 1.0, 1.0],
        );

        let subtitle = "Only open links from people you trust:";
        let sw = Self::string_width(subtitle, 7.5, 1.0);
        Self::draw_string(
            mesh,
            px + (panel_w - sw) * 0.5,
            py + 42.0,
            subtitle,
            7.5,
            11.0,
            1.0,
            [0.75, 0.80, 0.88, 0.95],
        );

        // URL box (wrap into up to 2 lines if long)
        let url_box_x = px + 18.0;
        let url_box_y = py + 64.0;
        let url_box_w = panel_w - 36.0;
        let url_box_h = 54.0;
        mesh.add_quad(url_box_x, url_box_y, url_box_w, url_box_h, white_uv, [0.04, 0.05, 0.08, 0.95]);

        let max_chars = ((url_box_w - 16.0) / 8.5).floor().max(24.0) as usize;
        let chars: Vec<char> = url.chars().collect();
        if chars.len() <= max_chars {
            Self::draw_string(
                mesh,
                url_box_x + 8.0,
                url_box_y + 20.0,
                url,
                7.5,
                11.0,
                1.0,
                [0.45, 0.82, 1.0, 1.0],
            );
        } else {
            let line1: String = chars[..max_chars].iter().collect();
            let rest = &chars[max_chars..];
            let line2: String = if rest.len() > max_chars {
                let mut s: String = rest[..max_chars.saturating_sub(3)].iter().collect();
                s.push_str("...");
                s
            } else {
                rest.iter().collect()
            };
            Self::draw_string(
                mesh,
                url_box_x + 8.0,
                url_box_y + 10.0,
                &line1,
                7.5,
                11.0,
                1.0,
                [0.45, 0.82, 1.0, 1.0],
            );
            Self::draw_string(
                mesh,
                url_box_x + 8.0,
                url_box_y + 28.0,
                &line2,
                7.5,
                11.0,
                1.0,
                [0.45, 0.82, 1.0, 1.0],
            );
        }

        // Three action buttons: Open in Browser, Copy URL, Cancel
        let btn_y = py + panel_h - 50.0;
        let btn_h = 34.0;
        let gap = 12.0;
        let total_btn_w = panel_w - 36.0;
        let btn_w = (total_btn_w - gap * 2.0) / 3.0;

        let open_rect = [px + 18.0, btn_y, btn_w, btn_h];
        let copy_rect = [px + 18.0 + btn_w + gap, btn_y, btn_w, btn_h];
        let cancel_rect = [px + 18.0 + (btn_w + gap) * 2.0, btn_y, btn_w, btn_h];

        let draw_btn = |mesh: &mut UiMesh, rect: [f32; 4], label: &str, base: [f32; 4], hover: [f32; 4]| {
            let hovered = Self::is_rect_hovered(cursor_pos, rect[0], rect[1], rect[2], rect[3]);
            mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, if hovered { hover } else { base });
            if hovered {
                mesh.add_quad(rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.9]);
                mesh.add_quad(rect[0] - 1.0, rect[1] + rect[3], rect[2] + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.9]);
                mesh.add_quad(rect[0] - 1.0, rect[1], 1.0, rect[3], white_uv, [1.0, 1.0, 1.0, 0.9]);
                mesh.add_quad(rect[0] + rect[2], rect[1], 1.0, rect[3], white_uv, [1.0, 1.0, 1.0, 0.9]);
            }
            let lw = Self::string_width(label, 8.0, 1.0);
            Self::draw_string(
                mesh,
                rect[0] + (rect[2] - lw) * 0.5,
                rect[1] + 11.0,
                label,
                8.0,
                11.5,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        };

        draw_btn(mesh, open_rect, "Open in Browser", [0.16, 0.52, 0.28, 0.95], [0.22, 0.68, 0.36, 1.0]);
        draw_btn(
            mesh,
            copy_rect,
            if copied_feedback { "Copied!" } else { "Copy URL" },
            [0.20, 0.42, 0.68, 0.95],
            [0.28, 0.56, 0.88, 1.0],
        );
        draw_btn(mesh, cancel_rect, "Cancel", [0.45, 0.22, 0.24, 0.95], [0.64, 0.30, 0.32, 1.0]);

        LinkConfirmModalHitboxes {
            panel_rect: [px, py, panel_w, panel_h],
            open_rect,
            copy_rect,
            cancel_rect,
        }
    }

    /// Draws the permission-gated "Admin Panel" button below the Pause Menu card on multiplayer servers.
    /// Returns its hitbox `[x, y, w, h]`.
    pub fn draw_pause_admin_button(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
    ) -> [f32; 4] {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let card_w = 340.0_f32.min(screen_w - 40.0);
        let card_h = 440.0_f32.min(screen_h - 40.0);
        let card_y = (screen_h - card_h) * 0.5;
        let btn_w = (card_w - 48.0).max(200.0);
        let btn_h = 34.0;
        let bx = (screen_w - btn_w) * 0.5;
        let by = (card_y + card_h + 10.0).min(screen_h - btn_h - 8.0);
        let hovered = Self::is_rect_hovered(cursor_pos, bx, by, btn_w, btn_h);
        let bg = if hovered {
            [0.24, 0.48, 0.78, 0.96]
        } else {
            [0.14, 0.30, 0.52, 0.92]
        };
        mesh.add_quad(bx - 1.5, by - 1.5, btn_w + 3.0, btn_h + 3.0, white_uv, [0.48, 0.78, 1.0, 0.85]);
        mesh.add_quad(bx, by, btn_w, btn_h, white_uv, bg);
        let label = "Admin Panel";
        let lw = Self::string_width(label, 9.0, 1.0);
        Self::draw_string(
            mesh,
            bx + (btn_w - lw) * 0.5,
            by + 10.0,
            label,
            9.0,
            12.5,
            1.0,
            [0.96, 0.98, 1.0, 1.0],
        );
        [bx, by, btn_w, btn_h]
    }

    /// Draws the unified, permission-gated in-game Admin Panel (Players, Bans, Chat, Permissions,
    /// Auto-Messages, and Server tabs) and returns clickable hitboxes for mouse and Virtual Cursor input.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_admin_panel(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        snapshot: &blockgame_core::admin_panel::AdminPanelSnapshot,
        active_tab: blockgame_core::admin_panel::AdminTab,
        input_text: &str,
        selected_player: Option<&str>,
        selected_group: Option<&str>,
        selected_perm_player: Option<&str>,
        perm_players_view: bool,
        node_scroll: usize,
        cursor_pos: [f32; 2],
    ) -> Vec<AdminUiHitbox> {
        use blockgame_core::admin_panel::{AdminPanelAction, AdminTab};
        use blockgame_core::automessage::AutoMessageOrder;
        use blockgame_core::moderation::WordFilterMode;

        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let mut hitboxes: Vec<AdminUiHitbox> = Vec::new();

        // Backdrop
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.02, 0.03, 0.05, 0.72]);

        let panel_w = 820.0_f32.min(screen_w - 32.0).max(520.0);
        let panel_h = 540.0_f32.min(screen_h - 32.0).max(380.0);
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Frame & background
        mesh.add_quad(px - 2.0, py - 2.0, panel_w + 4.0, panel_h + 4.0, white_uv, [0.36, 0.64, 0.92, 0.88]);
        mesh.add_quad(px, py, panel_w, panel_h, white_uv, [0.06, 0.08, 0.12, 0.96]);

        let mut push_btn = |mesh: &mut UiMesh,
                            rect: [f32; 4],
                            label: &str,
                            base: [f32; 4],
                            hover: [f32; 4],
                            action: AdminUiClickAction| {
            let hovered = Self::is_rect_hovered(cursor_pos, rect[0], rect[1], rect[2], rect[3]);
            mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, if hovered { hover } else { base });
            if hovered {
                mesh.add_quad(rect[0] - 1.0, rect[1] - 1.0, rect[2] + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.85]);
                mesh.add_quad(rect[0] - 1.0, rect[1] + rect[3], rect[2] + 2.0, 1.0, white_uv, [1.0, 1.0, 1.0, 0.85]);
            }
            let lw = Self::string_width(label, 7.2, 0.9);
            Self::draw_string(
                mesh,
                rect[0] + (rect[2] - lw).max(4.0) * 0.5,
                rect[1] + (rect[3] - 10.0) * 0.5,
                label,
                7.2,
                10.0,
                0.9,
                [0.98, 0.99, 1.0, 1.0],
            );
            hitboxes.push(AdminUiHitbox { rect, action });
        };

        // Header bar
        mesh.add_quad(px, py, panel_w, 36.0, white_uv, [0.09, 0.12, 0.18, 0.98]);
        Self::draw_string(
            mesh,
            px + 14.0,
            py + 10.0,
            "Admin Panel",
            10.5,
            14.5,
            1.1,
            [0.95, 0.97, 1.0, 1.0],
        );
        push_btn(
            mesh,
            [px + panel_w - 154.0, py + 6.0, 72.0, 24.0],
            "Refresh",
            [0.18, 0.36, 0.58, 0.95],
            [0.26, 0.50, 0.78, 1.0],
            AdminUiClickAction::Refresh,
        );
        push_btn(
            mesh,
            [px + panel_w - 74.0, py + 6.0, 62.0, 24.0],
            "Close",
            [0.52, 0.20, 0.22, 0.95],
            [0.72, 0.28, 0.30, 1.0],
            AdminUiClickAction::Close,
        );

        // Permission-gated Tab Bar
        let tab_y = py + 42.0;
        let mut tx = px + 12.0;
        for &tab in &snapshot.allowed_tabs {
            let tw = (Self::string_width(tab.label(), 7.8, 1.0) + 22.0).max(76.0);
            let is_active = tab == active_tab;
            push_btn(
                mesh,
                [tx, tab_y, tw, 26.0],
                tab.label(),
                if is_active {
                    [0.24, 0.52, 0.82, 0.98]
                } else {
                    [0.12, 0.16, 0.23, 0.92]
                },
                [0.30, 0.60, 0.92, 1.0],
                AdminUiClickAction::SelectTab(tab),
            );
            tx += tw + 6.0;
        }

        // Status banner
        let body_top = py + 76.0;
        let mut content_y = body_top;
        if !snapshot.status_message.is_empty() {
            let is_err = snapshot.status_message.starts_with("[Error]");
            mesh.add_quad(
                px + 12.0,
                content_y,
                panel_w - 24.0,
                22.0,
                white_uv,
                if is_err {
                    [0.34, 0.10, 0.12, 0.92]
                } else {
                    [0.09, 0.24, 0.14, 0.92]
                },
            );
            Self::draw_string(
                mesh,
                px + 18.0,
                content_y + 5.0,
                &snapshot.status_message,
                7.2,
                10.5,
                0.9,
                if is_err {
                    [1.0, 0.65, 0.65, 1.0]
                } else {
                    [0.65, 0.98, 0.72, 1.0]
                },
            );
            content_y += 28.0;
        }

        // Shared Input Bar at the bottom of the panel
        let input_y = py + panel_h - 38.0;
        mesh.add_quad(px + 12.0, input_y, panel_w - 24.0, 26.0, white_uv, [0.04, 0.05, 0.08, 0.96]);
        let prompt_hint = match active_tab {
            AdminTab::Players => "Input (offline player / reason / group):",
            AdminTab::Bans => "Input (username or IP to ban):",
            AdminTab::Chat => "Input (word to add/remove, or player for warn/mute):",
            AdminTab::Permissions => "Input (group name / prefix / custom node / player):",
            AdminTab::AutoMessages => "Input (new Auto-Message text, supports & colors & URLs):",
            AdminTab::Server => "Input (server broadcast message):",
        };
        Self::draw_string(
            mesh,
            px + 14.0,
            input_y - 14.0,
            prompt_hint,
            6.8,
            9.5,
            0.9,
            [0.68, 0.76, 0.86, 0.92],
        );
        let displayed_input = format!("> {}_", input_text);
        Self::draw_string(
            mesh,
            px + 18.0,
            input_y + 7.0,
            &displayed_input,
            7.6,
            11.0,
            1.0,
            [0.95, 0.98, 1.0, 1.0],
        );

        let content_x = px + 14.0;
        let content_w = panel_w - 28.0;

        if !snapshot.allowed_tabs.contains(&active_tab) {
            Self::draw_string(
                mesh,
                content_x,
                content_y + 12.0,
                "You do not have permission to view this tab.",
                8.0,
                11.5,
                1.0,
                [0.95, 0.55, 0.55, 1.0],
            );
            return hitboxes;
        }

        match active_tab {
            AdminTab::Players => {
                let left_w = (content_w * 0.44).max(200.0);
                let right_x = content_x + left_w + 14.0;
                let right_w = content_w - left_w - 14.0;

                Self::draw_string(
                    mesh,
                    content_x,
                    content_y,
                    &format!("Online Players ({})", snapshot.online_players.len()),
                    8.0,
                    11.0,
                    1.0,
                    [0.80, 0.90, 1.0, 1.0],
                );
                let mut ry = content_y + 18.0;
                for p in snapshot.online_players.iter().take(12) {
                    let is_sel = selected_player.is_some_and(|s| s.eq_ignore_ascii_case(&p.username));
                    let grp_str = if p.groups.is_empty() {
                        "default".to_string()
                    } else {
                        p.groups.join(",")
                    };
                    let row_label = format!(
                        "{} [{}] ({}) ({},{},{}) W:{}",
                        p.username, p.gamemode, grp_str, p.pos[0], p.pos[1], p.pos[2], p.warning_count
                    );
                    push_btn(
                        mesh,
                        [content_x, ry, left_w, 24.0],
                        &row_label,
                        if is_sel {
                            [0.22, 0.48, 0.76, 0.96]
                        } else {
                            [0.11, 0.14, 0.20, 0.90]
                        },
                        [0.26, 0.54, 0.84, 1.0],
                        AdminUiClickAction::SelectPlayer(p.username.clone()),
                    );
                    ry += 28.0;
                }

                let target_label = selected_player
                    .or_else(|| {
                        let t = input_text.trim();
                        if t.is_empty() { None } else { Some(t) }
                    })
                    .unwrap_or("(select player or type in input)");
                Self::draw_string(
                    mesh,
                    right_x,
                    content_y,
                    &format!("Target: {}", target_label),
                    8.0,
                    11.0,
                    1.0,
                    [0.98, 0.88, 0.45, 1.0],
                );

                let actions: &[(&str, &str, [f32; 4])] = &[
                    ("Kick", "player_kick", [0.58, 0.34, 0.16, 0.95]),
                    ("Ban", "player_ban", [0.62, 0.18, 0.20, 0.95]),
                    ("Ban IP", "player_banip", [0.52, 0.14, 0.18, 0.95]),
                    ("Mute 10m", "player_mute", [0.55, 0.42, 0.16, 0.95]),
                    ("Unmute", "player_unmute", [0.18, 0.52, 0.28, 0.95]),
                    ("Warn", "player_warn", [0.65, 0.46, 0.14, 0.95]),
                    ("Warnings", "player_warnings", [0.46, 0.38, 0.20, 0.95]),
                    ("TP To", "player_tpto", [0.18, 0.42, 0.66, 0.95]),
                    ("TP Here", "player_tphere", [0.18, 0.42, 0.66, 0.95]),
                    ("Creative", "player_gm_creative", [0.24, 0.46, 0.58, 0.95]),
                    ("Survival", "player_gm_survival", [0.24, 0.46, 0.58, 0.95]),
                    ("+ Group", "player_group_add", [0.24, 0.48, 0.36, 0.95]),
                    ("- Group", "player_group_remove", [0.48, 0.30, 0.26, 0.95]),
                    ("Reset Data", "player_reset", [0.50, 0.20, 0.22, 0.95]),
                ];
                let col_w = (right_w - 8.0) * 0.5;
                for (i, &(label, key, col)) in actions.iter().enumerate() {
                    let cx = right_x + (i % 2) as f32 * (col_w + 8.0);
                    let cy = content_y + 22.0 + (i / 2) as f32 * 28.0;
                    push_btn(
                        mesh,
                        [cx, cy, col_w, 26.0],
                        label,
                        col,
                        [
                            (col[0] + 0.14).min(1.0),
                            (col[1] + 0.14).min(1.0),
                            (col[2] + 0.14).min(1.0),
                            1.0,
                        ],
                        AdminUiClickAction::SubmitInput(key.to_string()),
                    );
                }
            }
            AdminTab::Bans => {
                push_btn(
                    mesh,
                    [content_x, content_y, 130.0, 24.0],
                    "+ Ban Player",
                    [0.62, 0.20, 0.22, 0.95],
                    [0.78, 0.28, 0.30, 1.0],
                    AdminUiClickAction::SubmitInput("ban_player".to_string()),
                );
                push_btn(
                    mesh,
                    [content_x + 138.0, content_y, 110.0, 24.0],
                    "+ Ban IP",
                    [0.54, 0.16, 0.20, 0.95],
                    [0.70, 0.24, 0.28, 1.0],
                    AdminUiClickAction::SubmitInput("ban_ip".to_string()),
                );

                let filter_q = input_text.trim().to_ascii_lowercase();
                let filtered_players: Vec<_> = snapshot
                    .banned_players
                    .iter()
                    .filter(|b| {
                        filter_q.is_empty()
                            || b.username.to_ascii_lowercase().contains(&filter_q)
                            || b.reason.to_ascii_lowercase().contains(&filter_q)
                            || b.banned_by.to_ascii_lowercase().contains(&filter_q)
                    })
                    .collect();
                let filtered_ips: Vec<_> = snapshot
                    .banned_ips
                    .iter()
                    .filter(|b| {
                        filter_q.is_empty()
                            || b.ip.to_ascii_lowercase().contains(&filter_q)
                            || b.reason.to_ascii_lowercase().contains(&filter_q)
                            || b.banned_by.to_ascii_lowercase().contains(&filter_q)
                    })
                    .collect();

                let mut ry = content_y + 34.0;
                let header = if filter_q.is_empty() {
                    format!(
                        "Banned Players ({}) & IPs ({})  [Type in input bar to search]",
                        filtered_players.len(),
                        filtered_ips.len()
                    )
                } else {
                    format!(
                        "Banned Players ({}) & IPs ({})  [Filter: \"{}\"]",
                        filtered_players.len(),
                        filtered_ips.len(),
                        input_text.trim()
                    )
                };
                Self::draw_string(
                    mesh,
                    content_x,
                    ry,
                    &header,
                    7.5,
                    10.8,
                    0.95,
                    [0.85, 0.90, 1.0, 1.0],
                );
                push_btn(
                    mesh,
                    [content_x + content_w - 118.0, ry - 2.0, 56.0, 18.0],
                    "Up",
                    [0.16, 0.22, 0.32, 0.95],
                    [0.24, 0.34, 0.48, 1.0],
                    AdminUiClickAction::ScrollNodes(-4),
                );
                push_btn(
                    mesh,
                    [content_x + content_w - 58.0, ry - 2.0, 56.0, 18.0],
                    "Down",
                    [0.16, 0.22, 0.32, 0.95],
                    [0.24, 0.34, 0.48, 1.0],
                    AdminUiClickAction::ScrollNodes(4),
                );
                ry += 20.0;

                enum BanRow<'a> {
                    Player(&'a blockgame_core::permissions::BanEntry),
                    Ip(&'a blockgame_core::permissions::IpBanEntry),
                }
                let mut rows: Vec<BanRow<'_>> = Vec::with_capacity(filtered_players.len() + filtered_ips.len());
                for b in filtered_players {
                    rows.push(BanRow::Player(b));
                }
                for b in filtered_ips {
                    rows.push(BanRow::Ip(b));
                }
                let page_size = 10usize;
                let max_scroll = rows.len().saturating_sub(page_size);
                let start = node_scroll.min(max_scroll);

                for row in rows.iter().skip(start).take(page_size) {
                    match row {
                        BanRow::Player(b) => {
                            let line = format!(
                                "{} - {} (by {}, {})",
                                b.username, b.reason, b.banned_by, b.created_at
                            );
                            Self::draw_string(
                                mesh,
                                content_x,
                                ry + 5.0,
                                &line,
                                7.0,
                                10.2,
                                0.88,
                                [0.95, 0.80, 0.80, 1.0],
                            );
                            push_btn(
                                mesh,
                                [content_x + content_w - 76.0, ry, 74.0, 22.0],
                                "Unban",
                                [0.18, 0.52, 0.28, 0.95],
                                [0.26, 0.68, 0.38, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(
                                    format!("unban {}", b.username),
                                )),
                            );
                        }
                        BanRow::Ip(b) => {
                            let line = format!(
                                "IP {} - {} (by {}, {})",
                                b.ip, b.reason, b.banned_by, b.created_at
                            );
                            Self::draw_string(
                                mesh,
                                content_x,
                                ry + 5.0,
                                &line,
                                7.0,
                                10.2,
                                0.88,
                                [0.95, 0.72, 0.72, 1.0],
                            );
                            push_btn(
                                mesh,
                                [content_x + content_w - 86.0, ry, 84.0, 22.0],
                                "Unban IP",
                                [0.18, 0.52, 0.28, 0.95],
                                [0.26, 0.68, 0.38, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(
                                    format!("unban-ip {}", b.ip),
                                )),
                            );
                        }
                    }
                    ry += 25.0;
                }
            }
            AdminTab::Chat => {
                // Row 1: Chat Lock, Slowmode presets, Clear Chat
                let locked = snapshot.chat_settings.chat_locked;
                push_btn(
                    mesh,
                    [content_x, content_y, 145.0, 26.0],
                    if locked { "Chat Lock: ON" } else { "Chat Lock: OFF" },
                    if locked { [0.65, 0.20, 0.20, 0.95] } else { [0.18, 0.45, 0.28, 0.95] },
                    [0.75, 0.30, 0.30, 1.0],
                    AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                        "chatlock {}",
                        if locked { "off" } else { "on" }
                    ))),
                );
                let sm = snapshot.chat_settings.slowmode_secs;
                let next_sm = match sm {
                    0 => 3,
                    3 => 10,
                    _ => 0,
                };
                push_btn(
                    mesh,
                    [content_x + 153.0, content_y, 140.0, 26.0],
                    &format!("Slowmode: {}s", sm),
                    [0.22, 0.40, 0.62, 0.95],
                    [0.30, 0.54, 0.80, 1.0],
                    AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                        "slowmode {}",
                        next_sm
                    ))),
                );
                push_btn(
                    mesh,
                    [content_x + 301.0, content_y, 110.0, 26.0],
                    "Clear Chat",
                    [0.52, 0.36, 0.16, 0.95],
                    [0.68, 0.48, 0.22, 1.0],
                    AdminUiClickAction::SendAction(AdminPanelAction::RunCommand("clearchat".to_string())),
                );

                // Row 2: Word Filter toggle, mode, add/remove word
                let wf_y = content_y + 34.0;
                let wf_on = snapshot.word_filter.enabled;
                push_btn(
                    mesh,
                    [content_x, wf_y, 145.0, 26.0],
                    if wf_on { "Word Filter: ON" } else { "Word Filter: OFF" },
                    if wf_on { [0.18, 0.52, 0.28, 0.95] } else { [0.35, 0.22, 0.24, 0.95] },
                    [0.28, 0.66, 0.38, 1.0],
                    AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                        "wordfilter {}",
                        if wf_on { "off" } else { "on" }
                    ))),
                );
                let next_mode = match snapshot.word_filter.mode {
                    WordFilterMode::Censor => "block",
                    WordFilterMode::Block => "censor",
                };
                push_btn(
                    mesh,
                    [content_x + 153.0, wf_y, 130.0, 26.0],
                    &format!("Mode: {}", snapshot.word_filter.mode.as_str()),
                    [0.26, 0.36, 0.58, 0.95],
                    [0.34, 0.48, 0.74, 1.0],
                    AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                        "wordfilter mode {}",
                        next_mode
                    ))),
                );
                push_btn(
                    mesh,
                    [content_x + 291.0, wf_y, 96.0, 26.0],
                    "+ Add Word",
                    [0.18, 0.48, 0.34, 0.95],
                    [0.26, 0.64, 0.44, 1.0],
                    AdminUiClickAction::SubmitInput("wf_add".to_string()),
                );
                push_btn(
                    mesh,
                    [content_x + 395.0, wf_y, 108.0, 26.0],
                    "- Remove Word",
                    [0.54, 0.24, 0.24, 0.95],
                    [0.70, 0.32, 0.32, 1.0],
                    AdminUiClickAction::SubmitInput("wf_remove".to_string()),
                );

                let words_preview = if snapshot.word_filter.words.is_empty() {
                    "Filtered words: (none)".to_string()
                } else {
                    format!("Filtered words: {}", snapshot.word_filter.words.join(", "))
                };
                Self::draw_string(
                    mesh,
                    content_x,
                    wf_y + 32.0,
                    &words_preview,
                    7.2,
                    10.5,
                    0.9,
                    [0.78, 0.84, 0.92, 0.95],
                );

                // Active Mutes & Recent Warnings
                let mut ry = wf_y + 54.0;
                Self::draw_string(
                    mesh,
                    content_x,
                    ry,
                    &format!(
                        "Active Mutes ({}) | Recent Warnings ({})",
                        snapshot.active_mutes.len(),
                        snapshot.recent_warnings.len()
                    ),
                    7.6,
                    11.0,
                    0.95,
                    [0.98, 0.86, 0.48, 1.0],
                );
                ry += 18.0;
                for m in snapshot.active_mutes.iter().take(4) {
                    let line = format!("[Muted] {} - {}", m.username, m.reason);
                    Self::draw_string(mesh, content_x, ry + 4.0, &line, 7.2, 10.0, 0.9, [0.98, 0.72, 0.72, 1.0]);
                    push_btn(
                        mesh,
                        [content_x + content_w - 78.0, ry, 76.0, 20.0],
                        "Unmute",
                        [0.18, 0.50, 0.28, 0.95],
                        [0.26, 0.66, 0.38, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                            "unmute {}",
                            m.username
                        ))),
                    );
                    ry += 24.0;
                }
                for w in snapshot.recent_warnings.iter().take(4) {
                    let line = format!("[Warn] {} (by {}): {}", w.username, w.warned_by, w.reason);
                    Self::draw_string(mesh, content_x, ry + 4.0, &line, 7.2, 10.0, 0.9, [0.96, 0.90, 0.65, 1.0]);
                    push_btn(
                        mesh,
                        [content_x + content_w - 78.0, ry, 76.0, 20.0],
                        "Clear",
                        [0.44, 0.24, 0.24, 0.95],
                        [0.60, 0.32, 0.32, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                            "clearwarnings {}",
                            w.username
                        ))),
                    );
                    ry += 24.0;
                }
            }
            AdminTab::AutoMessages => {
                if let Some(ref am) = snapshot.automessages {
                    push_btn(
                        mesh,
                        [content_x, content_y, 145.0, 26.0],
                        if am.enabled { "Auto-Msg: ON" } else { "Auto-Msg: OFF" },
                        if am.enabled { [0.18, 0.52, 0.28, 0.95] } else { [0.48, 0.20, 0.22, 0.95] },
                        [0.28, 0.68, 0.38, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                            "automessage {}",
                            if am.enabled { "off" } else { "on" }
                        ))),
                    );
                    let next_order = match am.order {
                        AutoMessageOrder::Sequential => "random",
                        AutoMessageOrder::Random => "sequential",
                    };
                    push_btn(
                        mesh,
                        [content_x + 153.0, content_y, 150.0, 26.0],
                        &format!("Order: {}", am.order.as_str()),
                        [0.22, 0.40, 0.62, 0.95],
                        [0.30, 0.54, 0.80, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                            "automessage order {}",
                            next_order
                        ))),
                    );
                    let next_interval = match am.interval_secs {
                        0..=59 => 60,
                        60..=299 => 300,
                        _ => 30,
                    };
                    push_btn(
                        mesh,
                        [content_x + 311.0, content_y, 130.0, 26.0],
                        &format!("Every: {}s", am.interval_secs),
                        [0.26, 0.38, 0.56, 0.95],
                        [0.34, 0.50, 0.72, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                            "automessage interval {}",
                            next_interval
                        ))),
                    );
                    push_btn(
                        mesh,
                        [content_x + 449.0, content_y, 120.0, 26.0],
                        "+ Add Message",
                        [0.18, 0.48, 0.34, 0.95],
                        [0.26, 0.64, 0.44, 1.0],
                        AdminUiClickAction::SubmitInput("am_add".to_string()),
                    );

                    let mut ry = content_y + 38.0;
                    for (idx, msg) in am.messages.iter().enumerate().take(10) {
                        let line = format!("{}. {}", idx + 1, msg);
                        Self::draw_colored_string(
                            mesh,
                            content_x,
                            ry + 4.0,
                            &line,
                            7.2,
                            10.5,
                            0.9,
                            [0.92, 0.95, 1.0, 1.0],
                        );
                        push_btn(
                            mesh,
                            [content_x + content_w - 134.0, ry, 56.0, 22.0],
                            "Edit",
                            [0.20, 0.42, 0.64, 0.95],
                            [0.28, 0.56, 0.82, 1.0],
                            AdminUiClickAction::SubmitInput(format!("am_edit:{}", idx + 1)),
                        );
                        push_btn(
                            mesh,
                            [content_x + content_w - 72.0, ry, 70.0, 22.0],
                            "Remove",
                            [0.52, 0.20, 0.22, 0.95],
                            [0.70, 0.28, 0.30, 1.0],
                            AdminUiClickAction::SendAction(AdminPanelAction::RunCommand(format!(
                                "automessage remove {}",
                                idx + 1
                            ))),
                        );
                        ry += 26.0;
                    }
                }
            }
            AdminTab::Server => {
                if let Some(ref stats) = snapshot.server_stats {
                    let info = format!(
                        "World: {}  |  Online: {}  |  TPS: {:.1}  |  Uptime: {}s",
                        stats.world_name, stats.online_count, stats.tps, stats.uptime_secs
                    );
                    Self::draw_string(mesh, content_x, content_y, &info, 8.2, 11.5, 1.0, [0.85, 0.92, 1.0, 1.0]);

                    let row1 = content_y + 28.0;
                    push_btn(
                        mesh,
                        [content_x, row1, 210.0, 28.0],
                        if stats.broadcast_achievements {
                            "Achievement Broadcasts: ON"
                        } else {
                            "Achievement Broadcasts: OFF"
                        },
                        if stats.broadcast_achievements {
                            [0.18, 0.50, 0.28, 0.95]
                        } else {
                            [0.45, 0.24, 0.24, 0.95]
                        },
                        [0.28, 0.64, 0.38, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::SetBroadcastAchievements(
                            !stats.broadcast_achievements,
                        )),
                    );
                    push_btn(
                        mesh,
                        [content_x + 220.0, row1, 130.0, 28.0],
                        "Broadcast /say",
                        [0.20, 0.42, 0.68, 0.95],
                        [0.28, 0.56, 0.86, 1.0],
                        AdminUiClickAction::SubmitInput("server_say".to_string()),
                    );
                    push_btn(
                        mesh,
                        [content_x + 360.0, row1, 110.0, 28.0],
                        "Set Spawn",
                        [0.26, 0.42, 0.56, 0.95],
                        [0.34, 0.56, 0.72, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand("setspawn".to_string())),
                    );

                    let row2 = row1 + 38.0;
                    push_btn(
                        mesh,
                        [content_x, row2, 140.0, 28.0],
                        "Save World",
                        [0.18, 0.48, 0.34, 0.95],
                        [0.26, 0.64, 0.44, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::SaveWorld),
                    );
                    push_btn(
                        mesh,
                        [content_x + 150.0, row2, 140.0, 28.0],
                        "Stop Server",
                        [0.62, 0.16, 0.18, 0.95],
                        [0.82, 0.24, 0.26, 1.0],
                        AdminUiClickAction::SendAction(AdminPanelAction::RunCommand("stop".to_string())),
                    );
                }
            }
            AdminTab::Permissions => {
                // Sub-view toggle: Groups vs Players
                push_btn(
                    mesh,
                    [content_x, content_y, 110.0, 24.0],
                    "Groups View",
                    if !perm_players_view { [0.24, 0.52, 0.82, 0.98] } else { [0.12, 0.16, 0.24, 0.92] },
                    [0.30, 0.60, 0.92, 1.0],
                    AdminUiClickAction::SetPermPlayersView(false),
                );
                push_btn(
                    mesh,
                    [content_x + 118.0, content_y, 110.0, 24.0],
                    "Players View",
                    if perm_players_view { [0.24, 0.52, 0.82, 0.98] } else { [0.12, 0.16, 0.24, 0.92] },
                    [0.30, 0.60, 0.92, 1.0],
                    AdminUiClickAction::SetPermPlayersView(true),
                );

                let sub_y = content_y + 32.0;
                let left_w = 190.0;
                let right_x = content_x + left_w + 14.0;
                let right_w = content_w - left_w - 14.0;

                if !perm_players_view {
                    // Groups View
                    push_btn(
                        mesh,
                        [content_x, sub_y, left_w, 22.0],
                        "+ Create Group (input)",
                        [0.18, 0.48, 0.34, 0.95],
                        [0.26, 0.64, 0.44, 1.0],
                        AdminUiClickAction::SubmitInput("perm_create_group".to_string()),
                    );
                    let mut gy = sub_y + 28.0;
                    for g in snapshot.permissions_groups.iter().take(10) {
                        let is_sel = selected_group.is_some_and(|s| s.eq_ignore_ascii_case(&g.name));
                        push_btn(
                            mesh,
                            [content_x, gy, left_w, 22.0],
                            &format!("{} (P:{})", g.name, g.priority),
                            if is_sel { [0.22, 0.48, 0.76, 0.96] } else { [0.11, 0.14, 0.20, 0.90] },
                            [0.26, 0.54, 0.84, 1.0],
                            AdminUiClickAction::SelectGroup(g.name.clone()),
                        );
                        gy += 26.0;
                    }

                    if let Some(grp) = snapshot
                        .permissions_groups
                        .iter()
                        .find(|g| selected_group.is_some_and(|s| s.eq_ignore_ascii_case(&g.name)))
                        .or_else(|| snapshot.permissions_groups.first())
                    {
                        // Group metadata + live color prefix preview
                        let preview = format!(
                            "Group: {} | Priority: {} | Preview: {}&rPlayer",
                            grp.name, grp.priority, grp.prefix
                        );
                        Self::draw_colored_string(
                            mesh,
                            right_x,
                            sub_y + 2.0,
                            &preview,
                            7.4,
                            10.5,
                            0.95,
                            [0.92, 0.96, 1.0, 1.0],
                        );

                        let meta_y = sub_y + 22.0;
                        push_btn(
                            mesh,
                            [right_x, meta_y, 110.0, 22.0],
                            "Set Prefix",
                            [0.22, 0.42, 0.64, 0.95],
                            [0.30, 0.56, 0.82, 1.0],
                            AdminUiClickAction::SubmitInput("perm_set_prefix".to_string()),
                        );
                        push_btn(
                            mesh,
                            [right_x + 116.0, meta_y, 68.0, 22.0],
                            "P +10",
                            [0.22, 0.46, 0.44, 0.95],
                            [0.30, 0.62, 0.58, 1.0],
                            AdminUiClickAction::SendAction(AdminPanelAction::UpdateGroupMeta {
                                name: grp.name.clone(),
                                prefix: grp.prefix.clone(),
                                priority: grp.priority + 10,
                                inherits: grp.inherits.clone(),
                            }),
                        );
                        push_btn(
                            mesh,
                            [right_x + 190.0, meta_y, 68.0, 22.0],
                            "P -10",
                            [0.44, 0.34, 0.22, 0.95],
                            [0.60, 0.46, 0.30, 1.0],
                            AdminUiClickAction::SendAction(AdminPanelAction::UpdateGroupMeta {
                                name: grp.name.clone(),
                                prefix: grp.prefix.clone(),
                                priority: grp.priority - 10,
                                inherits: grp.inherits.clone(),
                            }),
                        );
                        push_btn(
                            mesh,
                            [right_x + 264.0, meta_y, 108.0, 22.0],
                            "+ Custom Node",
                            [0.24, 0.48, 0.32, 0.95],
                            [0.32, 0.64, 0.42, 1.0],
                            AdminUiClickAction::SubmitInput("perm_group_custom_node".to_string()),
                        );
                        push_btn(
                            mesh,
                            [right_x + right_w - 84.0, meta_y, 84.0, 22.0],
                            "Delete Grp",
                            [0.56, 0.18, 0.20, 0.95],
                            [0.74, 0.26, 0.28, 1.0],
                            AdminUiClickAction::SendAction(AdminPanelAction::DeleteGroup {
                                name: grp.name.clone(),
                            }),
                        );

                        // Inheritance toggles for other groups
                        let inh_y = meta_y + 26.0;
                        Self::draw_string(
                            mesh,
                            right_x,
                            inh_y + 4.0,
                            "Inherits:",
                            7.0,
                            10.0,
                            0.9,
                            [0.75, 0.84, 0.96, 1.0],
                        );
                        let mut ix = right_x + 68.0;
                        for other_grp in &snapshot.permissions_groups {
                            if other_grp.name.eq_ignore_ascii_case(&grp.name) {
                                continue;
                            }
                            let is_inherited = grp
                                .inherits
                                .iter()
                                .any(|n| n.eq_ignore_ascii_case(&other_grp.name));
                            let pill_w = (other_grp.name.len() as f32 * 7.5 + 18.0).clamp(52.0, 110.0);
                            if ix + pill_w > right_x + right_w {
                                break;
                            }
                            let mut next_inherits = grp.inherits.clone();
                            if is_inherited {
                                next_inherits.retain(|n| !n.eq_ignore_ascii_case(&other_grp.name));
                            } else {
                                next_inherits.push(other_grp.name.clone());
                            }
                            push_btn(
                                mesh,
                                [ix, inh_y, pill_w, 19.0],
                                &format!("{}{}", if is_inherited { "✓ " } else { "+ " }, other_grp.name),
                                if is_inherited {
                                    [0.18, 0.50, 0.30, 0.95]
                                } else {
                                    [0.16, 0.20, 0.28, 0.92]
                                },
                                [0.28, 0.64, 0.42, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::UpdateGroupMeta {
                                    name: grp.name.clone(),
                                    prefix: grp.prefix.clone(),
                                    priority: grp.priority,
                                    inherits: next_inherits,
                                }),
                            );
                            ix += pill_w + 5.0;
                        }

                        // Scrollable known permission node catalog
                        let cat_y = inh_y + 26.0;
                        let page_size = 7usize;
                        let max_scroll = snapshot.known_permission_nodes.len().saturating_sub(page_size);
                        let start = node_scroll.min(max_scroll);

                        push_btn(
                            mesh,
                            [right_x + right_w - 118.0, cat_y - 2.0, 56.0, 18.0],
                            "Up",
                            [0.16, 0.22, 0.32, 0.95],
                            [0.24, 0.34, 0.48, 1.0],
                            AdminUiClickAction::ScrollNodes(-4),
                        );
                        push_btn(
                            mesh,
                            [right_x + right_w - 58.0, cat_y - 2.0, 56.0, 18.0],
                            "Down",
                            [0.16, 0.22, 0.32, 0.95],
                            [0.24, 0.34, 0.48, 1.0],
                            AdminUiClickAction::ScrollNodes(4),
                        );
                        Self::draw_string(
                            mesh,
                            right_x,
                            cat_y + 1.0,
                            &format!(
                                "Permission Nodes ({}-{} / {})",
                                start + 1,
                                (start + page_size).min(snapshot.known_permission_nodes.len()),
                                snapshot.known_permission_nodes.len()
                            ),
                            7.0,
                            10.0,
                            0.9,
                            [0.78, 0.86, 0.95, 1.0],
                        );

                        let mut ny = cat_y + 22.0;
                        for node in snapshot
                            .known_permission_nodes
                            .iter()
                            .skip(start)
                            .take(page_size)
                        {
                            let is_allow = grp.permissions.iter().any(|p| p.eq_ignore_ascii_case(node));
                            let is_deny = grp
                                .permissions
                                .iter()
                                .any(|p| p.eq_ignore_ascii_case(&format!("-{}", node)));
                            Self::draw_string(
                                mesh,
                                right_x,
                                ny + 4.0,
                                node,
                                6.8,
                                9.8,
                                0.9,
                                if is_allow {
                                    [0.55, 0.96, 0.65, 1.0]
                                } else if is_deny {
                                    [0.98, 0.55, 0.55, 1.0]
                                } else {
                                    [0.82, 0.86, 0.92, 0.90]
                                },
                            );
                            let bx = right_x + right_w - 172.0;
                            push_btn(
                                mesh,
                                [bx, ny, 52.0, 19.0],
                                "Allow",
                                if is_allow { [0.18, 0.58, 0.30, 0.98] } else { [0.13, 0.17, 0.22, 0.90] },
                                [0.26, 0.72, 0.40, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::SetGroupNode {
                                    group: grp.name.clone(),
                                    node: node.clone(),
                                    state: Some(true),
                                }),
                            );
                            push_btn(
                                mesh,
                                [bx + 56.0, ny, 56.0, 19.0],
                                "Neutral",
                                if !is_allow && !is_deny { [0.24, 0.34, 0.48, 0.95] } else { [0.13, 0.17, 0.22, 0.90] },
                                [0.32, 0.44, 0.60, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::SetGroupNode {
                                    group: grp.name.clone(),
                                    node: node.clone(),
                                    state: None,
                                }),
                            );
                            push_btn(
                                mesh,
                                [bx + 116.0, ny, 54.0, 19.0],
                                "Deny",
                                if is_deny { [0.62, 0.20, 0.22, 0.98] } else { [0.13, 0.17, 0.22, 0.90] },
                                [0.78, 0.28, 0.30, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::SetGroupNode {
                                    group: grp.name.clone(),
                                    node: node.clone(),
                                    state: Some(false),
                                }),
                            );
                            ny += 23.0;
                        }
                    }
                } else {
                    // Players View
                    let mut names: Vec<String> = snapshot
                        .permissions_players
                        .iter()
                        .map(|p| p.username.clone())
                        .collect();
                    for op in &snapshot.online_players {
                        if !names.iter().any(|n| n.eq_ignore_ascii_case(&op.username)) {
                            names.push(op.username.clone());
                        }
                    }
                    let mut py_row = sub_y;
                    for name in names.iter().take(10) {
                        let is_sel = selected_perm_player.is_some_and(|s| s.eq_ignore_ascii_case(name));
                        push_btn(
                            mesh,
                            [content_x, py_row, left_w, 22.0],
                            name,
                            if is_sel { [0.22, 0.48, 0.76, 0.96] } else { [0.11, 0.14, 0.20, 0.90] },
                            [0.26, 0.54, 0.84, 1.0],
                            AdminUiClickAction::SelectPermPlayer(name.clone()),
                        );
                        py_row += 26.0;
                    }

                    let target_user = selected_perm_player
                        .or_else(|| names.first().map(|s| s.as_str()))
                        .unwrap_or("");
                    if !target_user.is_empty() {
                        Self::draw_string(
                            mesh,
                            right_x,
                            sub_y + 2.0,
                            &format!("Player Groups for '{}':", target_user),
                            7.6,
                            11.0,
                            0.95,
                            [0.98, 0.88, 0.48, 1.0],
                        );
                        let p_rec = snapshot
                            .permissions_players
                            .iter()
                            .find(|p| p.username.eq_ignore_ascii_case(target_user));
                        let mut gx = right_x;
                        let mut gy = sub_y + 22.0;
                        for g in &snapshot.permissions_groups {
                            let assigned = p_rec
                                .map(|p| p.groups.iter().any(|pg| pg.eq_ignore_ascii_case(&g.name)))
                                .unwrap_or_else(|| g.name.eq_ignore_ascii_case("default"));
                            let gw = (Self::string_width(&g.name, 7.2, 0.9) + 28.0).max(74.0);
                            if gx + gw > right_x + right_w {
                                gx = right_x;
                                gy += 28.0;
                            }
                            push_btn(
                                mesh,
                                [gx, gy, gw, 22.0],
                                &format!("{} {}", if assigned { "[x]" } else { "[ ]" }, g.name),
                                if assigned { [0.18, 0.52, 0.28, 0.95] } else { [0.14, 0.18, 0.25, 0.92] },
                                [0.26, 0.66, 0.38, 1.0],
                                AdminUiClickAction::SendAction(AdminPanelAction::SetPlayerGroup {
                                    username: target_user.to_string(),
                                    group: g.name.clone(),
                                    assigned: !assigned,
                                }),
                            );
                            gx += gw + 6.0;
                        }

                        let node_y = gy + 38.0;
                        push_btn(
                            mesh,
                            [right_x, node_y, 124.0, 24.0],
                            "+ Allow Node",
                            [0.18, 0.50, 0.30, 0.95],
                            [0.26, 0.66, 0.40, 1.0],
                            AdminUiClickAction::SubmitInput("perm_player_allow".to_string()),
                        );
                        push_btn(
                            mesh,
                            [right_x + 132.0, node_y, 120.0, 24.0],
                            "- Deny Node",
                            [0.56, 0.20, 0.22, 0.95],
                            [0.74, 0.28, 0.30, 1.0],
                            AdminUiClickAction::SubmitInput("perm_player_deny".to_string()),
                        );
                        push_btn(
                            mesh,
                            [right_x + 260.0, node_y, 110.0, 24.0],
                            "Clear Node",
                            [0.26, 0.34, 0.48, 0.95],
                            [0.36, 0.46, 0.62, 1.0],
                            AdminUiClickAction::SubmitInput("perm_player_clear".to_string()),
                        );
                    }
                }
            }
        }

        hitboxes
    }

    /// Draws a compact proximity voice chat status badge and nearby speaker indicators.
    pub fn draw_voice_hud_badge(
        mesh: &mut UiMesh,
        screen_w: f32,
        voice_mode: u8,
        transmitting: bool,
        ptt_key_name: &str,
        nearby_speakers: &[String],
    ) {
        if voice_mode == 0 && nearby_speakers.is_empty() {
            return;
        }
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let badge = if voice_mode == 0 {
            "Voice: OFF".to_string()
        } else if transmitting {
            "MIC: LIVE".to_string()
        } else if voice_mode == 2 {
            "Voice: VAD".to_string()
        } else {
            format!("Voice: [{}]", ptt_key_name)
        };
        let bw = Self::string_width(&badge, 7.5, 1.0) + 14.0;
        let bx = screen_w - bw - 10.0;
        let mut by = 10.0;
        let bg = if transmitting {
            [0.12, 0.45, 0.22, 0.85]
        } else {
            [0.04, 0.05, 0.08, 0.65]
        };
        let fg = if transmitting {
            [0.45, 1.0, 0.55, 1.0]
        } else {
            [0.80, 0.88, 0.96, 0.92]
        };
        mesh.add_quad(bx, by, bw, 20.0, white_uv, bg);
        Self::draw_string(mesh, bx + 7.0, by + 4.5, &badge, 7.5, 11.0, 1.0, fg);
        by += 24.0;

        for speaker in nearby_speakers.iter().take(4) {
            let label = format!("* {}", speaker);
            let sw = Self::string_width(&label, 7.5, 1.0) + 12.0;
            let sx = screen_w - sw - 10.0;
            mesh.add_quad(sx, by, sw, 18.0, white_uv, [0.06, 0.14, 0.10, 0.72]);
            Self::draw_string(
                mesh,
                sx + 6.0,
                by + 3.5,
                &label,
                7.5,
                10.5,
                1.0,
                [0.55, 0.96, 0.65, 0.96],
            );
            by += 20.0;
        }
    }

    /// Renders the Stardew Valley-style fishing HUD overlay (cast status, bite alert, or minigame).
    pub fn build_fishing_hud(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        state: &FishingHudState,
    ) {
        Self::build_fishing_hud_with_input_mode(
            mesh,
            screen_w,
            screen_h,
            state,
            InputMode::KeyboardMouse,
            ControllerFamily::Xbox,
            GamepadButton::LeftTrigger,
        );
    }

    /// Renders the fishing HUD overlay with dynamic Keyboard/Mouse or Controller button glyphs.
    pub fn build_fishing_hud_with_input_mode(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        state: &FishingHudState,
        input_mode: InputMode,
        glyph_family: ControllerFamily,
        use_button: GamepadButton,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(241);

        match &state.phase {
            FishingHudPhase::Waiting => {
                if input_mode == InputMode::Controller {
                    let prefix = format!("Fishing ({}) - Waiting for bite... ", state.tackle_summary);
                    let suffix = " to reel";
                    let pw = Self::string_width(&prefix, 8.0, 1.0);
                    let sw = Self::string_width(suffix, 8.0, 1.0);
                    let tw = pw + 16.0 + sw + 22.0;
                    let bx = (screen_w - tw) * 0.5;
                    let by = screen_h * 0.72;
                    mesh.add_quad(bx, by, tw, 24.0, white_uv, [0.04, 0.08, 0.14, 0.72]);
                    Self::draw_string(
                        mesh,
                        bx + 9.0,
                        by + 6.0,
                        &prefix,
                        8.0,
                        11.5,
                        1.0,
                        [0.85, 0.94, 1.0, 0.95],
                    );
                    Self::draw_gamepad_glyph(
                        mesh,
                        bx + 9.0 + pw,
                        by + 4.0,
                        16.0,
                        glyph_family,
                        use_button,
                        [1.0; 4],
                    );
                    Self::draw_string(
                        mesh,
                        bx + 9.0 + pw + 18.0,
                        by + 6.0,
                        suffix,
                        8.0,
                        11.5,
                        1.0,
                        [0.85, 0.94, 1.0, 0.95],
                    );
                } else {
                    let msg = format!(
                        "Fishing ({}) - Waiting for bite... [Right-Click to reel]",
                        state.tackle_summary
                    );
                    let tw = Self::string_width(&msg, 8.0, 1.0) + 18.0;
                    let bx = (screen_w - tw) * 0.5;
                    let by = screen_h * 0.72;
                    mesh.add_quad(bx, by, tw, 24.0, white_uv, [0.04, 0.08, 0.14, 0.72]);
                    Self::draw_string(
                        mesh,
                        bx + 9.0,
                        by + 6.0,
                        &msg,
                        8.0,
                        11.5,
                        1.0,
                        [0.85, 0.94, 1.0, 0.95],
                    );
                }
            }
            FishingHudPhase::Biting => {
                if input_mode == InputMode::Controller {
                    let prefix = "! BITE !  ";
                    let suffix = " to Hook!";
                    let pw = Self::string_width(prefix, 12.0, 1.0);
                    let sw = Self::string_width(suffix, 12.0, 1.0);
                    let tw = pw + 20.0 + sw + 30.0;
                    let bx = (screen_w - tw) * 0.5;
                    let by = screen_h * 0.36;
                    mesh.add_quad(bx - 2.0, by - 2.0, tw + 4.0, 34.0, white_uv, [0.95, 0.72, 0.12, 0.95]);
                    mesh.add_quad(bx, by, tw, 30.0, white_uv, [0.14, 0.08, 0.02, 0.90]);
                    Self::draw_string(
                        mesh,
                        bx + 14.0,
                        by + 7.5,
                        prefix,
                        12.0,
                        15.5,
                        1.0,
                        [1.0, 0.92, 0.30, 1.0],
                    );
                    Self::draw_gamepad_glyph(
                        mesh,
                        bx + 14.0 + pw,
                        by + 5.0,
                        20.0,
                        glyph_family,
                        use_button,
                        [1.0; 4],
                    );
                    Self::draw_string(
                        mesh,
                        bx + 14.0 + pw + 22.0,
                        by + 7.5,
                        suffix,
                        12.0,
                        15.5,
                        1.0,
                        [1.0, 0.92, 0.30, 1.0],
                    );
                } else {
                    let msg = "! BITE !  Right-Click to Hook!";
                    let tw = Self::string_width(msg, 12.0, 1.0) + 28.0;
                    let bx = (screen_w - tw) * 0.5;
                    let by = screen_h * 0.36;
                    mesh.add_quad(bx - 2.0, by - 2.0, tw + 4.0, 34.0, white_uv, [0.95, 0.72, 0.12, 0.95]);
                    mesh.add_quad(bx, by, tw, 30.0, white_uv, [0.14, 0.08, 0.02, 0.90]);
                    Self::draw_string(
                        mesh,
                        bx + 14.0,
                        by + 7.5,
                        msg,
                        12.0,
                        15.5,
                        1.0,
                        [1.0, 0.92, 0.30, 1.0],
                    );
                }
            }
            FishingHudPhase::Minigame {
                bar_pos,
                bar_height,
                fish_pos,
                progress,
                fish_in_bar,
                fish_tile,
                fish_name,
                is_rare,
            } => {
                // Stardew-style vertical column on right-center of screen
                let track_h = (screen_h * 0.46).clamp(240.0, 360.0);
                let track_w = 34.0;
                let prog_w = 14.0;
                let panel_w = track_w + prog_w + 30.0;
                let panel_h = track_h + 62.0;
                let px = (screen_w * 0.5 + 150.0).min(screen_w - panel_w - 20.0);
                let py = (screen_h - panel_h) * 0.5;

                // Outer wooden frame
                mesh.add_quad(px, py, panel_w, panel_h, white_uv, [0.36, 0.22, 0.12, 0.95]);
                mesh.add_quad(
                    px + 3.0,
                    py + 3.0,
                    panel_w - 6.0,
                    panel_h - 6.0,
                    white_uv,
                    [0.14, 0.10, 0.08, 0.92],
                );

                // Water track background (0 = bottom, 1 = top)
                let tx = px + 11.0;
                let ty = py + 28.0;
                mesh.add_quad(tx, ty, track_w, track_h, white_uv, [0.06, 0.22, 0.42, 0.96]);
                mesh.add_quad(
                    tx + 2.0,
                    ty + 2.0,
                    track_w - 4.0,
                    track_h - 4.0,
                    white_uv,
                    [0.08, 0.32, 0.58, 0.96],
                );

                // Green catch bar (bar_pos is bottom edge in 0..=1)
                let bh_px = (*bar_height * track_h).max(20.0);
                let bar_bottom_y = ty + track_h - (*bar_pos * track_h);
                let bar_top_y = (bar_bottom_y - bh_px).max(ty);
                let bar_color = if *fish_in_bar {
                    [0.24, 0.95, 0.36, 0.92]
                } else {
                    [0.18, 0.72, 0.28, 0.78]
                };
                mesh.add_quad(
                    tx + 3.0,
                    bar_top_y,
                    track_w - 6.0,
                    (bar_bottom_y - bar_top_y).max(4.0),
                    white_uv,
                    bar_color,
                );
                // Bright inner highlight on catch bar
                mesh.add_quad(
                    tx + 5.0,
                    bar_top_y + 2.0,
                    track_w - 10.0,
                    ((bar_bottom_y - bar_top_y) - 4.0).max(2.0),
                    white_uv,
                    if *fish_in_bar {
                        [0.52, 1.0, 0.62, 0.65]
                    } else {
                        [0.35, 0.85, 0.45, 0.45]
                    },
                );

                // Fish sprite positioned at fish_pos (0 = bottom, 1 = top)
                let fish_size = 24.0;
                let fish_cy = ty + track_h - (*fish_pos * track_h);
                let fish_y = (fish_cy - fish_size * 0.5).clamp(ty + 2.0, ty + track_h - fish_size - 2.0);
                let fish_uv = TextureAtlas::get_uv_bounds(*fish_tile);
                mesh.add_quad(
                    tx + (track_w - fish_size) * 0.5,
                    fish_y,
                    fish_size,
                    fish_size,
                    fish_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                if *is_rare {
                    // Small golden crown/pip next to rare fish
                    mesh.add_quad(
                        tx + track_w - 8.0,
                        fish_y,
                        6.0,
                        6.0,
                        white_uv,
                        [1.0, 0.85, 0.15, 1.0],
                    );
                }

                // Vertical catch progress meter on right side of track
                let mx = tx + track_w + 8.0;
                mesh.add_quad(mx, ty, prog_w, track_h, white_uv, [0.06, 0.06, 0.08, 0.95]);
                let fill_h = (progress.clamp(0.0, 1.0) * (track_h - 4.0)).max(2.0);
                let p_color = if *progress > 0.65 {
                    [0.25, 0.94, 0.35, 1.0]
                } else if *progress > 0.30 {
                    [0.96, 0.82, 0.18, 1.0]
                } else {
                    [0.94, 0.25, 0.20, 1.0]
                };
                mesh.add_quad(
                    mx + 2.0,
                    ty + track_h - 2.0 - fill_h,
                    prog_w - 4.0,
                    fill_h,
                    white_uv,
                    p_color,
                );

                // Header & footer labels
                let title = if *is_rare {
                    format!("* {} *", fish_name)
                } else {
                    "REELING!".to_string()
                };
                let tw = Self::string_width(&title, 7.5, 1.0);
                Self::draw_string(
                    mesh,
                    px + (panel_w - tw) * 0.5,
                    py + 9.0,
                    &title,
                    7.5,
                    11.0,
                    1.0,
                    if *is_rare {
                        [1.0, 0.88, 0.25, 1.0]
                    } else {
                        [0.92, 0.96, 1.0, 1.0]
                    },
                );

                if input_mode == InputMode::Controller {
                    let hint = "Hold";
                    let hw = Self::string_width(hint, 6.5, 1.0);
                    let total_w = 14.0 + 4.0 + hw;
                    let sx = px + (panel_w - total_w) * 0.5;
                    let sy = py + panel_h - 26.0;
                    Self::draw_gamepad_glyph(
                        mesh,
                        sx,
                        sy - 2.0,
                        14.0,
                        glyph_family,
                        use_button,
                        [1.0; 4],
                    );
                    Self::draw_string(
                        mesh,
                        sx + 18.0,
                        sy + 1.0,
                        hint,
                        6.5,
                        9.5,
                        1.0,
                        [0.85, 0.92, 0.80, 0.95],
                    );
                } else {
                    let hint = "Hold CLICK/SPACE";
                    let hw = Self::string_width(hint, 6.0, 1.0);
                    Self::draw_string(
                        mesh,
                        px + (panel_w - hw) * 0.5,
                        py + panel_h - 24.0,
                        hint,
                        6.0,
                        9.0,
                        1.0,
                        [0.85, 0.92, 0.80, 0.95],
                    );
                }
            }
        }
    }

    fn draw_wrapped_string(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        max_w: f32,
        text: &str,
        char_w: f32,
        char_h: f32,
        spacing: f32,
        line_step: f32,
        color: [f32; 4],
    ) -> f32 {
        let mut cur_y = y;
        let mut line = String::new();
        for word in text.split_whitespace() {
            let cand = if line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", line, word)
            };
            if !line.is_empty() && Self::string_width(&cand, char_w, spacing) > max_w {
                Self::draw_string(mesh, x, cur_y, &line, char_w, char_h, spacing, color);
                cur_y += line_step;
                line = word.to_string();
            } else {
                line = cand;
            }
        }
        if !line.is_empty() {
            Self::draw_string(mesh, x, cur_y, &line, char_w, char_h, spacing, color);
            cur_y += line_step;
        }
        cur_y
    }

    /// Returns `(skill_row_rects, close_button_rect, panel_rect)` for the Skills modal screen.
    pub fn skills_screen_rects(
        screen_w: f32,
        screen_h: f32,
    ) -> ([[f32; 4]; blockgame_core::skills::SKILL_COUNT], [f32; 4], [f32; 4]) {
        let panel_w = (screen_w - 40.0).clamp(540.0, 840.0);
        let panel_h = (screen_h - 40.0).clamp(430.0, 540.0);
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        let list_x = px + 14.0;
        let list_w = 270.0_f32.min(panel_w * 0.36);
        let content_y = py + 52.0;
        let content_h = panel_h - 66.0;
        let gap = 6.0;
        let n = blockgame_core::skills::SKILL_COUNT;
        let row_h = ((content_h - gap * (n as f32 - 1.0)) / n as f32).max(36.0);

        let mut rows = [[0.0; 4]; blockgame_core::skills::SKILL_COUNT];
        for (i, rect) in rows.iter_mut().enumerate() {
            *rect = [list_x, content_y + i as f32 * (row_h + gap), list_w, row_h];
        }

        let close_rect = [px + panel_w - 92.0, py + 11.0, 78.0, 26.0];
        let panel_rect = [px, py, panel_w, panel_h];
        (rows, close_rect, panel_rect)
    }

    fn skill_accent_color(skill: blockgame_core::skills::SkillKind) -> [f32; 4] {
        use blockgame_core::skills::SkillKind;
        match skill {
            SkillKind::Combat => [0.92, 0.34, 0.34, 1.0],
            SkillKind::Woodcutting => [0.40, 0.80, 0.36, 1.0],
            SkillKind::Mining => [0.48, 0.72, 0.96, 1.0],
            SkillKind::Fishing => [0.28, 0.82, 0.92, 1.0],
            SkillKind::Digging => [0.85, 0.68, 0.38, 1.0],
            SkillKind::Farming => [0.68, 0.90, 0.32, 1.0],
            SkillKind::Agility => [0.94, 0.74, 0.25, 1.0],
        }
    }

    /// Builds the interactive MMO Skills & Milestones screen (`KeyK` default).
    pub fn build_skills_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        skills: &blockgame_core::skills::PlayerSkills,
        selected_idx: usize,
        cursor_pos: [f32; 2],
    ) {
        use blockgame_core::skills::{PlayerSkills, SkillKind, MAX_SKILL_LEVEL, SKILL_COUNT};
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Backdrop
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.04, 0.05, 0.08, 0.80],
        );

        let (row_rects, close_rect, panel) = Self::skills_screen_rects(screen_w, screen_h);
        let [px, py, pw, ph] = panel;
        let selected_idx = selected_idx.min(SKILL_COUNT - 1);

        // Main panel frame
        mesh.add_quad(px, py, pw, ph, white_uv, [0.42, 0.48, 0.58, 0.95]);
        mesh.add_quad(
            px + 2.0,
            py + 2.0,
            pw - 4.0,
            ph - 4.0,
            white_uv,
            [0.09, 0.11, 0.15, 0.97],
        );
        // Header strip
        mesh.add_quad(
            px + 2.0,
            py + 2.0,
            pw - 4.0,
            42.0,
            white_uv,
            [0.14, 0.17, 0.23, 0.98],
        );

        Self::draw_string(
            mesh,
            px + 16.0,
            py + 13.0,
            "MMO Skills",
            12.0,
            17.0,
            1.2,
            [1.0, 0.92, 0.45, 1.0],
        );

        let total_str = format!(
            "Total Lv: {} / {}",
            skills.total_level(),
            SKILL_COUNT as u32 * MAX_SKILL_LEVEL as u32
        );
        let tsw = Self::string_width(&total_str, 8.5, 1.0);
        Self::draw_string(
            mesh,
            close_rect[0] - tsw - 18.0,
            py + 17.0,
            &total_str,
            8.5,
            12.0,
            1.0,
            [0.82, 0.88, 0.95, 1.0],
        );

        Self::draw_menu_button(
            mesh,
            close_rect[0],
            close_rect[1],
            close_rect[2],
            close_rect[3],
            "Close",
            cursor_pos,
            true,
        );

        // Left skill list
        for (i, skill) in SkillKind::ALL.iter().enumerate() {
            let r = row_rects[i];
            let prog = skills.get(*skill);
            let is_selected = i == selected_idx;
            let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3]);
            let accent = Self::skill_accent_color(*skill);

            let border = if is_selected {
                accent
            } else if hovered {
                [0.72, 0.76, 0.84, 0.95]
            } else {
                [0.26, 0.30, 0.38, 0.9]
            };
            let bg = if is_selected {
                [0.17, 0.21, 0.28, 0.98]
            } else {
                [0.12, 0.14, 0.19, 0.92]
            };

            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
            mesh.add_quad(r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0, white_uv, bg);
            // Left accent pip
            mesh.add_quad(r[0] + 2.0, r[1] + 2.0, 4.0, r[3] - 4.0, white_uv, accent);

            Self::draw_string(
                mesh,
                r[0] + 12.0,
                r[1] + 7.0,
                skill.name(),
                8.5,
                12.0,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            );

            let lvl_str = format!("Lv {}", prog.level);
            let lsw = Self::string_width(&lvl_str, 8.5, 1.0);
            Self::draw_string(
                mesh,
                r[0] + r[2] - lsw - 10.0,
                r[1] + 7.0,
                &lvl_str,
                8.5,
                12.0,
                1.0,
                accent,
            );

            // XP progress bar
            let bar_x = r[0] + 12.0;
            let bar_w = r[2] - 22.0;
            let bar_y = r[1] + r[3] * 0.52;
            let bar_h = 6.0;
            mesh.add_quad(bar_x, bar_y, bar_w, bar_h, white_uv, [0.05, 0.06, 0.08, 0.95]);
            let needed = PlayerSkills::xp_for_next_level(prog.level);
            let frac = if prog.level >= MAX_SKILL_LEVEL || needed == 0 {
                1.0
            } else {
                (prog.xp as f32 / needed as f32).clamp(0.0, 1.0)
            };
            if frac > 0.0 {
                mesh.add_quad(bar_x, bar_y, (bar_w * frac).max(2.0), bar_h, white_uv, accent);
            }

            let xp_str = if prog.level >= MAX_SKILL_LEVEL {
                "MAX LEVEL (1000)".to_string()
            } else {
                format!("XP: {} / {}", prog.xp, needed)
            };
            Self::draw_string(
                mesh,
                bar_x,
                bar_y + 10.0,
                &xp_str,
                6.2,
                8.5,
                0.8,
                [0.72, 0.78, 0.86, 0.95],
            );
        }

        // Right detail pane
        let detail_x = row_rects[0][0] + row_rects[0][2] + 14.0;
        let detail_y = row_rects[0][1];
        let detail_w = (px + pw - 14.0) - detail_x;
        let detail_h = ph - 66.0;
        let selected_skill = SkillKind::from_index(selected_idx).unwrap_or(SkillKind::Combat);
        let prog = skills.get(selected_skill);
        let accent = Self::skill_accent_color(selected_skill);

        mesh.add_quad(
            detail_x,
            detail_y,
            detail_w,
            detail_h,
            white_uv,
            [0.24, 0.28, 0.36, 0.95],
        );
        mesh.add_quad(
            detail_x + 2.0,
            detail_y + 2.0,
            detail_w - 4.0,
            detail_h - 4.0,
            white_uv,
            [0.11, 0.13, 0.18, 0.96],
        );

        let pad_x = detail_x + 12.0;
        let inner_w = detail_w - 24.0;
        let mut cy = detail_y + 10.0;

        Self::draw_string(
            mesh,
            pad_x,
            cy,
            selected_skill.name(),
            12.0,
            16.0,
            1.2,
            accent,
        );
        let badge = format!("Level {} / {}", prog.level, MAX_SKILL_LEVEL);
        let bw = Self::string_width(&badge, 9.0, 1.0);
        Self::draw_string(
            mesh,
            detail_x + detail_w - bw - 12.0,
            cy + 2.0,
            &badge,
            9.0,
            12.5,
            1.0,
            [1.0, 1.0, 1.0, 1.0],
        );
        cy += 22.0;

        cy = Self::draw_wrapped_string(
            mesh,
            pad_x,
            cy,
            inner_w,
            selected_skill.description(),
            6.8,
            9.5,
            0.8,
            12.0,
            [0.78, 0.83, 0.90, 1.0],
        );
        cy += 2.0;

        let perk_line = format!("Active Perks: {}", skills.perks_summary(selected_skill));
        cy = Self::draw_wrapped_string(
            mesh,
            pad_x,
            cy,
            inner_w,
            &perk_line,
            7.0,
            10.0,
            0.8,
            13.0,
            [0.50, 0.96, 0.65, 1.0],
        );
        cy += 6.0;

        Self::draw_string(
            mesh,
            pad_x,
            cy,
            "Milestones:",
            8.0,
            11.0,
            1.0,
            [0.98, 0.86, 0.42, 1.0],
        );
        cy += 15.0;

        let milestones = selected_skill.milestones();
        let avail_h = (detail_y + detail_h - 10.0 - cy).max(140.0);
        let m_gap = 4.0;
        let m_h = ((avail_h - m_gap * (milestones.len() as f32 - 1.0)) / milestones.len() as f32)
            .max(24.0);

        for (idx, m) in milestones.iter().enumerate() {
            let unlocked = prog.level >= m.level;
            let my = cy + idx as f32 * (m_h + m_gap);
            let m_border = if unlocked {
                [0.88, 0.74, 0.28, 0.95]
            } else {
                [0.24, 0.27, 0.34, 0.85]
            };
            let m_bg = if unlocked {
                [0.16, 0.22, 0.16, 0.95]
            } else {
                [0.09, 0.10, 0.14, 0.92]
            };
            mesh.add_quad(pad_x, my, inner_w, m_h, white_uv, m_border);
            mesh.add_quad(
                pad_x + 1.5,
                my + 1.5,
                inner_w - 3.0,
                m_h - 3.0,
                white_uv,
                m_bg,
            );

            let title_text = format!("Lv {}: {}", m.level, m.name);
            let status_text = if unlocked { "UNLOCKED" } else { "LOCKED" };
            let st_w = Self::string_width(status_text, 6.5, 0.8);
            Self::draw_string(
                mesh,
                pad_x + 7.0,
                my + 4.0,
                &title_text,
                7.2,
                10.0,
                0.8,
                if unlocked {
                    [1.0, 0.92, 0.48, 1.0]
                } else {
                    [0.64, 0.68, 0.76, 1.0]
                },
            );
            Self::draw_string(
                mesh,
                pad_x + inner_w - st_w - 7.0,
                my + 4.0,
                status_text,
                6.5,
                9.0,
                0.8,
                if unlocked {
                    [0.36, 0.95, 0.48, 1.0]
                } else {
                    [0.50, 0.54, 0.62, 0.9]
                },
            );
            Self::draw_string(
                mesh,
                pad_x + 7.0,
                my + 17.0,
                m.description,
                6.4,
                8.8,
                0.7,
                if unlocked {
                    [0.86, 0.94, 0.86, 1.0]
                } else {
                    [0.54, 0.58, 0.66, 0.95]
                },
            );
        }
    }

    /// Draws an animated HUD toast banner when a skill levels up or unlocks a milestone.
    pub fn draw_skill_levelup_banner(
        mesh: &mut UiMesh,
        screen_w: f32,
        title: &str,
        subtitle: &str,
        alpha: f32,
    ) {
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 {
            return;
        }
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let tw = Self::string_width(title, 9.5, 1.0);
        let sw = Self::string_width(subtitle, 7.2, 0.8);
        let box_w = tw.max(sw).max(240.0) + 32.0;
        let box_h = if subtitle.is_empty() { 34.0 } else { 52.0 };
        let bx = (screen_w - box_w) * 0.5;
        let by = 42.0;

        mesh.add_quad(bx, by, box_w, box_h, white_uv, [0.95, 0.80, 0.25, 0.92 * a]);
        mesh.add_quad(
            bx + 2.0,
            by + 2.0,
            box_w - 4.0,
            box_h - 4.0,
            white_uv,
            [0.08, 0.12, 0.16, 0.90 * a],
        );
        Self::draw_string(
            mesh,
            bx + (box_w - tw) * 0.5,
            by + 9.0,
            title,
            9.5,
            13.0,
            1.0,
            [1.0, 0.92, 0.42, a],
        );
        if !subtitle.is_empty() {
            Self::draw_string(
                mesh,
                bx + (box_w - sw) * 0.5,
                by + 28.0,
                subtitle,
                7.2,
                10.0,
                0.8,
                [0.52, 0.96, 0.65, a],
            );
        }
    }

    /// Returns `(title, description)` for UI display, obscuring locked Hidden Achievements.
    pub fn achievement_display_text(
        def: &blockgame_core::achievement::AchievementDef,
        unlocked: bool,
    ) -> (&'static str, &'static str) {
        if def.hidden && !unlocked {
            ("???", "Hidden Achievement - keep exploring to unlock!")
        } else {
            (def.title, def.description)
        }
    }

    /// Draws a top-right HUD toast banner when an Achievement is unlocked.
    pub fn draw_achievement_toast(
        mesh: &mut UiMesh,
        screen_w: f32,
        title: &str,
        description: &str,
        icon_tile: u16,
        alpha: f32,
    ) {
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.01 {
            return;
        }
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let icon_uv = TextureAtlas::get_uv_bounds(icon_tile);
        let header = "ACHIEVEMENT UNLOCKED!";
        let hw = Self::string_width(header, 6.8, 0.8);
        let tw = Self::string_width(title, 9.2, 1.0);
        let dw = Self::string_width(description, 6.6, 0.7);
        let text_w = hw.max(tw).max(dw).min(300.0);
        let box_w = (text_w + 76.0).max(270.0).min(screen_w - 32.0);
        let box_h = 66.0;
        let bx = (screen_w - box_w - 18.0).max(16.0);
        let by = 18.0;

        // Gold border & dark slate plate
        mesh.add_quad(bx, by, box_w, box_h, white_uv, [0.95, 0.78, 0.25, 0.95 * a]);
        mesh.add_quad(
            bx + 2.0,
            by + 2.0,
            box_w - 4.0,
            box_h - 4.0,
            white_uv,
            [0.07, 0.10, 0.14, 0.93 * a],
        );

        // Icon frame + TextureAtlas item icon
        let ix = bx + 12.0;
        let iy = by + 13.0;
        mesh.add_quad(ix - 2.0, iy - 2.0, 40.0, 40.0, white_uv, [0.85, 0.68, 0.24, 0.80 * a]);
        mesh.add_quad(ix, iy, 36.0, 36.0, white_uv, [0.12, 0.15, 0.21, 0.95 * a]);
        mesh.add_quad(ix + 4.0, iy + 4.0, 28.0, 28.0, icon_uv, [1.0, 1.0, 1.0, a]);

        let tx = bx + 60.0;
        Self::draw_string(mesh, tx, by + 9.0, header, 6.8, 9.5, 0.8, [0.98, 0.82, 0.28, a]);
        Self::draw_string(mesh, tx, by + 24.0, title, 9.2, 13.0, 1.0, [0.98, 0.98, 0.95, a]);
        Self::draw_string(
            mesh,
            tx,
            by + 43.0,
            description,
            6.6,
            9.5,
            0.7,
            [0.72, 0.82, 0.90, a],
        );
    }

    /// Computes interactive rects for the categorized Achievements screen:
    /// `(category_tab_rects, close_button_rect, panel_rect)`.
    pub fn achievements_screen_rects(
        screen_w: f32,
        screen_h: f32,
    ) -> ([[f32; 4]; 9], [f32; 4], [f32; 4]) {
        let pw = 820.0_f32.min(screen_w - 48.0).max(460.0);
        let ph = 540.0_f32.min(screen_h - 48.0).max(340.0);
        let px = (screen_w - pw) * 0.5;
        let py = (screen_h - ph) * 0.5;
        let close_rect = [px + pw - 86.0, py + 9.0, 74.0, 26.0];

        let tab_gap = 4.0;
        let tab_area_w = pw - 28.0;
        let tab_w = (tab_area_w - tab_gap * 8.0) / 9.0;
        let tab_y = py + 52.0;
        let tab_h = 28.0;
        let mut tabs = [[0.0; 4]; 9];
        for (i, slot) in tabs.iter_mut().enumerate() {
            *slot = [px + 14.0 + (tab_w + tab_gap) * i as f32, tab_y, tab_w, tab_h];
        }
        (tabs, close_rect, [px, py, pw, ph])
    }

    /// Builds the categorized Achievements screen (accessible from Main Menu, Pause Menu, and `J` hotkey).
    pub fn build_achievements_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        engine: &blockgame_core::achievement::AchievementEngine,
        item_registry: &blockgame_core::item::ItemRegistry,
        selected_category: usize,
        scroll_offset: usize,
        in_game_eligibility: Option<bool>,
        cursor_pos: [f32; 2],
    ) {
        use blockgame_core::achievement::{
            achievements_in_category, AchievementCategory, AchievementRequirement,
        };
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Backdrop
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.04, 0.05, 0.08, 0.82]);

        let (tab_rects, close_rect, panel) = Self::achievements_screen_rects(screen_w, screen_h);
        let [px, py, pw, ph] = panel;
        let cat_idx = selected_category.min(AchievementCategory::ALL.len() - 1);
        let active_cat = AchievementCategory::ALL[cat_idx];

        // Main panel frame
        mesh.add_quad(px, py, pw, ph, white_uv, [0.46, 0.52, 0.62, 0.95]);
        mesh.add_quad(
            px + 2.0,
            py + 2.0,
            pw - 4.0,
            ph - 4.0,
            white_uv,
            [0.08, 0.10, 0.14, 0.97],
        );
        // Header bar
        mesh.add_quad(
            px + 2.0,
            py + 2.0,
            pw - 4.0,
            44.0,
            white_uv,
            [0.13, 0.16, 0.22, 0.98],
        );

        Self::draw_string(
            mesh,
            px + 16.0,
            py + 13.0,
            "ACHIEVEMENTS",
            12.0,
            17.0,
            1.2,
            [1.0, 0.88, 0.35, 1.0],
        );

        let (unlocked_total, total_count) = engine.completion_counts();
        let pct = if total_count > 0 {
            (unlocked_total * 100) / total_count
        } else {
            0
        };
        let summary = format!("{unlocked_total} / {total_count} ({pct}%)");
        let sw = Self::string_width(&summary, 8.5, 1.0);
        Self::draw_string(
            mesh,
            close_rect[0] - sw - 18.0,
            py + 17.0,
            &summary,
            8.5,
            12.0,
            1.0,
            [0.85, 0.92, 0.98, 1.0],
        );

        if let Some(false) = in_game_eligibility {
            let warn = "ELIGIBILITY DISABLED";
            let ww = Self::string_width(warn, 7.0, 0.8);
            Self::draw_string(
                mesh,
                close_rect[0] - sw - ww - 36.0,
                py + 18.0,
                warn,
                7.0,
                10.0,
                0.8,
                [0.98, 0.45, 0.42, 1.0],
            );
        }

        // Close button
        let close_hover = Self::point_in_rect(cursor_pos, close_rect);
        mesh.add_quad(
            close_rect[0],
            close_rect[1],
            close_rect[2],
            close_rect[3],
            white_uv,
            if close_hover {
                [0.70, 0.25, 0.25, 0.95]
            } else {
                [0.35, 0.20, 0.22, 0.90]
            },
        );
        let cw = Self::string_width("Close", 7.8, 0.8);
        Self::draw_string(
            mesh,
            close_rect[0] + (close_rect[2] - cw) * 0.5,
            close_rect[1] + 7.0,
            "Close",
            7.8,
            11.0,
            0.8,
            [0.98, 0.96, 0.96, 1.0],
        );

        // Category tabs
        for (i, cat) in AchievementCategory::ALL.iter().enumerate() {
            let r = tab_rects[i];
            let active = i == cat_idx;
            let hovered = Self::point_in_rect(cursor_pos, r);
            let bg = if active {
                [0.24, 0.34, 0.48, 0.98]
            } else if hovered {
                [0.18, 0.23, 0.31, 0.95]
            } else {
                [0.12, 0.15, 0.20, 0.92]
            };
            let border = if active {
                [0.95, 0.78, 0.28, 0.98]
            } else {
                [0.30, 0.35, 0.44, 0.85]
            };
            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
            mesh.add_quad(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, white_uv, bg);
            let lbl = cat.label();
            let lw = Self::string_width(lbl, 6.8, 0.6);
            Self::draw_string(
                mesh,
                r[0] + (r[2] - lw) * 0.5,
                r[1] + 9.0,
                lbl,
                6.8,
                10.0,
                0.6,
                if active {
                    [1.0, 0.94, 0.55, 1.0]
                } else {
                    [0.80, 0.86, 0.92, 0.95]
                },
            );
        }

        // Category title & overall progress bar
        let list_x = px + 16.0;
        let list_w = pw - 32.0;
        let sub_y = py + 88.0;
        Self::draw_string(
            mesh,
            list_x,
            sub_y,
            active_cat.full_label(),
            9.0,
            13.0,
            0.9,
            [0.90, 0.94, 0.98, 1.0],
        );

        let bar_w = 200.0;
        let bar_x = px + pw - 16.0 - bar_w;
        let bar_y = sub_y + 2.0;
        mesh.add_quad(bar_x, bar_y, bar_w, 10.0, white_uv, [0.16, 0.20, 0.26, 0.95]);
        let fill = if total_count > 0 {
            bar_w * (unlocked_total as f32 / total_count as f32)
        } else {
            0.0
        };
        if fill > 0.5 {
            mesh.add_quad(bar_x, bar_y, fill, 10.0, white_uv, [0.92, 0.76, 0.24, 0.95]);
        }

        // Achievement cards for selected category
        let items = achievements_in_category(active_cat);
        let cards_top = py + 110.0;
        let card_h = 64.0;
        let card_gap = 8.0;
        let avail_h = (py + ph - 26.0) - cards_top;
        let visible_rows = ((avail_h + card_gap) / (card_h + card_gap)).floor().max(1.0) as usize;
        let max_scroll = items.len().saturating_sub(visible_rows);
        let start = scroll_offset.min(max_scroll);

        for (row, def) in items.iter().skip(start).take(visible_rows).enumerate() {
            let cy = cards_top + row as f32 * (card_h + card_gap);
            let prog = engine.progress_for(def);
            let (title, desc) = Self::achievement_display_text(def, prog.unlocked);

            let border = if prog.unlocked {
                [0.92, 0.76, 0.26, 0.92]
            } else {
                [0.28, 0.33, 0.42, 0.85]
            };
            let bg = if prog.unlocked {
                [0.12, 0.16, 0.20, 0.96]
            } else {
                [0.09, 0.11, 0.15, 0.94]
            };
            mesh.add_quad(list_x, cy, list_w, card_h, white_uv, border);
            mesh.add_quad(list_x + 1.5, cy + 1.5, list_w - 3.0, card_h - 3.0, white_uv, bg);

            // Icon box
            let ix = list_x + 12.0;
            let iy = cy + 12.0;
            mesh.add_quad(ix, iy, 40.0, 40.0, white_uv, [0.15, 0.18, 0.24, 0.95]);
            if def.hidden && !prog.unlocked {
                let qw = Self::string_width("?", 14.0, 1.0);
                Self::draw_string(
                    mesh,
                    ix + (40.0 - qw) * 0.5,
                    iy + 10.0,
                    "?",
                    14.0,
                    20.0,
                    1.0,
                    [0.65, 0.70, 0.80, 0.95],
                );
            } else {
                let tile = item_registry
                    .get(def.icon)
                    .map_or(Self::WHITE_TILE, |d| d.icon_tile as u16);
                let icon_uv = TextureAtlas::get_uv_bounds(tile);
                let tint = if prog.unlocked {
                    [1.0, 1.0, 1.0, 1.0]
                } else {
                    [0.45, 0.48, 0.54, 0.85]
                };
                mesh.add_quad(ix + 6.0, iy + 6.0, 28.0, 28.0, icon_uv, tint);
            }

            // Title & Description
            let tx = list_x + 64.0;
            Self::draw_string(
                mesh,
                tx,
                cy + 10.0,
                title,
                9.2,
                13.0,
                0.9,
                if prog.unlocked {
                    [1.0, 0.92, 0.45, 1.0]
                } else {
                    [0.84, 0.88, 0.94, 0.95]
                },
            );
            Self::draw_string(
                mesh,
                tx,
                cy + 28.0,
                desc,
                7.0,
                10.0,
                0.7,
                [0.68, 0.75, 0.84, 0.95],
            );

            // Status badge / progress bar
            let status_x = list_x + list_w - 154.0;
            if prog.unlocked {
                let badge = "UNLOCKED";
                let bw = Self::string_width(badge, 7.5, 0.8);
                Self::draw_string(
                    mesh,
                    list_x + list_w - bw - 14.0,
                    cy + 24.0,
                    badge,
                    7.5,
                    11.0,
                    0.8,
                    [0.45, 0.94, 0.58, 1.0],
                );
            } else if !matches!(def.requirement, AchievementRequirement::OneShot)
                && !(def.hidden && !prog.unlocked)
            {
                let ptext = format!("{} / {}", prog.current, prog.target);
                let pw_t = Self::string_width(&ptext, 7.0, 0.7);
                Self::draw_string(
                    mesh,
                    list_x + list_w - pw_t - 14.0,
                    cy + 14.0,
                    &ptext,
                    7.0,
                    10.0,
                    0.7,
                    [0.78, 0.86, 0.94, 0.95],
                );
                let pbx = status_x;
                let pby = cy + 34.0;
                let pbw = 140.0;
                mesh.add_quad(pbx, pby, pbw, 9.0, white_uv, [0.16, 0.20, 0.26, 0.95]);
                let frac = if prog.target > 0 {
                    (prog.current as f32 / prog.target as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if frac > 0.0 {
                    mesh.add_quad(pbx, pby, pbw * frac, 9.0, white_uv, [0.38, 0.74, 0.96, 0.95]);
                }
            } else {
                let badge = "LOCKED";
                let bw = Self::string_width(badge, 7.2, 0.8);
                Self::draw_string(
                    mesh,
                    list_x + list_w - bw - 14.0,
                    cy + 24.0,
                    badge,
                    7.2,
                    10.5,
                    0.8,
                    [0.52, 0.58, 0.68, 0.90],
                );
            }
        }

        // Scroll hint footer
        if items.len() > visible_rows {
            let hint = format!(
                "Scroll to view more ({}-{} of {})",
                start + 1,
                (start + visible_rows).min(items.len()),
                items.len()
            );
            let hw = Self::string_width(&hint, 6.8, 0.7);
            Self::draw_string(
                mesh,
                px + (pw - hw) * 0.5,
                py + ph - 18.0,
                &hint,
                6.8,
                9.5,
                0.7,
                [0.64, 0.72, 0.82, 0.88],
            );
        }
    }

    /// Draws the top-of-screen Boss Health Bar when a boss is in combat and within 500 blocks (Issue #9).
    pub fn draw_boss_hud(
        mesh: &mut UiMesh,
        screen_w: f32,
        boss: &blockgame_core::temple::BossHudState,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let accent = if boss.invulnerable {
            [0.82, 0.94, 1.0, 1.0]
        } else {
            boss.kind.boss_theme_color()
        };
        let phase_roman = match boss.phase {
            3 => "PHASE III",
            2 => "PHASE II",
            _ => "PHASE I",
        };
        let header = if boss.invulnerable {
            format!("{}  [{} - SHIELDED! DODGE!]", boss.name.to_uppercase(), phase_roman)
        } else {
            format!("{}  [{}]", boss.name.to_uppercase(), phase_roman)
        };
        let hp_text = if boss.invulnerable {
            format!(
                "INVULNERABLE  ({} / {} HP)",
                boss.health.ceil() as i32,
                boss.max_health.ceil() as i32
            )
        } else {
            format!(
                "{} / {} HP",
                boss.health.ceil() as i32,
                boss.max_health.ceil() as i32
            )
        };

        let bar_w = (screen_w * 0.42).clamp(320.0, 540.0);
        let box_w = bar_w + 24.0;
        let box_h = 46.0;
        let bx = (screen_w - box_w) * 0.5;
        let by = 12.0;

        // Outer themed border + dark slate panel
        mesh.add_quad(bx, by, box_w, box_h, white_uv, [accent[0], accent[1], accent[2], 0.92]);
        mesh.add_quad(
            bx + 2.0,
            by + 2.0,
            box_w - 4.0,
            box_h - 4.0,
            white_uv,
            [0.06, 0.07, 0.10, 0.92],
        );

        // Centered Boss Name + Phase header
        let hw = Self::string_width(&header, 8.8, 0.95);
        Self::draw_string(
            mesh,
            bx + (box_w - hw) * 0.5,
            by + 6.0,
            &header,
            8.8,
            12.0,
            0.95,
            [1.0, 0.96, 0.86, 1.0],
        );

        // Health bar track + smooth fill + top gloss
        let bar_x = bx + 12.0;
        let bar_y = by + 23.0;
        let bar_h = 15.0;
        mesh.add_quad(bar_x, bar_y, bar_w, bar_h, white_uv, [0.14, 0.14, 0.18, 0.95]);
        let frac = (boss.health / boss.max_health.max(1.0)).clamp(0.0, 1.0);
        let fill_w = (bar_w - 2.0) * frac;
        if fill_w > 0.5 {
            mesh.add_quad(
                bar_x + 1.0,
                bar_y + 1.0,
                fill_w,
                bar_h - 2.0,
                white_uv,
                accent,
            );
            mesh.add_quad(
                bar_x + 1.0,
                bar_y + 1.0,
                fill_w,
                4.0,
                white_uv,
                [1.0, 1.0, 1.0, 0.25],
            );
        }

        // Centered HP numeric readout on the bar
        let hpw = Self::string_width(&hp_text, 7.0, 0.8);
        Self::draw_string(
            mesh,
            bar_x + (bar_w - hpw) * 0.5,
            bar_y + 3.0,
            &hp_text,
            7.0,
            10.0,
            0.8,
            [1.0, 1.0, 1.0, 0.96],
        );
    }
}

#[derive(Debug, Clone)]
pub enum FishingHudPhase {
    Waiting,
    Biting,
    Minigame {
        bar_pos: f32,
        bar_height: f32,
        fish_pos: f32,
        progress: f32,
        fish_in_bar: bool,
        fish_tile: u16,
        fish_name: String,
        is_rare: bool,
    },
}

#[derive(Debug, Clone)]
pub struct FishingHudState {
    pub phase: FishingHudPhase,
    pub tackle_summary: String,
}

/// Active tab in the Main Menu Character Creator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterCreatorTab {
    Presets = 0,
    Body = 1,
    FaceHair = 2,
    Clothing = 3,
}

impl CharacterCreatorTab {
    pub const ALL: [Self; 4] = [Self::Presets, Self::Body, Self::FaceHair, Self::Clothing];

    pub fn label(self) -> &'static str {
        match self {
            Self::Presets => "Presets",
            Self::Body => "Body",
            Self::FaceHair => "Face & Hair",
            Self::Clothing => "Clothing",
        }
    }
}

/// Pure view state passed to `UiBuilder::build_character_creator_screen`.
#[derive(Debug, Clone)]
pub struct CharacterCreatorUiState {
    pub active_tab: CharacterCreatorTab,
    pub appearance: blockgame_core::character::CharacterAppearance,
    pub presets: Vec<blockgame_core::character::CharacterPreset>,
    pub selected_preset_name: Option<String>,
    pub preset_name_input: String,
    pub preset_input_focused: bool,
    pub face_zoom: bool,
    pub walk_preview: bool,
    pub active_color_target: usize,
    pub status_message: Option<String>,
}

/// Deterministic hit-test and viewport layout for the Character Creator screen.
#[derive(Debug, Clone)]
pub struct CharacterCreatorLayout {
    /// `[x, y, w, h]` sub-viewport on the left side for live 3D turntable rendering.
    pub preview_viewport: [f32; 4],
    pub btn_zoom: [f32; 4],
    pub btn_walk: [f32; 4],
    pub btn_randomize: [f32; 4],
    pub tab_rects: [[f32; 4]; 4],
    pub btn_revert: [f32; 4],
    pub btn_done: [f32; 4],
    /// `(select_rect, optional_delete_rect)` for each preset row on the Presets tab.
    pub preset_rows: Vec<([f32; 4], Option<[f32; 4]>)>,
    pub preset_name_input: [f32; 4],
    pub btn_save_preset: [f32; 4],
    /// `(rect, group_id, variant_idx)` for style selector buttons in Body / FaceHair / Clothing tabs.
    /// Group IDs: 0=BodyFrame, 1=EyeStyle, 2=HairStyle, 3=FacialHair, 4=ShirtStyle, 5=PantsStyle, 6=ShoeStyle.
    pub option_buttons: Vec<([f32; 4], usize, usize)>,
    /// `(rect, color_target_idx)` for selecting which color slot the swatch grid & RGB sliders edit.
    pub color_target_buttons: Vec<([f32; 4], usize)>,
    /// `(rect, color)` for curated color swatches.
    pub swatch_rects: Vec<([f32; 4], blockgame_core::character::RgbColor)>,
    /// Horizontal track rects for R, G, B sliders (`0..=255`).
    pub rgb_slider_rects: [[f32; 4]; 3],
}

impl UiBuilder {
    #[inline]
    fn point_in_rect(p: [f32; 2], r: [f32; 4]) -> bool {
        Self::is_rect_hovered(p, r[0], r[1], r[2], r[3])
    }

    /// Procedurally draws a stylized 2D Character Portrait derived from `appearance`.
    pub fn draw_character_portrait(
        mesh: &mut UiMesh,
        x: f32,
        y: f32,
        size: f32,
        appearance: &blockgame_core::character::CharacterAppearance,
    ) {
        use blockgame_core::character::{EyeStyle, FacialHairStyle, HairStyle};
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let app = appearance.sanitized();

        // Dark framed portrait background
        mesh.add_quad(x, y, size, size, white_uv, [0.10, 0.13, 0.18, 0.96]);
        let pad = (size * 0.05).max(1.0);
        mesh.add_quad(
            x + pad,
            y + pad,
            size - pad * 2.0,
            size - pad * 2.0,
            white_uv,
            [0.16, 0.21, 0.29, 0.96],
        );

        let skin = app.skin_tone.to_rgba_f32(1.0);
        let skin_shade = app.skin_tone.scale(0.86).to_rgba_f32(1.0);
        let hair = app.hair_color.to_rgba_f32(1.0);
        let hair_dark = app.hair_color.scale(0.78).to_rgba_f32(1.0);
        let iris = app.eye_color.to_rgba_f32(1.0);
        let shirt = app.shirt_primary.to_rgba_f32(1.0);
        let shirt_a = app.shirt_accent.to_rgba_f32(1.0);

        // Shoulders / shirt collar at bottom
        mesh.add_quad(
            x + size * 0.18,
            y + size * 0.78,
            size * 0.64,
            size * 0.18,
            white_uv,
            shirt,
        );
        mesh.add_quad(
            x + size * 0.36,
            y + size * 0.78,
            size * 0.28,
            size * 0.08,
            white_uv,
            shirt_a,
        );

        // Back hair for long/bob/curly/twin buns
        match app.hair_style {
            HairStyle::LongFlowing => {
                mesh.add_quad(x + size * 0.17, y + size * 0.20, size * 0.66, size * 0.66, white_uv, hair_dark);
            }
            HairStyle::Bob => {
                mesh.add_quad(x + size * 0.18, y + size * 0.20, size * 0.64, size * 0.54, white_uv, hair_dark);
            }
            HairStyle::Curly => {
                mesh.add_quad(x + size * 0.14, y + size * 0.10, size * 0.72, size * 0.44, white_uv, hair);
            }
            HairStyle::TwinBuns => {
                mesh.add_quad(x + size * 0.12, y + size * 0.12, size * 0.18, size * 0.18, white_uv, hair);
                mesh.add_quad(x + size * 0.70, y + size * 0.12, size * 0.18, size * 0.18, white_uv, hair);
            }
            _ => {}
        }

        // Neck
        mesh.add_quad(
            x + size * 0.41,
            y + size * 0.70,
            size * 0.18,
            size * 0.12,
            white_uv,
            skin_shade,
        );

        // Ears
        mesh.add_quad(x + size * 0.20, y + size * 0.42, size * 0.08, size * 0.16, white_uv, skin_shade);
        mesh.add_quad(x + size * 0.72, y + size * 0.42, size * 0.08, size * 0.16, white_uv, skin_shade);

        // Head silhouette (slightly tapered)
        mesh.add_quad(x + size * 0.25, y + size * 0.22, size * 0.50, size * 0.50, white_uv, skin);
        mesh.add_quad(x + size * 0.30, y + size * 0.68, size * 0.40, size * 0.06, white_uv, skin_shade);

        // Eyes
        let eye_h = match app.eye_style {
            EyeStyle::Wide => size * 0.11,
            EyeStyle::Almond | EyeStyle::Sharp => size * 0.075,
            EyeStyle::Standard => size * 0.09,
        };
        let eye_y = y + size * 0.44;
        for ex in [x + size * 0.32, x + size * 0.54] {
            mesh.add_quad(ex, eye_y, size * 0.13, eye_h, white_uv, [0.95, 0.95, 0.97, 1.0]);
            mesh.add_quad(ex + size * 0.03, eye_y + eye_h * 0.1, size * 0.075, eye_h * 0.85, white_uv, iris);
            mesh.add_quad(ex + size * 0.05, eye_y + eye_h * 0.25, size * 0.035, eye_h * 0.5, white_uv, [0.08, 0.08, 0.10, 1.0]);
        }

        // Facial hair
        match app.facial_hair {
            FacialHairStyle::None => {}
            FacialHairStyle::Stubble => {
                let st = app.skin_tone.lerp(app.hair_color, 0.35).to_rgba_f32(1.0);
                mesh.add_quad(x + size * 0.30, y + size * 0.60, size * 0.40, size * 0.13, white_uv, st);
            }
            FacialHairStyle::Mustache => {
                mesh.add_quad(x + size * 0.37, y + size * 0.58, size * 0.26, size * 0.06, white_uv, hair);
            }
            FacialHairStyle::ShortBeard => {
                mesh.add_quad(x + size * 0.29, y + size * 0.59, size * 0.42, size * 0.15, white_uv, hair);
            }
            FacialHairStyle::FullBeard => {
                mesh.add_quad(x + size * 0.27, y + size * 0.58, size * 0.46, size * 0.23, white_uv, hair);
            }
        }

        // Top hair / bangs
        if app.hair_style != HairStyle::Bald {
            mesh.add_quad(x + size * 0.23, y + size * 0.16, size * 0.54, size * 0.15, white_uv, hair);
            if matches!(app.hair_style, HairStyle::Spiky) {
                mesh.add_quad(x + size * 0.32, y + size * 0.09, size * 0.12, size * 0.10, white_uv, hair);
                mesh.add_quad(x + size * 0.52, y + size * 0.10, size * 0.12, size * 0.09, white_uv, hair);
            } else if matches!(app.hair_style, HairStyle::SweptBangs | HairStyle::SidePart) {
                mesh.add_quad(x + size * 0.26, y + size * 0.27, size * 0.36, size * 0.08, white_uv, hair);
            }
        }
    }

    /// Computes the deterministic layout of the Main Menu Character Creator screen.
    pub fn character_creator_layout(
        screen_w: f32,
        screen_h: f32,
        state: &CharacterCreatorUiState,
    ) -> CharacterCreatorLayout {
        use blockgame_core::character::{
            BodyFrame, EyeStyle, FacialHairStyle, HairStyle, PantsStyle, ShirtStyle, ShoeStyle,
            CLOTHING_COLOR_SWATCHES, EYE_COLOR_SWATCHES, HAIR_COLOR_SWATCHES, SKIN_TONE_SWATCHES,
        };

        let margin = 20.0;
        let top_bar_h = 56.0;
        let right_w = (screen_w * 0.50).clamp(430.0, 620.0).min(screen_w - 280.0);
        let right_x = screen_w - right_w - margin;
        let left_w = (right_x - margin * 2.0).max(200.0);
        let preview_h = (screen_h - top_bar_h - 90.0).max(200.0);
        let preview_viewport = [margin, top_bar_h + 6.0, left_w, preview_h];

        // Bottom controls under 3D preview
        let ctrl_y = preview_viewport[1] + preview_viewport[3] + 12.0;
        let ctrl_gap = 8.0;
        let ctrl_w = (left_w - ctrl_gap * 2.0) / 3.0;
        let btn_zoom = [margin, ctrl_y, ctrl_w, 34.0];
        let btn_walk = [margin + ctrl_w + ctrl_gap, ctrl_y, ctrl_w, 34.0];
        let btn_randomize = [margin + (ctrl_w + ctrl_gap) * 2.0, ctrl_y, ctrl_w, 34.0];

        // Right panel tabs
        let panel_y = top_bar_h + 6.0;
        let panel_h = (screen_h - panel_y - margin).max(300.0);
        let tab_gap = 6.0;
        let inner_x = right_x + 14.0;
        let inner_w = right_w - 28.0;
        let tab_w = (inner_w - tab_gap * 3.0) / 4.0;
        let tab_y = panel_y + 12.0;
        let tab_rects = [
            [inner_x, tab_y, tab_w, 30.0],
            [inner_x + (tab_w + tab_gap), tab_y, tab_w, 30.0],
            [inner_x + (tab_w + tab_gap) * 2.0, tab_y, tab_w, 30.0],
            [inner_x + (tab_w + tab_gap) * 3.0, tab_y, tab_w, 30.0],
        ];

        // Bottom Revert / Done buttons on right panel
        let footer_y = panel_y + panel_h - 46.0;
        let footer_w = (inner_w - 12.0) * 0.5;
        let btn_revert = [inner_x, footer_y, footer_w, 34.0];
        let btn_done = [inner_x + footer_w + 12.0, footer_y, footer_w, 34.0];

        let mut preset_rows = Vec::new();
        let mut preset_name_input = [0.0; 4];
        let mut btn_save_preset = [0.0; 4];
        let mut option_buttons = Vec::new();
        let mut color_target_buttons = Vec::new();
        let mut swatch_rects = Vec::new();
        let mut rgb_slider_rects = [[0.0; 4]; 3];

        let content_y = tab_y + 42.0;

        match state.active_tab {
            CharacterCreatorTab::Presets => {
                // Save Custom Preset bar at top of Presets tab
                let save_w = 132.0;
                preset_name_input = [inner_x, content_y + 16.0, inner_w - save_w - 8.0, 32.0];
                btn_save_preset = [inner_x + inner_w - save_w, content_y + 16.0, save_w, 32.0];

                let list_y = content_y + 62.0;
                let row_h = 34.0;
                let max_rows = (((footer_y - 12.0) - list_y) / (row_h + 4.0)).floor().max(1.0) as usize;
                for (i, preset) in state.presets.iter().take(max_rows).enumerate() {
                    let ry = list_y + (i as f32) * (row_h + 4.0);
                    if preset.is_starter {
                        preset_rows.push(([inner_x, ry, inner_w, row_h], None));
                    } else {
                        let del_w = 34.0;
                        preset_rows.push((
                            [inner_x, ry, inner_w - del_w - 6.0, row_h],
                            Some([inner_x + inner_w - del_w, ry, del_w, row_h]),
                        ));
                    }
                }
            }
            CharacterCreatorTab::Body => {
                // Body Frame buttons (group 0)
                let bw = (inner_w - 12.0) / 3.0;
                let by = content_y + 18.0;
                for i in 0..BodyFrame::ALL.len() {
                    option_buttons.push(([inner_x + (bw + 6.0) * (i as f32), by, bw, 34.0], 0, i));
                }

                // Skin Tone swatches + RGB sliders
                let sw_y = by + 64.0;
                Self::layout_swatches_and_rgb(
                    inner_x,
                    sw_y,
                    inner_w,
                    &SKIN_TONE_SWATCHES,
                    &mut swatch_rects,
                    &mut rgb_slider_rects,
                );
            }
            CharacterCreatorTab::FaceHair => {
                // Eye Style (group 1, 4 buttons)
                let ew = (inner_w - 18.0) / 4.0;
                let ey = content_y + 16.0;
                for i in 0..EyeStyle::ALL.len() {
                    option_buttons.push(([inner_x + (ew + 6.0) * (i as f32), ey, ew, 26.0], 1, i));
                }

                // Hair Style (group 2, 10 buttons in 2 rows of 5)
                let hw = (inner_w - 24.0) / 5.0;
                let hy = ey + 44.0;
                for i in 0..HairStyle::ALL.len() {
                    let row = i / 5;
                    let col = i % 5;
                    option_buttons.push((
                        [
                            inner_x + (hw + 6.0) * (col as f32),
                            hy + (row as f32) * 30.0,
                            hw,
                            26.0,
                        ],
                        2,
                        i,
                    ));
                }

                // Facial Hair (group 3, 5 buttons in 1 row)
                let fw = (inner_w - 24.0) / 5.0;
                let fy = hy + 78.0;
                for i in 0..FacialHairStyle::ALL.len() {
                    option_buttons.push(([inner_x + (fw + 6.0) * (i as f32), fy, fw, 26.0], 3, i));
                }

                // Color target selector: 0 = Eye Color, 1 = Hair Color
                let cty = fy + 42.0;
                let ctw = (inner_w - 8.0) * 0.5;
                color_target_buttons.push(([inner_x, cty, ctw, 26.0], 0));
                color_target_buttons.push(([inner_x + ctw + 8.0, cty, ctw, 26.0], 1));

                let swatches: &[blockgame_core::character::RgbColor] = if state.active_color_target == 0 {
                    &EYE_COLOR_SWATCHES
                } else {
                    &HAIR_COLOR_SWATCHES
                };
                Self::layout_swatches_and_rgb(
                    inner_x,
                    cty + 34.0,
                    inner_w,
                    swatches,
                    &mut swatch_rects,
                    &mut rgb_slider_rects,
                );
            }
            CharacterCreatorTab::Clothing => {
                // Shirt Style (group 4, 6 buttons in 2 rows of 3)
                let sw = (inner_w - 12.0) / 3.0;
                let sy = content_y + 16.0;
                for i in 0..ShirtStyle::ALL.len() {
                    let row = i / 3;
                    let col = i % 3;
                    option_buttons.push((
                        [
                            inner_x + (sw + 6.0) * (col as f32),
                            sy + (row as f32) * 28.0,
                            sw,
                            24.0,
                        ],
                        4,
                        i,
                    ));
                }

                // Pants Style (group 5, 5 buttons)
                let pw = (inner_w - 24.0) / 5.0;
                let py = sy + 72.0;
                for i in 0..PantsStyle::ALL.len() {
                    option_buttons.push(([inner_x + (pw + 6.0) * (i as f32), py, pw, 24.0], 5, i));
                }

                // Shoe Style (group 6, 4 buttons)
                let shw = (inner_w - 18.0) / 4.0;
                let shy = py + 42.0;
                for i in 0..ShoeStyle::ALL.len() {
                    option_buttons.push(([inner_x + (shw + 6.0) * (i as f32), shy, shw, 24.0], 6, i));
                }

                // Color target selector: 0 = Shirt Primary, 1 = Shirt Accent, 2 = Pants, 3 = Shoes
                let cty = shy + 38.0;
                let ctw = (inner_w - 18.0) / 4.0;
                for i in 0..4 {
                    color_target_buttons.push(([inner_x + (ctw + 6.0) * (i as f32), cty, ctw, 24.0], i));
                }

                Self::layout_swatches_and_rgb(
                    inner_x,
                    cty + 32.0,
                    inner_w,
                    &CLOTHING_COLOR_SWATCHES,
                    &mut swatch_rects,
                    &mut rgb_slider_rects,
                );
            }
        }

        CharacterCreatorLayout {
            preview_viewport,
            btn_zoom,
            btn_walk,
            btn_randomize,
            tab_rects,
            btn_revert,
            btn_done,
            preset_rows,
            preset_name_input,
            btn_save_preset,
            option_buttons,
            color_target_buttons,
            swatch_rects,
            rgb_slider_rects,
        }
    }

    fn layout_swatches_and_rgb(
        x: f32,
        y: f32,
        w: f32,
        swatches: &[blockgame_core::character::RgbColor],
        out_swatches: &mut Vec<([f32; 4], blockgame_core::character::RgbColor)>,
        out_sliders: &mut [[f32; 4]; 3],
    ) {
        let cols = 7usize;
        let gap = 5.0;
        let sw_size = ((w - gap * (cols as f32 - 1.0)) / (cols as f32)).min(32.0);
        for (i, &c) in swatches.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;
            out_swatches.push((
                [
                    x + (col as f32) * (sw_size + gap),
                    y + (row as f32) * (sw_size + gap),
                    sw_size,
                    sw_size,
                ],
                c,
            ));
        }
        let rows = (swatches.len() + cols - 1) / cols;
        let slider_y = y + (rows as f32) * (sw_size + gap) + 12.0;
        let label_w = 34.0;
        let track_w = (w - label_w - 42.0).max(80.0);
        for ch in 0..3 {
            out_sliders[ch] = [x + label_w, slider_y + (ch as f32) * 24.0, track_w, 16.0];
        }
    }

    /// Builds the Main Menu split-screen Character Creator UI overlay (left side stays transparent for the 3D turntable).
    pub fn build_character_creator_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        state: &CharacterCreatorUiState,
        cursor_pos: [f32; 2],
    ) {
        use blockgame_core::character::{
            BodyFrame, EyeStyle, FacialHairStyle, HairStyle, PantsStyle, ShirtStyle, ShoeStyle,
        };

        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let layout = Self::character_creator_layout(screen_w, screen_h, state);

        // Top header bar
        mesh.add_quad(0.0, 0.0, screen_w, 56.0, white_uv, [0.05, 0.07, 0.11, 0.94]);
        mesh.add_quad(0.0, 54.0, screen_w, 2.0, white_uv, [0.85, 0.68, 0.28, 0.90]);
        Self::draw_string(
            mesh,
            24.0,
            16.0,
            "CHARACTER CREATOR",
            14.0,
            20.0,
            1.6,
            [0.98, 0.97, 0.94, 1.0],
        );
        let hint = "Drag character to rotate  |  Customize freely";
        Self::draw_string(
            mesh,
            24.0,
            39.0,
            hint,
            7.5,
            10.0,
            0.8,
            [0.75, 0.80, 0.88, 0.85],
        );

        // Left preview frame border (interior stays clear so the 3D turntable shows through!)
        let pv = layout.preview_viewport;
        let border = 2.0;
        let frame_col = [0.35, 0.42, 0.55, 0.85];
        mesh.add_quad(pv[0] - border, pv[1] - border, pv[2] + border * 2.0, border, white_uv, frame_col);
        mesh.add_quad(pv[0] - border, pv[1] + pv[3], pv[2] + border * 2.0, border, white_uv, frame_col);
        mesh.add_quad(pv[0] - border, pv[1], border, pv[3], white_uv, frame_col);
        mesh.add_quad(pv[0] + pv[2], pv[1], border, pv[3], white_uv, frame_col);

        // Preview control buttons
        let zoom_label = if state.face_zoom { "View: Full Body" } else { "View: Face Zoom" };
        let walk_label = if state.walk_preview { "Pose: Walking" } else { "Pose: Idle" };
        Self::draw_menu_button(mesh, layout.btn_zoom[0], layout.btn_zoom[1], layout.btn_zoom[2], layout.btn_zoom[3], zoom_label, cursor_pos, true);
        Self::draw_menu_button(mesh, layout.btn_walk[0], layout.btn_walk[1], layout.btn_walk[2], layout.btn_walk[3], walk_label, cursor_pos, true);
        Self::draw_menu_button(mesh, layout.btn_randomize[0], layout.btn_randomize[1], layout.btn_randomize[2], layout.btn_randomize[3], "Randomize", cursor_pos, true);

        // Right customization panel
        let right_x = layout.tab_rects[0][0] - 14.0;
        let right_w = screen_w - right_x - 20.0;
        let panel_y = 62.0;
        let panel_h = (screen_h - panel_y - 20.0).max(300.0);
        mesh.add_quad(right_x, panel_y, right_w, panel_h, white_uv, [0.36, 0.41, 0.52, 0.85]);
        mesh.add_quad(right_x + 1.5, panel_y + 1.5, right_w - 3.0, panel_h - 3.0, white_uv, [0.07, 0.09, 0.13, 0.95]);

        // Tabs
        for (i, tab) in CharacterCreatorTab::ALL.iter().enumerate() {
            let r = layout.tab_rects[i];
            let active = *tab == state.active_tab;
            let hovered = Self::point_in_rect(cursor_pos, r);
            let bg = if active {
                [0.24, 0.34, 0.48, 0.98]
            } else if hovered {
                [0.16, 0.21, 0.30, 0.95]
            } else {
                [0.11, 0.14, 0.20, 0.95]
            };
            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, bg);
            if active {
                mesh.add_quad(r[0], r[1] + r[3] - 2.5, r[2], 2.5, white_uv, [0.95, 0.78, 0.30, 1.0]);
            }
            let lbl = tab.label();
            let lw = Self::string_width(lbl, 8.0, 1.0);
            Self::draw_string(
                mesh,
                r[0] + (r[2] - lw) * 0.5,
                r[1] + 9.0,
                lbl,
                8.0,
                11.0,
                1.0,
                if active { [1.0, 0.98, 0.92, 1.0] } else { [0.78, 0.82, 0.90, 1.0] },
            );
        }

        let inner_x = layout.tab_rects[0][0];
        let content_y = layout.tab_rects[0][1] + 42.0;

        // Draw active tab contents
        match state.active_tab {
            CharacterCreatorTab::Presets => {
                Self::draw_text_input_box(
                    mesh,
                    layout.preset_name_input[0],
                    layout.preset_name_input[1],
                    layout.preset_name_input[2],
                    layout.preset_name_input[3],
                    "Save Active Character as Custom Preset:",
                    &state.preset_name_input,
                    "Enter preset name...",
                    state.preset_input_focused,
                    cursor_pos,
                );
                Self::draw_menu_button(
                    mesh,
                    layout.btn_save_preset[0],
                    layout.btn_save_preset[1],
                    layout.btn_save_preset[2],
                    layout.btn_save_preset[3],
                    "Save Preset",
                    cursor_pos,
                    true,
                );

                for (i, (row_r, del_r)) in layout.preset_rows.iter().enumerate() {
                    if let Some(preset) = state.presets.get(i) {
                        let selected = state
                            .selected_preset_name
                            .as_deref()
                            .map(|n| n.eq_ignore_ascii_case(&preset.name))
                            .unwrap_or(false);
                        let hovered = Self::point_in_rect(cursor_pos, *row_r);
                        let bg = if selected {
                            [0.20, 0.32, 0.44, 0.96]
                        } else if hovered {
                            [0.15, 0.20, 0.28, 0.94]
                        } else {
                            [0.10, 0.13, 0.19, 0.92]
                        };
                        mesh.add_quad(row_r[0], row_r[1], row_r[2], row_r[3], white_uv, bg);
                        if selected {
                            mesh.add_quad(row_r[0], row_r[1], 3.0, row_r[3], white_uv, [0.95, 0.78, 0.30, 1.0]);
                        }
                        Self::draw_character_portrait(
                            mesh,
                            row_r[0] + 6.0,
                            row_r[1] + 4.0,
                            26.0,
                            &preset.appearance,
                        );
                        Self::draw_string(
                            mesh,
                            row_r[0] + 40.0,
                            row_r[1] + 11.0,
                            &preset.name,
                            8.5,
                            12.0,
                            1.0,
                            [0.96, 0.97, 1.0, 1.0],
                        );
                        let badge = if preset.is_starter { "STARTER" } else { "CUSTOM" };
                        let bw = Self::string_width(badge, 7.0, 0.8);
                        Self::draw_string(
                            mesh,
                            row_r[0] + row_r[2] - bw - 10.0,
                            row_r[1] + 12.0,
                            badge,
                            7.0,
                            10.0,
                            0.8,
                            if preset.is_starter {
                                [0.92, 0.78, 0.35, 0.90]
                            } else {
                                [0.55, 0.84, 0.96, 0.90]
                            },
                        );

                        if let Some(dr) = del_r {
                            let d_hov = Self::point_in_rect(cursor_pos, *dr);
                            mesh.add_quad(
                                dr[0],
                                dr[1],
                                dr[2],
                                dr[3],
                                white_uv,
                                if d_hov { [0.68, 0.22, 0.22, 0.96] } else { [0.36, 0.15, 0.17, 0.92] },
                            );
                            Self::draw_string(mesh, dr[0] + 12.0, dr[1] + 10.0, "X", 9.0, 13.0, 1.0, [1.0, 0.95, 0.95, 1.0]);
                        }
                    }
                }
            }
            CharacterCreatorTab::Body => {
                Self::draw_string(mesh, inner_x, content_y, "BODY FRAME", 8.0, 11.0, 1.0, [0.95, 0.78, 0.30, 0.95]);
                for &(r, group, idx) in &layout.option_buttons {
                    if group == 0 {
                        let active = state.appearance.body_frame as usize == idx;
                        Self::draw_creator_chip(mesh, r, BodyFrame::ALL[idx].display_name(), active, cursor_pos);
                    }
                }
                Self::draw_string(mesh, inner_x, content_y + 64.0, "SKIN TONE", 8.0, 11.0, 1.0, [0.95, 0.78, 0.30, 0.95]);
                Self::draw_swatches_and_rgb(
                    mesh,
                    &layout.swatch_rects,
                    &layout.rgb_slider_rects,
                    state.appearance.skin_tone,
                    cursor_pos,
                );
            }
            CharacterCreatorTab::FaceHair => {
                Self::draw_string(mesh, inner_x, content_y, "EYE STYLE", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);
                Self::draw_string(mesh, inner_x, content_y + 44.0, "HAIRSTYLE", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);
                Self::draw_string(mesh, inner_x, content_y + 122.0, "FACIAL HAIR (INHERITS HAIR COLOR)", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);

                for &(r, group, idx) in &layout.option_buttons {
                    let (label, active) = match group {
                        1 => (EyeStyle::ALL[idx].display_name(), state.appearance.eye_style as usize == idx),
                        2 => (HairStyle::ALL[idx].display_name(), state.appearance.hair_style as usize == idx),
                        3 => (FacialHairStyle::ALL[idx].display_name(), state.appearance.facial_hair as usize == idx),
                        _ => ("", false),
                    };
                    Self::draw_creator_chip(mesh, r, label, active, cursor_pos);
                }

                for &(r, target) in &layout.color_target_buttons {
                    let label = if target == 0 { "Eye Color" } else { "Hair Color" };
                    Self::draw_creator_chip(mesh, r, label, state.active_color_target == target, cursor_pos);
                }
                let active_color = if state.active_color_target == 0 {
                    state.appearance.eye_color
                } else {
                    state.appearance.hair_color
                };
                Self::draw_swatches_and_rgb(
                    mesh,
                    &layout.swatch_rects,
                    &layout.rgb_slider_rects,
                    active_color,
                    cursor_pos,
                );
            }
            CharacterCreatorTab::Clothing => {
                Self::draw_string(mesh, inner_x, content_y, "SHIRT STYLE", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);
                Self::draw_string(mesh, inner_x, content_y + 72.0, "PANTS STYLE", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);
                Self::draw_string(mesh, inner_x, content_y + 114.0, "SHOE STYLE", 7.5, 10.0, 0.9, [0.95, 0.78, 0.30, 0.95]);

                for &(r, group, idx) in &layout.option_buttons {
                    let (label, active) = match group {
                        4 => (ShirtStyle::ALL[idx].display_name(), state.appearance.shirt_style as usize == idx),
                        5 => (PantsStyle::ALL[idx].display_name(), state.appearance.pants_style as usize == idx),
                        6 => (ShoeStyle::ALL[idx].display_name(), state.appearance.shoe_style as usize == idx),
                        _ => ("", false),
                    };
                    Self::draw_creator_chip(mesh, r, label, active, cursor_pos);
                }

                let target_labels = ["Shirt Main", "Shirt Accent", "Pants", "Shoes"];
                for &(r, target) in &layout.color_target_buttons {
                    Self::draw_creator_chip(mesh, r, target_labels[target], state.active_color_target == target, cursor_pos);
                }
                let active_color = match state.active_color_target {
                    0 => state.appearance.shirt_primary,
                    1 => state.appearance.shirt_accent,
                    2 => state.appearance.pants_color,
                    _ => state.appearance.shoe_color,
                };
                Self::draw_swatches_and_rgb(
                    mesh,
                    &layout.swatch_rects,
                    &layout.rgb_slider_rects,
                    active_color,
                    cursor_pos,
                );
            }
        }

        // Optional status banner above footer
        if let Some(ref msg) = state.status_message {
            Self::draw_string(
                mesh,
                inner_x,
                layout.btn_revert[1] - 18.0,
                msg,
                7.5,
                10.5,
                0.8,
                [0.62, 0.94, 0.68, 1.0],
            );
        }

        // Footer Revert & Done buttons
        Self::draw_menu_button(
            mesh,
            layout.btn_revert[0],
            layout.btn_revert[1],
            layout.btn_revert[2],
            layout.btn_revert[3],
            "Revert Changes",
            cursor_pos,
            true,
        );
        Self::draw_menu_button(
            mesh,
            layout.btn_done[0],
            layout.btn_done[1],
            layout.btn_done[2],
            layout.btn_done[3],
            "Done (Save & Back)",
            cursor_pos,
            true,
        );
    }

    fn draw_creator_chip(
        mesh: &mut UiMesh,
        r: [f32; 4],
        label: &str,
        active: bool,
        cursor_pos: [f32; 2],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        let hovered = Self::point_in_rect(cursor_pos, r);
        let border_col = if active {
            [0.95, 0.78, 0.30, 1.0]
        } else if hovered {
            [0.55, 0.65, 0.80, 0.9]
        } else {
            [0.28, 0.33, 0.42, 0.85]
        };
        let bg = if active {
            [0.22, 0.33, 0.48, 0.96]
        } else if hovered {
            [0.16, 0.21, 0.30, 0.94]
        } else {
            [0.11, 0.14, 0.20, 0.92]
        };
        mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border_col);
        mesh.add_quad(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, white_uv, bg);

        let char_w = if label.len() > 10 { 6.2 } else { 7.0 };
        let lw = Self::string_width(label, char_w, 0.7);
        Self::draw_string(
            mesh,
            r[0] + (r[2] - lw).max(4.0) * 0.5,
            r[1] + (r[3] - 10.0) * 0.5,
            label,
            char_w,
            10.0,
            0.7,
            if active { [1.0, 0.98, 0.92, 1.0] } else { [0.85, 0.88, 0.94, 1.0] },
        );
    }

    fn draw_swatches_and_rgb(
        mesh: &mut UiMesh,
        swatches: &[([f32; 4], blockgame_core::character::RgbColor)],
        sliders: &[[f32; 4]; 3],
        current: blockgame_core::character::RgbColor,
        cursor_pos: [f32; 2],
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        for &(r, col) in swatches {
            let selected = col == current;
            let hovered = Self::point_in_rect(cursor_pos, r);
            let border = if selected {
                [1.0, 0.86, 0.32, 1.0]
            } else if hovered {
                [0.85, 0.90, 0.98, 0.95]
            } else {
                [0.24, 0.28, 0.36, 0.90]
            };
            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
            let pad = if selected { 2.5 } else { 1.5 };
            mesh.add_quad(
                r[0] + pad,
                r[1] + pad,
                r[2] - pad * 2.0,
                r[3] - pad * 2.0,
                white_uv,
                col.to_rgba_f32(1.0),
            );
        }

        let channels = [
            ("R", current.r, [0.88, 0.28, 0.28, 1.0]),
            ("G", current.g, [0.28, 0.82, 0.36, 1.0]),
            ("B", current.b, [0.30, 0.56, 0.94, 1.0]),
        ];
        for (i, &(lbl, val, fill)) in channels.iter().enumerate() {
            let tr = sliders[i];
            Self::draw_string(mesh, tr[0] - 26.0, tr[1] + 2.0, lbl, 8.0, 11.0, 1.0, fill);
            mesh.add_quad(tr[0], tr[1], tr[2], tr[3], white_uv, [0.16, 0.19, 0.26, 0.96]);
            let frac = (val as f32) / 255.0;
            mesh.add_quad(tr[0], tr[1], tr[2] * frac, tr[3], white_uv, fill);
            let knob_x = tr[0] + tr[2] * frac - 4.0;
            mesh.add_quad(knob_x, tr[1] - 2.0, 8.0, tr[3] + 4.0, white_uv, [0.98, 0.98, 1.0, 1.0]);
            let num = format!("{}", val);
            Self::draw_string(mesh, tr[0] + tr[2] + 8.0, tr[1] + 2.0, &num, 7.5, 10.5, 0.8, [0.88, 0.90, 0.95, 1.0]);
        }
    }

    /// Builds the 3-tab Village Shopkeeper UI (`Buy`, `Sell`, `Enhance`).
    pub fn build_shop_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        snapshot: &blockgame_core::ShopUiSnapshot,
        active_tab: ShopUiTab,
        selected_enhance_slot: Option<usize>,
        status_message: &str,
        inventory: &Inventory,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.0, 0.0, 0.0, 0.68]);

        let panel_w = 680.0f32.min(screen_w - 40.0);
        let panel_h = 520.0f32.min(screen_h - 40.0);
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        // Main panel frame
        mesh.add_quad(px - 2.0, py - 2.0, panel_w + 4.0, panel_h + 4.0, white_uv, [0.82, 0.68, 0.30, 0.95]);
        mesh.add_quad(px, py, panel_w, panel_h, white_uv, [0.10, 0.12, 0.17, 0.96]);

        // Header: Shopkeeper name, Tier, Coins, Restock timer
        Self::draw_string(mesh, px + 18.0, py + 14.0, &snapshot.name, 10.0, 14.0, 1.0, [1.0, 0.90, 0.54, 1.0]);
        let tier_str = format!("Tier {}/{}", snapshot.shop_tier, blockgame_core::MAX_SHOP_TIER);
        Self::draw_string(mesh, px + 18.0, py + 34.0, &tier_str, 8.0, 11.0, 0.8, [0.55, 0.92, 1.0, 1.0]);

        // XP progress bar
        let bar_x = px + 110.0;
        let bar_y = py + 36.0;
        let bar_w = 180.0;
        let bar_h = 9.0;
        mesh.add_quad(bar_x, bar_y, bar_w, bar_h, white_uv, [0.18, 0.22, 0.30, 1.0]);
        let frac = if let Some(next_req) = snapshot.next_tier_coins {
            (snapshot.coins_spent as f32 / next_req.max(1) as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        mesh.add_quad(bar_x, bar_y, bar_w * frac, bar_h, white_uv, [0.28, 0.86, 0.54, 1.0]);

        let player_coins = inventory.count_item(blockgame_core::ItemId::GOLD_COIN);
        let coins_str = format!("Coins: {}", player_coins);
        Self::draw_string(mesh, px + panel_w - 200.0, py + 14.0, &coins_str, 9.0, 12.5, 0.9, [1.0, 0.86, 0.28, 1.0]);

        let days_left = (snapshot.ticks_until_restock as f32) / (blockgame_core::TICKS_PER_DAY as f32);
        let restock_str = format!("Restock: {:.1}d", days_left);
        Self::draw_string(mesh, px + panel_w - 200.0, py + 34.0, &restock_str, 7.5, 10.5, 0.8, [0.75, 0.80, 0.90, 1.0]);

        // Close button [X]
        let close_rect = [px + panel_w - 34.0, py + 10.0, 24.0, 24.0];
        let close_hov = point_in_rect(cursor_pos, close_rect);
        mesh.add_quad(close_rect[0], close_rect[1], close_rect[2], close_rect[3], white_uv, if close_hov { [0.78, 0.22, 0.22, 1.0] } else { [0.46, 0.18, 0.20, 0.95] });
        Self::draw_string(mesh, close_rect[0] + 7.0, close_rect[1] + 5.0, "X", 9.0, 13.0, 0.5, [1.0, 1.0, 1.0, 1.0]);

        // 3 Tabs: Buy / Sell / Enhance (Enhance is greyed & locked until Tier 4 or if profession has no enhancements)
        let enhance_unlocked = snapshot.profession.has_enhancements()
            && snapshot.shop_tier >= blockgame_core::ENHANCE_UNLOCK_TIER;
        let tabs: [(ShopUiTab, &str, bool); 3] = [
            (ShopUiTab::Buy, "Buy", true),
            (ShopUiTab::Sell, "Sell", true),
            (
                ShopUiTab::Enhance,
                if !snapshot.profession.has_enhancements() {
                    "Enhance (N/A)"
                } else if enhance_unlocked {
                    "Enhance"
                } else {
                    "Enhance (Tier 4)"
                },
                enhance_unlocked,
            ),
        ];
        let tab_y = py + 58.0;
        let tab_h = 28.0;
        let tab_w = (panel_w - 48.0) / 3.0;
        for (i, &(tab, label, enabled)) in tabs.iter().enumerate() {
            let tx = px + 18.0 + (i as f32) * (tab_w + 6.0);
            let rect = [tx, tab_y, tab_w, tab_h];
            let is_active = active_tab == tab;
            let bg = if !enabled {
                [0.16, 0.17, 0.20, 0.85]
            } else if is_active {
                [0.24, 0.44, 0.68, 0.98]
            } else if point_in_rect(cursor_pos, rect) {
                [0.20, 0.28, 0.40, 0.95]
            } else {
                [0.15, 0.18, 0.25, 0.92]
            };
            let fg = if !enabled {
                [0.48, 0.50, 0.56, 1.0]
            } else if is_active {
                [1.0, 0.96, 0.80, 1.0]
            } else {
                [0.84, 0.88, 0.94, 1.0]
            };
            mesh.add_quad(tx, tab_y, tab_w, tab_h, white_uv, bg);
            Self::draw_string(mesh, tx + 12.0, tab_y + 8.0, label, 8.0, 11.5, 0.8, fg);
        }

        let content_y = tab_y + tab_h + 12.0;
        let content_h = panel_h - (content_y - py) - 34.0;

        match active_tab {
            ShopUiTab::Buy => {
                let row_h = 34.0;
                let max_rows = (content_h / (row_h + 4.0)).floor() as usize;
                for (i, offer) in snapshot.buy_offers.iter().take(max_rows).enumerate() {
                    let ry = content_y + (i as f32) * (row_h + 4.0);
                    let row_rect = [px + 18.0, ry, panel_w - 36.0, row_h];
                    let can_buy = offer.unlocked
                        && offer.stock_remaining > 0
                        && player_coins >= offer.price_coins;
                    let hov = can_buy && point_in_rect(cursor_pos, row_rect);
                    let bg = if !offer.unlocked {
                        [0.13, 0.14, 0.17, 0.80]
                    } else if hov {
                        [0.20, 0.30, 0.42, 0.96]
                    } else {
                        [0.15, 0.18, 0.24, 0.92]
                    };
                    mesh.add_quad(row_rect[0], row_rect[1], row_rect[2], row_rect[3], white_uv, bg);

                    if let Some(def) = registry.get(offer.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        mesh.add_quad(row_rect[0] + 6.0, ry + 4.0, 26.0, 26.0, icon_uv, [1.0, 1.0, 1.0, if offer.unlocked { 1.0 } else { 0.45 }]);
                        let name = format!("{} x{}", def.display_name, offer.count);
                        let color = if offer.unlocked { [0.95, 0.96, 0.98, 1.0] } else { [0.50, 0.52, 0.58, 1.0] };
                        Self::draw_string(mesh, row_rect[0] + 40.0, ry + 10.0, &name, 8.0, 11.0, 0.8, color);
                    }

                    let tier_tag = if offer.is_shared {
                        format!("T{} SHARED", offer.tier)
                    } else {
                        format!("T{}", offer.tier)
                    };
                    Self::draw_string(
                        mesh,
                        row_rect[0] + 290.0,
                        ry + 11.0,
                        &tier_tag,
                        7.0,
                        10.0,
                        0.7,
                        if offer.is_shared { [0.98, 0.72, 0.30, 1.0] } else { [0.64, 0.72, 0.84, 1.0] },
                    );

                    let stock_txt = format!("Stock {}/{}", offer.stock_remaining, offer.max_stock);
                    Self::draw_string(
                        mesh,
                        row_rect[0] + 385.0,
                        ry + 11.0,
                        &stock_txt,
                        7.2,
                        10.0,
                        0.7,
                        if offer.stock_remaining > 0 { [0.72, 0.92, 0.76, 1.0] } else { [0.85, 0.38, 0.38, 1.0] },
                    );

                    let price_txt = if !offer.unlocked {
                        format!("Tier {}", offer.tier)
                    } else {
                        format!("{}g", offer.price_coins)
                    };
                    Self::draw_string(
                        mesh,
                        row_rect[0] + row_rect[2] - 88.0,
                        ry + 10.0,
                        &price_txt,
                        8.2,
                        11.5,
                        0.8,
                        if can_buy { [1.0, 0.88, 0.32, 1.0] } else { [0.62, 0.54, 0.44, 1.0] },
                    );
                }
            }
            ShopUiTab::Sell => {
                let budget_txt = format!(
                    "Daily Buyback Budget: {} / {} Gold Coins",
                    snapshot.daily_sell_remaining, snapshot.daily_sell_limit
                );
                Self::draw_string(mesh, px + 18.0, content_y, &budget_txt, 8.0, 11.0, 0.8, [1.0, 0.86, 0.42, 1.0]);
                let list_y = content_y + 22.0;
                let row_h = 38.0;
                for (i, offer) in snapshot.sell_offers.iter().enumerate() {
                    let ry = list_y + (i as f32) * (row_h + 6.0);
                    let row_rect = [px + 18.0, ry, panel_w - 36.0, row_h];
                    let have = inventory.count_item(offer.item_id);
                    let can_sell = have >= offer.count as u32
                        && snapshot.daily_sell_remaining >= offer.payout_coins;
                    let hov = can_sell && point_in_rect(cursor_pos, row_rect);
                    let bg = if hov {
                        [0.20, 0.34, 0.26, 0.96]
                    } else {
                        [0.15, 0.18, 0.24, 0.92]
                    };
                    mesh.add_quad(row_rect[0], row_rect[1], row_rect[2], row_rect[3], white_uv, bg);

                    if let Some(def) = registry.get(offer.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        mesh.add_quad(row_rect[0] + 6.0, ry + 6.0, 26.0, 26.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                        let label = format!("Sell {} x{} (Have: {})", def.display_name, offer.count, have);
                        Self::draw_string(mesh, row_rect[0] + 40.0, ry + 12.0, &label, 8.0, 11.0, 0.8, [0.94, 0.95, 0.98, 1.0]);
                    }
                    let pay_txt = format!("+{}g", offer.payout_coins);
                    Self::draw_string(
                        mesh,
                        row_rect[0] + row_rect[2] - 88.0,
                        ry + 12.0,
                        &pay_txt,
                        8.5,
                        12.0,
                        0.8,
                        if can_sell { [0.46, 0.95, 0.56, 1.0] } else { [0.56, 0.56, 0.60, 1.0] },
                    );
                }
            }
            ShopUiTab::Enhance => {
                // Left column: eligible unenhanced inventory items; Right column: Enhancement offers
                let left_x = px + 18.0;
                let left_w = 270.0;
                let right_x = left_x + left_w + 14.0;
                let right_w = panel_w - 36.0 - left_w - 14.0;

                Self::draw_string(mesh, left_x, content_y, "1. Select Unenhanced Item:", 7.8, 10.8, 0.8, [0.80, 0.88, 0.98, 1.0]);
                Self::draw_string(mesh, right_x, content_y, "2. Apply Enhancement:", 7.8, 10.8, 0.8, [0.80, 0.88, 0.98, 1.0]);

                let list_y = content_y + 20.0;
                let eligible = eligible_enhance_inventory_slots(snapshot, inventory, registry);
                for (row_i, &inv_idx) in eligible.iter().take(9).enumerate() {
                    let ry = list_y + (row_i as f32) * 36.0;
                    let rect = [left_x, ry, left_w, 32.0];
                    let is_sel = selected_enhance_slot == Some(inv_idx);
                    let bg = if is_sel {
                        [0.24, 0.44, 0.64, 0.98]
                    } else if point_in_rect(cursor_pos, rect) {
                        [0.19, 0.25, 0.34, 0.95]
                    } else {
                        [0.14, 0.17, 0.22, 0.92]
                    };
                    mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, bg);
                    let slot = inventory.get(inv_idx);
                    if let Some(def) = registry.get(slot.item_id) {
                        let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                        mesh.add_quad(rect[0] + 4.0, ry + 4.0, 24.0, 24.0, icon_uv, [1.0, 1.0, 1.0, 1.0]);
                        Self::draw_string(mesh, rect[0] + 34.0, ry + 10.0, def.display_name, 7.6, 10.5, 0.7, [0.95, 0.96, 1.0, 1.0]);
                    }
                }

                let selected_item = selected_enhance_slot
                    .map(|i| inventory.get(i))
                    .filter(|s| !s.is_empty());

                for (i, offer) in snapshot.enhance_offers.iter().enumerate() {
                    let ry = list_y + (i as f32) * 62.0;
                    let rect = [right_x, ry, right_w, 56.0];
                    let compat = selected_item.map_or(false, |s| {
                        !s.is_enhanced() && offer.enhancement.is_compatible_with(s.item_id, registry)
                    });
                    let dyn_price = selected_item.map_or(offer.base_price_coins, |s| {
                        offer.price_for_item(s.item_id, registry)
                    });
                    let can_apply = offer.unlocked
                        && offer.stock_remaining > 0
                        && compat
                        && player_coins >= dyn_price;
                    let hov = can_apply && point_in_rect(cursor_pos, rect);
                    let bg = if !offer.unlocked {
                        [0.13, 0.14, 0.17, 0.80]
                    } else if hov {
                        [0.22, 0.38, 0.52, 0.98]
                    } else {
                        [0.15, 0.19, 0.26, 0.92]
                    };
                    mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, bg);

                    let title = format!(
                        "{} (T{} SHARED {}/{})",
                        offer.enhancement.display_name(),
                        offer.tier,
                        offer.stock_remaining,
                        offer.max_stock
                    );
                    Self::draw_string(
                        mesh,
                        rect[0] + 10.0,
                        ry + 8.0,
                        &title,
                        7.8,
                        11.0,
                        0.75,
                        if offer.unlocked { [0.58, 0.92, 1.0, 1.0] } else { [0.50, 0.52, 0.56, 1.0] },
                    );
                    Self::draw_string(
                        mesh,
                        rect[0] + 10.0,
                        ry + 24.0,
                        offer.enhancement.description(),
                        7.0,
                        10.0,
                        0.7,
                        [0.82, 0.86, 0.92, 1.0],
                    );
                    let price_str = format!("Cost: {}g + Item", dyn_price);
                    Self::draw_string(
                        mesh,
                        rect[0] + 10.0,
                        ry + 39.0,
                        &price_str,
                        7.4,
                        10.2,
                        0.75,
                        if can_apply { [1.0, 0.88, 0.34, 1.0] } else { [0.65, 0.55, 0.45, 1.0] },
                    );
                }
            }
        }

        // Bottom status bar
        if !status_message.is_empty() {
            Self::draw_string(
                mesh,
                px + 18.0,
                py + panel_h - 24.0,
                status_message,
                7.8,
                11.0,
                0.8,
                [0.98, 0.88, 0.52, 1.0],
            );
        }
    }

    /// Hit-tests a left click on the 3-tab Shop UI.
    pub fn hit_test_shop_screen(
        screen_w: f32,
        screen_h: f32,
        snapshot: &blockgame_core::ShopUiSnapshot,
        active_tab: ShopUiTab,
        inventory: &Inventory,
        cursor_pos: [f32; 2],
        registry: &ItemRegistry,
    ) -> Option<ShopUiAction> {
        let panel_w = 680.0f32.min(screen_w - 40.0);
        let panel_h = 520.0f32.min(screen_h - 40.0);
        let px = (screen_w - panel_w) * 0.5;
        let py = (screen_h - panel_h) * 0.5;

        let close_rect = [px + panel_w - 34.0, py + 10.0, 24.0, 24.0];
        if point_in_rect(cursor_pos, close_rect) {
            return Some(ShopUiAction::Close);
        }

        let enhance_unlocked = snapshot.profession.has_enhancements()
            && snapshot.shop_tier >= blockgame_core::ENHANCE_UNLOCK_TIER;
        let tab_y = py + 58.0;
        let tab_h = 28.0;
        let tab_w = (panel_w - 48.0) / 3.0;
        let tabs = [
            (ShopUiTab::Buy, true),
            (ShopUiTab::Sell, true),
            (ShopUiTab::Enhance, enhance_unlocked),
        ];
        for (i, &(tab, enabled)) in tabs.iter().enumerate() {
            let tx = px + 18.0 + (i as f32) * (tab_w + 6.0);
            if point_in_rect(cursor_pos, [tx, tab_y, tab_w, tab_h]) {
                if enabled {
                    return Some(ShopUiAction::SwitchTab(tab));
                }
                return None;
            }
        }

        let content_y = tab_y + tab_h + 12.0;
        let content_h = panel_h - (content_y - py) - 34.0;

        match active_tab {
            ShopUiTab::Buy => {
                let row_h = 34.0;
                let max_rows = (content_h / (row_h + 4.0)).floor() as usize;
                for (i, offer) in snapshot.buy_offers.iter().take(max_rows).enumerate() {
                    let ry = content_y + (i as f32) * (row_h + 4.0);
                    if point_in_rect(cursor_pos, [px + 18.0, ry, panel_w - 36.0, row_h]) {
                        return Some(ShopUiAction::Buy(offer.offer_idx));
                    }
                }
            }
            ShopUiTab::Sell => {
                let list_y = content_y + 22.0;
                let row_h = 38.0;
                for (i, offer) in snapshot.sell_offers.iter().enumerate() {
                    let ry = list_y + (i as f32) * (row_h + 6.0);
                    if point_in_rect(cursor_pos, [px + 18.0, ry, panel_w - 36.0, row_h]) {
                        return Some(ShopUiAction::Sell(offer.offer_idx));
                    }
                }
            }
            ShopUiTab::Enhance => {
                let left_x = px + 18.0;
                let left_w = 270.0;
                let right_x = left_x + left_w + 14.0;
                let right_w = panel_w - 36.0 - left_w - 14.0;
                let list_y = content_y + 20.0;

                let eligible = eligible_enhance_inventory_slots(snapshot, inventory, registry);
                for (row_i, &inv_idx) in eligible.iter().take(9).enumerate() {
                    let ry = list_y + (row_i as f32) * 36.0;
                    if point_in_rect(cursor_pos, [left_x, ry, left_w, 32.0]) {
                        return Some(ShopUiAction::SelectEnhanceSlot(inv_idx));
                    }
                }
                for (i, offer) in snapshot.enhance_offers.iter().enumerate() {
                    let ry = list_y + (i as f32) * 62.0;
                    if point_in_rect(cursor_pos, [right_x, ry, right_w, 56.0]) {
                        return Some(ShopUiAction::ApplyEnhance(offer.offer_idx));
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopUiTab {
    Buy,
    Sell,
    Enhance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopUiAction {
    SwitchTab(ShopUiTab),
    Buy(u16),
    Sell(u16),
    SelectEnhanceSlot(usize),
    ApplyEnhance(u16),
    Close,
}

fn eligible_enhance_inventory_slots(
    snapshot: &blockgame_core::ShopUiSnapshot,
    inventory: &Inventory,
    registry: &ItemRegistry,
) -> Vec<usize> {
    let mut out = Vec::new();
    for idx in 0..inventory.len() {
        let slot = inventory.get(idx);
        if slot.is_empty() || slot.is_enhanced() {
            continue;
        }
        if snapshot
            .enhance_offers
            .iter()
            .any(|o| o.enhancement.is_compatible_with(slot.item_id, registry))
        {
            out.push(idx);
        }
    }
    out
}

#[inline]
fn point_in_rect(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[0] + r[2] && p[1] >= r[1] && p[1] <= r[1] + r[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockgame_core::item::ItemId;
    use blockgame_core::player::PlayerStats;

    #[test]
    fn test_draw_stack_number_generates_quads() {
        let mut mesh = UiMesh::new();
        // count = 1 does not draw number
        UiBuilder::draw_stack_number(&mut mesh, 0.0, 0.0, 40.0, 1);
        assert_eq!(mesh.indices.len(), 0);

        // count = 64 draws two digit quads (2 * 6 = 12 indices)
        UiBuilder::draw_stack_number(&mut mesh, 0.0, 0.0, 40.0, 64);
        assert_eq!(mesh.indices.len(), 12);
    }

    #[test]
    fn test_draw_tooltip_generates_quads() {
        let mut mesh = UiMesh::new();
        UiBuilder::draw_tooltip(&mut mesh, 1280.0, 720.0, 100.0, 100.0, "Torch");
        // 2 background quads (border + bg) + 5 letter quads = 7 quads = 42 indices
        assert_eq!(mesh.indices.len(), 7 * 6);
    }

    #[test]
    fn test_build_hud_held_item() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::TORCH, 16));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_empty_hand() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        // Empty hand draws the player's arm and fist
        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 1.5, 0.2, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_block() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::WOOD_LOG, 16));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_pickaxe() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::IRON_PICKAXE, 1));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.3, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_stick() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::STICK, 4));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_ingot_and_coal() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::IRON_INGOT, 5));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        assert!(mesh.indices.len() > 0);

        let mut mesh_coal = UiMesh::new();
        inv.set(0, ItemSlot::new(ItemId::COAL, 3));
        UiBuilder::build_hud(
            &mut mesh_coal,
            1280.0,
            720.0,
            &inv,
            0,
            &registry,
            0.0,
            0.0,
            &stats,
        );
        assert!(mesh_coal.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_hearts_and_hunger() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let mut stats = PlayerStats::new();
        stats.health = 15.0; // 7 full hearts + 1 half heart + 2 empty hearts
        stats.hunger = 17.0; // 8 full drumsticks + 1 half drumstick + 1 empty drumstick

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_death_screen() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let mut stats = PlayerStats::new();
        stats.health = 0.0;
        stats.is_dead = true;

        UiBuilder::build_hud(
            &mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats,
        );
        // Generates death screen quads (red backdrop, You Died text, Score text, Respawn button)
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_chest_screen() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let chest_slots = vec![ItemSlot::new(ItemId::DIAMOND, 64); 27];
        let cursor_held = ItemSlot::EMPTY;

        UiBuilder::build_chest_screen(
            &mut mesh,
            1280.0,
            720.0,
            &chest_slots,
            &inv,
            0,
            &cursor_held,
            [640.0, 360.0],
            &registry,
            None,
            None,
        );

        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);
    }

    #[test]
    fn test_draw_jei_panel() {
        let mut mesh = UiMesh::new();
        let registry = ItemRegistry::new();
        let catalog = [ItemId::DIAMOND_PICKAXE, ItemId::IRON_INGOT, ItemId::CHEST];
        let params = JeiParams {
            catalog_items: &catalog,
            page: 0,
            total_pages: 1,
            search_query: "pick",
            search_focused: true,
        };
        let mut hovered = None;
        UiBuilder::draw_jei_panel(
            &mut mesh,
            1280.0,
            720.0,
            &params,
            [1050.0, 200.0],
            &registry,
            &mut hovered,
        );
        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);
    }

    #[test]
    fn test_draw_recipe_modal() {
        let mut mesh = UiMesh::new();
        let registry = ItemRegistry::new();
        let display_recipe = blockgame_core::crafting::DisplayRecipe {
            name: "Diamond Pickaxe",
            category: blockgame_core::crafting::RecipeCategory::Crafting3x3,
            inputs: [
                Some(ItemId::DIAMOND),
                Some(ItemId::DIAMOND),
                Some(ItemId::DIAMOND),
                None,
                Some(ItemId::STICK),
                None,
                None,
                Some(ItemId::STICK),
                None,
            ],
            output: ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1),
        };
        let params = RecipeModalParams {
            target_item: ItemId::DIAMOND_PICKAXE,
            recipe: &display_recipe,
            recipe_index: 0,
            total_recipes: 1,
            is_usage_mode: false,
        };
        let mut hovered = None;
        UiBuilder::draw_recipe_modal(
            &mut mesh,
            1280.0,
            720.0,
            &params,
            [640.0, 360.0],
            &registry,
            &mut hovered,
        );
        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);
    }

    #[test]
    fn test_draw_recipe_modal_world_gen() {
        let mut mesh = UiMesh::new();
        let registry = ItemRegistry::new();
        let display_recipe = blockgame_core::crafting::DisplayRecipe {
            name: "Dirt",
            category: blockgame_core::crafting::RecipeCategory::WorldGen,
            inputs: [None; 9],
            output: ItemSlot::new(ItemId::DIRT, 1),
        };
        let params = RecipeModalParams {
            target_item: ItemId::DIRT,
            recipe: &display_recipe,
            recipe_index: 0,
            total_recipes: 1,
            is_usage_mode: false,
        };
        let mut hovered = None;
        UiBuilder::draw_recipe_modal(
            &mut mesh,
            1280.0,
            720.0,
            &params,
            [640.0, 360.0],
            &registry,
            &mut hovered,
        );
        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);
    }

    #[test]
    fn test_build_furnace_screen() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let mut furnace = FurnaceState::new();
        furnace.input = ItemSlot::new(ItemId::RAW_IRON, 5);
        furnace.fuel = ItemSlot::new(ItemId::COAL, 3);
        furnace.output = ItemSlot::new(ItemId::IRON_INGOT, 2);
        furnace.burn_time_remaining = 40.0;
        furnace.max_burn_time = 80.0;
        furnace.cook_progress = 4.0;
        let cursor_held = ItemSlot::EMPTY;

        UiBuilder::build_furnace_screen(
            &mut mesh,
            1280.0,
            720.0,
            &furnace,
            &inv,
            0,
            &cursor_held,
            [640.0, 360.0],
            &registry,
            None,
            None,
        );

        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);
    }

    #[test]
    fn test_build_hud_underwater_and_bubbles() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let mut stats = blockgame_core::player::PlayerStats::new();

        // Normal HUD
        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        let normal_quads = mesh.indices.len() / 6;

        // Submerged HUD (adds underwater overlay + 10 bubble quads)
        stats.is_submerged = true;
        stats.breath = 12.0;
        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        let submerged_quads = mesh.indices.len() / 6;

        assert!(submerged_quads > normal_quads, "Submerged HUD must generate extra quads for underwater tint and bubbles");
    }

    #[test]
    fn test_build_hud_held_seagrass_crossed_3d_plant() {
        let registry = ItemRegistry::new();

        // 3D Block (Dirt) generates 3 isometric faces (18 indices)
        let mut mesh_block = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_block, 1280.0, 720.0, ItemSlot::new(ItemId::DIRT, 1), &registry, 0.0, 0.0);
        let block_quads = mesh_block.indices.len() / 6;
        assert_eq!(block_quads, 3, "Solid blocks should generate 3 isometric cube faces");

        // Plant item (Seagrass) generates 3D crossed-quad in-world plant geometry with dirt clod (>= 2 quads)
        let mut mesh_seagrass = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_seagrass, 1280.0, 720.0, ItemSlot::new(ItemId::SEAGRASS, 1), &registry, 0.0, 0.0);
        let seagrass_quads = mesh_seagrass.indices.len() / 6;
        assert!(seagrass_quads >= 2, "Seagrass in hand should generate 3D crossed-quad in-world plant geometry instead of a flat 2D sticker");
    }

    #[test]
    fn test_build_hud_held_3d_stairs_fence_door_bed_flower() {
        let registry = ItemRegistry::new();

        // 3D Stairs
        let mut mesh_stairs = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_stairs, 1280.0, 720.0, ItemSlot::new(ItemId::OAK_STAIRS, 1), &registry, 0.0, 0.0);
        assert!(mesh_stairs.indices.len() / 6 >= 5, "Stairs in hand must generate 3D stepped facets");

        // 3D Oak Fence
        let mut mesh_fence = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_fence, 1280.0, 720.0, ItemSlot::new(ItemId::OAK_FENCE, 1), &registry, 0.0, 0.0);
        assert!(mesh_fence.indices.len() / 6 >= 5, "Fence in hand must generate 3D post and cross-rails");

        // 3D Oak Door
        let mut mesh_door = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_door, 1280.0, 720.0, ItemSlot::new(ItemId::OAK_DOOR, 1), &registry, 0.0, 0.0);
        assert!(mesh_door.indices.len() / 6 >= 3, "Door in hand must generate 3D upright door slab with thickness");

        // 3D Bed
        let mut mesh_bed = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_bed, 1280.0, 720.0, ItemSlot::new(ItemId::BED, 1), &registry, 0.0, 0.0);
        assert!(mesh_bed.indices.len() / 6 >= 6, "Bed in hand must generate 3D pillow, mattress, frame, and legs");

        // 3D Flower (Poppy)
        let mut mesh_flower = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_flower, 1280.0, 720.0, ItemSlot::new(ItemId::POPPY, 1), &registry, 0.0, 0.0);
        assert!(mesh_flower.indices.len() / 6 >= 2, "Flower in hand must generate 3D crossed billboard quads with base clod");
    }

    #[test]
    fn test_build_hud_held_3d_fishing_rod_bow_bucket_food_and_volumetric() {
        let registry = ItemRegistry::new();

        // 3D Fishing Rod
        let mut mesh_rod = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_rod, 1280.0, 720.0, ItemSlot::new(ItemId::WOODEN_FISHING_ROD, 1), &registry, 0.0, 0.0);
        assert!(mesh_rod.indices.len() / 6 >= 5, "Fishing rod in hand must generate 3D pole, reel, line, and bobber");

        // 3D Bow
        let mut mesh_bow = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_bow, 1280.0, 720.0, ItemSlot::new(ItemId::BOW, 1), &registry, 0.0, 0.0);
        assert!(mesh_bow.indices.len() / 6 >= 5, "Bow in hand must generate 3D curved stave and bowstring");

        // 3D Water Bucket
        let mut mesh_bucket = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_bucket, 1280.0, 720.0, ItemSlot::new(ItemId::WATER_BUCKET, 1), &registry, 0.0, 0.0);
        assert!(mesh_bucket.indices.len() / 6 >= 5, "Bucket in hand must generate 3D body, rim, handle, and liquid");

        // 3D Bread
        let mut mesh_bread = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_bread, 1280.0, 720.0, ItemSlot::new(ItemId::BREAD, 1), &registry, 0.0, 0.0);
        assert!(mesh_bread.indices.len() / 6 >= 4, "Bread in hand must generate 3D scored golden loaf");

        // 3D Apple
        let mut mesh_apple = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_apple, 1280.0, 720.0, ItemSlot::new(ItemId::APPLE, 1), &registry, 0.0, 0.0);
        assert!(mesh_apple.indices.len() / 6 >= 4, "Apple in hand must generate 3D fruit facets, stem, and leaf");

        // Held 2D Item with First-Person Arm & Hand (Wheat Seeds)
        let mut mesh_seeds = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_seeds, 1280.0, 720.0, ItemSlot::new(ItemId::WHEAT_SEEDS, 1), &registry, 0.0, 0.0);
        assert!(mesh_seeds.indices.len() / 6 >= 5, "Held item must render first-person arm, sleeve, knuckles, and item sprite");
    }

    #[test]
    fn test_build_title_screen() {
        let mut mesh = UiMesh::new();
        UiBuilder::build_title_screen(&mut mesh, 1280.0, 720.0, [640.0, 360.0]);
        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);

        let rects = UiBuilder::title_screen_button_rects(1280.0, 720.0);
        assert_eq!(rects.len(), UiBuilder::TITLE_BUTTON_COUNT);
        // Hover check inside first button (Singleplayer)
        let r0 = rects[0];
        assert!(UiBuilder::is_rect_hovered(
            [r0[0] + 10.0, r0[1] + 10.0],
            r0[0],
            r0[1],
            r0[2],
            r0[3]
        ));

        // Test with recent world metadata
        let sample_world = blockgame_core::save::WorldMetadata::new("Wilderness Outpost", 98765);
        UiBuilder::build_title_screen_with_recent_world(
            &mut mesh,
            1280.0,
            720.0,
            [640.0, 360.0],
            Some(&sample_world),
        );
        assert!(mesh.indices.len() > 0);

        // Test guide screen across all pages and button rects
        for page in 0..3 {
            UiBuilder::build_guide_screen(&mut mesh, 1280.0, 720.0, page, [640.0, 360.0]);
            assert!(mesh.indices.len() > 0);
        }
        let guide_rects = UiBuilder::guide_screen_button_rects(1280.0, 720.0);
        assert_eq!(guide_rects.len(), 3);
    }

    #[test]
    fn test_build_world_select_and_create_screens() {
        let mut mesh = UiMesh::new();
        // Empty world list
        UiBuilder::build_world_select_screen(&mut mesh, 1280.0, 720.0, &[], None, [640.0, 360.0]);
        assert!(mesh.indices.len() > 0);

        // Populated world list with selection
        let worlds = vec![
            blockgame_core::save::WorldMetadata::new("Alpha Survival", 1337),
            blockgame_core::save::WorldMetadata::new("Creative Islands", 4242),
        ];
        UiBuilder::build_world_select_screen(
            &mut mesh,
            1280.0,
            720.0,
            &worlds,
            Some(0),
            [640.0, 200.0],
        );
        assert!(mesh.indices.len() > 0);

        // Create New World screen
        UiBuilder::build_create_world_screen(
            &mut mesh,
            1280.0,
            720.0,
            "My New World",
            "12345",
            0,
            blockgame_core::GameMode::Survival,
            [640.0, 360.0],
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_pause_and_settings_menus() {
        let mut mesh = UiMesh::new();
        UiBuilder::build_pause_menu(
            &mut mesh,
            1280.0,
            720.0,
            [640.0, 360.0],
            Some("World saved successfully!"),
        );
        assert!(mesh.indices.len() > 0);

        UiBuilder::build_notice_or_settings_screen(
            &mut mesh,
            1280.0,
            720.0,
            "Settings",
            &["Render Distance: 8 Chunks", "FOV: 70 Degrees"],
            [640.0, 360.0],
        );
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_multiplayer_lobby_and_nametags() {
        let mut mesh = UiMesh::new();
        let sample_srv = blockgame_core::directory::DirectoryServerEntry {
            id: "srv-1".to_string(),
            name: "Official Survival".to_string(),
            motd: "&bWelcome to BlockGame!".to_string(),
            host: "15.204.81.97".to_string(),
            port: 25565,
            players: 3,
            max_players: 20,
            version: "0.1.0".to_string(),
            voice_enabled: true,
        };
        let sample_app = blockgame_core::character::CharacterAppearance::default();
        UiBuilder::build_multiplayer_lobby_screen(
            &mut mesh,
            1280.0,
            720.0,
            "Alice_99",
            "secret123",
            "127.0.0.1:25565",
            &sample_app,
            "Wayfarer",
            0,
            Some(("Ready to connect", false)),
            &[sample_srv],
            Some(0),
            false,
            None,
            [640.0, 360.0],
        );
        assert!(mesh.indices.len() > 0);

        let mut tag_mesh = UiMesh::new();
        let cam = crate::camera::Camera::new(glam::Vec3::new(0.0, 66.0, 0.0), 0.0, 0.0);
        let vp = cam.build_view_projection_matrix(1280.0 / 720.0);
        let mut p = blockgame_core::net::PlayerModelState::new(
            2,
            "BuilderBob",
            glam::Vec3::new(4.0, 64.5, 0.0),
            sample_app,
        );
        p.health = 14.0;
        UiBuilder::draw_player_nametags(&mut tag_mesh, 1280.0, 720.0, vp, cam.position, &[p.clone()]);
        assert!(tag_mesh.indices.len() > 0);

        let chat = vec![blockgame_core::net::ChatEntry {
            sender: "BuilderBob".to_string(),
            prefix: "&b[Builder] ".to_string(),
            message: "Hello world!".to_string(),
            is_system: false,
            age: 1.0,
        }];
        let _ = UiBuilder::draw_multiplayer_hud_overlays(
            &mut tag_mesh,
            1280.0,
            720.0,
            &chat,
            true,
            "Hi Bob",
            0,
            [0.0, 0.0],
            true,
            &[p],
            "Alice_99",
            true,
            "127.0.0.1:25565",
        );
        assert!(tag_mesh.indices.len() > 50);

        // Test clickable chat buttons
        let mut button_mesh = UiMesh::new();
        let tpa_chat = vec![blockgame_core::net::ChatEntry {
            sender: "Server".to_string(),
            prefix: String::new(),
            message: "<Alice> has requested to teleport to you. [Accept] [Deny]".to_string(),
            is_system: true,
            age: 0.0,
        }];
        let hitboxes = UiBuilder::draw_multiplayer_hud_overlays(
            &mut button_mesh,
            1280.0,
            720.0,
            &tpa_chat,
            true,
            "",
            0,
            [0.0, 0.0],
            false,
            &[],
            "Bob",
            false,
            "127.0.0.1:25565",
        );
        assert_eq!(hitboxes.len(), 2);
        assert_eq!(hitboxes[0].action, ChatAction::Command("/tpaccept".to_string()));
        assert_eq!(hitboxes[1].action, ChatAction::Command("/tpdeny".to_string()));
    }

    #[test]
    fn test_build_character_creator_screen_and_portrait() {
        let profile = blockgame_core::CharacterProfile::default();
        for tab in CharacterCreatorTab::ALL {
            let state = CharacterCreatorUiState {
                active_tab: tab,
                appearance: profile.active_appearance,
                presets: profile.all_presets(),
                selected_preset_name: profile.selected_preset_name.clone(),
                preset_name_input: "My Hero".to_string(),
                preset_input_focused: false,
                face_zoom: false,
                walk_preview: true,
                active_color_target: 0,
                status_message: Some("Ready".to_string()),
            };
            let mut mesh = UiMesh::new();
            UiBuilder::build_character_creator_screen(&mut mesh, 1280.0, 720.0, &state, [640.0, 360.0]);
            assert!(!mesh.vertices.is_empty());
            assert!(!mesh.indices.is_empty());
        }

        let mut portrait_mesh = UiMesh::new();
        UiBuilder::draw_character_portrait(
            &mut portrait_mesh,
            10.0,
            10.0,
            48.0,
            &profile.active_appearance,
        );
        assert!(!portrait_mesh.indices.is_empty());
    }

    #[test]
    fn test_achievement_ui_hidden_masking_and_rendering() {
        let hidden_def = blockgame_core::achievement::achievement_by_id("hidden_cure_poison")
            .expect("hidden_cure_poison exists");
        assert_eq!(
            UiBuilder::achievement_display_text(hidden_def, false),
            ("???", "Hidden Achievement - keep exploring to unlock!")
        );
        assert_eq!(
            UiBuilder::achievement_display_text(hidden_def, true),
            (hidden_def.title, hidden_def.description)
        );

        let mut engine = blockgame_core::achievement::AchievementEngine::default();
        engine.record_event(&blockgame_core::achievement::AchievementEvent::BlockMined {
            block_id: blockgame_core::block::BlockId::WOOD_LOG.0,
        });
        let item_registry = blockgame_core::item::ItemRegistry::new();

        for cat_idx in 0..blockgame_core::achievement::AchievementCategory::ALL.len() {
            let mut mesh = UiMesh::new();
            UiBuilder::build_achievements_screen(
                &mut mesh,
                1280.0,
                720.0,
                &engine,
                &item_registry,
                cat_idx,
                0,
                Some(true),
                [640.0, 360.0],
            );
            assert!(!mesh.vertices.is_empty());
            assert!(!mesh.indices.is_empty());
        }

        let mut toast_mesh = UiMesh::new();
        let tile = item_registry
            .get(blockgame_core::item::ItemId::WOOD_LOG)
            .unwrap()
            .icon_tile as u16;
        UiBuilder::draw_achievement_toast(
            &mut toast_mesh,
            1280.0,
            "Timber!",
            "Chop your first log.",
            tile,
            1.0,
        );
        assert!(!toast_mesh.vertices.is_empty());
        assert!(!toast_mesh.indices.is_empty());
    }

    #[test]
    fn test_controller_glyphs_virtual_cursor_quick_menu_and_settings_tab() {
        // Every Controller Family emits valid glyph quads
        for family in ControllerFamily::ALL {
            let mut mesh = UiMesh::new();
            UiBuilder::draw_gamepad_glyph(
                &mut mesh,
                10.0,
                10.0,
                20.0,
                family,
                GamepadButton::South,
                [1.0; 4],
            );
            assert_eq!(mesh.vertices.len(), 4);
            assert_eq!(mesh.indices.len(), 6);
        }

        // Virtual Cursor emits 4 layered quads
        let mut cursor_mesh = UiMesh::new();
        UiBuilder::draw_virtual_cursor(&mut cursor_mesh, 640.0, 360.0, 1.0);
        assert_eq!(cursor_mesh.vertices.len(), 16);

        // Quick Menu renders cleanly with glyph prompts
        let mut qm_mesh = UiMesh::new();
        UiBuilder::build_quick_menu(
            &mut qm_mesh,
            1280.0,
            720.0,
            false,
            ControllerFamily::PlayStation,
            GamepadButton::South,
            GamepadButton::East,
            [640.0, 360.0],
        );
        assert!(!qm_mesh.indices.is_empty());

        // Settings Tab 2 (Controller) renders with all 16 bindings & tuning sliders
        let mut settings_mesh = UiMesh::new();
        let settings_state = SettingsUiState {
            render_distance: 6,
            fov_degrees: 70.0,
            mouse_sensitivity: 1.0,
            master_volume: 0.8,
            sfx_volume: 0.9,
            music_volume: 0.65,
            creature_volume: 0.85,
            invert_y: false,
            fullscreen: false,
            vsync: true,
            graphics_quality_fast: false,
            view_bobbing: true,
            show_debug_hud: false,
            ui_scale: 1.0,
            voice_mode: 1,
            voice_volume: 1.0,
            voice_input_device: String::new(),
            controller_sensitivity: 1.2,
            controller_invert_y: false,
            controller_deadzone: 0.15,
            controller_rumble_intensity: 0.8,
            controller_glyph_style: GlyphStyle::Auto,
            controller_swap_confirm_cancel: false,
            controller_sneak_toggle: true,
            active_controller_label: "Steam Deck (Steam Deck)".to_string(),
            active_glyph_family: ControllerFamily::SteamDeck,
        };
        let pad_binds = [
            ("Mine / Attack", GamepadButton::RightTrigger),
            ("Jump / Fly Up", GamepadButton::South),
        ];
        UiBuilder::build_settings_screen(
            &mut settings_mesh,
            1280.0,
            720.0,
            settings_state,
            2,
            None,
            None,
            &[],
            Some(1),
            &pad_binds,
            [640.0, 360.0],
        );
        assert!(!settings_mesh.indices.is_empty());

        // Dynamic Pause & Fishing HUD prompts in Controller InputMode
        let mut pause_mesh = UiMesh::new();
        UiBuilder::build_pause_menu_with_input_mode(
            &mut pause_mesh,
            1280.0,
            720.0,
            [640.0, 360.0],
            None,
            InputMode::Controller,
            ControllerFamily::Nintendo,
        );
        assert!(!pause_mesh.indices.is_empty());

        let mut fish_mesh = UiMesh::new();
        let fish_state = FishingHudState {
            phase: FishingHudPhase::Biting,
            tackle_summary: "Oak Rod".to_string(),
        };
        UiBuilder::build_fishing_hud_with_input_mode(
            &mut fish_mesh,
            1280.0,
            720.0,
            &fish_state,
            InputMode::Controller,
            ControllerFamily::Xbox,
            GamepadButton::LeftTrigger,
        );
        assert!(!fish_mesh.indices.is_empty());
    }

    #[test]
    fn test_parse_chat_segments_http_and_https_links() {
        let segs = UiBuilder::parse_chat_segments(
            "Check out https://example.com/wiki_(help). and http://localhost:8080/test!",
            1.0,
        );
        assert_eq!(
            segs,
            vec![
                ChatSegment::Text("Check out "),
                ChatSegment::Link {
                    label: "https://example.com/wiki_(help)",
                    url: "https://example.com/wiki_(help)".to_string(),
                },
                ChatSegment::Text(". and "),
                ChatSegment::Link {
                    label: "http://localhost:8080/test",
                    url: "http://localhost:8080/test".to_string(),
                },
                ChatSegment::Text("!"),
            ]
        );
    }

    #[test]
    fn test_parse_chat_segments_ignores_non_http_schemes_and_preserves_buttons() {
        let segs = UiBuilder::parse_chat_segments(
            "Avoid file:///etc/passwd and javascript:alert(1) [Accept] https://blockgame.dev",
            1.0,
        );
        assert_eq!(segs.len(), 4);
        assert!(matches!(segs[0], ChatSegment::Text(t) if t.contains("file:///etc/passwd")));
        assert!(matches!(segs[1], ChatSegment::Button { label: "Accept", .. }));
        assert_eq!(segs[2], ChatSegment::Text(" "));
        assert_eq!(
            segs[3],
            ChatSegment::Link {
                label: "https://blockgame.dev",
                url: "https://blockgame.dev".to_string(),
            }
        );
    }

    #[test]
    fn test_chat_link_hitbox_and_confirm_modal() {
        let mut mesh = UiMesh::new();
        let entry = blockgame_core::net::ChatEntry {
            sender: "Steve".to_string(),
            prefix: String::new(),
            message: "Join https://discord.gg/blockgame now!".to_string(),
            is_system: false,
            age: 0.0,
        };
        let hitboxes = UiBuilder::draw_multiplayer_hud_overlays(
            &mut mesh,
            1280.0,
            720.0,
            &[entry],
            true,
            "",
            0,
            [0.0, 0.0],
            false,
            &[],
            "Steve",
            false,
            "",
        );
        assert_eq!(hitboxes.len(), 1);
        assert_eq!(
            hitboxes[0].action,
            ChatAction::OpenUrl("https://discord.gg/blockgame".to_string())
        );

        let mut modal_mesh = UiMesh::new();
        let modal_hb = UiBuilder::draw_link_confirm_modal(
            &mut modal_mesh,
            1280.0,
            720.0,
            "https://discord.gg/blockgame",
            false,
            [640.0, 360.0],
        );
        assert!(!modal_mesh.indices.is_empty());
        assert!(modal_hb.open_rect[2] > 0.0 && modal_hb.copy_rect[2] > 0.0 && modal_hb.cancel_rect[2] > 0.0);
    }

    #[test]
    fn test_admin_panel_tab_gating_and_pause_button() {
        use blockgame_core::admin_panel::{AdminPanelSnapshot, AdminTab};

        let mut pause_mesh = UiMesh::new();
        let pause_btn = UiBuilder::draw_pause_admin_button(&mut pause_mesh, 1280.0, 720.0, [640.0, 600.0]);
        assert!(pause_btn[2] > 0.0 && pause_btn[3] > 0.0);

        let snap = AdminPanelSnapshot {
            allowed_tabs: vec![AdminTab::Players, AdminTab::Chat],
            ..Default::default()
        };
        let mut mesh = UiMesh::new();
        let hbs = UiBuilder::draw_admin_panel(
            &mut mesh,
            1280.0,
            720.0,
            &snap,
            AdminTab::Players,
            "",
            None,
            None,
            None,
            false,
            0,
            [0.0, 0.0],
        );
        let tab_actions: Vec<AdminTab> = hbs
            .iter()
            .filter_map(|h| match h.action {
                AdminUiClickAction::SelectTab(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(tab_actions, vec![AdminTab::Players, AdminTab::Chat]);
    }
}
