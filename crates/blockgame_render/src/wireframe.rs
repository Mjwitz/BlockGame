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
            LineVertex { position: [x0, y0, z0] }, LineVertex { position: [x1, y0, z0] },
            LineVertex { position: [x1, y0, z0] }, LineVertex { position: [x1, y0, z1] },
            LineVertex { position: [x1, y0, z1] }, LineVertex { position: [x0, y0, z1] },
            LineVertex { position: [x0, y0, z1] }, LineVertex { position: [x0, y0, z0] },
            // Top 4 edges
            LineVertex { position: [x0, y1, z0] }, LineVertex { position: [x1, y1, z0] },
            LineVertex { position: [x1, y1, z0] }, LineVertex { position: [x1, y1, z1] },
            LineVertex { position: [x1, y1, z1] }, LineVertex { position: [x0, y1, z1] },
            LineVertex { position: [x0, y1, z1] }, LineVertex { position: [x0, y1, z0] },
            // 4 Vertical corner pillars
            LineVertex { position: [x0, y0, z0] }, LineVertex { position: [x0, y1, z0] },
            LineVertex { position: [x1, y0, z0] }, LineVertex { position: [x1, y1, z0] },
            LineVertex { position: [x1, y0, z1] }, LineVertex { position: [x1, y1, z1] },
            LineVertex { position: [x0, y0, z1] }, LineVertex { position: [x0, y1, z1] },
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
            LineVertex { position: [x0, y0, z0] }, LineVertex { position: [x1, y0, z0] },
            LineVertex { position: [x1, y0, z0] }, LineVertex { position: [x1, y0, z1] },
            LineVertex { position: [x1, y0, z1] }, LineVertex { position: [x0, y0, z1] },
            LineVertex { position: [x0, y0, z1] }, LineVertex { position: [x0, y0, z0] },
            LineVertex { position: [x0, y1, z0] }, LineVertex { position: [x1, y1, z0] },
            LineVertex { position: [x1, y1, z0] }, LineVertex { position: [x1, y1, z1] },
            LineVertex { position: [x1, y1, z1] }, LineVertex { position: [x0, y1, z1] },
            LineVertex { position: [x0, y1, z1] }, LineVertex { position: [x0, y1, z0] },
            LineVertex { position: [x0, y0, z0] }, LineVertex { position: [x0, y1, z0] },
            LineVertex { position: [x1, y0, z0] }, LineVertex { position: [x1, y1, z0] },
            LineVertex { position: [x1, y0, z1] }, LineVertex { position: [x1, y1, z1] },
            LineVertex { position: [x0, y0, z1] }, LineVertex { position: [x0, y1, z1] },
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
            vertices.push(LineVertex { position: [bx0 + u0, by1 + eps, bz0 + v0] });
            vertices.push(LineVertex { position: [bx0 + u1, by1 + eps, bz0 + v1] });

            // NegY (bottom face)
            vertices.push(LineVertex { position: [bx0 + u0, by0 - eps, bz0 + v0] });
            vertices.push(LineVertex { position: [bx0 + u1, by0 - eps, bz0 + v1] });

            // PosZ (front face)
            vertices.push(LineVertex { position: [bx0 + u0, by0 + v0, bz1 + eps] });
            vertices.push(LineVertex { position: [bx0 + u1, by0 + v1, bz1 + eps] });

            // NegZ (back face)
            vertices.push(LineVertex { position: [bx0 + u0, by0 + v0, bz0 - eps] });
            vertices.push(LineVertex { position: [bx0 + u1, by0 + v1, bz0 - eps] });

            // PosX (right face)
            vertices.push(LineVertex { position: [bx1 + eps, by0 + v0, bz0 + u0] });
            vertices.push(LineVertex { position: [bx1 + eps, by0 + v1, bz0 + u1] });

            // NegX (left face)
            vertices.push(LineVertex { position: [bx0 - eps, by0 + v0, bz0 + u0] });
            vertices.push(LineVertex { position: [bx0 - eps, by0 + v1, bz0 + u1] });
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
