use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::atlas::TextureAtlas;
use blockgame_core::{
    crafting::CraftingGrid,
    item::{Inventory, ItemId, ItemRegistry, ItemSlot},
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
            base_idx, base_idx + 1, base_idx + 2,
            base_idx, base_idx + 2, base_idx + 3,
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

    /// Draws a single character glyph using the pixel font in the atlas.
    pub fn draw_char(mesh: &mut UiMesh, x: f32, y: f32, w: f32, h: f32, c: u8, color: [f32; 4]) {
        if c == b' ' {
            return;
        }
        let uv = TextureAtlas::get_uv_bounds(c as u16);
        mesh.add_quad(x, y, w, h, uv, color);
    }

    /// Draws an ASCII string using the pixel font.
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
            Self::draw_char(mesh, x, y, char_w, char_h, b, color);
            x += char_w + spacing;
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
        let char_w = 9.0;
        let char_h = 12.0;
        let spacing = -1.0;
        let total_w = text.len() as f32 * (char_w + spacing);
        let x = slot_x + slot_size - total_w - 3.0;
        let y = slot_y + slot_size - char_h - 2.0;
        Self::draw_string(mesh, x, y, &text, char_w, char_h, spacing, [1.0, 1.0, 1.0, 1.0]);
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
        let char_w = 8.0;
        let char_h = 11.0;
        let spacing = 0.0;
        let text_w = text.len() as f32 * (char_w + spacing);
        let box_w = text_w + 14.0;
        let box_h = 22.0;

        let tx = (cursor_x + 12.0).clamp(4.0, (screen_w - box_w - 4.0).max(4.0));
        let ty = (cursor_y - 22.0).clamp(4.0, (screen_h - box_h - 4.0).max(4.0));

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
        mesh.add_quad(
            tx,
            ty,
            box_w,
            box_h,
            white_uv,
            [0.08, 0.04, 0.16, 0.94],
        );

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

            let p_arm_bot_r = [screen_w + 40.0 * scale + bob_x + punch_x * 0.3, screen_h + 30.0 * scale];
            let p_arm_bot_l = [screen_w - 95.0 * scale + bob_x + punch_x * 0.3, screen_h + 30.0 * scale];
            let p_arm_bot_m = [(p_arm_bot_l[0] + p_arm_bot_r[0]) * 0.5, (p_arm_bot_l[1] + p_arm_bot_r[1]) * 0.5];

            let p_sleeve_r = [screen_w - 20.0 * scale + bob_x + punch_x * 0.6, screen_h - 130.0 * scale + bob_y + punch_y * 0.6];
            let p_sleeve_l = [screen_w - 145.0 * scale + bob_x + punch_x * 0.6, screen_h - 85.0 * scale + bob_y + punch_y * 0.6];
            let p_sleeve_m = [(p_sleeve_l[0] + p_sleeve_r[0]) * 0.5, (p_sleeve_l[1] + p_sleeve_r[1]) * 0.5];

            let p_wrist_r = [screen_w - 75.0 * scale + bob_x + punch_x, screen_h - 195.0 * scale + bob_y + punch_y];
            let p_wrist_l = [screen_w - 180.0 * scale + bob_x + punch_x, screen_h - 150.0 * scale + bob_y + punch_y];
            let p_wrist_m = [(p_wrist_l[0] + p_wrist_r[0]) * 0.5, (p_wrist_l[1] + p_wrist_r[1]) * 0.5];

            let p_hand = [screen_w - 155.0 * scale + bob_x + punch_x * 1.1, screen_h - 200.0 * scale + bob_y + punch_y * 1.1];

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
            let p_fist_tip_l = [p_hand[0] - 25.0 * scale + punch_x * 0.2, p_hand[1] - 25.0 * scale + punch_y * 0.2];
            let p_fist_tip_r = [p_hand[0] + 40.0 * scale + punch_x * 0.2, p_hand[1] - 45.0 * scale + punch_y * 0.2];
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
                let v_side_top = v_min + v_h * (1.0 / 16.0);
                let v_side_bottom = v_min + v_h * (16.0 / 16.0);

                let torch_uvs = [
                    [u_side_left, v_side_bottom],
                    [u_side_right, v_side_bottom],
                    [u_side_right, v_side_top],
                    [u_side_left, v_side_top],
                ];

                // 1. Shaft Front Face
                mesh.add_quad_vertices(
                    [p0, p1, p2, p3],
                    torch_uvs,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

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

                let top_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosY);
                let side_tile = block_registry.get_texture(block_id, blockgame_core::direction::Direction::PosZ);

                let [u0_t, v0_t, u1_t, v1_t] = TextureAtlas::get_uv_bounds(top_tile);
                let [u0_s, v0_s, u1_s, v1_s] = TextureAtlas::get_uv_bounds(side_tile);

                // Top Face (diamond)
                let p_top_n = [bc[0], bc[1] - bs * 0.65];
                let p_top_e = [bc[0] + bs * 1.05, bc[1] - bs * 0.16];
                let p_top_s = [bc[0], bc[1] + bs * 0.35];
                let p_top_w = [bc[0] - bs * 1.05, bc[1] - bs * 0.16];

                let uvs_top = [
                    [u0_t, v0_t],
                    [u1_t, v0_t],
                    [u1_t, v1_t],
                    [u0_t, v1_t],
                ];
                mesh.add_quad_vertices(
                    [p_top_w, p_top_n, p_top_e, p_top_s],
                    uvs_top,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

                // Left Face (parallelogram facing front-left)
                let h_face = bs * 1.10;
                let p_left_bot_w = [p_top_w[0], p_top_w[1] + h_face];
                let p_left_bot_e = [p_top_s[0], p_top_s[1] + h_face];
                let uvs_left = [
                    [u0_s, v0_s],
                    [u1_s, v0_s],
                    [u1_s, v1_s],
                    [u0_s, v1_s],
                ];
                mesh.add_quad_vertices(
                    [p_top_w, p_top_s, p_left_bot_e, p_left_bot_w],
                    uvs_left,
                    [[0.82, 0.82, 0.82, 1.0]; 4],
                );

                // Right Face (parallelogram facing front-right)
                let p_right_bot_w = p_left_bot_e;
                let p_right_bot_e = [p_top_e[0], p_top_e[1] + h_face];
                let uvs_right = [
                    [u0_s, v0_s],
                    [u1_s, v0_s],
                    [u1_s, v1_s],
                    [u0_s, v1_s],
                ];
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
                || active_slot.item_id == ItemId::IRON_PICKAXE
                || active_slot.item_id == ItemId::DIAMOND_PICKAXE;

            let is_hoe = active_slot.item_id == ItemId::WOODEN_HOE
                || active_slot.item_id == ItemId::STONE_HOE
                || active_slot.item_id == ItemId::IRON_HOE
                || active_slot.item_id == ItemId::DIAMOND_HOE;

            let is_axe = active_slot.item_id == ItemId::WOODEN_AXE
                || active_slot.item_id == ItemId::STONE_AXE
                || active_slot.item_id == ItemId::IRON_AXE
                || active_slot.item_id == ItemId::DIAMOND_AXE;

            let is_shovel = active_slot.item_id == ItemId::WOODEN_SHOVEL
                || active_slot.item_id == ItemId::STONE_SHOVEL
                || active_slot.item_id == ItemId::IRON_SHOVEL
                || active_slot.item_id == ItemId::DIAMOND_SHOVEL;

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
                let h0 = [handle_base[0] - norm[0] * half_w, handle_base[1] - norm[1] * half_w];
                let h1 = [handle_base[0] + norm[0] * half_w, handle_base[1] + norm[1] * half_w];
                let h2 = [handle_tip[0] + norm[0] * half_w, handle_tip[1] + norm[1] * half_w];
                let h3 = [handle_tip[0] - norm[0] * half_w, handle_tip[1] - norm[1] * half_w];
                mesh.add_quad_vertices(
                    [h0, h1, h2, h3],
                    white_uv,
                    [[0.60, 0.42, 0.24, 1.0]; 4],
                );

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
                    ItemId::DIAMOND_PICKAXE => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => ( // Iron Pickaxe
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
                let tip_l_in = [tip_l[0] - dir[0] * 8.0 * scale, tip_l[1] - dir[1] * 8.0 * scale];
                mesh.add_quad_vertices(
                    [head_bot, tip_l_in, tip_l, head_top],
                    white_uv,
                    [col_head_main; 4],
                );

                // Right Blade Quad
                let tip_r_in = [tip_r[0] - dir[0] * 8.0 * scale, tip_r[1] - dir[1] * 8.0 * scale];
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
                let c0 = [head_center[0] - norm[0] * 14.0 * scale - dir[0] * 12.0 * scale, head_center[1] - norm[1] * 14.0 * scale - dir[1] * 12.0 * scale];
                let c1 = [head_center[0] + norm[0] * 14.0 * scale - dir[0] * 12.0 * scale, head_center[1] + norm[0] * 14.0 * scale - dir[1] * 12.0 * scale];
                let c2 = [head_center[0] + norm[0] * 14.0 * scale + dir[0] * 12.0 * scale, head_center[1] + norm[0] * 14.0 * scale + dir[1] * 12.0 * scale];
                let c3 = [head_center[0] - norm[0] * 14.0 * scale + dir[0] * 12.0 * scale, head_center[1] - norm[0] * 14.0 * scale + dir[1] * 12.0 * scale];
                mesh.add_quad_vertices(
                    [c0, c1, c2, c3],
                    white_uv,
                    [[0.30, 0.30, 0.34, 1.0]; 4],
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
                let h0 = [handle_base[0] - norm[0] * half_w, handle_base[1] - norm[1] * half_w];
                let h1 = [handle_base[0] + norm[0] * half_w, handle_base[1] + norm[1] * half_w];
                let h2 = [handle_tip[0] + norm[0] * half_w, handle_tip[1] + norm[1] * half_w];
                let h3 = [handle_tip[0] - norm[0] * half_w, handle_tip[1] - norm[1] * half_w];
                mesh.add_quad_vertices(
                    [h0, h1, h2, h3],
                    white_uv,
                    [[0.60, 0.42, 0.24, 1.0]; 4],
                );

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
                    ItemId::DIAMOND_HOE => (
                        [0.25, 0.88, 0.92, 1.0],
                        [0.55, 1.00, 1.00, 1.0],
                        [0.15, 0.60, 0.65, 1.0],
                    ),
                    _ => ( // Iron Hoe
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
                let tip_l_in = [tip_l[0] - dir[0] * 8.0 * scale, tip_l[1] - dir[1] * 8.0 * scale];

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

                let h0 = [handle_base[0] - norm[0] * half_w, handle_base[1] - norm[1] * half_w];
                let h1 = [handle_base[0] + norm[0] * half_w, handle_base[1] + norm[1] * half_w];
                let h2 = [handle_tip[0] + norm[0] * half_w, handle_tip[1] + norm[1] * half_w];
                let h3 = [handle_tip[0] - norm[0] * half_w, handle_tip[1] - norm[1] * half_w];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices([h0_b, h0, h3, h3_b], white_uv, [[0.38, 0.25, 0.13, 1.0]; 4]);

                let (col_head_main, col_head_top, _col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_AXE => ([0.68, 0.48, 0.28, 1.0], [0.85, 0.65, 0.42, 1.0], [0.45, 0.30, 0.16, 1.0]),
                    ItemId::STONE_AXE => ([0.55, 0.55, 0.58, 1.0], [0.75, 0.75, 0.78, 1.0], [0.35, 0.35, 0.38, 1.0]),
                    ItemId::DIAMOND_AXE => ([0.25, 0.88, 0.92, 1.0], [0.55, 1.00, 1.00, 1.0], [0.15, 0.60, 0.65, 1.0]),
                    _ => ([0.82, 0.85, 0.90, 1.0], [0.96, 0.98, 1.00, 1.0], [0.55, 0.58, 0.65, 1.0]),
                };

                let head_center = handle_tip;
                let blade_len = 50.0 * scale;
                let p_top_tip = [head_center[0] - norm[0] * blade_len + dir[0] * 20.0 * scale, head_center[1] - norm[1] * blade_len + dir[1] * 20.0 * scale];
                let p_bot_tip = [head_center[0] - norm[0] * (blade_len - 10.0 * scale) - dir[0] * 35.0 * scale, head_center[1] - norm[1] * (blade_len - 10.0 * scale) - dir[1] * 35.0 * scale];
                let p_top_back = [head_center[0] + norm[0] * 15.0 * scale + dir[0] * 12.0 * scale, head_center[1] + norm[0] * 15.0 * scale + dir[1] * 12.0 * scale];
                let p_bot_back = [head_center[0] + norm[0] * 15.0 * scale - dir[0] * 20.0 * scale, head_center[1] + norm[0] * 15.0 * scale - dir[1] * 20.0 * scale];

                mesh.add_quad_vertices([p_bot_tip, p_bot_back, p_top_back, p_top_tip], white_uv, [col_head_main; 4]);
                let p_top_tip_b = [p_top_tip[0] + thick[0], p_top_tip[1] + thick[1]];
                let p_top_back_b = [p_top_back[0] + thick[0], p_top_back[1] + thick[1]];
                mesh.add_quad_vertices([p_top_tip, p_top_back, p_top_back_b, p_top_tip_b], white_uv, [col_head_top; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(mesh, screen_w - 75.0 * scale, screen_h - 45.0 * scale, 30.0 * scale, active_slot.count);
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

                let h0 = [handle_base[0] - norm[0] * half_w, handle_base[1] - norm[1] * half_w];
                let h1 = [handle_base[0] + norm[0] * half_w, handle_base[1] + norm[1] * half_w];
                let h2 = [handle_tip[0] + norm[0] * half_w, handle_tip[1] + norm[1] * half_w];
                let h3 = [handle_tip[0] - norm[0] * half_w, handle_tip[1] - norm[1] * half_w];
                mesh.add_quad_vertices([h0, h1, h2, h3], white_uv, [[0.60, 0.42, 0.24, 1.0]; 4]);
                let h0_b = [h0[0] + thick[0], h0[1] + thick[1]];
                let h3_b = [h3[0] + thick[0], h3[1] + thick[1]];
                mesh.add_quad_vertices([h0_b, h0, h3, h3_b], white_uv, [[0.38, 0.25, 0.13, 1.0]; 4]);

                let (col_head_main, col_head_top, _col_head_side) = match active_slot.item_id {
                    ItemId::WOODEN_SHOVEL => ([0.68, 0.48, 0.28, 1.0], [0.85, 0.65, 0.42, 1.0], [0.45, 0.30, 0.16, 1.0]),
                    ItemId::STONE_SHOVEL => ([0.55, 0.55, 0.58, 1.0], [0.75, 0.75, 0.78, 1.0], [0.35, 0.35, 0.38, 1.0]),
                    ItemId::DIAMOND_SHOVEL => ([0.25, 0.88, 0.92, 1.0], [0.55, 1.00, 1.00, 1.0], [0.15, 0.60, 0.65, 1.0]),
                    _ => ([0.82, 0.85, 0.90, 1.0], [0.96, 0.98, 1.00, 1.0], [0.55, 0.58, 0.65, 1.0]),
                };

                let blade_w = 20.0 * scale;
                let blade_len = 50.0 * scale;
                let b_base_l = [handle_tip[0] - norm[0] * blade_w, handle_tip[1] - norm[1] * blade_w];
                let b_base_r = [handle_tip[0] + norm[0] * blade_w, handle_tip[1] + norm[1] * blade_w];
                let b_tip_l = [b_base_l[0] + dir[0] * blade_len, b_base_l[1] + dir[1] * blade_len];
                let b_tip_r = [b_base_r[0] + dir[0] * blade_len, b_base_r[1] + dir[1] * blade_len];

                mesh.add_quad_vertices([b_base_l, b_base_r, b_tip_r, b_tip_l], white_uv, [col_head_main; 4]);
                let b_tip_l_b = [b_tip_l[0] + thick[0], b_tip_l[1] + thick[1]];
                let b_tip_r_b = [b_tip_r[0] + thick[0], b_tip_r[1] + thick[1]];
                mesh.add_quad_vertices([b_tip_l, b_tip_r, b_tip_r_b, b_tip_l_b], white_uv, [col_head_top; 4]);

                if active_slot.count > 1 {
                    Self::draw_stack_number(mesh, screen_w - 75.0 * scale, screen_h - 45.0 * scale, 30.0 * scale, active_slot.count);
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
                mesh.add_quad_vertices(
                    [s0, s1, s2, s3],
                    white_uv,
                    [[0.62, 0.44, 0.25, 1.0]; 4],
                );

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
                    _ => ( // Iron Ingot
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
                mesh.add_quad_vertices(
                    [t_sw, t_nw, t_ne, t_se],
                    white_uv,
                    [col_top; 4],
                );

                // Front Face
                let f_bl = [t_sw[0], t_sw[1] + ih];
                let f_br = [t_se[0], t_se[1] + ih];
                mesh.add_quad_vertices(
                    [t_sw, t_se, f_br, f_bl],
                    white_uv,
                    [col_front; 4],
                );

                // Right Face
                let r_br = [t_ne[0], t_ne[1] + ih];
                mesh.add_quad_vertices(
                    [t_se, t_ne, r_br, f_br],
                    white_uv,
                    [col_side; 4],
                );

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
                    _ => ( // Coal
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
                mesh.add_quad_vertices(
                    [c_tl, c_top, c_tr, c_mid],
                    white_uv,
                    [col_f1; 4],
                );

                // Facet 2: Front-left facet
                let c_bl = [cc[0] - cs * 0.60, cc[1] + cs * 0.75];
                let c_bm = [cc[0], cc[1] + cs * 0.95];
                mesh.add_quad_vertices(
                    [c_tl, c_mid, c_bm, c_bl],
                    white_uv,
                    [col_f2; 4],
                );

                // Facet 3: Front-right facet
                let c_br = [cc[0] + cs * 0.70, cc[1] + cs * 0.65];
                mesh.add_quad_vertices(
                    [c_mid, c_tr, c_br, c_bm],
                    white_uv,
                    [col_f3; 4],
                );

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
                let icon_uvs = [
                    [u0, v1],
                    [u1, v1],
                    [u1, v0],
                    [u0, v0],
                ];

                let p0 = [ix, iy + size];
                let p1 = [ix + size, iy + size];
                let p2 = [ix + size, iy];
                let p3 = [ix, iy];

                // Front Face with clean item texture (transparent background, no borders)
                mesh.add_quad_vertices(
                    [p0, p1, p2, p3],
                    icon_uvs,
                    [[1.0, 1.0, 1.0, 1.0]; 4],
                );

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
        mesh.clear();

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
            Self::draw_held_item(mesh, screen_w, screen_h, held_slot, registry, walk_distance, punch_time);
        }

        // 3. Hotbar Slots (9 slots, each 44x44 px with 4px gap)
        let slot_size = 44.0;
        let slot_gap = 4.0;
        let total_w = 9.0 * slot_size + 8.0 * slot_gap;
        let start_x = (screen_w - total_w) * 0.5;
        let start_y = screen_h - slot_size - 14.0;

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
                    let icon_pad = 7.0;
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
                }
            }
        }

        // 4. Player Health Hearts (10 hearts immediately above the left hotbar)
        let icon_size = 16.0;
        let icon_spacing = 15.0;
        let bars_y = start_y - 20.0;

        let heart_bg_uv = TextureAtlas::get_uv_bounds(Self::HEART_CONTAINER_TILE);
        let heart_full_uv = TextureAtlas::get_uv_bounds(Self::HEART_FULL_TILE);
        let heart_half_uv = TextureAtlas::get_uv_bounds(Self::HEART_HALF_TILE);

        for i in 0..10 {
            let hx = start_x + (i as f32) * icon_spacing;
            // Always draw heart container background
            mesh.add_quad(hx, bars_y, icon_size, icon_size, heart_bg_uv, [1.0, 1.0, 1.0, 1.0]);

            let hp_val = (i as f32 + 1.0) * 2.0;
            if stats.health >= hp_val {
                mesh.add_quad(hx, bars_y, icon_size, icon_size, heart_full_uv, [1.0, 1.0, 1.0, 1.0]);
            } else if stats.health >= hp_val - 1.0 {
                mesh.add_quad(hx, bars_y, icon_size, icon_size, heart_half_uv, [1.0, 1.0, 1.0, 1.0]);
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
            mesh.add_quad(dx, bars_y, icon_size, icon_size, hunger_bg_uv, [1.0, 1.0, 1.0, 1.0]);

            let hunger_val = (i as f32 + 1.0) * 2.0;
            if stats.hunger >= hunger_val {
                mesh.add_quad(dx, bars_y, icon_size, icon_size, hunger_full_uv, [1.0, 1.0, 1.0, 1.0]);
            } else if stats.hunger >= hunger_val - 1.0 {
                mesh.add_quad(dx, bars_y, icon_size, icon_size, hunger_half_uv, [1.0, 1.0, 1.0, 1.0]);
            }
        }

        // 6. Full-screen Red Damage Flash
        if stats.damage_flash > 0.0 {
            let flash_alpha = ((stats.damage_flash / 0.5) * 0.40).clamp(0.0, 0.40);
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
            mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.85, 0.08, 0.08, flash_alpha]);
        }

        // 7. Full-screen Death Screen Overlay (When Player is Dead)
        if stats.is_dead {
            let white_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
            // Deep red backdrop
            mesh.add_quad(0.0, 0.0, screen_w, screen_h, white_uv, [0.55, 0.06, 0.06, 0.76]);

            // Big "You Died!" Title
            let title = "You Died!";
            let title_w = title.len() as f32 * 20.0;
            let title_x = (screen_w - title_w) * 0.5;
            let title_y = screen_h * 0.32;
            Self::draw_string(mesh, title_x, title_y, title, 16.0, 22.0, 4.0, [1.0, 0.95, 0.95, 1.0]);

            // Subtitle
            let subtitle = "Score: 0";
            let sub_w = subtitle.len() as f32 * 10.0;
            let sub_x = (screen_w - sub_w) * 0.5;
            Self::draw_string(mesh, sub_x, title_y + 36.0, subtitle, 9.0, 13.0, 1.0, [0.85, 0.85, 0.85, 0.95]);

            // "Respawn" Button
            let btn_w = 200.0;
            let btn_h = 44.0;
            let btn_x = (screen_w - btn_w) * 0.5;
            let btn_y = screen_h * 0.50;

            let border_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);
            mesh.add_quad(btn_x - 2.0, btn_y - 2.0, btn_w + 4.0, btn_h + 4.0, border_uv, [1.0, 1.0, 1.0, 1.0]);
            mesh.add_quad(btn_x, btn_y, btn_w, btn_h, white_uv, [0.22, 0.22, 0.24, 0.95]);

            let respawn_text = "Respawn";
            let respawn_w = respawn_text.len() as f32 * 11.0;
            let text_x = btn_x + (btn_w - respawn_w) * 0.5;
            let text_y = btn_y + 14.0;
            Self::draw_string(mesh, text_x, text_y, respawn_text, 10.0, 15.0, 1.0, [1.0, 1.0, 1.0, 1.0]);
        }
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
    ) {
        mesh.clear();

        let mut hovered_item_name: Option<&str> = None;

        // 1. Semi-transparent backdrop overlay
        let backdrop_uv = TextureAtlas::get_uv_bounds(Self::WHITE_TILE);
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, backdrop_uv, [0.0, 0.0, 0.0, 0.65]);

        let slot_size = 40.0;
        let slot_gap = 4.0;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        // 2. Dynamic Crafting Grid (2x2 Player Crafting or 3x3 Crafting Table) + Result Slot
        let grid_w = crafting_grid.width;
        let grid_h = crafting_grid.height;

        let craft_x = (screen_w - ((grid_w as f32 + 2.0) * (slot_size + slot_gap) + 20.0)) * 0.5;
        let craft_y = screen_h * 0.5 - (if grid_h == 3 { 185.0 } else { 160.0 });

        // Header text above crafting grid
        let title = if grid_w == 3 { "Crafting Table (3x3)" } else { "Crafting (2x2)" };
        Self::draw_string(mesh, craft_x, craft_y - 18.0, title, 8.0, 10.0, 1.0, [0.95, 0.95, 0.95, 1.0]);

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
        let result_x = craft_x + (grid_w as f32) * (slot_size + slot_gap) + 28.0;
        let result_y = craft_y + ((grid_h as f32 - 1.0) * (slot_size + slot_gap) * 0.5);
        mesh.add_quad(
            result_x,
            result_y,
            slot_size + 6.0,
            slot_size + 6.0,
            slot_uv,
            [1.0, 0.9, 0.6, 1.0],
        );
        if let Some(res) = crafting_result {
            if let Some(def) = registry.get(res.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                let r_size = slot_size + 6.0;
                mesh.add_quad(
                    result_x + icon_pad,
                    result_y + icon_pad,
                    r_size - icon_pad * 2.0,
                    r_size - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, result_x, result_y, r_size, res.count);

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
        let inv_w = 9.0 * (slot_size + slot_gap);
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

        // 5. Cursor Held Item Stack (follows mouse position)
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                let hx = cursor_pos[0] - hold_size * 0.5;
                let hy = cursor_pos[1] - hold_size * 0.5;
                mesh.add_quad(
                    hx,
                    hy,
                    hold_size,
                    hold_size,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
                Self::draw_stack_number(mesh, hx, hy, hold_size, cursor_held.count);
            }
        }

        // 6. Hover Tooltip (drawn on top of all slots and cursor items)
        if let Some(name) = hovered_item_name {
            Self::draw_tooltip(mesh, screen_w, screen_h, cursor_pos[0], cursor_pos[1], name);
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

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_empty_hand() {
        let mut mesh = UiMesh::new();
        let inv = Inventory::new(36);
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        // Empty hand draws the player's arm and fist
        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 1.5, 0.2, &stats);
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_block() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::WOOD_LOG, 16));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_pickaxe() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::IRON_PICKAXE, 1));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.3, &stats);
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_stick() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::STICK, 4));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        assert!(mesh.indices.len() > 0);
    }

    #[test]
    fn test_build_hud_held_3d_ingot_and_coal() {
        let mut mesh = UiMesh::new();
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::new(ItemId::IRON_INGOT, 5));
        let registry = ItemRegistry::new();
        let stats = PlayerStats::new();

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        assert!(mesh.indices.len() > 0);

        let mut mesh_coal = UiMesh::new();
        inv.set(0, ItemSlot::new(ItemId::COAL, 3));
        UiBuilder::build_hud(&mut mesh_coal, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
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

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
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

        UiBuilder::build_hud(&mut mesh, 1280.0, 720.0, &inv, 0, &registry, 0.0, 0.0, &stats);
        // Generates death screen quads (red backdrop, You Died text, Score text, Respawn button)
        assert!(mesh.indices.len() > 0);
    }
}
