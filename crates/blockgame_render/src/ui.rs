use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::atlas::TextureAtlas;
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
    pub invert_y: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    pub view_bobbing: bool,
    pub show_debug_hud: bool,
    pub ui_scale: f32,
    pub voice_mode: u8,
    pub voice_volume: f32,
    pub voice_input_device: String,
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
        let char_w = 10.0;
        let char_h = 14.0;
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

    /// Draws a Minecraft-style durability bar beneath damaged tools in inventory/hotbar slots.
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

            let col_sleeve_top = [0.0, 0.68, 0.70, 1.0];
            let col_sleeve_side = [0.0, 0.50, 0.52, 1.0];
            let col_skin_top = [0.88, 0.64, 0.48, 1.0];
            let col_skin_side = [0.72, 0.48, 0.35, 1.0];
            let col_skin_light = [0.93, 0.69, 0.53, 1.0];

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
                [col_skin_side; 4],
            );
            mesh.add_quad_vertices(
                [p_sleeve_m, p_sleeve_r, p_wrist_r, p_wrist_m],
                white_uv,
                [col_skin_top; 4],
            );

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

        let is_flat_plant_or_crop = match def.placeable_block {
            Some(b) => {
                b == blockgame_core::block::BlockId::SEAGRASS
                    || b == blockgame_core::block::BlockId::TALL_GRASS
                    || b == blockgame_core::block::BlockId::LILY_PAD
                    || b == blockgame_core::block::BlockId::WHEAT_0
                    || b == blockgame_core::block::BlockId::WHEAT_1
                    || b == blockgame_core::block::BlockId::WHEAT_2
                    || b == blockgame_core::block::BlockId::WHEAT_3
                    || b == blockgame_core::block::BlockId::OAK_DOOR_CLOSED
                    || b == blockgame_core::block::BlockId::BED
                    || b == blockgame_core::block::BlockId::OAK_FENCE
                    || b.is_stairs()
                    || b.is_flower()
            }
            None => false,
        };

        if let Some(block_id) = def.placeable_block.filter(|_| !is_flat_plant_or_crop) {
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
            } else {
                // -----------------------------------------------------------------
                // 3D HELD BLOCK: Prominent isometric 3D block with top and side shading
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
            } else {
                // -----------------------------------------------------------------
                // GENERIC / 2D ITEM: Clean sprite quad with NO bounding-box bevels
                // -----------------------------------------------------------------
                let item_punch_x = -40.0 * scale * punch_swing;
                let item_punch_y = 25.0 * scale * punch_swing;

                let size = 110.0 * scale;
                let ix = screen_w - size - 40.0 * scale + bob_x + item_punch_x;
                let iy = screen_h - size - 40.0 * scale + bob_y + item_punch_y;

                let [u0, v0, u1, v1] = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_uvs = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];

                let p0 = [ix, iy + size];
                let p1 = [ix + size, iy + size];
                let p2 = [ix + size, iy];
                let p3 = [ix, iy];

                // Front Face with clean item texture (transparent background, no borders)
                mesh.add_quad_vertices([p0, p1, p2, p3], icon_uvs, [[1.0, 1.0, 1.0, 1.0]; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(
                        mesh,
                        ix + size - 35.0 * scale,
                        iy + size - 25.0 * scale,
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
        mesh.clear();

        // 0. Full-screen Underwater Screen Overlay (When Player is Submerged)
        if stats.is_submerged && !stats.is_dead {
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
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
            Self::draw_held_item(
                mesh,
                screen_w,
                screen_h,
                held_slot,
                registry,
                walk_distance,
                punch_time,
            );
        }

        // 3. Hotbar Slots (9 slots, each 50x50 px with 5px gap)
        let slot_size = 50.0;
        let slot_gap = 5.0;
        let total_w = 9.0 * slot_size + 8.0 * slot_gap;
        let start_x = (screen_w - total_w) * 0.5;
        let start_y = screen_h - slot_size - 16.0;

        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        for i in 0..9 {
            let x = start_x + (i as f32) * (slot_size + slot_gap);
            let y = start_y;

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

        // 4. Player Health Hearts (10 hearts immediately above the left hotbar)
        let icon_size = 18.0;
        let icon_spacing = 17.0;
        let bars_y = start_y - 24.0;

        let heart_bg_uv = TextureAtlas::get_uv_bounds(Self::HEART_CONTAINER_TILE);
        let heart_full_uv = TextureAtlas::get_uv_bounds(Self::HEART_FULL_TILE);
        let heart_half_uv = TextureAtlas::get_uv_bounds(Self::HEART_HALF_TILE);

        for i in 0..10 {
            let hx = start_x + (i as f32) * icon_spacing;
            // Always draw heart container background
            mesh.add_quad(
                hx,
                bars_y,
                icon_size,
                icon_size,
                heart_bg_uv,
                [1.0, 1.0, 1.0, 1.0],
            );

            let hp_val = (i as f32 + 1.0) * 2.0;
            if stats.health >= hp_val {
                mesh.add_quad(
                    hx,
                    bars_y,
                    icon_size,
                    icon_size,
                    heart_full_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            } else if stats.health >= hp_val - 1.0 {
                mesh.add_quad(
                    hx,
                    bars_y,
                    icon_size,
                    icon_size,
                    heart_half_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }

        // 4b. Armor Points Bar (10 chestplate icons above health hearts when armor > 0)
        if armor_points > 0 {
            let armor_y = bars_y - icon_size - 4.0;
            let armor_uv = TextureAtlas::get_uv_bounds(Self::ARMOR_ICON_TILE);
            for i in 0..10u8 {
                let ax = start_x + (i as f32) * icon_spacing;
                let threshold = (i + 1) * 2;
                let tint = if armor_points >= threshold {
                    [1.0, 1.0, 1.0, 1.0]
                } else if armor_points >= threshold - 1 {
                    [0.65, 0.70, 0.78, 0.9]
                } else {
                    [0.2, 0.2, 0.25, 0.55]
                };
                mesh.add_quad(ax, armor_y, icon_size, icon_size, armor_uv, tint);
            }
        }

        // 5. Player Hunger Drumsticks (10 drumsticks immediately above the right hotbar)
        let hunger_bg_uv = TextureAtlas::get_uv_bounds(Self::HUNGER_CONTAINER_TILE);
        let hunger_full_uv = TextureAtlas::get_uv_bounds(Self::HUNGER_FULL_TILE);
        let hunger_half_uv = TextureAtlas::get_uv_bounds(Self::HUNGER_HALF_TILE);

        let hotbar_right = start_x + total_w;
        for i in 0..10 {
            let dx = hotbar_right - (i as f32 + 1.0) * icon_spacing;
            // Always draw drumstick container background
            mesh.add_quad(
                dx,
                bars_y,
                icon_size,
                icon_size,
                hunger_bg_uv,
                [1.0, 1.0, 1.0, 1.0],
            );

            let hunger_val = (i as f32 + 1.0) * 2.0;
            if stats.hunger >= hunger_val {
                mesh.add_quad(
                    dx,
                    bars_y,
                    icon_size,
                    icon_size,
                    hunger_full_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            } else if stats.hunger >= hunger_val - 1.0 {
                mesh.add_quad(
                    dx,
                    bars_y,
                    icon_size,
                    icon_size,
                    hunger_half_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }

        // 6. Oxygen / Breath Bubbles (10 bubbles directly above hunger bar when submerged or recovering breath)
        if (stats.is_submerged || stats.breath < stats.max_breath) && !stats.is_dead {
            let bubble_full_uv = TextureAtlas::get_uv_bounds(Self::BUBBLE_FULL_TILE);
            let bubble_pop_uv = TextureAtlas::get_uv_bounds(Self::BUBBLE_POP_TILE);
            let bubbles_y = bars_y - 20.0;

            let bubbles_remaining = ((stats.breath / stats.max_breath) * 10.0).ceil() as i32;
            for i in 0..10 {
                let bx = hotbar_right - (i as f32 + 1.0) * icon_spacing;
                if i < bubbles_remaining {
                    mesh.add_quad(
                        bx,
                        bubbles_y,
                        icon_size,
                        icon_size,
                        bubble_full_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                } else if i == bubbles_remaining && stats.drown_timer > 0.0 {
                    mesh.add_quad(
                        bx,
                        bubbles_y,
                        icon_size,
                        icon_size,
                        bubble_pop_uv,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                }
            }
        }

        // 7. Full-screen Red Damage Flash
        if stats.damage_flash > 0.0 {
            let flash_alpha = ((stats.damage_flash / 0.5) * 0.40).clamp(0.0, 0.40);
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
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
                            hovered_item_name = Some(def.display_name);
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
                    hovered_item_name = Some(def.display_name);
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
                            hovered_item_name = Some(def.display_name);
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
                        hovered_item_name = Some(def.display_name);
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
                        hovered_item_name = Some(def.display_name);
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

        // 6. Hover Tooltip (drawn on top of all slots and cursor items)
        if let Some(name) = hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], name);
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

    /// Draws a classic beveled voxel-styled button with drop-shadowed centered label.
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

        // Outer black frame
        mesh.add_quad(
            x - 2.0,
            y - 2.0,
            w + 4.0,
            h + 4.0,
            white_uv,
            [0.06, 0.06, 0.08, 0.96],
        );

        let (bg_color, top_highlight, bottom_shadow, text_color) = if !enabled {
            (
                [0.16, 0.16, 0.19, 0.92],
                [0.22, 0.22, 0.26, 0.92],
                [0.10, 0.10, 0.12, 0.92],
                [0.52, 0.52, 0.55, 0.95],
            )
        } else if hovered {
            (
                [0.34, 0.40, 0.62, 0.96],
                [0.56, 0.64, 0.90, 0.98],
                [0.20, 0.24, 0.42, 0.98],
                [1.0, 1.0, 0.55, 1.0],
            )
        } else {
            (
                [0.27, 0.28, 0.32, 0.95],
                [0.45, 0.46, 0.52, 0.95],
                [0.15, 0.16, 0.19, 0.95],
                [0.95, 0.95, 0.97, 1.0],
            )
        };

        // Main button fill
        mesh.add_quad(x, y, w, h, white_uv, bg_color);
        // Top/left 3D bevel highlight
        mesh.add_quad(x, y, w, 2.0, white_uv, top_highlight);
        mesh.add_quad(x, y, 2.0, h, white_uv, top_highlight);
        // Bottom/right 3D bevel shadow
        mesh.add_quad(x, y + h - 3.0, w, 3.0, white_uv, bottom_shadow);
        mesh.add_quad(x + w - 2.0, y, 2.0, h, white_uv, bottom_shadow);

        let char_w = 11.0;
        let char_h = 15.0;
        let spacing = 1.0;
        let text_w = Self::string_width(label, char_w, spacing);
        let tx = x + (w - text_w) * 0.5;
        let ty = y + (h - char_h) * 0.5;

        // Text drop shadow
        Self::draw_string(
            mesh,
            tx + 1.5,
            ty + 1.5,
            label,
            char_w,
            char_h,
            spacing,
            [0.08, 0.08, 0.10, 0.92],
        );
        Self::draw_string(mesh, tx, ty, label, char_w, char_h, spacing, text_color);
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

    /// Returns the 4 main button rectangles `[Singleplayer, Multiplayer, Settings, Quit Game]`
    /// on the Title Screen as `([x, y, w, h])`.
    pub fn title_screen_button_rects(screen_w: f32, screen_h: f32) -> [[f32; 4]; 4] {
        let btn_w = 340.0;
        let btn_h = 44.0;
        let gap = 14.0;
        let btn_x = (screen_w - btn_w) * 0.5;
        let start_y = screen_h * 0.44;
        [
            [btn_x, start_y, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap), btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 2.0, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 3.0 + 8.0, btn_w, btn_h],
        ]
    }

    /// Builds the Title Screen & Main Menu overlay.
    pub fn build_title_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Subtle atmospheric vignette / darkening over the 3D background panorama
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.04, 0.06, 0.10, 0.38],
        );

        // Title Banner Panel
        let title_text = "BLOCKGAME";
        let title_char_w = 28.0;
        let title_char_h = 38.0;
        let title_spacing = 4.0;
        let title_w = Self::string_width(title_text, title_char_w, title_spacing);
        let title_x = (screen_w - title_w) * 0.5;
        let title_y = screen_h * 0.17;

        // Deep 3D block shadow for title
        Self::draw_string(
            mesh,
            title_x + 4.0,
            title_y + 4.0,
            title_text,
            title_char_w,
            title_char_h,
            title_spacing,
            [0.08, 0.09, 0.12, 0.95],
        );
        Self::draw_string(
            mesh,
            title_x,
            title_y,
            title_text,
            title_char_w,
            title_char_h,
            title_spacing,
            [0.95, 0.96, 0.98, 1.0],
        );

        // Subtitle / Edition Banner
        let sub_text = "VOXEL SURVIVAL EDITION";
        let sub_w = Self::string_width(sub_text, 10.5, 2.0);
        let sub_x = (screen_w - sub_w) * 0.5;
        let sub_y = title_y + title_char_h + 12.0;
        Self::draw_string(
            mesh,
            sub_x + 1.5,
            sub_y + 1.5,
            sub_text,
            10.5,
            14.0,
            2.0,
            [0.08, 0.08, 0.10, 0.90],
        );
        Self::draw_string(
            mesh,
            sub_x,
            sub_y,
            sub_text,
            10.5,
            14.0,
            2.0,
            [1.0, 0.86, 0.28, 1.0],
        );

        let rects = Self::title_screen_button_rects(screen_w, screen_h);
        let labels = ["Singleplayer", "Multiplayer", "Settings", "Quit Game"];
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

        // Bottom footer version text
        let ver_text = "BlockGame v0.1.0 - Faithful 32x";
        Self::draw_string(
            mesh,
            12.0,
            screen_h - 22.0,
            ver_text,
            8.5,
            12.0,
            0.8,
            [0.80, 0.82, 0.86, 0.85],
        );
    }

    /// Returns the world list row rectangles (up to 5 visible slots) and the 4 bottom action buttons
    /// `[Play Selected World, Create New World, Delete World, Back]` for the World Select screen.
    pub fn world_select_layout(
        screen_w: f32,
        screen_h: f32,
        world_count: usize,
    ) -> (Vec<[f32; 4]>, [[f32; 4]; 4]) {
        let list_w = 560.0_f32.min(screen_w - 60.0);
        let list_x = (screen_w - list_w) * 0.5;
        let list_top = 100.0;
        let row_h = 58.0;
        let row_gap = 8.0;
        let max_rows = world_count.min(5);

        let mut rows = Vec::with_capacity(max_rows);
        for i in 0..max_rows {
            let ry = list_top + (i as f32) * (row_h + row_gap);
            rows.push([list_x, ry, list_w, row_h]);
        }

        let btn_w = 272.0;
        let btn_h = 40.0;
        let col_gap = 16.0;
        let row_btn_gap = 10.0;
        let left_x = (screen_w - (btn_w * 2.0 + col_gap)) * 0.5;
        let right_x = left_x + btn_w + col_gap;
        let top_btn_y = (screen_h - 115.0).max(list_top + 5.0 * (row_h + row_gap) + 16.0);
        let bot_btn_y = top_btn_y + btn_h + row_btn_gap;

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

        // Darkened menu backdrop
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.05, 0.06, 0.09, 0.78],
        );

        // Screen Header
        let header = "Select World";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - header_w) * 0.5,
            38.0,
            header,
            14.0,
            19.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        let list_w = 560.0_f32.min(screen_w - 60.0);
        let list_x = (screen_w - list_w) * 0.5;
        let list_top = 92.0;
        let list_h = 338.0;

        // Inset list container panel
        mesh.add_quad(
            list_x - 8.0,
            list_top,
            list_w + 16.0,
            list_h,
            white_uv,
            [0.02, 0.03, 0.05, 0.85],
        );

        let (row_rects, btn_rects) = Self::world_select_layout(screen_w, screen_h, worlds.len());

        if worlds.is_empty() {
            let empty_msg = "No saved worlds found. Click 'Create New World' to start!";
            let ew = Self::string_width(empty_msg, 9.5, 1.0);
            Self::draw_string(
                mesh,
                (screen_w - ew) * 0.5,
                list_top + list_h * 0.45,
                empty_msg,
                9.5,
                14.0,
                1.0,
                [0.68, 0.70, 0.76, 0.95],
            );
        } else {
            for (i, rect) in row_rects.iter().enumerate() {
                let world = &worlds[i];
                let is_selected = selected_index == Some(i);
                let is_hovered =
                    Self::is_rect_hovered(cursor_pos, rect[0], rect[1], rect[2], rect[3]);

                let border_color = if is_selected {
                    [0.95, 0.85, 0.35, 1.0]
                } else if is_hovered {
                    [0.48, 0.52, 0.65, 0.95]
                } else {
                    [0.22, 0.24, 0.30, 0.90]
                };

                let fill_color = if is_selected {
                    [0.16, 0.20, 0.32, 0.95]
                } else if is_hovered {
                    [0.12, 0.14, 0.22, 0.92]
                } else {
                    [0.08, 0.09, 0.13, 0.90]
                };

                mesh.add_quad(
                    rect[0] - 2.0,
                    rect[1] - 2.0,
                    rect[2] + 4.0,
                    rect[3] + 4.0,
                    white_uv,
                    border_color,
                );
                mesh.add_quad(rect[0], rect[1], rect[2], rect[3], white_uv, fill_color);

                // World Name
                Self::draw_string(
                    mesh,
                    rect[0] + 14.0,
                    rect[1] + 10.0,
                    &world.name,
                    11.0,
                    15.0,
                    1.0,
                    [0.98, 0.98, 1.0, 1.0],
                );

                // Folder & Seed details
                let play_mins = (world.total_time / 60.0) as u32;
                let play_secs = (world.total_time as u32) % 60;
                let details = format!(
                    "Folder: {}  |  Seed: {}  |  Played: {}m {}s",
                    world.folder_name, world.seed, play_mins, play_secs
                );
                Self::draw_string(
                    mesh,
                    rect[0] + 14.0,
                    rect[1] + 33.0,
                    &details,
                    8.5,
                    12.0,
                    0.8,
                    [0.65, 0.68, 0.75, 0.95],
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
            "Play Selected World",
            cursor_pos,
            has_selection,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[1][0],
            btn_rects[1][1],
            btn_rects[1][2],
            btn_rects[1][3],
            "Create New World",
            cursor_pos,
            true,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[2][0],
            btn_rects[2][1],
            btn_rects[2][2],
            btn_rects[2][3],
            "Delete World",
            cursor_pos,
            has_selection,
        );
        Self::draw_menu_button(
            mesh,
            btn_rects[3][0],
            btn_rects[3][1],
            btn_rects[3][2],
            btn_rects[3][3],
            "Back",
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
        let box_w = 420.0;
        let box_h = 42.0;
        let box_x = (screen_w - box_w) * 0.5;
        let name_y = screen_h * 0.30;
        let seed_y = name_y + 95.0;

        let btn_w = 202.0;
        let btn_h = 42.0;
        let gap = 16.0;
        let btn_y = seed_y + 90.0;
        let create_x = (screen_w - (btn_w * 2.0 + gap)) * 0.5;
        let cancel_x = create_x + btn_w + gap;

        (
            [box_x, name_y, box_w, box_h],
            [box_x, seed_y, box_w, box_h],
            [create_x, btn_y, btn_w, btn_h],
            [cancel_x, btn_y, btn_w, btn_h],
        )
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
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Darkened backdrop
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.05, 0.06, 0.09, 0.80],
        );

        let header = "Create New World";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - header_w) * 0.5,
            52.0,
            header,
            14.0,
            19.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        let (name_rect, seed_rect, create_rect, cancel_rect) =
            Self::create_world_layout(screen_w, screen_h);

        Self::draw_text_input_box(
            mesh,
            name_rect[0],
            name_rect[1],
            name_rect[2],
            name_rect[3],
            "World Name:",
            name_input,
            "New World",
            focused_field == 0,
            cursor_pos,
        );

        let folder_preview = format!(
            "Will be saved in: saves/{}",
            blockgame_core::save::sanitize_folder_name(if name_input.trim().is_empty() {
                "New World"
            } else {
                name_input
            })
        );
        Self::draw_string(
            mesh,
            name_rect[0],
            name_rect[1] + name_rect[3] + 8.0,
            &folder_preview,
            8.5,
            12.0,
            0.8,
            [0.60, 0.63, 0.70, 0.95],
        );

        Self::draw_text_input_box(
            mesh,
            seed_rect[0],
            seed_rect[1],
            seed_rect[2],
            seed_rect[3],
            "Seed for the World Generator (optional):",
            seed_input,
            "Leave blank for a random seed",
            focused_field == 1,
            cursor_pos,
        );

        Self::draw_menu_button(
            mesh,
            create_rect[0],
            create_rect[1],
            create_rect[2],
            create_rect[3],
            "Create New World",
            cursor_pos,
            !name_input.trim().is_empty(),
        );
        Self::draw_menu_button(
            mesh,
            cancel_rect[0],
            cancel_rect[1],
            cancel_rect[2],
            cancel_rect[3],
            "Cancel",
            cursor_pos,
            true,
        );
    }

    /// Returns the 4 Pause Menu button rectangles:
    /// `[Back to Game, Save World, Settings, Save & Quit to Title]`.
    pub fn pause_menu_button_rects(screen_w: f32, screen_h: f32) -> [[f32; 4]; 4] {
        let btn_w = 320.0;
        let btn_h = 42.0;
        let gap = 14.0;
        let btn_x = (screen_w - btn_w) * 0.5;
        let start_y = screen_h * 0.34;
        [
            [btn_x, start_y, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap), btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 2.0, btn_w, btn_h],
            [btn_x, start_y + (btn_h + gap) * 3.0 + 8.0, btn_w, btn_h],
        ]
    }

    /// Builds the In-Game Pause Menu overlay.
    pub fn build_pause_menu(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        cursor_pos: [f32; 2],
        status_message: Option<&str>,
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Translucent dark overlay over live 3D game world
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.04, 0.05, 0.08, 0.68],
        );

        let header = "Game Menu";
        let header_w = Self::string_width(header, 14.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - header_w) * 0.5,
            screen_h * 0.22,
            header,
            14.0,
            19.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        let rects = Self::pause_menu_button_rects(screen_w, screen_h);
        let labels = [
            "Back to Game",
            "Save World",
            "Settings",
            "Save & Quit to Title",
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

        if let Some(msg) = status_message {
            let mw = Self::string_width(msg, 10.0, 1.0);
            Self::draw_string(
                mesh,
                (screen_w - mw) * 0.5,
                rects[3][1] + rects[3][3] + 22.0,
                msg,
                10.0,
                14.0,
                1.0,
                [0.45, 0.95, 0.55, 1.0],
            );
        }
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
            "Done",
            cursor_pos,
            true,
        );
    }

    /// Returns the tab button rectangle `[x, y, w, h]` for tab `0` (General) or `1` (Controls).
    pub fn settings_tab_rect(screen_w: f32, screen_h: f32, tab_idx: usize) -> [f32; 4] {
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        let y = screen_h * 0.13;
        [start_x + (tab_idx as f32) * (col_w + gap_x), y, col_w, 34.0]
    }

    /// Returns the slider rectangle `[x, y, w, h]` for slider index `0..7`.
    pub fn settings_slider_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        let start_y = screen_h * 0.21;
        let row_h = 28.0;
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
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        let base_y = screen_h * 0.21 + 4.0 * (28.0 + 10.0) + 8.0;
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

    /// Returns the keybind button rectangle `[x, y, w, h]` for keybind index `0..13`.
    pub fn settings_keybind_rect(screen_w: f32, screen_h: f32, index: usize) -> [f32; 4] {
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        let start_y = screen_h * 0.21;
        let row_h = 28.0;
        let gap_y = 8.0;
        let col = (index / 7) as f32;
        let row = (index % 7) as f32;
        [
            start_x + col * (col_w + gap_x),
            start_y + row * (row_h + gap_y),
            col_w,
            row_h,
        ]
    }

    /// Returns `[x, y, w, h]` for the `Reset to Defaults` button on the Settings screen.
    pub fn settings_reset_button_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        [start_x, screen_h * 0.82, col_w, 42.0]
    }

    /// Returns `[x, y, w, h]` for the `Done` button on the Settings screen.
    pub fn settings_done_button_rect(screen_w: f32, screen_h: f32) -> [f32; 4] {
        let col_w = 270.0;
        let gap_x = 24.0;
        let total_w = col_w * 2.0 + gap_x;
        let start_x = (screen_w - total_w) * 0.5;
        [start_x + col_w + gap_x, screen_h * 0.82, col_w, 42.0]
    }

    /// Builds the interactive Settings screen with General and Controls tabs.
    pub fn build_settings_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        settings: SettingsUiState,
        settings_tab: usize,
        active_slider: Option<usize>,
        listening_keybind: Option<usize>,
        keybinds: &[(&str, &str)],
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
            [0.05, 0.06, 0.09, 0.82],
        );

        let title = "Settings";
        let tw = Self::string_width(title, 15.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - tw) * 0.5,
            screen_h * 0.05,
            title,
            15.0,
            20.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        for (tab_idx, tab_label) in ["General", "Controls"].iter().enumerate() {
            let r = Self::settings_tab_rect(screen_w, screen_h, tab_idx);
            let is_active = settings_tab == tab_idx;
            let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3]);
            let border = if is_active {
                [0.92, 0.82, 0.45, 1.0]
            } else if hovered {
                [0.70, 0.74, 0.82, 0.95]
            } else {
                [0.34, 0.37, 0.44, 0.88]
            };
            let bg = if is_active {
                [0.20, 0.36, 0.28, 0.96]
            } else {
                [0.12, 0.14, 0.19, 0.92]
            };
            mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
            mesh.add_quad(r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0, white_uv, bg);
            let lw = Self::string_width(tab_label, 10.5, 1.0);
            Self::draw_string(
                mesh,
                r[0] + (r[2] - lw) * 0.5,
                r[1] + (r[3] - 14.0) * 0.5,
                tab_label,
                10.5,
                14.0,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            );
        }

        if settings_tab == 0 {
            let sliders: [(String, f32); 7] = [
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
            ];

            for (idx, (label, norm)) in sliders.iter().enumerate() {
                let r = Self::settings_slider_rect(screen_w, screen_h, idx);
                let hovered = Self::is_rect_hovered(cursor_pos, r[0], r[1], r[2], r[3])
                    || active_slider == Some(idx);

                let border = if hovered {
                    [0.88, 0.78, 0.42, 0.95]
                } else {
                    [0.34, 0.37, 0.44, 0.90]
                };
                mesh.add_quad(r[0], r[1], r[2], r[3], white_uv, border);
                mesh.add_quad(
                    r[0] + 2.0,
                    r[1] + 2.0,
                    r[2] - 4.0,
                    r[3] - 4.0,
                    white_uv,
                    [0.10, 0.12, 0.16, 0.96],
                );

                let fill_w = ((r[2] - 4.0) * *norm).max(0.0);
                if fill_w > 0.0 {
                    mesh.add_quad(
                        r[0] + 2.0,
                        r[1] + 2.0,
                        fill_w,
                        r[3] - 4.0,
                        white_uv,
                        if hovered {
                            [0.24, 0.50, 0.34, 0.85]
                        } else {
                            [0.18, 0.38, 0.28, 0.80]
                        },
                    );
                }

                let thumb_w = 8.0;
                let thumb_x = (r[0] + 2.0 + (r[2] - 4.0 - thumb_w) * *norm)
                    .clamp(r[0] + 2.0, r[0] + r[2] - 2.0 - thumb_w);
                mesh.add_quad(
                    thumb_x,
                    r[1] + 2.0,
                    thumb_w,
                    r[3] - 4.0,
                    white_uv,
                    if hovered {
                        [0.95, 0.88, 0.55, 1.0]
                    } else {
                        [0.75, 0.78, 0.85, 0.95]
                    },
                );

                let lw = Self::string_width(label, 9.5, 1.0);
                Self::draw_string(
                    mesh,
                    r[0] + (r[2] - lw) * 0.5,
                    r[1] + (r[3] - 13.0) * 0.5,
                    label,
                    9.5,
                    13.0,
                    1.0,
                    [1.0, 1.0, 1.0, 1.0],
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
            ];

            for (idx, label) in toggles.iter().enumerate() {
                let r = Self::settings_toggle_rect(screen_w, screen_h, idx);
                Self::draw_menu_button(mesh, r[0], r[1], r[2], r[3], label, cursor_pos, true);
            }
        } else {
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
                        [0.95, 0.82, 0.35, 1.0],
                    );
                    mesh.add_quad(
                        r[0] + 2.0,
                        r[1] + 2.0,
                        r[2] - 4.0,
                        r[3] - 4.0,
                        white_uv,
                        [0.22, 0.20, 0.10, 0.96],
                    );
                    let lw = Self::string_width(&label, 9.5, 1.0);
                    Self::draw_string(
                        mesh,
                        r[0] + (r[2] - lw) * 0.5,
                        r[1] + (r[3] - 13.0) * 0.5,
                        &label,
                        9.5,
                        13.0,
                        1.0,
                        [1.0, 0.95, 0.65, 1.0],
                    );
                } else {
                    Self::draw_menu_button(
                        mesh, r[0], r[1], r[2], r[3], &label, cursor_pos, true,
                    );
                }
            }
        }

        let reset_r = Self::settings_reset_button_rect(screen_w, screen_h);
        Self::draw_menu_button(
            mesh,
            reset_r[0],
            reset_r[1],
            reset_r[2],
            reset_r[3],
            "Reset to Defaults",
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
            "Done",
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

    /// Returns layout rectangles for the Multiplayer Lobby screen:
    /// `(username_box, password_box, address_box, [skin_btn, join_btn, host_btn, back_btn])`.
    pub fn multiplayer_lobby_layout(
        screen_w: f32,
        screen_h: f32,
    ) -> ([f32; 4], [f32; 4], [f32; 4], [[f32; 4]; 4]) {
        let field_w = 380.0;
        let field_h = 36.0;
        let fx = (screen_w - field_w) * 0.5;

        let user_rect = [fx, screen_h * 0.23, field_w, field_h];
        let pass_rect = [fx, screen_h * 0.35, field_w, field_h];
        let addr_rect = [fx, screen_h * 0.47, field_w, field_h];

        let skin_btn = [fx, screen_h * 0.56, field_w, 36.0];
        let half_w = (field_w - 12.0) * 0.5;
        let join_btn = [fx, screen_h * 0.65, half_w, 40.0];
        let host_btn = [fx + half_w + 12.0, screen_h * 0.65, half_w, 40.0];
        let back_btn = [fx, screen_h * 0.74, field_w, 38.0];

        (
            user_rect,
            pass_rect,
            addr_rect,
            [skin_btn, join_btn, host_btn, back_btn],
        )
    }

    /// Builds the interactive Multiplayer Lobby Screen (Username, Password, Server Address, Skin Preview, Join & Host buttons).
    pub fn build_multiplayer_lobby_screen(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        username_input: &str,
        password_input: &str,
        address_input: &str,
        skin_id: u8,
        focused_field: usize,
        status_message: Option<(&str, bool)>,
        cursor_pos: [f32; 2],
    ) {
        mesh.clear();
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

        // Dark translucent backdrop
        mesh.add_quad(
            0.0,
            0.0,
            screen_w,
            screen_h,
            white_uv,
            [0.05, 0.06, 0.09, 0.82],
        );

        let header = "Multiplayer Server & Authentication";
        let header_w = Self::string_width(header, 13.0, 1.5);
        Self::draw_string(
            mesh,
            (screen_w - header_w) * 0.5,
            screen_h * 0.11,
            header,
            13.0,
            18.0,
            1.5,
            [1.0, 1.0, 1.0, 1.0],
        );

        let sub = "New usernames register automatically; returning usernames verify password";
        let sub_w = Self::string_width(sub, 8.0, 1.0);
        Self::draw_string(
            mesh,
            (screen_w - sub_w) * 0.5,
            screen_h * 0.155,
            sub,
            8.0,
            11.0,
            1.0,
            [0.72, 0.78, 0.88, 0.90],
        );

        let (user_r, pass_r, addr_r, btns) = Self::multiplayer_lobby_layout(screen_w, screen_h);

        Self::draw_text_input_box(
            mesh,
            user_r[0],
            user_r[1],
            user_r[2],
            user_r[3],
            "Username (3-16 chars, alphanumeric/_):",
            username_input,
            "Player1",
            focused_field == 0,
            cursor_pos,
        );

        let masked_pass: String = std::iter::repeat('*').take(password_input.len()).collect();
        Self::draw_text_input_box(
            mesh,
            pass_r[0],
            pass_r[1],
            pass_r[2],
            pass_r[3],
            "Account Password (salted SHA-256):",
            &masked_pass,
            "Leave blank or enter secret password",
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

        let skin_label = format!(
            "Character Skin: {}",
            blockgame_core::net::player_skin_name(skin_id)
        );
        Self::draw_menu_button(
            mesh,
            btns[0][0],
            btns[0][1],
            btns[0][2],
            btns[0][3],
            &skin_label,
            cursor_pos,
            true,
        );

        // Draw 32x32 Character Face Avatar preview on the left side of the Skin button
        let face_tile = 272 + ((skin_id as u16) % 4) * 4;
        let face_uv = TextureAtlas::get_uv_bounds(face_tile);
        mesh.add_quad(
            btns[0][0] + 8.0,
            btns[0][1] + 4.0,
            28.0,
            28.0,
            face_uv,
            [1.0, 1.0, 1.0, 1.0],
        );

        let can_connect =
            username_input.trim().len() >= 3 && !address_input.trim().is_empty();

        Self::draw_menu_button(
            mesh,
            btns[1][0],
            btns[1][1],
            btns[1][2],
            btns[1][3],
            "Join Server",
            cursor_pos,
            can_connect,
        );
        Self::draw_menu_button(
            mesh,
            btns[2][0],
            btns[2][1],
            btns[2][2],
            btns[2][3],
            "Host & Play",
            cursor_pos,
            username_input.trim().len() >= 3,
        );
        Self::draw_menu_button(
            mesh,
            btns[3][0],
            btns[3][1],
            btns[3][2],
            btns[3][3],
            "Back to Title",
            cursor_pos,
            true,
        );

        if let Some((msg, is_err)) = status_message {
            let mw = Self::string_width(msg, 9.0, 1.0);
            let color = if is_err {
                [1.0, 0.42, 0.42, 1.0]
            } else {
                [0.45, 0.95, 0.58, 1.0]
            };
            Self::draw_string(
                mesh,
                (screen_w - mw) * 0.5,
                screen_h * 0.84,
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

    /// Draws the in-game Multiplayer Chat Box (bottom-left) and the Tab Online Player List overlay.
    pub fn draw_multiplayer_hud_overlays(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        chat_history: &[blockgame_core::net::ChatEntry],
        chat_open: bool,
        chat_input: &str,
        tab_held: bool,
        online_players: &[blockgame_core::net::PlayerModelState],
        local_username: &str,
        is_hosting: bool,
        server_addr: &str,
    ) {
        let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);

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
        let visible_messages: Vec<&blockgame_core::net::ChatEntry> = chat_history
            .iter()
            .rev()
            .filter(|m| chat_open || m.age < 9.0)
            .take(8)
            .collect();

        let chat_x = 14.0;
        let chat_w = 420.0_f32.min(screen_w * 0.45);
        let mut msg_y = screen_h - 140.0;

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
            Self::draw_colored_string(
                mesh,
                chat_x + 6.0,
                msg_y + 4.0,
                &line,
                8.0,
                11.5,
                1.0,
                text_col,
            );
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
                let face_tile = 272 + ((p.skin_id as u16) % 4) * 4;
                let face_uv = TextureAtlas::get_uv_bounds(face_tile);
                mesh.add_quad(
                    px + 14.0,
                    ry + 3.0,
                    17.0,
                    17.0,
                    face_uv,
                    [1.0, 1.0, 1.0, 1.0],
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
    fn test_build_hud_held_seagrass_flat_item() {
        let registry = ItemRegistry::new();

        // 3D Block (Dirt) generates 3 isometric faces (18 indices)
        let mut mesh_block = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_block, 1280.0, 720.0, ItemSlot::new(ItemId::DIRT, 1), &registry, 0.0, 0.0);
        let block_quads = mesh_block.indices.len() / 6;
        assert_eq!(block_quads, 3, "Solid blocks should generate 3 isometric cube faces");

        // Plant item (Seagrass) generates 1 flat 2D sprite face (6 indices)
        let mut mesh_seagrass = UiMesh::new();
        UiBuilder::draw_held_item(&mut mesh_seagrass, 1280.0, 720.0, ItemSlot::new(ItemId::SEAGRASS, 1), &registry, 0.0, 0.0);
        let seagrass_quads = mesh_seagrass.indices.len() / 6;
        assert_eq!(seagrass_quads, 1, "Seagrass in hand should generate 1 flat 2D item sprite quad instead of a 3D cube");
    }

    #[test]
    fn test_build_title_screen() {
        let mut mesh = UiMesh::new();
        UiBuilder::build_title_screen(&mut mesh, 1280.0, 720.0, [640.0, 360.0]);
        assert!(mesh.indices.len() > 0);
        assert!(mesh.vertices.len() > 0);

        let rects = UiBuilder::title_screen_button_rects(1280.0, 720.0);
        assert_eq!(rects.len(), 4);
        // Hover check inside first button (Singleplayer)
        let r0 = rects[0];
        assert!(UiBuilder::is_rect_hovered(
            [r0[0] + 10.0, r0[1] + 10.0],
            r0[0],
            r0[1],
            r0[2],
            r0[3]
        ));
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
        UiBuilder::build_multiplayer_lobby_screen(
            &mut mesh,
            1280.0,
            720.0,
            "Alice_99",
            "secret123",
            "127.0.0.1:25565",
            1,
            0,
            Some(("Ready to connect", false)),
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
            2,
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
        UiBuilder::draw_multiplayer_hud_overlays(
            &mut tag_mesh,
            1280.0,
            720.0,
            &chat,
            true,
            "Hi Bob",
            true,
            &[p],
            "Alice_99",
            true,
            "127.0.0.1:25565",
        );
        assert!(tag_mesh.indices.len() > 50);
    }
}
