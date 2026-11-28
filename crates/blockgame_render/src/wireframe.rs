use bytemuck::{Pod, Zeroable};
use glam::IVec3;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
}

impl LineVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

pub struct WireframePipeline {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl WireframePipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Wireframe Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/wireframe.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Wireframe Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Wireframe Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[LineVertex::desc()],
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
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::pipeline::DEPTH_FORMAT,
                depth_write_enabled: false, // Don't occlude lines
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2, // Pull slightly toward camera to eliminate z-fighting
                    slope_scale: -2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { render_pipeline }
    }

    /// Builds a vertex buffer for the 12 bounding box wireframe edges around a voxel coordinate with custom bounds.
    pub fn create_custom_box_buffer(
        device: &wgpu::Device,
        voxel: IVec3,
        min_offset: glam::Vec3,
        max_offset: glam::Vec3,
    ) -> wgpu::Buffer {
        let x0 = voxel.x as f32 + min_offset.x - 0.002;
        let y0 = voxel.y as f32 + min_offset.y - 0.002;
        let z0 = voxel.z as f32 + min_offset.z - 0.002;
        let x1 = voxel.x as f32 + max_offset.x + 0.002;
        let y1 = voxel.y as f32 + max_offset.y + 0.002;
        let z1 = voxel.z as f32 + max_offset.z + 0.002;

        let vertices: [LineVertex; 24] = [
            // Bottom 4 edges
            LineVertex {
                position: [x0, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z0],
            },
            // Top 4 edges
            LineVertex {
                position: [x0, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z0],
            },
            // 4 Vertical corner pillars
            LineVertex {
                position: [x0, y0, z0],
            },
            LineVertex {
                position: [x0, y1, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
        ];

        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Wireframe Box Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    /// Builds a vertex buffer for the 12 bounding box wireframe edges around a standard 1x1x1 voxel coordinate.
    pub fn create_box_buffer(device: &wgpu::Device, voxel: IVec3) -> wgpu::Buffer {
        Self::create_custom_box_buffer(device, voxel, glam::Vec3::ZERO, glam::Vec3::ONE)
    }

    /// Builds a vertex buffer for progressive fracture/cracking lines across all 6 faces of a voxel coordinate.
    /// Stage ranges from 0 (faint crack) to 9 (severe shattering).
    /// Returns `(wgpu::Buffer, u32 vertex_count)`.
    pub fn create_cracking_box_buffer(
        device: &wgpu::Device,
        voxel: IVec3,
        stage: usize,
    ) -> (wgpu::Buffer, u32) {
        const CRACK_SEGMENTS: &[([f32; 2], [f32; 2])] = &[
            // Stage 0
            ([0.45, 0.40], [0.55, 0.55]),
            ([0.55, 0.55], [0.48, 0.68]),
            // Stage 1
            ([0.45, 0.40], [0.32, 0.35]),
            ([0.55, 0.55], [0.70, 0.50]),
            // Stage 2
            ([0.70, 0.50], [0.82, 0.38]),
            ([0.48, 0.68], [0.58, 0.82]),
            ([0.32, 0.35], [0.20, 0.42]),
            // Stage 3
            ([0.55, 0.55], [0.60, 0.40]),
            ([0.60, 0.40], [0.75, 0.25]),
            ([0.48, 0.68], [0.35, 0.78]),
            // Stage 4
            ([0.82, 0.38], [0.92, 0.28]),
            ([0.20, 0.42], [0.10, 0.50]),
            ([0.58, 0.82], [0.72, 0.90]),
            ([0.35, 0.78], [0.22, 0.88]),
            // Stage 5
            ([0.75, 0.25], [0.85, 0.12]),
            ([0.32, 0.35], [0.38, 0.20]),
            ([0.38, 0.20], [0.28, 0.08]),
            ([0.60, 0.40], [0.50, 0.25]),
            // Stage 6
            ([0.50, 0.25], [0.55, 0.10]),
            ([0.70, 0.50], [0.80, 0.65]),
            ([0.80, 0.65], [0.90, 0.75]),
            ([0.20, 0.42], [0.15, 0.30]),
            // Stage 7
            ([0.35, 0.78], [0.45, 0.88]),
            ([0.45, 0.88], [0.40, 0.95]),
            ([0.80, 0.65], [0.72, 0.78]),
            ([0.15, 0.30], [0.05, 0.20]),
            // Stage 8
            ([0.58, 0.82], [0.50, 0.75]),
            ([0.72, 0.78], [0.85, 0.88]),
            ([0.22, 0.88], [0.12, 0.95]),
            ([0.85, 0.12], [0.95, 0.05]),
            // Stage 9
            ([0.10, 0.50], [0.02, 0.60]),
            ([0.90, 0.75], [0.98, 0.85]),
            ([0.28, 0.08], [0.18, 0.02]),
            ([0.72, 0.90], [0.82, 0.98]),
        ];

        let num_segments = match stage {
            0 => 2,
            1 => 4,
            2 => 7,
            3 => 10,
            4 => 14,
            5 => 18,
            6 => 22,
            7 => 26,
            8 => 30,
            _ => 34,
        };

        let mut vertices: Vec<LineVertex> = Vec::with_capacity(24 + num_segments * 12);

        // 1. Add bounding box edges
        let x0 = voxel.x as f32 - 0.002;
        let y0 = voxel.y as f32 - 0.002;
        let z0 = voxel.z as f32 - 0.002;
        let x1 = voxel.x as f32 + 1.002;
        let y1 = voxel.y as f32 + 1.002;
        let z1 = voxel.z as f32 + 1.002;

        let box_edges: [LineVertex; 24] = [
            LineVertex {
                position: [x0, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y0, z0],
            },
            LineVertex {
                position: [x0, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
            LineVertex {
                position: [x0, y1, z0],
            },
            LineVertex {
                position: [x0, y0, z0],
            },
            LineVertex {
                position: [x0, y1, z0],
            },
            LineVertex {
                position: [x1, y0, z0],
            },
            LineVertex {
                position: [x1, y1, z0],
            },
            LineVertex {
                position: [x1, y0, z1],
            },
            LineVertex {
                position: [x1, y1, z1],
            },
            LineVertex {
                position: [x0, y0, z1],
            },
            LineVertex {
                position: [x0, y1, z1],
            },
        ];
        vertices.extend_from_slice(&box_edges);

        // 2. Add fracture segments on 6 cube faces
        let eps = 0.003;
        let bx0 = voxel.x as f32;
        let by0 = voxel.y as f32;
        let bz0 = voxel.z as f32;
        let bx1 = bx0 + 1.0;
        let by1 = by0 + 1.0;
        let bz1 = bz0 + 1.0;

        for &([u0, v0], [u1, v1]) in &CRACK_SEGMENTS[..num_segments] {
            // PosY (top face)
            vertices.push(LineVertex {
                position: [bx0 + u0, by1 + eps, bz0 + v0],
            });
            vertices.push(LineVertex {
                position: [bx0 + u1, by1 + eps, bz0 + v1],
            });

            // NegY (bottom face)
            vertices.push(LineVertex {
                position: [bx0 + u0, by0 - eps, bz0 + v0],
            });
            vertices.push(LineVertex {
                position: [bx0 + u1, by0 - eps, bz0 + v1],
            });

            // PosZ (front face)
            vertices.push(LineVertex {
                position: [bx0 + u0, by0 + v0, bz1 + eps],
            });
            vertices.push(LineVertex {
                position: [bx0 + u1, by0 + v1, bz1 + eps],
            });

            // NegZ (back face)
            vertices.push(LineVertex {
                position: [bx0 + u0, by0 + v0, bz0 - eps],
            });
            vertices.push(LineVertex {
                position: [bx0 + u1, by0 + v1, bz0 - eps],
            });

            // PosX (right face)
            vertices.push(LineVertex {
                position: [bx1 + eps, by0 + v0, bz0 + u0],
            });
            vertices.push(LineVertex {
                position: [bx1 + eps, by0 + v1, bz0 + u1],
            });

            // NegX (left face)
            vertices.push(LineVertex {
                position: [bx0 - eps, by0 + v0, bz0 + u0],
            });
            vertices.push(LineVertex {
                position: [bx0 - eps, by0 + v1, bz0 + u1],
            });
        }

        let vertex_count = vertices.len() as u32;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cracking Box Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        (buffer, vertex_count)
    }
}

/// Per-vertex colored 3D line vertex used for claim chunk-border overlays.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ColoredLineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl ColoredLineVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ColoredLineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Maps claim ownership relation to its 3D border RGBA color:
/// Green = own, Cyan = trusted, Red = other player, Gold = Admin Claim.
pub fn claim_border_color(relation: blockgame_core::claims::ClaimRelation) -> [f32; 4] {
    use blockgame_core::claims::ClaimRelation;
    match relation {
        ClaimRelation::Own => [0.22, 0.92, 0.36, 0.88],
        ClaimRelation::Trusted => [0.20, 0.88, 0.96, 0.88],
        ClaimRelation::Other => [0.95, 0.25, 0.25, 0.88],
        ClaimRelation::Admin => [1.00, 0.82, 0.20, 0.92],
    }
}

/// Builds 3D line-list vertices for color-coded vertical chunk boundary pillars and horizontal rings
/// around `player_y` for all synced nearby claimed chunks.
pub fn build_claim_border_vertices(
    claims: &[blockgame_core::claims::SyncedClaimChunk],
    player_y: f32,
) -> Vec<ColoredLineVertex> {
    let mut verts = Vec::with_capacity(claims.len() * 40);
    let y_min = (player_y - 24.0).max(0.0);
    let y_max = player_y + 40.0;
    let ring_step = 8.0;

    for c in claims {
        let color = claim_border_color(c.relation);
        let x0 = (c.chunk_x * 16) as f32 + 0.03;
        let x1 = ((c.chunk_x + 1) * 16) as f32 - 0.03;
        let z0 = (c.chunk_z * 16) as f32 + 0.03;
        let z1 = ((c.chunk_z + 1) * 16) as f32 - 0.03;

        // 4 vertical corner pillars + 4 mid-edge vertical guides
        let verticals = [
            (x0, z0),
            (x1, z0),
            (x1, z1),
            (x0, z1),
            ((x0 + x1) * 0.5, z0),
            ((x0 + x1) * 0.5, z1),
            (x0, (z0 + z1) * 0.5),
            (x1, (z0 + z1) * 0.5),
        ];
        for (vx, vz) in verticals {
            verts.push(ColoredLineVertex { position: [vx, y_min, vz], color });
            verts.push(ColoredLineVertex { position: [vx, y_max, vz], color });
        }

        // Horizontal perimeter rings every 8 blocks vertically
        let mut ry = (y_min / ring_step).floor() * ring_step;
        while ry <= y_max {
            let edges = [
                ([x0, ry, z0], [x1, ry, z0]),
                ([x1, ry, z0], [x1, ry, z1]),
                ([x1, ry, z1], [x0, ry, z1]),
                ([x0, ry, z1], [x0, ry, z0]),
            ];
            for (a, b) in edges {
                verts.push(ColoredLineVertex { position: a, color });
                verts.push(ColoredLineVertex { position: b, color });
            }
            ry += ring_step;
        }
    }
    verts
}

/// Computes an entry/exit notice when moving from `prev_owner` to `cur_owner` display names.
/// Returns `None` when remaining inside the same claim owner or remaining in wilderness.
pub fn claim_transition_notice(
    prev_owner: Option<&str>,
    cur_owner: Option<&str>,
) -> Option<String> {
    if prev_owner == cur_owner {
        return None;
    }
    match (prev_owner, cur_owner) {
        (_, Some(entered)) => Some(format!("&6Entered {}", entered)),
        (Some(left), None) => Some(format!("&7Left {} (Wilderness)", left)),
        (None, None) => None,
    }
}

const CLAIM_BORDER_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    camera_pos: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    sun_tint: vec4<f32>,
    ambient_tint: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

/// Render pipeline for color-coded 3D claim chunk borders.
pub struct ClaimBorderPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
}

impl ClaimBorderPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Claim Border Shader"),
            source: wgpu::ShaderSource::Wgsl(CLAIM_BORDER_WGSL.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Claim Border Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Claim Border Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ColoredLineVertex::desc()],
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
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::pipeline::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self { render_pipeline }
    }

    pub fn create_border_buffer(
        device: &wgpu::Device,
        claims: &[blockgame_core::claims::SyncedClaimChunk],
        player_y: f32,
    ) -> Option<(wgpu::Buffer, u32)> {
        let verts = build_claim_border_vertices(claims, player_y);
        if verts.is_empty() {
            return None;
        }
        let count = verts.len() as u32;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Claim Border Vertex Buffer"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Some((buffer, count))
    }
}

#[cfg(test)]
mod claim_border_tests {
    use super::*;
    use blockgame_core::claims::{ClaimRelation, SyncedClaimChunk};

    #[test]
    fn claim_border_colors_and_vertices() {
        let own = claim_border_color(ClaimRelation::Own);
        let trusted = claim_border_color(ClaimRelation::Trusted);
        let other = claim_border_color(ClaimRelation::Other);
        let admin = claim_border_color(ClaimRelation::Admin);
        assert!(own[1] > own[0] && own[1] > own[2], "Own must be green");
        assert!(trusted[2] > trusted[0], "Trusted must be cyan");
        assert!(other[0] > other[1] && other[0] > other[2], "Other must be red");
        assert!(admin[0] > 0.9 && admin[1] > 0.7, "Admin must be gold");

        let verts = build_claim_border_vertices(
            &[SyncedClaimChunk {
                chunk_x: 2,
                chunk_z: -1,
                relation: ClaimRelation::Admin,
                display_name: "Spawn Plaza (Admin Claim)".to_string(),
            }],
            64.0,
        );
        assert!(!verts.is_empty());
        assert_eq!(verts.len() % 2, 0, "LineList requires even vertex count");
    }

    #[test]
    fn claim_transition_notices_avoid_spam_within_same_owner() {
        assert_eq!(claim_transition_notice(None, None), None);
        assert_eq!(
            claim_transition_notice(None, Some("Steve's Claim")),
            Some("&6Entered Steve's Claim".to_string())
        );
        assert_eq!(
            claim_transition_notice(Some("Steve's Claim"), Some("Steve's Claim")),
            None
        );
        assert_eq!(
            claim_transition_notice(Some("Steve's Claim"), None),
            Some("&7Left Steve's Claim (Wilderness)".to_string())
        );
    }
}
