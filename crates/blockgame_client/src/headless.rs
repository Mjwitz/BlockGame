use blockgame_core::{block::BlockRegistry, chunk::Chunk};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, ChunkMesher, VoxelPipeline,
};
use glam::Vec3;

pub async fn render_chunk_to_image(
    chunk: &Chunk,
    registry: &BlockRegistry,
    width: u32,
    height: u32,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Initializing wgpu instance for offscreen rendering...");
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        backend_options: Default::default(),
    });

    log::info!("Requesting wgpu adapter...");
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or("Failed to find suitable wgpu adapter")?;

    let info = adapter.get_info();
    log::info!("Using graphics adapter: {:?} ({:?})", info.name, info.backend);

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Offscreen Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await?;

    let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;

    // Render target texture
    let render_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Offscreen Render Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Depth texture
    let (_depth_tex, depth_view) = create_depth_texture(&device, width, height);

    // Staging buffer to read back pixels to CPU
    // Note: bytes_per_row must be aligned to 256 bytes in wgpu
    let unaligned_bytes_per_row = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let bytes_per_row = (unaligned_bytes_per_row + align - 1) & !(align - 1);
    let buffer_size = (bytes_per_row * height) as wgpu::BufferAddress;

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Generate mesh from chunk
    let mesher = ChunkMesher::new(registry);
    let mesh = mesher.mesh_chunk(chunk);
    log::info!(
        "Chunk mesh generated: {} vertices, {} faces",
        mesh.vertices.len(),
        mesh.face_count()
    );

    let atlas = TextureAtlas::generate_default();
    let pipeline = VoxelPipeline::new(&device, &queue, target_format, &atlas);
    let gpu_mesh = pipeline
        .upload_mesh(&device, &mesh)
        .ok_or("Mesh was empty")?;

    // Position camera to look down at the chunk diagonally
    let camera = Camera::new(
        Vec3::new(26.0, 24.0, 26.0),
        (-135.0_f32).to_radians(),
        (-30.0_f32).to_radians(),
    );
    let mut camera_uniform = CameraUniform::default();
    camera.update_uniform(&mut camera_uniform, width as f32 / height as f32);
    pipeline.update_camera(&queue, &camera_uniform);

    // Encode render commands
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Offscreen Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Voxel Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.53,
                        g: 0.81,
                        b: 0.98,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
    }

    // Copy rendered texture to staging buffer
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &render_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    // Map staging buffer to CPU
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()??;

    let data = buffer_slice.get_mapped_range();
    let mut image_bytes = Vec::with_capacity((width * height * 4) as usize);

    for row in 0..height {
        let start = (row * bytes_per_row) as usize;
        let end = start + (width * 4) as usize;
        image_bytes.extend_from_slice(&data[start..end]);
    }
    drop(data);
    staging_buffer.unmap();

    // Save PNG image
    image::save_buffer(
        output_path,
        &image_bytes,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;

    log::info!("Successfully saved rendered image to {}", output_path);
    Ok(())
}
