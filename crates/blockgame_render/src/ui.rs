use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::atlas::TextureAtlas;
use blockgame_core::{
    crafting::CraftingGrid,
    item::{Inventory, ItemRegistry, ItemSlot},
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

    /// Builds the standard gameplay HUD: center crosshair and 9-slot bottom hotbar.
    pub fn build_hud(
        mesh: &mut UiMesh,
        screen_w: f32,
        screen_h: f32,
        inventory: &Inventory,
        active_slot: usize,
        registry: &ItemRegistry,
    ) {
        mesh.clear();

        // 1. Center Crosshair (24x24 px)
        let ch_size = 24.0;
        let ch_x = (screen_w - ch_size) * 0.5;
        let ch_y = (screen_h - ch_size) * 0.5;
        let ch_uv = TextureAtlas::get_uv_bounds(Self::CROSSHAIR_TILE);
        mesh.add_quad(ch_x, ch_y, ch_size, ch_size, ch_uv, [1.0, 1.0, 1.0, 1.0]);

        // 2. Hotbar Slots (9 slots, each 44x44 px with 4px gap)
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
            if i == active_slot {
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

            // Item Icon
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
                }
            }
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

        // 1. Semi-transparent backdrop overlay
        let full_uv = [0.0, 0.0, 1.0, 1.0];
        mesh.add_quad(0.0, 0.0, screen_w, screen_h, full_uv, [0.0, 0.0, 0.0, 0.55]);

        let slot_size = 40.0;
        let slot_gap = 4.0;
        let slot_uv = TextureAtlas::get_uv_bounds(Self::SLOT_BG_TILE);
        let highlight_uv = TextureAtlas::get_uv_bounds(Self::SLOT_HIGHLIGHT_TILE);

        // 2. Crafting 2x2 Grid + Result Slot (Top center)
        let craft_x = (screen_w - (4.0 * (slot_size + slot_gap) + 40.0)) * 0.5;
        let craft_y = screen_h * 0.5 - 160.0;

        for cy in 0..2 {
            for cx in 0..2 {
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
                    }
                }
            }
        }

        // Crafting Result Slot (separated by gap)
        let result_x = craft_x + 2.0 * (slot_size + slot_gap) + 32.0;
        let result_y = craft_y + (slot_size * 0.5);
        mesh.add_quad(result_x, result_y, slot_size + 6.0, slot_size + 6.0, slot_uv, [1.0, 0.9, 0.6, 1.0]);
        if let Some(res) = crafting_result {
            if let Some(def) = registry.get(res.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let icon_pad = 6.0;
                mesh.add_quad(
                    result_x + icon_pad,
                    result_y + icon_pad,
                    slot_size + 6.0 - icon_pad * 2.0,
                    slot_size + 6.0 - icon_pad * 2.0,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }

        // 3. Backpack Storage Grid (3 rows x 9 columns)
        let inv_w = 9.0 * (slot_size + slot_gap);
        let inv_start_x = (screen_w - inv_w) * 0.5;
        let inv_start_y = craft_y + 110.0;

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
                }
            }
        }

        // 5. Cursor Held Item Stack (follows mouse position)
        if !cursor_held.is_empty() {
            if let Some(def) = registry.get(cursor_held.item_id) {
                let icon_uv = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
                let hold_size = 36.0;
                mesh.add_quad(
                    cursor_pos[0] - hold_size * 0.5,
                    cursor_pos[1] - hold_size * 0.5,
                    hold_size,
                    hold_size,
                    icon_uv,
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
        }
    }
}
