use std::sync::Arc;

use blockgame_core::{
    block::{BlockId, BlockRegistry},
    item::{Inventory, ItemRegistry},
    physics::AABB,
    raycast::raycast_voxel,
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    create_depth_texture, DayNightCycle, UiBuilder, UiMesh, UiPipeline, VoxelPipeline,
    WireframePipeline,
};
use glam::{IVec3, Vec3};
use wgpu::util::DeviceExt;

use crate::world_manager::WorldManager;

pub async fn render_world_to_image(
    seed: u32,
    render_distance: i32,
    width: u32,
    height: u32,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Initializing wgpu for multi-chunk world landscape rendering...");
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

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Offscreen Landscape Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        )
        .await?;

    let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;

    let render_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Landscape Render Target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let (_depth_tex, depth_view) = create_depth_texture(&device, width, height);

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

    let atlas = TextureAtlas::generate_default();
    let pipeline = VoxelPipeline::new(&device, &queue, target_format, &atlas);
    let wireframe_pipeline = WireframePipeline::new(
        &device,
        target_format,
        &pipeline.camera_bind_group_layout,
    );
    let ui_pipeline = UiPipeline::new(
        &device,
        target_format,
        &pipeline.texture_bind_group_layout,
    );

    let registry = Arc::new(BlockRegistry::new());
    let item_registry = Arc::new(ItemRegistry::new());
    let inventory = Inventory::starter_inventory();
    let mut world_manager = WorldManager::new(seed, render_distance, registry.clone());
    world_manager.init_spawn_area(&device, &pipeline);

    // 1. Build a player outpost structure with torches demonstrating dual-channel lighting!
    let dummy_player_box = AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0));
    let base_x = 0;
    let base_z = 0;
    let base_y = 24;

    // Build Cobblestone foundation & Planks walls
    for dy in 0_i32..4_i32 {
        let y = base_y + dy;
        for dx in -3_i32..=3_i32 {
            for dz in -3_i32..=3_i32 {
                let is_wall = dx == -3 || dx == 3 || dz == -3 || dz == 3;
                let is_corner = (dx.abs() == 3) && (dz.abs() == 3);
                let is_door = (dx == 0 && dz == 3) && (dy == 0 || dy == 1);
                let is_window = (dx.abs() == 3 || dz == -3) && (dy == 1 || dy == 2) && (dx % 2 == 0);

                let pos = IVec3::new(base_x + dx, y, base_z + dz);

                if is_door {
                    world_manager.break_block(pos, &device, &pipeline);
                } else if is_corner {
                    world_manager.place_block(pos, BlockId::WOOD_LOG, &dummy_player_box, &device, &pipeline);
                } else if is_window {
                    world_manager.place_block(pos, BlockId::GLASS, &dummy_player_box, &device, &pipeline);
                } else if is_wall {
                    if dy == 0 {
                        world_manager.place_block(pos, BlockId::COBBLESTONE, &dummy_player_box, &device, &pipeline);
                    } else {
                        world_manager.place_block(pos, BlockId::PLANKS, &dummy_player_box, &device, &pipeline);
                    }
                }
            }
        }
    }

    // Place entrance torches flanking the door on the outside ground and one inside the cabin
    world_manager.place_block(
        IVec3::new(base_x - 1, base_y + 1, base_z + 4),
        BlockId::TORCH,
        &dummy_player_box,
        &device,
        &pipeline,
    );
    world_manager.place_block(
        IVec3::new(base_x + 1, base_y + 1, base_z + 4),
        BlockId::TORCH,
        &dummy_player_box,
        &device,
        &pipeline,
    );
    world_manager.place_block(
        IVec3::new(base_x, base_y + 1, base_z),
        BlockId::TORCH,
        &dummy_player_box,
        &device,
        &pipeline,
    );

    // 2. Configure sunset/dusk atmosphere
    let mut day_night = DayNightCycle::new();
    day_night.set_time(12200.0); // Rich sunset / dusk transition
    let sky = day_night.sky_color();

    // Position camera with a 3/4 perspective showing cabin, glowing torches, and sunset landscape
    let eye = Vec3::new(-3.5, (base_y + 2) as f32 + 0.8, 9.0);
    let camera = Camera::new(
        eye,
        (-72.0_f32).to_radians(),
        (-10.0_f32).to_radians(),
    );
    let mut camera_uniform = CameraUniform::default();
    camera.update_uniform_with_atmosphere(&mut camera_uniform, width as f32 / height as f32, &day_night);
    pipeline.update_camera(&queue, &camera_uniform);

    // Spawn item drops on the grass in clear view of the camera: wood log and slender glowing torch
    let mut item_drop = blockgame_core::entity::ItemDrop::new(
        Vec3::new(-2.8, (base_y + 2) as f32 + 0.15, 7.2),
        blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 4),
    );
    item_drop.age = 1.2;
    world_manager.item_drops.push(item_drop);

    let mut torch_drop = blockgame_core::entity::ItemDrop::new(
        Vec3::new(-2.2, (base_y + 2) as f32 + 0.15, 6.7),
        blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TORCH, 2),
    );
    torch_drop.age = 0.8;
    world_manager.item_drops.push(torch_drop);

    let drops_mesh = blockgame_render::build_item_drops_mesh(
        &world_manager.item_drops,
        &item_registry,
    );
    let item_drop_gpu_mesh = pipeline.upload_mesh(&device, &drops_mesh);

    // 3. Perform DDA raycast from crosshair
    let hit = {
        let world_guard = world_manager.world().lock().unwrap();
        raycast_voxel(camera.position, camera.forward(), 6.0, &world_guard, &registry)
    };

    let wireframe_buffer = hit.map(|h| {
        let world_guard = world_manager.world().lock().unwrap();
        let block = world_guard.get_block(h.hit_voxel);
        if block == BlockId::TORCH {
            WireframePipeline::create_custom_box_buffer(
                &device,
                h.hit_voxel,
                Vec3::new(0.35, 0.0, 0.35),
                Vec3::new(0.65, 0.65, 0.65),
            )
        } else {
            WireframePipeline::create_box_buffer(&device, h.hit_voxel)
        }
    });

    // 4. Encode render commands
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Landscape Render Encoder"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Voxel Landscape Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: sky[0] as f64,
                        g: sky[1] as f64,
                        b: sky[2] as f64,
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
        world_manager.render(&mut render_pass);

        if let Some(drop_mesh) = &item_drop_gpu_mesh {
            drop_mesh.draw(&mut render_pass);
        }

        if let Some(buf) = &wireframe_buffer {
            render_pass.set_pipeline(&wireframe_pipeline.render_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buf.slice(..));
            render_pass.draw(0..24, 0..1);
        }
    }

    // 5. Render 2D UI Overlay Pass (Crosshair, Held Torch Item & Hotbar with stack numbers)
    let mut ui_mesh = UiMesh::new();
    let screen_w = width as f32;
    let screen_h = height as f32;
    ui_pipeline.update_screen_size(&queue, screen_w, screen_h);
    UiBuilder::build_hud(&mut ui_mesh, screen_w, screen_h, &inventory, 3, &item_registry, 0.0);

    if !ui_mesh.indices.is_empty() {
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Headless UI VB"),
            contents: bytemuck::cast_slice(&ui_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Headless UI IB"),
            contents: bytemuck::cast_slice(&ui_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Headless UI Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_view,
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

        ui_pass.set_pipeline(&ui_pipeline.render_pipeline);
        ui_pass.set_bind_group(0, &ui_pipeline.screen_bind_group, &[]);
        ui_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
        ui_pass.set_vertex_buffer(0, vb.slice(..));
        ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        ui_pass.draw_indexed(0..ui_mesh.indices.len() as u32, 0, 0..1);
    }

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
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );

    queue.submit(Some(encoder.finish()));

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

    image::save_buffer(
        output_path,
        &image_bytes,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;

    log::info!("Successfully saved rendered interaction image to {}", output_path);

    // Also render first-person view with a held 3D block (Wood Log)
    {
        let mut block_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Block Held Encoder"),
        });

        {
            let mut render_pass = block_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Voxel Block Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: sky[0] as f64,
                            g: sky[1] as f64,
                            b: sky[2] as f64,
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
            world_manager.render(&mut render_pass);

            if let Some(drop_mesh) = &item_drop_gpu_mesh {
                drop_mesh.draw(&mut render_pass);
            }
        }

        let mut block_ui_mesh = UiMesh::new();
        // Slot 1 is Wood Log!
        UiBuilder::build_hud(&mut block_ui_mesh, screen_w, screen_h, &inventory, 1, &item_registry, 0.0);

        if !block_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block UI VB"),
                contents: bytemuck::cast_slice(&block_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Block UI IB"),
                contents: bytemuck::cast_slice(&block_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = block_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Block UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
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

            ui_pass.set_pipeline(&ui_pipeline.render_pipeline);
            ui_pass.set_bind_group(0, &ui_pipeline.screen_bind_group, &[]);
            ui_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            ui_pass.set_vertex_buffer(0, vb.slice(..));
            ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            ui_pass.draw_indexed(0..block_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        block_encoder.copy_texture_to_buffer(
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
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        queue.submit(Some(block_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut block_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            block_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "block_held_render.png",
            &block_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved block held render image to block_held_render.png");
    }

    // Also render inventory screen overlay with hover tooltip and stack counts
    let mut inv_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Inventory Render Encoder"),
    });

    let mut inv_ui_mesh = UiMesh::new();
    let slot_size = 40.0;
    let slot_gap = 4.0;
    let inv_w = 9.0 * (slot_size + slot_gap);
    let inv_start_x = (screen_w - inv_w) * 0.5;
    let craft_y = screen_h * 0.5 - 160.0;
    let inv_start_y = craft_y + 110.0;
    let hotbar_y = inv_start_y + 3.0 * (slot_size + slot_gap) + 16.0;

    // Place cursor over slot 3 (Torch) to trigger the hover tooltip
    let cursor_pos = [
        inv_start_x + 3.0 * (slot_size + slot_gap) + 20.0,
        hotbar_y + 20.0,
    ];

    let mut crafting_grid = blockgame_core::crafting::CraftingGrid::new_2x2();
    crafting_grid.set(0, 0, blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 1));
    let crafting_result = Some(blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::PLANKS, 4));

    UiBuilder::build_inventory_screen(
        &mut inv_ui_mesh,
        screen_w,
        screen_h,
        &inventory,
        3,
        &crafting_grid,
        crafting_result,
        &blockgame_core::item::ItemSlot::EMPTY,
        cursor_pos,
        &item_registry,
    );

    if !inv_ui_mesh.indices.is_empty() {
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Inv UI VB"),
            contents: bytemuck::cast_slice(&inv_ui_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Inv UI IB"),
            contents: bytemuck::cast_slice(&inv_ui_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        {
            let mut ui_pass = inv_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Inv UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
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

            ui_pass.set_pipeline(&ui_pipeline.render_pipeline);
            ui_pass.set_bind_group(0, &ui_pipeline.screen_bind_group, &[]);
            ui_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            ui_pass.set_vertex_buffer(0, vb.slice(..));
            ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            ui_pass.draw_indexed(0..inv_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        inv_encoder.copy_texture_to_buffer(
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
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        queue.submit(Some(inv_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut inv_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            inv_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "inventory_render.png",
            &inv_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved inventory interaction image to inventory_render.png");
    }

    Ok(())
}
