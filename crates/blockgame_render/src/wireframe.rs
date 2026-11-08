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

    /// Builds a vertex buffer for the 12 bounding box wireframe edges around a voxel coordinate.
    pub fn create_box_buffer(device: &wgpu::Device, voxel: IVec3) -> wgpu::Buffer {
        let x0 = voxel.x as f32 - 0.002;
        let y0 = voxel.y as f32 - 0.002;
        let z0 = voxel.z as f32 - 0.002;
        let x1 = voxel.x as f32 + 1.002;
        let y1 = voxel.y as f32 + 1.002;
        let z1 = voxel.z as f32 + 1.002;

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
}
