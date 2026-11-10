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
    let wireframe_pipeline =
        WireframePipeline::new(&device, target_format, &pipeline.camera_bind_group_layout);
    let ui_pipeline = UiPipeline::new(&device, target_format, &pipeline.texture_bind_group_layout);

    let registry = Arc::new(BlockRegistry::new());
    let item_registry = Arc::new(ItemRegistry::new());
    let inventory = Inventory::starter_inventory();
    let mut world_manager = WorldManager::new(seed, render_distance, registry.clone());
    world_manager.init_spawn_area(&device, &pipeline);

    // 1. Build a player outpost structure with torches demonstrating dual-channel lighting!
    let dummy_player_box = AABB::new(
        Vec3::new(100.0, 100.0, 100.0),
        Vec3::new(101.0, 101.0, 101.0),
    );
    let base_x = 0;
    let base_z = 0;
    let base_y = {
        let world_guard = world_manager.world().lock().unwrap();
        let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
        while y > 1 {
            let block = world_guard.get_block(IVec3::new(base_x, y, base_z));
            if !block.is_air() {
                break;
            }
            y -= 1;
        }
        y
    };

    // Build Cobblestone foundation & Planks walls
    for dy in 0_i32..4_i32 {
        let y = base_y + dy;
        for dx in -3_i32..=3_i32 {
            for dz in -3_i32..=3_i32 {
                let is_wall = dx == -3 || dx == 3 || dz == -3 || dz == 3;
                let is_corner = (dx.abs() == 3) && (dz.abs() == 3);
                let is_door = (dx == 0 && dz == 3) && (dy == 0 || dy == 1);
                let is_window =
                    (dx.abs() == 3 || dz == -3) && (dy == 1 || dy == 2) && (dx % 2 == 0);

                let pos = IVec3::new(base_x + dx, y, base_z + dz);

                if is_door {
                    world_manager.break_block(pos, &device, &pipeline);
                } else if is_corner {
                    world_manager.place_block(
                        pos,
                        BlockId::WOOD_LOG,
                        &dummy_player_box,
                        &device,
                        &pipeline,
                    );
                } else if is_window {
                    world_manager.place_block(
                        pos,
                        BlockId::GLASS,
                        &dummy_player_box,
                        &device,
                        &pipeline,
                    );
                } else if is_wall {
                    if dy == 0 {
                        world_manager.place_block(
                            pos,
                            BlockId::COBBLESTONE,
                            &dummy_player_box,
                            &device,
                            &pipeline,
                        );
                    } else {
                        world_manager.place_block(
                            pos,
                            BlockId::PLANKS,
                            &dummy_player_box,
                            &device,
                            &pipeline,
                        );
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

    // Place Crafting Table and Chest outside the cabin
    world_manager.place_block(
        IVec3::new(base_x - 2, base_y + 1, base_z + 4),
        BlockId::CRAFTING_TABLE,
        &dummy_player_box,
        &device,
        &pipeline,
    );
    world_manager.place_block(
        IVec3::new(base_x + 2, base_y + 1, base_z + 4),
        BlockId::CHEST,
        &dummy_player_box,
        &device,
        &pipeline,
    );

    {
        let mut world_guard = world_manager.world().lock().unwrap();
        let chest = world_guard.get_or_create_chest(IVec3::new(base_x + 2, base_y + 1, base_z + 4));
        chest[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::DIAMOND, 12);
        chest[1] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::GOLD_INGOT, 24);
        chest[2] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::IRON_INGOT, 32);
        chest[3] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COAL, 48);
        chest[4] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BREAD, 16);
    }

    // 2. Configure sunset/dusk atmosphere
    let mut day_night = DayNightCycle::new();
    day_night.set_time(12200.0); // Rich sunset / dusk transition
    let sky = day_night.sky_color();

    // Find ground elevation at camera location (-3.5, 9.0)
    let cam_ground_y = {
        let world_guard = world_manager.world().lock().unwrap();
        let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
        while y > 1 {
            let block = world_guard.get_block(IVec3::new(-4, y, 9));
            if !block.is_air() {
                break;
            }
            y -= 1;
        }
        y
    };

    // Position camera with a 3/4 perspective showing cabin, glowing torches, and sunset landscape
    let eye = Vec3::new(-3.5, (cam_ground_y as f32 + 2.0).max((base_y + 2) as f32 + 1.2), 9.0);
    let camera = Camera::new(eye, (-72.0_f32).to_radians(), (-10.0_f32).to_radians());
    let mut camera_uniform = CameraUniform::default();
    camera.update_uniform_with_atmosphere(
        &mut camera_uniform,
        width as f32 / height as f32,
        &day_night,
    );
    pipeline.update_camera(&queue, &camera_uniform);

    // Spawn item drops on the grass in clear view of the camera: wood log and slender glowing torch
    let drop_ground_y = {
        let world_guard = world_manager.world().lock().unwrap();
        let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
        while y > 1 {
            let block = world_guard.get_block(IVec3::new(-3, y, 7));
            if !block.is_air() {
                break;
            }
            y -= 1;
        }
        y
    };
    let mut item_drop = blockgame_core::entity::ItemDrop::new(
        Vec3::new(-2.8, (drop_ground_y as f32 + 1.0) + 0.15, 7.2),
        blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 4),
    );
    item_drop.age = 1.2;
    world_manager.item_drops.push(item_drop);

    let mut torch_drop = blockgame_core::entity::ItemDrop::new(
        Vec3::new(-2.2, (drop_ground_y as f32 + 1.0) + 0.15, 6.7),
        blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TORCH, 2),
    );
    torch_drop.age = 0.8;
    world_manager.item_drops.push(torch_drop);

    let drops_mesh =
        blockgame_render::build_item_drops_mesh(&world_manager.item_drops, &item_registry);
    let item_drop_gpu_mesh = pipeline.upload_mesh(&device, &drops_mesh);

    let animals_mesh = blockgame_render::build_animals_mesh(&world_manager.animals);
    let animal_gpu_mesh = pipeline.upload_mesh(&device, &animals_mesh);

    // 3. Perform DDA raycast from crosshair
    let hit = {
        let world_guard = world_manager.world().lock().unwrap();
        raycast_voxel(
            camera.position,
            camera.forward(),
            6.0,
            &world_guard,
            &registry,
        )
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

        if let Some(anim_mesh) = &animal_gpu_mesh {
            anim_mesh.draw(&mut render_pass);
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
    let stats = blockgame_core::PlayerStats::new();
    UiBuilder::build_hud(
        &mut ui_mesh,
        screen_w,
        screen_h,
        &inventory,
        3,
        &item_registry,
        0.0,
        0.0,
        &stats,
    );

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
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
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

    log::info!(
        "Successfully saved rendered interaction image to {}",
        output_path
    );

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
        UiBuilder::build_hud(
            &mut block_ui_mesh,
            screen_w,
            screen_h,
            &inventory,
            1,
            &item_registry,
            0.0,
            0.0,
            &stats,
        );

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
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
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

    // Also render first-person view with 3D pickaxe held in hand
    {
        let mut pick_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Pickaxe Render Encoder"),
        });

        {
            let mut render_pass = pick_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Voxel Pickaxe Pass"),
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

        let mut pick_ui_mesh = UiMesh::new();
        // Slot 0 is Stone Pickaxe!
        UiBuilder::build_hud(
            &mut pick_ui_mesh,
            screen_w,
            screen_h,
            &inventory,
            0,
            &item_registry,
            0.0,
            0.0,
            &stats,
        );

        if !pick_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Pick UI VB"),
                contents: bytemuck::cast_slice(&pick_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Pick UI IB"),
                contents: bytemuck::cast_slice(&pick_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = pick_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Pick UI Pass"),
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
            ui_pass.draw_indexed(0..pick_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        pick_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(pick_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut pick_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            pick_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "pickaxe_held_render.png",
            &pick_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved pickaxe held render image to pickaxe_held_render.png");
    }

    // Also render first-person view with empty hand punching
    {
        let mut hand_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Hand Punch Encoder"),
        });

        {
            let mut render_pass = hand_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Voxel Hand Pass"),
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

        let mut empty_inventory = inventory.clone();
        empty_inventory.slots[8] = blockgame_core::item::ItemSlot::EMPTY;
        let mut hand_ui_mesh = UiMesh::new();
        // Slot 8 is empty! punch_time = 0.5 (peak punch swing)
        UiBuilder::build_hud(
            &mut hand_ui_mesh,
            screen_w,
            screen_h,
            &empty_inventory,
            8,
            &item_registry,
            0.0,
            0.5,
            &stats,
        );

        if !hand_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Hand UI VB"),
                contents: bytemuck::cast_slice(&hand_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Hand UI IB"),
                contents: bytemuck::cast_slice(&hand_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = hand_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Hand UI Pass"),
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
            ui_pass.draw_indexed(0..hand_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        hand_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(hand_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut hand_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            hand_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "hand_punch_render.png",
            &hand_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved hand punch render image to hand_punch_render.png");
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
    crafting_grid.set(
        0,
        0,
        blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 1),
    );
    let crafting_result = Some(blockgame_core::item::ItemSlot::new(
        blockgame_core::item::ItemId::PLANKS,
        4,
    ));

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
        None,
        None,
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
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
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

    // Render Phase 3 Farming and Animals showcase scene
    {
        // 1. Plant a patch of wheat crops in various stages (0..3) on farmland
        let dummy_player_box = AABB::new(
            Vec3::new(100.0, 100.0, 100.0),
            Vec3::new(101.0, 101.0, 101.0),
        );
        let farm_x = 0;
        let farm_z = 5;
        let farm_y = base_y;

        world_manager.place_block(
            IVec3::new(farm_x, farm_y, farm_z),
            BlockId::FARMLAND,
            &dummy_player_box,
            &device,
            &pipeline,
        );
        world_manager.place_block(
            IVec3::new(farm_x, farm_y + 1, farm_z),
            BlockId::WHEAT_0,
            &dummy_player_box,
            &device,
            &pipeline,
        );

        world_manager.place_block(
            IVec3::new(farm_x + 1, farm_y, farm_z),
            BlockId::FARMLAND,
            &dummy_player_box,
            &device,
            &pipeline,
        );
        world_manager.place_block(
            IVec3::new(farm_x + 1, farm_y + 1, farm_z),
            BlockId::WHEAT_1,
            &dummy_player_box,
            &device,
            &pipeline,
        );

        world_manager.place_block(
            IVec3::new(farm_x + 2, farm_y, farm_z),
            BlockId::FARMLAND,
            &dummy_player_box,
            &device,
            &pipeline,
        );
        world_manager.place_block(
            IVec3::new(farm_x + 2, farm_y + 1, farm_z),
            BlockId::WHEAT_2,
            &dummy_player_box,
            &device,
            &pipeline,
        );

        world_manager.place_block(
            IVec3::new(farm_x + 3, farm_y, farm_z),
            BlockId::FARMLAND,
            &dummy_player_box,
            &device,
            &pipeline,
        );
        world_manager.place_block(
            IVec3::new(farm_x + 3, farm_y + 1, farm_z),
            BlockId::WHEAT_3,
            &dummy_player_box,
            &device,
            &pipeline,
        );

        // Position 3 animals near the farm
        world_manager.animals.clear();
        let mut cow = blockgame_core::animal::AnimalEntity::new(
            1,
            blockgame_core::animal::AnimalKind::Cow,
            Vec3::new(-1.8, (farm_y + 1) as f32, 5.0),
        );
        cow.yaw = (40.0_f32).to_radians();
        world_manager.animals.push(cow);

        let mut pig = blockgame_core::animal::AnimalEntity::new(
            2,
            blockgame_core::animal::AnimalKind::Pig,
            Vec3::new(-0.4, (farm_y + 1) as f32, 5.2),
        );
        pig.yaw = (15.0_f32).to_radians();
        world_manager.animals.push(pig);

        let mut chicken = blockgame_core::animal::AnimalEntity::new(
            3,
            blockgame_core::animal::AnimalKind::Chicken,
            Vec3::new(1.2, (farm_y + 1) as f32, 6.2),
        );
        chicken.yaw = (-30.0_f32).to_radians();
        world_manager.animals.push(chicken);

        let animals_mesh = blockgame_render::build_animals_mesh(&world_manager.animals);
        let animal_gpu_mesh = pipeline.upload_mesh(&device, &animals_mesh);

        // Position camera with open 3/4 perspective showing animals, wheat crops, and cabin
        let farm_cam_ground_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(-3, y, 9));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            y
        };
        let farm_cam_pos = Vec3::new(-3.0, (farm_cam_ground_y as f32 + 2.0).max((farm_y + 2) as f32 + 1.2), 8.8);
        let farm_cam = Camera::new(
            farm_cam_pos,
            (-50.0_f32).to_radians(),
            (-12.0_f32).to_radians(),
        );
        let mut farm_uniform = CameraUniform::default();
        farm_cam.update_uniform_with_atmosphere(
            &mut farm_uniform,
            width as f32 / height as f32,
            &day_night,
        );
        pipeline.update_camera(&queue, &farm_uniform);

        let mut farm_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Farming & Animals Encoder"),
        });

        {
            let mut render_pass = farm_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Farming & Animals Pass"),
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

            if let Some(anim_mesh) = &animal_gpu_mesh {
                anim_mesh.draw(&mut render_pass);
            }
        }

        let mut farm_ui_mesh = UiMesh::new();
        let screen_w = width as f32;
        let screen_h = height as f32;
        // Hotbar slot 0 = Iron Hoe
        let mut farm_inventory = inventory.clone();
        farm_inventory.slots[0] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::IRON_HOE, 1);
        farm_inventory.slots[1] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WHEAT_SEEDS, 32);
        farm_inventory.slots[2] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BREAD, 5);
        farm_inventory.slots[3] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COOKED_STEAK, 8);

        UiBuilder::build_hud(
            &mut farm_ui_mesh,
            screen_w,
            screen_h,
            &farm_inventory,
            0,
            &item_registry,
            0.0,
            0.0,
            &stats,
        );

        if !farm_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Farm UI VB"),
                contents: bytemuck::cast_slice(&farm_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Farm UI IB"),
                contents: bytemuck::cast_slice(&farm_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = farm_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Farm UI Pass"),
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
            ui_pass.draw_indexed(0..farm_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        farm_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(farm_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut farm_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            farm_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "farming_and_animals_render.png",
            &farm_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!(
            "Successfully saved farming & animals showcase image to farming_and_animals_render.png"
        );
    }

    // Render verification of held 2D item (Bread) with no outlines
    {
        let mut bread_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bread Render Encoder"),
        });

        {
            let mut render_pass = bread_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bread Pass"),
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

        let mut bread_ui_mesh = UiMesh::new();
        let screen_w = width as f32;
        let screen_h = height as f32;
        let mut bread_inv = inventory.clone();
        bread_inv.slots[0] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BREAD, 8);
        UiBuilder::build_hud(
            &mut bread_ui_mesh,
            screen_w,
            screen_h,
            &bread_inv,
            0,
            &item_registry,
            0.0,
            0.0,
            &stats,
        );

        if !bread_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bread UI VB"),
                contents: bytemuck::cast_slice(&bread_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bread UI IB"),
                contents: bytemuck::cast_slice(&bread_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = bread_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bread UI Pass"),
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
            ui_pass.draw_indexed(0..bread_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        bread_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(bread_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut bread_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            bread_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "held_bread_render.png",
            &bread_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved held bread render to held_bread_render.png");
    }

    // Render verification of placed Chest and 3x3 Crafting Table, with player holding a 3D Chest block
    {
        let chest_cam_pos = Vec3::new(base_x as f32, (base_y + 2) as f32, (base_z + 8) as f32);
        let chest_cam = Camera::new(
            chest_cam_pos,
            -std::f32::consts::FRAC_PI_2,
            (-12.0_f32).to_radians(),
        );
        let mut chest_uniform = CameraUniform::default();
        chest_cam.update_uniform_with_atmosphere(
            &mut chest_uniform,
            width as f32 / height as f32,
            &day_night,
        );
        pipeline.update_camera(&queue, &chest_uniform);

        let mut chest_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Chest & Crafting Table Encoder"),
        });

        {
            let mut render_pass = chest_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Chest Pass"),
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
        }

        let mut chest_ui_mesh = UiMesh::new();
        let screen_w = width as f32;
        let screen_h = height as f32;
        let mut chest_inv = inventory.clone();
        chest_inv.slots[0] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CHEST, 4);
        UiBuilder::build_hud(
            &mut chest_ui_mesh,
            screen_w,
            screen_h,
            &chest_inv,
            0,
            &item_registry,
            0.0,
            0.0,
            &stats,
        );

        if !chest_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chest UI VB"),
                contents: bytemuck::cast_slice(&chest_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chest UI IB"),
                contents: bytemuck::cast_slice(&chest_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = chest_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Chest UI Pass"),
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
            ui_pass.draw_indexed(0..chest_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        chest_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(chest_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut chest_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            chest_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "chest_and_crafting_table_render.png",
            &chest_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved chest and crafting table showcase image to chest_and_crafting_table_render.png");
    }

    // Render verification of Chest Container UI Screen
    {
        let mut chest_ui_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Chest UI Screen Encoder"),
        });

        let mut chest_screen_mesh = UiMesh::new();
        let screen_w = width as f32;
        let screen_h = height as f32;

        let chest_slots = {
            let world_guard = world_manager.world().lock().unwrap();
            world_guard
                .get_chest(IVec3::new(base_x + 2, base_y + 1, base_z + 4))
                .cloned()
                .unwrap()
        };

        let mut inv_with_items = inventory.clone();
        inv_with_items.slots[0] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::DIAMOND_PICKAXE, 1);
        inv_with_items.slots[1] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::IRON_AXE, 1);
        inv_with_items.slots[2] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CHEST, 8);
        inv_with_items.slots[3] =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BREAD, 12);

        UiBuilder::build_chest_screen(
            &mut chest_screen_mesh,
            screen_w,
            screen_h,
            &chest_slots,
            &inv_with_items,
            0,
            &blockgame_core::item::ItemSlot::EMPTY,
            [screen_w * 0.5, screen_h * 0.5],
            &item_registry,
            None,
            None,
        );

        if !chest_screen_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chest Screen VB"),
                contents: bytemuck::cast_slice(&chest_screen_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chest Screen IB"),
                contents: bytemuck::cast_slice(&chest_screen_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = chest_ui_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Chest Screen Pass"),
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
            ui_pass.draw_indexed(0..chest_screen_mesh.indices.len() as u32, 0, 0..1);
        }

        chest_ui_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(chest_ui_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut chest_ui_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            chest_ui_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "chest_ui_render.png",
            &chest_ui_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved chest container UI showcase image to chest_ui_render.png");
    }

    // Also render JEI (Just Enough Items) Item Index & Recipe Modal Showcase
    {
        let mut jei_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("JEI UI Render Encoder"),
        });

        {
            let mut render_pass = jei_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Voxel JEI Pass"),
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
        }

        let mut jei_ui_mesh = UiMesh::new();
        let all_catalog: Vec<blockgame_core::item::ItemId> = item_registry
            .all_items()
            .into_iter()
            .map(|def| def.id)
            .collect();

        let jei_params = blockgame_render::ui::JeiParams {
            catalog_items: &all_catalog,
            page: 0,
            total_pages: 1,
            search_query: "",
            search_focused: false,
        };

        let display_recipe = blockgame_core::crafting::DisplayRecipe {
            name: "Diamond Pickaxe",
            category: blockgame_core::crafting::RecipeCategory::Crafting3x3,
            inputs: [
                Some(blockgame_core::item::ItemId::DIAMOND),
                Some(blockgame_core::item::ItemId::DIAMOND),
                Some(blockgame_core::item::ItemId::DIAMOND),
                None,
                Some(blockgame_core::item::ItemId::STICK),
                None,
                None,
                Some(blockgame_core::item::ItemId::STICK),
                None,
            ],
            output: blockgame_core::item::ItemSlot::new(
                blockgame_core::item::ItemId::DIAMOND_PICKAXE,
                1,
            ),
        };

        let modal_params = blockgame_render::ui::RecipeModalParams {
            target_item: blockgame_core::item::ItemId::DIAMOND_PICKAXE,
            recipe: &display_recipe,
            recipe_index: 0,
            total_recipes: 1,
            is_usage_mode: false,
        };

        // Position cursor over the output slot to highlight it and show tooltip
        let modal_w = 460.0;
        let modal_h = 340.0;
        let mx = (screen_w - modal_w) * 0.5;
        let my = (screen_h - modal_h) * 0.5;
        let box_x = mx + 20.0;
        let box_y = my + 76.0;
        let slot_size = UiBuilder::SLOT_SIZE;
        let slot_gap = UiBuilder::SLOT_GAP;
        let grid_start_x = box_x + 28.0;
        let grid_start_y = box_y + 24.0;
        let arrow_x = grid_start_x + 3.0 * (slot_size + slot_gap) + 24.0;
        let out_x = arrow_x + 64.0;
        let out_y = grid_start_y + (slot_size + slot_gap) * 1.5 - 28.0;
        let cursor_pos = [out_x + 28.0, out_y + 28.0];

        let mut crafting_grid = blockgame_core::crafting::CraftingGrid::new_3x3();
        crafting_grid.set(
            0,
            0,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 1),
        );
        let crafting_result = Some(blockgame_core::item::ItemSlot::new(
            blockgame_core::item::ItemId::PLANKS,
            4,
        ));

        UiBuilder::build_inventory_screen(
            &mut jei_ui_mesh,
            screen_w,
            screen_h,
            &inventory,
            0,
            &crafting_grid,
            crafting_result,
            &blockgame_core::item::ItemSlot::EMPTY,
            cursor_pos,
            &item_registry,
            Some(&jei_params),
            Some(&modal_params),
        );

        if !jei_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("JEI UI VB"),
                contents: bytemuck::cast_slice(&jei_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("JEI UI IB"),
                contents: bytemuck::cast_slice(&jei_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = jei_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("JEI UI Pass"),
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
            ui_pass.draw_indexed(0..jei_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        jei_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(jei_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut jei_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            jei_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "jei_recipe_browser_render.png",
            &jei_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!(
            "Successfully saved JEI recipe browser showcase image to jei_recipe_browser_render.png"
        );
    }

    // =========================================================================
    // 10. FURNACE UI SHOWCASE: Render Furnace Screen with fire flame & cooking
    // =========================================================================
    {
        let mut furnace_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Furnace UI Encoder"),
        });

        // 1. Draw 3D world scene in background
        {
            let mut rpass = furnace_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Furnace UI 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.52,
                            g: 0.75,
                            b: 0.96,
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

            rpass.set_pipeline(&pipeline.render_pipeline);
            rpass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            rpass.set_bind_group(1, &pipeline.texture_bind_group, &[]);

            for gpu_chunk in world_manager.gpu_chunks.values() {
                rpass.set_vertex_buffer(0, gpu_chunk.vertex_buffer.slice(..));
                rpass.set_index_buffer(gpu_chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..gpu_chunk.index_count, 0, 0..1);
            }
        }

        // 2. Build and Render Furnace Screen
        let mut furnace_ui_mesh = UiMesh::new();
        let screen_w = width as f32;
        let screen_h = height as f32;
        ui_pipeline.update_screen_size(&queue, screen_w, screen_h);

        let mut furnace_inv = Inventory::new(36);
        furnace_inv.set(
            0,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COBBLESTONE, 64),
        );
        furnace_inv.set(
            1,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COAL, 32),
        );
        furnace_inv.set(
            2,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RAW_COPPER, 18),
        );
        furnace_inv.set(
            3,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RAW_GOLD, 8),
        );
        furnace_inv.set(
            4,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RAW_BEEF, 10),
        );
        furnace_inv.set(
            5,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::IRON_PICKAXE, 1),
        );
        furnace_inv.set(
            6,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::FURNACE, 2),
        );
        furnace_inv.set(
            9,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 48),
        );
        furnace_inv.set(
            10,
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SAND, 32),
        );

        let mut furnace_state = blockgame_core::world::FurnaceState::new();
        furnace_state.input =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RAW_IRON, 16);
        furnace_state.fuel =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COAL, 5);
        furnace_state.output =
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::IRON_INGOT, 4);
        furnace_state.burn_time_remaining = 52.0;
        furnace_state.max_burn_time = 80.0;
        furnace_state.cook_progress = 5.2;

        let catalog: Vec<blockgame_core::item::ItemId> = item_registry
            .all_items()
            .into_iter()
            .map(|def| def.id)
            .collect();
        let jei_params = blockgame_render::ui::JeiParams {
            catalog_items: &catalog,
            page: 0,
            total_pages: ((catalog.len() + 47) / 48).max(1),
            search_query: "",
            search_focused: false,
        };

        let slot_size = UiBuilder::SLOT_SIZE;
        let slot_gap = UiBuilder::SLOT_GAP;
        let inv_w = 9.0 * (slot_size + slot_gap) - slot_gap;
        let start_x = (screen_w - inv_w) * 0.5;
        let center_x = start_x + (inv_w - slot_size) * 0.5 - 55.0;
        let input_y = screen_h * 0.5 - 220.0 + 10.0;
        let cursor_pos = [center_x + slot_size * 0.5, input_y + slot_size * 0.5]; // Hovering over Raw Iron input slot

        UiBuilder::build_furnace_screen(
            &mut furnace_ui_mesh,
            screen_w,
            screen_h,
            &furnace_state,
            &furnace_inv,
            0,
            &blockgame_core::item::ItemSlot::EMPTY,
            cursor_pos,
            &item_registry,
            Some(&jei_params),
            None,
        );

        if !furnace_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Furnace UI VB"),
                contents: bytemuck::cast_slice(&furnace_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Furnace UI IB"),
                contents: bytemuck::cast_slice(&furnace_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = furnace_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Furnace UI Pass"),
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
            ui_pass.draw_indexed(0..furnace_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        furnace_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(furnace_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut furnace_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            furnace_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "furnace_ui_render.png",
            &furnace_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Furnace UI showcase image to furnace_ui_render.png");
    }

    // Showcase: Water and New Terrain Features (Birch & Spruce trees, Water pond with sand bed, Tall grass)
    {
        let pond_x = -8;
        let pond_z = -4;
        let pond_y = base_y;

        let dummy_box = AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0));

        // Dig a natural pond and fill with translucent water and sand bed
        for dx in -3..=3 {
            for dz in -3..=3 {
                let dist_sq = dx * dx + dz * dz;
                if dist_sq <= 9 {
                    let wx = pond_x + dx;
                    let wz = pond_z + dz;
                    // Bed
                    world_manager.place_block(IVec3::new(wx, pond_y - 1, wz), BlockId::SAND, &dummy_box, &device, &pipeline);
                    // Water
                    world_manager.place_block(IVec3::new(wx, pond_y, wz), BlockId::WATER, &dummy_box, &device, &pipeline);
                    // Clear air above
                    world_manager.break_block(IVec3::new(wx, pond_y + 1, wz), &device, &pipeline);
                    world_manager.break_block(IVec3::new(wx, pond_y + 2, wz), &device, &pipeline);
                }
            }
        }

        // Place Birch & Spruce logs and planks on the shoreline
        world_manager.place_block(IVec3::new(pond_x + 4, pond_y + 1, pond_z), BlockId::BIRCH_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x + 4, pond_y + 2, pond_z), BlockId::BIRCH_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x + 3, pond_y + 1, pond_z + 2), BlockId::SPRUCE_PLANKS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x + 2, pond_y + 1, pond_z + 3), BlockId::BIRCH_PLANKS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x - 3, pond_y + 1, pond_z + 2), BlockId::TALL_GRASS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x - 2, pond_y + 1, pond_z + 3), BlockId::TALL_GRASS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(pond_x + 1, pond_y + 1, pond_z - 4), BlockId::TALL_GRASS, &dummy_box, &device, &pipeline);

        // Morning sunlight
        let mut pond_day_night = DayNightCycle::new();
        pond_day_night.set_time(6000.0); // Bright sunny day
        let pond_sky = pond_day_night.sky_color();

        let pond_cam_pos = Vec3::new((pond_x as f32) - 4.5, (pond_y as f32) + 3.2, (pond_z as f32) + 5.5);
        let pond_cam = Camera::new(pond_cam_pos, (-45.0_f32).to_radians(), (-20.0_f32).to_radians());
        let mut pond_uniform = CameraUniform::default();
        pond_cam.update_uniform_with_atmosphere(&mut pond_uniform, width as f32 / height as f32, &pond_day_night);
        pipeline.update_camera(&queue, &pond_uniform);

        let mut water_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Water & Terrain Encoder"),
        });

        {
            let mut render_pass = water_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Water & Terrain Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: pond_sky[0] as f64,
                            g: pond_sky[1] as f64,
                            b: pond_sky[2] as f64,
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

            // Water pass
            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with Birch Log in hotbar
        let mut water_ui_mesh = UiMesh::new();
        let stats = blockgame_core::PlayerStats::new();
        let mut water_inv = inventory.clone();
        water_inv.slots[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BIRCH_LOG, 16);
        water_inv.slots[1] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SPRUCE_PLANKS, 32);
        water_inv.slots[2] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TALL_GRASS, 8);
        UiBuilder::build_hud(&mut water_ui_mesh, screen_w, screen_h, &water_inv, 0, &item_registry, 0.0, 0.0, &stats);

        if !water_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water UI VB"),
                contents: bytemuck::cast_slice(&water_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water UI IB"),
                contents: bytemuck::cast_slice(&water_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = water_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Water UI Pass"),
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
            ui_pass.draw_indexed(0..water_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        water_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(water_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut water_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            water_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "terrain_and_water_render.png",
            &water_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Terrain and Water showcase image to terrain_and_water_render.png");

        // 12. Underwater Submersion & Oxygen Bubbles Showcase Render
        let underwater_cam_pos = Vec3::new(pond_x as f32, (pond_y as f32) + 0.35, pond_z as f32);
        let underwater_cam = Camera::new(underwater_cam_pos, (-15.0_f32).to_radians(), (10.0_f32).to_radians());
        let mut underwater_uniform = CameraUniform::default();
        underwater_cam.update_uniform_with_atmosphere_water(
            &mut underwater_uniform,
            width as f32 / height as f32,
            &pond_day_night,
            true, // is_underwater = true triggers underwater fog & filtered sunlight
        );
        pipeline.update_camera(&queue, &underwater_uniform);

        let mut underwater_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Underwater Encoder"),
        });

        {
            let mut render_pass = underwater_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Underwater 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.22,
                            b: 0.45,
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

            // 1. Opaque geometry pass (lakebed sand, stone, shoreline, trees)
            render_pass.set_pipeline(&pipeline.render_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render(&mut render_pass);

            // 2. Translucent water geometry pass (water surface visible from underneath)
            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with Underwater Screen Overlay and Oxygen Bubbles
        let mut underwater_ui_mesh = UiMesh::new();
        let mut underwater_stats = blockgame_core::PlayerStats::new();
        underwater_stats.is_submerged = true;
        underwater_stats.breath = 10.5; // 7 full bubbles remaining
        UiBuilder::build_hud(
            &mut underwater_ui_mesh,
            screen_w,
            screen_h,
            &water_inv,
            0,
            &item_registry,
            0.0,
            0.0,
            &underwater_stats,
        );

        if !underwater_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Underwater UI VB"),
                contents: bytemuck::cast_slice(&underwater_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Underwater UI IB"),
                contents: bytemuck::cast_slice(&underwater_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = underwater_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Underwater UI Pass"),
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
            ui_pass.draw_indexed(0..underwater_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        underwater_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(underwater_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut underwater_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            underwater_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "underwater_render.png",
            &underwater_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Underwater showcase image to underwater_render.png");
    }

    // 13. Biome Expansion Showcase Render (Acacia, Cactus, Sandstone, Snow, Ice, Clay, Lily Pad)
    {
        let bio_x = -14;
        let bio_z = 0;
        let bio_y = base_y;
        let dummy_box = AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0));

        // Desert Section with Cactus and Sandstone
        world_manager.place_block(IVec3::new(bio_x, bio_y, bio_z), BlockId::SANDSTONE, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x, bio_y + 1, bio_z), BlockId::SAND, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x, bio_y + 2, bio_z), BlockId::CACTUS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x, bio_y + 3, bio_z), BlockId::CACTUS, &dummy_box, &device, &pipeline);

        // Savanna Acacia Section
        world_manager.place_block(IVec3::new(bio_x + 2, bio_y + 1, bio_z - 1), BlockId::ACACIA_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x + 2, bio_y + 2, bio_z - 1), BlockId::ACACIA_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x + 2, bio_y + 3, bio_z - 1), BlockId::ACACIA_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x + 1, bio_y + 1, bio_z - 1), BlockId::ACACIA_PLANKS, &dummy_box, &device, &pipeline);

        // Tundra Snow & Ice Section
        world_manager.place_block(IVec3::new(bio_x + 3, bio_y + 1, bio_z + 1), BlockId::SNOW_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x + 2, bio_y + 1, bio_z + 2), BlockId::ICE, &dummy_box, &device, &pipeline);

        // Swamp Water Pool with Clay & Lily Pad
        world_manager.place_block(IVec3::new(bio_x - 1, bio_y, bio_z + 1), BlockId::CLAY, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x - 2, bio_y, bio_z + 1), BlockId::WATER, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(bio_x - 2, bio_y + 1, bio_z + 1), BlockId::LILY_PAD, &dummy_box, &device, &pipeline);

        // Camera positioned to view all new biome features
        let bio_day_night = DayNightCycle::new();
        let bio_cam_pos = Vec3::new((bio_x as f32) + 2.5, (bio_y as f32) + 2.8, (bio_z as f32) + 4.2);
        let bio_cam = Camera::new(bio_cam_pos, (-140.0_f32).to_radians(), (-15.0_f32).to_radians());
        let mut bio_uniform = CameraUniform::default();
        bio_cam.update_uniform_with_atmosphere_water(
            &mut bio_uniform,
            width as f32 / height as f32,
            &bio_day_night,
            false,
        );
        pipeline.update_camera(&queue, &bio_uniform);

        let mut bio_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Biome Expansion Encoder"),
        });

        {
            let mut render_pass = bio_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Biome Expansion 3D Pass"),
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
            world_manager.render(&mut render_pass);

            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with new biome items in hotbar
        let mut bio_ui_mesh = UiMesh::new();
        let stats = blockgame_core::PlayerStats::new();
        let mut bio_inv = inventory.clone();
        bio_inv.slots[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::ACACIA_PLANKS, 64);
        bio_inv.slots[1] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CACTUS, 16);
        bio_inv.slots[2] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SANDSTONE, 32);
        bio_inv.slots[3] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SNOW_BLOCK, 16);
        bio_inv.slots[4] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::ICE, 8);
        bio_inv.slots[5] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CLAY, 24);
        bio_inv.slots[6] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::LILY_PAD, 12);
        bio_inv.slots[7] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::GRAVEL, 32);
        UiBuilder::build_hud(&mut bio_ui_mesh, screen_w, screen_h, &bio_inv, 0, &item_registry, 0.0, 0.0, &stats);

        if !bio_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Biome UI VB"),
                contents: bytemuck::cast_slice(&bio_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Biome UI IB"),
                contents: bytemuck::cast_slice(&bio_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = bio_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Biome UI Pass"),
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
            ui_pass.draw_indexed(0..bio_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        bio_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(bio_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut bio_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            bio_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "biome_expansion_render.png",
            &bio_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Biome Expansion showcase image to biome_expansion_render.png");
    }

    // 14. Phase 3 Showcase Render (Jungle, Badlands Strata, and Biome Atmospheric Fog/Tint)
    {
        let p3_x = -16;
        let p3_z = -4;
        let p3_y = base_y;
        let dummy_box = AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0));

        // Badlands Canyon Sedimentary Strata (Terracotta and Red Sand)
        world_manager.place_block(IVec3::new(p3_x, p3_y, p3_z), BlockId::TERRACOTTA, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x, p3_y + 1, p3_z), BlockId::RED_SAND, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x, p3_y + 2, p3_z), BlockId::TERRACOTTA, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 1, p3_y + 1, p3_z), BlockId::RED_SAND, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 1, p3_y + 2, p3_z), BlockId::TERRACOTTA, &dummy_box, &device, &pipeline);

        // Jungle Tree Section (Jungle Log, Leaves, Planks)
        world_manager.place_block(IVec3::new(p3_x + 3, p3_y + 1, p3_z + 1), BlockId::JUNGLE_PLANKS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 4, p3_y + 1, p3_z + 1), BlockId::JUNGLE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 4, p3_y + 2, p3_z + 1), BlockId::JUNGLE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 4, p3_y + 3, p3_z + 1), BlockId::JUNGLE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 4, p3_y + 4, p3_z + 1), BlockId::JUNGLE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 3, p3_y + 3, p3_z + 1), BlockId::JUNGLE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p3_x + 5, p3_y + 3, p3_z + 1), BlockId::JUNGLE_LEAVES, &dummy_box, &device, &pipeline);

        // Camera viewing the Badlands & Jungle scene with Badlands/Jungle atmospheric tinting
        let p3_day_night = DayNightCycle::new();
        let p3_cam_pos = Vec3::new((p3_x as f32) + 2.0, (p3_y as f32) + 2.5, (p3_z as f32) + 4.5);
        let p3_cam = Camera::new(p3_cam_pos, (-140.0_f32).to_radians(), (-12.0_f32).to_radians());
        let mut p3_uniform = CameraUniform::default();
        let badlands_fog = [0.85, 0.58, 0.44];
        let badlands_tint = [1.08, 0.88, 0.75];
        p3_cam.update_uniform_with_atmosphere_water_tint(
            &mut p3_uniform,
            width as f32 / height as f32,
            &p3_day_night,
            false,
            badlands_fog,
            badlands_tint,
        );
        pipeline.update_camera(&queue, &p3_uniform);

        let mut p3_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Phase 3 Biome Atmosphere Encoder"),
        });

        {
            let mut render_pass = p3_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 3 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.82,
                            g: 0.62,
                            b: 0.50,
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

            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with Phase 3 items in hotbar
        let mut p3_ui_mesh = UiMesh::new();
        let stats = blockgame_core::PlayerStats::new();
        let mut p3_inv = inventory.clone();
        p3_inv.slots[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::JUNGLE_PLANKS, 64);
        p3_inv.slots[1] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::JUNGLE_LOG, 32);
        p3_inv.slots[2] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::JUNGLE_LEAVES, 16);
        p3_inv.slots[3] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TERRACOTTA, 48);
        p3_inv.slots[4] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RED_SAND, 64);
        p3_inv.slots[5] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SANDSTONE, 32);
        p3_inv.slots[6] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CLAY, 16);
        p3_inv.slots[7] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CACTUS, 16);
        p3_inv.slots[8] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::ACACIA_PLANKS, 64);
        UiBuilder::build_hud(&mut p3_ui_mesh, screen_w, screen_h, &p3_inv, 0, &item_registry, 0.0, 0.0, &stats);

        if !p3_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 3 UI VB"),
                contents: bytemuck::cast_slice(&p3_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 3 UI IB"),
                contents: bytemuck::cast_slice(&p3_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = p3_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 3 UI Pass"),
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
            ui_pass.draw_indexed(0..p3_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        p3_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(p3_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut p3_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            p3_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "phase_3_biome_atmosphere_render.png",
            &p3_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Phase 3 Biome Atmosphere showcase image to phase_3_biome_atmosphere_render.png");
    }

    // 15. Phase 4 Showcase Render (Rivers, Taiga, Mushroom Fields & Fungal Strata)
    {
        let p4_x = -6;
        let p4_z = 4;
        let dummy_box = AABB::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(101.0, 101.0, 101.0));
        let p4_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(p4_x, y, p4_z));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            y
        };

        // 1. Mushroom Fields Terrain (Mycelium ground + Giant Red & Brown Mushrooms)
        for dx in -4..=0 {
            for dz in -3..=5 {
                world_manager.place_block(IVec3::new(p4_x + dx, p4_y, p4_z + dz), BlockId::MYCELIUM, &dummy_box, &device, &pipeline);
            }
        }
        // Giant Red Mushroom
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 1, p4_z - 1), BlockId::MUSHROOM_STEM, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 2, p4_z - 1), BlockId::MUSHROOM_STEM, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 3, p4_z - 1), BlockId::MUSHROOM_STEM, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 4, p4_z - 1), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 3, p4_y + 4, p4_z - 1), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 4, p4_z - 1), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 4, p4_z - 2), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 4, p4_z), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 3, p4_y + 3, p4_z - 1), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 3, p4_z - 1), BlockId::RED_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);

        // Giant Brown Mushroom
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 1, p4_z + 1), BlockId::MUSHROOM_STEM, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 2, p4_z + 1), BlockId::MUSHROOM_STEM, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 3, p4_z + 1), BlockId::BROWN_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 2, p4_y + 3, p4_z + 1), BlockId::BROWN_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x, p4_y + 3, p4_z + 1), BlockId::BROWN_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 3, p4_z), BlockId::BROWN_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x - 1, p4_y + 3, p4_z + 2), BlockId::BROWN_MUSHROOM_BLOCK, &dummy_box, &device, &pipeline);

        // 2. Taiga Terrain (Podzol floor + Mossy Cobblestone boulders + Spruce Tree)
        for dx in 1..=5 {
            for dz in -3..=5 {
                world_manager.place_block(IVec3::new(p4_x + dx, p4_y, p4_z + dz), BlockId::PODZOL, &dummy_box, &device, &pipeline);
            }
        }
        // Mossy Cobblestone boulder on Podzol
        world_manager.place_block(IVec3::new(p4_x + 1, p4_y + 1, p4_z + 1), BlockId::MOSSY_COBBLESTONE, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 2, p4_y + 1, p4_z + 1), BlockId::MOSSY_COBBLESTONE, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 1, p4_y + 1, p4_z), BlockId::MOSSY_COBBLESTONE, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 1, p4_y + 2, p4_z + 1), BlockId::MOSSY_COBBLESTONE, &dummy_box, &device, &pipeline);

        // Spruce tree on Podzol
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 1, p4_z - 1), BlockId::SPRUCE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 2, p4_z - 1), BlockId::SPRUCE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 3, p4_z - 1), BlockId::SPRUCE_LOG, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 4, p4_z - 1), BlockId::SPRUCE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 2, p4_y + 3, p4_z - 1), BlockId::SPRUCE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 4, p4_y + 3, p4_z - 1), BlockId::SPRUCE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 3, p4_z - 2), BlockId::SPRUCE_LEAVES, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p4_x + 3, p4_y + 3, p4_z), BlockId::SPRUCE_LEAVES, &dummy_box, &device, &pipeline);

        // 3. Camera positioned right in front of the scene at eye level
        let p4_day_night = DayNightCycle::new();
        let p4_cam_pos = Vec3::new((p4_x as f32) + 0.5, (p4_y as f32) + 2.0, (p4_z as f32) + 3.6);
        let p4_cam = Camera::new(p4_cam_pos, (-90.0_f32).to_radians(), (-16.0_f32).to_radians());
        let mut p4_uniform = CameraUniform::default();
        let mushroom_fog = [0.72, 0.60, 0.78];
        let mushroom_tint = [1.04, 0.92, 1.06];
        p4_cam.update_uniform_with_atmosphere_water_tint(
            &mut p4_uniform,
            width as f32 / height as f32,
            &p4_day_night,
            false,
            mushroom_fog,
            mushroom_tint,
        );
        pipeline.update_camera(&queue, &p4_uniform);

        let mut p4_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Phase 4 River Mushrooms Encoder"),
        });

        {
            let mut render_pass = p4_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 4 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.70,
                            g: 0.60,
                            b: 0.76,
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

            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with Phase 4 items in hotbar
        let mut p4_ui_mesh = UiMesh::new();
        let stats = blockgame_core::PlayerStats::new();
        let mut p4_inv = inventory.clone();
        p4_inv.slots[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::PODZOL, 64);
        p4_inv.slots[1] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::MOSSY_COBBLESTONE, 32);
        p4_inv.slots[2] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::MYCELIUM, 64);
        p4_inv.slots[3] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::RED_MUSHROOM_BLOCK, 48);
        p4_inv.slots[4] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::BROWN_MUSHROOM_BLOCK, 48);
        p4_inv.slots[5] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::MUSHROOM_STEM, 32);
        p4_inv.slots[6] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SPRUCE_LOG, 16);
        p4_inv.slots[7] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SPRUCE_LEAVES, 16);
        p4_inv.slots[8] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COBBLESTONE, 64);
        UiBuilder::build_hud(&mut p4_ui_mesh, screen_w, screen_h, &p4_inv, 0, &item_registry, 0.0, 0.0, &stats);

        if !p4_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 4 UI VB"),
                contents: bytemuck::cast_slice(&p4_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 4 UI IB"),
                contents: bytemuck::cast_slice(&p4_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = p4_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 4 UI Pass"),
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
            ui_pass.draw_indexed(0..p4_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        p4_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(p4_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut p4_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            p4_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "phase_4_biome_river_mushrooms_render.png",
            &p4_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Phase 4 Biome River & Mushrooms showcase image to phase_4_biome_river_mushrooms_render.png");
    }

    // Pass 16: Phase 5 - Dark Forest (Roofed Forest) & Coral Reefs Showcase
    {
        let p5_x = -6_i32;
        let p5_z = 4_i32;
        let p5_y = {
            let world_guard = world_manager.world().lock().unwrap();
            let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
            while y > 1 {
                let block = world_guard.get_block(IVec3::new(p5_x, y, p5_z));
                if !block.is_air() {
                    break;
                }
                y -= 1;
            }
            y
        };

        let dummy_box = AABB::new(Vec3::new(999.0, 999.0, 999.0), Vec3::new(0.6, 1.8, 0.6));

        // Clear showcase area of any random natural terrain obstructions
        {
            let mut world = world_manager.world().lock().unwrap();
            for dx in -5..=6 {
                for dz in -5..=5 {
                    for dy in 0..=6 {
                        world.set_block(IVec3::new(p5_x + dx, p5_y + dy, p5_z + dz), BlockId::AIR);
                    }
                }
            }
        }

        // 1. Dark Forest Environment (left side): Grass, Dark Oak 2x2 Tree, Dark Oak Planks
        for dx in -4..=0 {
            for dz in -4..=1 {
                world_manager.place_block(IVec3::new(p5_x + dx, p5_y, p5_z + dz), BlockId::GRASS, &dummy_box, &device, &pipeline);
            }
        }
        // Dark Oak 2x2 Trunk
        for ty in 1..=4 {
            world_manager.place_block(IVec3::new(p5_x - 2, p5_y + ty, p5_z - 2), BlockId::DARK_OAK_LOG, &dummy_box, &device, &pipeline);
            world_manager.place_block(IVec3::new(p5_x - 1, p5_y + ty, p5_z - 2), BlockId::DARK_OAK_LOG, &dummy_box, &device, &pipeline);
            world_manager.place_block(IVec3::new(p5_x - 2, p5_y + ty, p5_z - 1), BlockId::DARK_OAK_LOG, &dummy_box, &device, &pipeline);
            world_manager.place_block(IVec3::new(p5_x - 1, p5_y + ty, p5_z - 1), BlockId::DARK_OAK_LOG, &dummy_box, &device, &pipeline);
        }
        // Dark Oak Leaves Roof Canopy
        for dy in 3..=5 {
            let rad: i32 = if dy == 5 { 1 } else { 2 };
            for dz in -rad..=rad {
                for dx in -rad..=rad {
                    world_manager.place_block(IVec3::new(p5_x - 1 + dx, p5_y + dy, p5_z - 1 + dz), BlockId::DARK_OAK_LEAVES, &dummy_box, &device, &pipeline);
                }
            }
        }
        // Dark Oak Planks placed on ground
        world_manager.place_block(IVec3::new(p5_x - 3, p5_y + 1, p5_z), BlockId::DARK_OAK_PLANKS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x - 2, p5_y + 1, p5_z), BlockId::DARK_OAK_PLANKS, &dummy_box, &device, &pipeline);

        // 2. Coral Reef Lagoon (right side): Sand seabed, Coral pillars, Sea Lantern, Prismarine, Seagrass, and Water
        for dx in 1..=5 {
            for dz in -4..=1 {
                world_manager.place_block(IVec3::new(p5_x + dx, p5_y - 1, p5_z + dz), BlockId::SAND, &dummy_box, &device, &pipeline);
            }
        }
        // Coral Formations & Prismarine & Sea Lantern
        world_manager.place_block(IVec3::new(p5_x + 1, p5_y, p5_z), BlockId::PRISMARINE, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 2, p5_y, p5_z - 1), BlockId::CORAL_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 2, p5_y + 1, p5_z - 1), BlockId::CORAL_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 2, p5_y + 2, p5_z - 1), BlockId::SEA_LANTERN, &dummy_box, &device, &pipeline);

        world_manager.place_block(IVec3::new(p5_x + 3, p5_y, p5_z - 2), BlockId::TUBE_CORAL_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 3, p5_y + 1, p5_z - 2), BlockId::TUBE_CORAL_BLOCK, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 4, p5_y, p5_z - 2), BlockId::TUBE_CORAL_BLOCK, &dummy_box, &device, &pipeline);

        world_manager.place_block(IVec3::new(p5_x + 3, p5_y, p5_z), BlockId::SEAGRASS, &dummy_box, &device, &pipeline);
        world_manager.place_block(IVec3::new(p5_x + 4, p5_y, p5_z - 1), BlockId::SEAGRASS, &dummy_box, &device, &pipeline);

        // Water basin in the lagoon
        for dx in 1..=4 {
            for dz in -3..=1 {
                let p = IVec3::new(p5_x + dx, p5_y, p5_z + dz);
                let b = {
                    let world = world_manager.world().lock().unwrap();
                    world.get_block(p)
                };
                if b.is_air() {
                    world_manager.place_block(p, BlockId::WATER, &dummy_box, &device, &pipeline);
                }
            }
        }

        // 3. Camera positioned in front of the scene
        let p5_day_night = DayNightCycle::new();
        let p5_cam_pos = Vec3::new((p5_x as f32) + 0.6, (p5_y as f32) + 2.3, (p5_z as f32) + 3.2);
        let p5_cam = Camera::new(p5_cam_pos, (-90.0_f32).to_radians(), (-20.0_f32).to_radians());
        let mut p5_uniform = CameraUniform::default();
        let reef_fog = [0.26, 0.58, 0.68];
        let reef_tint = [0.94, 1.04, 1.08];
        p5_cam.update_uniform_with_atmosphere_water_tint(
            &mut p5_uniform,
            width as f32 / height as f32,
            &p5_day_night,
            false,
            reef_fog,
            reef_tint,
        );
        pipeline.update_camera(&queue, &p5_uniform);

        let mut p5_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Phase 5 Dark Forest Reef Encoder"),
        });

        {
            let mut render_pass = p5_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 5 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.28,
                            g: 0.55,
                            b: 0.65,
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

            render_pass.set_pipeline(&pipeline.water_pipeline);
            render_pass.set_bind_group(0, &pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &pipeline.texture_bind_group, &[]);
            world_manager.render_water(&mut render_pass);
        }

        // Draw HUD overlay with Phase 5 items in hotbar
        let mut p5_ui_mesh = UiMesh::new();
        let stats = blockgame_core::PlayerStats::new();
        let mut p5_inv = inventory.clone();
        p5_inv.slots[0] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::DARK_OAK_LOG, 64);
        p5_inv.slots[1] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::DARK_OAK_PLANKS, 64);
        p5_inv.slots[2] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::DARK_OAK_LEAVES, 64);
        p5_inv.slots[3] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::CORAL_BLOCK, 64);
        p5_inv.slots[4] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TUBE_CORAL_BLOCK, 64);
        p5_inv.slots[5] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::PRISMARINE, 64);
        p5_inv.slots[6] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SEA_LANTERN, 64);
        p5_inv.slots[7] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::SEAGRASS, 64);
        p5_inv.slots[8] = blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::MOSSY_COBBLESTONE, 64);
        UiBuilder::build_hud(&mut p5_ui_mesh, screen_w, screen_h, &p5_inv, 6, &item_registry, 0.0, 0.0, &stats);

        if !p5_ui_mesh.indices.is_empty() {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 5 UI VB"),
                contents: bytemuck::cast_slice(&p5_ui_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Phase 5 UI IB"),
                contents: bytemuck::cast_slice(&p5_ui_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let mut ui_pass = p5_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Phase 5 UI Pass"),
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
            ui_pass.draw_indexed(0..p5_ui_mesh.indices.len() as u32, 0, 0..1);
        }

        p5_encoder.copy_texture_to_buffer(
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

        queue.submit(Some(p5_encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut p5_image_bytes = Vec::with_capacity((width * height * 4) as usize);

        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            p5_image_bytes.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging_buffer.unmap();

        image::save_buffer(
            "phase_5_dark_forest_coral_reefs_render.png",
            &p5_image_bytes,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )?;
        log::info!("Successfully saved Phase 5 Dark Forest & Coral Reefs showcase image to phase_5_dark_forest_coral_reefs_render.png");
    }

    Ok(())
}
