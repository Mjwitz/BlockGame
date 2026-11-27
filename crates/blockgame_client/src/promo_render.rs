use std::path::Path;
use std::sync::Arc;

use blockgame_core::{
    animal::{AnimalEntity, AnimalKind},
    block::{BlockId, BlockRegistry},
    character::starter_presets,
    item::{ItemId, ItemRegistry},
    net::PlayerModelState,
    physics::AABB,
};
use blockgame_render::{
    atlas::TextureAtlas,
    camera::{Camera, CameraUniform},
    character_pipeline::CharacterPipeline,
    create_depth_texture, DayNightCycle, VoxelPipeline,
};
use glam::{IVec3, Vec3};
use wgpu::util::DeviceExt;

use crate::world_manager::WorldManager;

struct SceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: VoxelPipeline,
    character_pipeline: CharacterPipeline,
    render_texture: wgpu::Texture,
    render_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    staging_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    bytes_per_row: u32,
}

impl SceneRenderer {
    async fn new(width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            backend_options: Default::default(),
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .or(instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: true,
                })
                .await)
            .ok_or("Failed to find suitable wgpu adapter (even fallback)")?;

        log::info!("Promo renderer adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Promo Screenshot Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let render_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Promo Render Target"),
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
            label: Some("Promo Staging Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let atlas = TextureAtlas::generate_default();
        let pipeline = VoxelPipeline::new(&device, &queue, target_format, &atlas);
        let character_pipeline =
            CharacterPipeline::new(&device, target_format, &pipeline.camera_bind_group_layout);

        Ok(Self {
            device,
            queue,
            pipeline,
            character_pipeline,
            render_texture,
            render_view,
            depth_view,
            staging_buffer,
            width,
            height,
            bytes_per_row,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_and_save(
        &self,
        world_manager: &WorldManager,
        camera: &Camera,
        day_night: &DayNightCycle,
        animals: &[AnimalEntity],
        turbines: &[(IVec3, f32, u8)],
        players: &[PlayerModelState],
        item_registry: &ItemRegistry,
        output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pixels = self.render_frame_rgba(
            world_manager,
            camera,
            day_night,
            animals,
            turbines,
            players,
            item_registry,
            1.35,
        )?;
        image::save_buffer(
            output_path,
            &pixels,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )?;
        log::info!("Saved promo screenshot: {}", output_path.display());
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_frame_rgba(
        &self,
        world_manager: &WorldManager,
        camera: &Camera,
        day_night: &DayNightCycle,
        animals: &[AnimalEntity],
        turbines: &[(IVec3, f32, u8)],
        players: &[PlayerModelState],
        item_registry: &ItemRegistry,
        time_secs: f32,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let aspect = self.width as f32 / self.height as f32;
        let mut camera_uniform = CameraUniform::default();
        camera.update_uniform_with_atmosphere(&mut camera_uniform, aspect, day_night);
        self.pipeline.update_camera(&self.queue, &camera_uniform);

        let sky = day_night.sky_color();

        let celestial_mesh =
            blockgame_render::build_celestial_mesh(camera.position, day_night.sun_direction());
        let celestial_gpu = self.pipeline.upload_mesh(&self.device, &celestial_mesh);

        let animals_mesh = blockgame_render::build_animals_mesh(animals);
        let animals_gpu = self.pipeline.upload_mesh(&self.device, &animals_mesh);

        let rotor_mesh = blockgame_render::build_wind_turbine_rotors_mesh(turbines, time_secs);
        let rotor_gpu = self.pipeline.upload_mesh(&self.device, &rotor_mesh);

        let held_mesh =
            blockgame_render::build_player_held_items_mesh(players, item_registry, time_secs);
        let held_gpu = self.pipeline.upload_mesh(&self.device, &held_mesh);

        let char_mesh = blockgame_render::build_character_rig_mesh(players, time_secs);
        let char_buffers = if !char_mesh.indices.is_empty() {
            let vb = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Promo Char VB"),
                    contents: bytemuck::cast_slice(&char_mesh.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ib = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Promo Char IB"),
                    contents: bytemuck::cast_slice(&char_mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            Some((vb, ib, char_mesh.indices.len() as u32))
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Promo Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Promo 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_view,
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
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 1. Sky celestial sun & moon
            render_pass.set_pipeline(&self.pipeline.celestial_pipeline);
            render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.pipeline.texture_bind_group, &[]);
            if let Some(m) = &celestial_gpu {
                render_pass.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                render_pass.set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..m.index_count, 0, 0..1);
            }

            // 2. Opaque world chunks & entities
            render_pass.set_pipeline(&self.pipeline.render_pipeline);
            render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.pipeline.texture_bind_group, &[]);
            let frustum = camera.build_frustum(aspect);
            world_manager.render_with_frustum(&mut render_pass, Some(&frustum));

            for opt_mesh in [&animals_gpu, &rotor_gpu, &held_gpu] {
                if let Some(m) = opt_mesh {
                    render_pass.set_vertex_buffer(0, m.vertex_buffer.slice(..));
                    render_pass
                        .set_index_buffer(m.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..m.index_count, 0, 0..1);
                }
            }

            // 3. Stylized low-poly 3D characters
            if let Some((vb, ib, count)) = &char_buffers {
                render_pass.set_pipeline(&self.character_pipeline.render_pipeline);
                render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..*count, 0, 0..1);
            }

            // 4. Translucent water & ice pass
            render_pass.set_pipeline(&self.pipeline.water_pipeline);
            render_pass.set_bind_group(0, &self.pipeline.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.pipeline.texture_bind_group, &[]);
            world_manager.render_water_with_frustum(
                &mut render_pass,
                Some(&frustum),
                Some(camera.position),
            );
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.render_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()??;

        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in 0..self.height {
            let start = (row * self.bytes_per_row) as usize;
            let end = start + (self.width * 4) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }
        drop(data);
        self.staging_buffer.unmap();

        Ok(pixels)
    }
}

fn surface_block_and_y(world_manager: &WorldManager, x: i32, z: i32) -> (i32, BlockId) {
    let world_guard = world_manager.world().lock().unwrap();
    let mut y = (blockgame_core::chunk::VERTICAL_CHUNKS * 16) - 2;
    while y > 1 {
        let b = world_guard.get_block(IVec3::new(x, y, z));
        if !b.is_air()
            && !b.is_leaves()
            && !b.is_log()
            && !b.is_flower()
            && b != BlockId::TALL_GRASS
        {
            return (y, b);
        }
        y -= 1;
    }
    (64, BlockId::AIR)
}

/// Find a natural, dry, relatively flat ground clearing embedded in real terrain.
fn find_dry_clearing(wm: &WorldManager, search_radius: i32) -> IVec3 {
    let mut best = IVec3::new(0, 64, 0);
    let mut best_score = i32::MAX;

    for cx in (-search_radius..=search_radius).step_by(4) {
        for cz in (-search_radius..=search_radius).step_by(4) {
            let (cy, cb) = surface_block_and_y(wm, cx, cz);
            if cb.is_water() || cb == BlockId::ICE || cb == BlockId::AIR {
                continue;
            }
            let mut min_y = cy;
            let mut max_y = cy;
            let mut water_hits = 0;
            for dx in (-8..=8).step_by(4) {
                for dz in (-8..=8).step_by(4) {
                    let (y, b) = surface_block_and_y(wm, cx + dx, cz + dz);
                    if b.is_water() || b == BlockId::ICE {
                        water_hits += 1;
                    }
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
            if water_hits > 0 {
                continue;
            }
            let relief = max_y - min_y;
            let bonus = if cb == BlockId::GRASS { 0 } else { 4 };
            let score = relief * 3 + bonus;
            if score < best_score {
                best_score = score;
                best = IVec3::new(cx, cy, cz);
            }
        }
    }
    best
}

/// Find a natural dry shoreline next to real water for a dock.
fn find_shoreline(wm: &WorldManager, search_radius: i32) -> Option<(IVec3, IVec3)> {
    for cx in (-search_radius..=search_radius).step_by(2) {
        for cz in (-search_radius..=search_radius).step_by(2) {
            let (cy, cb) = surface_block_and_y(wm, cx, cz);
            if cb.is_water() || cb == BlockId::ICE || cb == BlockId::AIR {
                continue;
            }
            // Check 4 cardinal directions for open water 2..12 blocks out
            for (dx, dz) in [(0, -1), (0, 1), (1, 0), (-1, 0)] {
                let mut water_count = 0;
                let mut water_y = cy;
                for step in 2..=11 {
                    let (wy, wb) = surface_block_and_y(wm, cx + dx * step, cz + dz * step);
                    if wb.is_water() {
                        water_count += 1;
                        water_y = wy;
                    }
                }
                if water_count >= 9 && (cy - water_y).abs() <= 2 {
                    return Some((IVec3::new(cx, water_y, cz), IVec3::new(dx, 0, dz)));
                }
            }
        }
    }
    None
}

fn look_at_camera(eye: Vec3, target: Vec3) -> Camera {
    let dir = (target - eye).normalize();
    let pitch = dir.y.clamp(-0.98, 0.98).asin();
    let yaw = dir.z.atan2(dir.x);
    Camera::new(eye, yaw, pitch)
}

fn set_block(
    wm: &mut WorldManager,
    pos: IVec3,
    block: BlockId,
    facing: u8,
    device: &wgpu::Device,
    pipeline: &VoxelPipeline,
) {
    let dummy = AABB::new(Vec3::splat(9999.0), Vec3::splat(10000.0));
    // Always break existing non-air block first so place_block_with_facing never rejects replacement!
    let existing = {
        let g = wm.world().lock().unwrap();
        g.get_block(pos)
    };
    if !existing.is_air() {
        wm.break_block(pos, device, pipeline);
    }
    if !block.is_air() {
        if facing != 0 {
            wm.place_block_with_facing(pos, block, facing, &dummy, device, pipeline);
        } else {
            wm.place_block(pos, block, &dummy, device, pipeline);
        }
    }
}

/// Gently blend a small flat yard into natural terrain without creating a floating box.
fn grade_yard(
    wm: &mut WorldManager,
    cx: i32,
    cz: i32,
    base_y: i32,
    rx: i32,
    rz: i32,
    device: &wgpu::Device,
    pipeline: &VoxelPipeline,
) {
    for dx in -rx..=rx {
        for dz in -rz..=rz {
            let x = cx + dx;
            let z = cz + dz;
            // Clear trees/grass/hills right inside the build footprint
            for y in (base_y + 1)..=(base_y + 16) {
                set_block(wm, IVec3::new(x, y, z), BlockId::AIR, 0, device, pipeline);
            }
            // Fill any dips underneath down to solid ground
            for y in (base_y - 4)..base_y {
                let b = { wm.world().lock().unwrap().get_block(IVec3::new(x, y, z)) };
                if b.is_air() || b.is_water() {
                    set_block(wm, IVec3::new(x, y, z), BlockId::DIRT, 0, device, pipeline);
                }
            }
            set_block(wm, IVec3::new(x, base_y, z), BlockId::GRASS, 0, device, pipeline);
        }
    }
}

pub async fn render_promo_screenshots(
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = Path::new(output_dir);
    std::fs::create_dir_all(out_dir)?;

    let renderer = SceneRenderer::new(1600, 900).await?;
    let registry = Arc::new(BlockRegistry::new());
    let item_registry = Arc::new(ItemRegistry::new());
    let presets = starter_presets();

    // =========================================================================
    // SHOT 1: Hero Sweeping Procedural Landscape (Seed 1337, Render Dist 8)
    // =========================================================================
    {
        let mut wm = WorldManager::new(1337, 8, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        // Scan for the highest ridge overlooking a forested/water valley
        let mut best_pos = IVec3::new(0, 72, 0);
        let mut best_y = 0;
        for x in (-48..=48).step_by(6) {
            for z in (-48..=48).step_by(6) {
                let (y, b) = surface_block_and_y(&wm, x, z);
                if !b.is_water() && y > best_y {
                    best_y = y;
                    best_pos = IVec3::new(x, y, z);
                }
            }
        }
        let mut low_pos = IVec3::new(-best_pos.x, 62, -best_pos.z);
        let mut low_y = 255;
        for x in (-64..=64).step_by(8) {
            for z in (-64..=64).step_by(8) {
                let d2 = (x - best_pos.x).pow(2) + (z - best_pos.z).pow(2);
                if d2 > 36 * 36 {
                    let (y, _) = surface_block_and_y(&wm, x, z);
                    if y < low_y {
                        low_y = y;
                        low_pos = IVec3::new(x, y, z);
                    }
                }
            }
        }

        let eye = Vec3::new(
            best_pos.x as f32 + 0.5,
            best_pos.y as f32 + 7.5,
            best_pos.z as f32 + 0.5,
        );
        let target = Vec3::new(
            low_pos.x as f32 + 0.5,
            (low_pos.y as f32 + 2.0).min(best_pos.y as f32 - 2.0),
            low_pos.z as f32 + 0.5,
        );
        let cam = look_at_camera(eye, target);
        let mut dn = DayNightCycle::new();
        dn.set_time(2400.0); // Crisp sunlit morning sky

        renderer.render_and_save(
            &wm,
            &cam,
            &dn,
            &[],
            &[],
            &[],
            &item_registry,
            &out_dir.join("promo_hero.png"),
        )?;
    }

    // =========================================================================
    // SHOT 2: Electrical Power & Storage Workshop (Nestled in Natural Terrain)
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 8, 8, &renderer.device, &renderer.pipeline);

        // Cobblestone courtyard apron
        for dx in -6..=6 {
            for dz in -4..=6 {
                set_block(
                    &mut wm,
                    IVec3::new(cx + dx, cy, cz + dz),
                    BlockId::COBBLESTONE,
                    0,
                    &renderer.device,
                    &renderer.pipeline,
                );
            }
        }

        // Timber & brick workshop
        for dy in 1..=5 {
            for dx in -5..=5 {
                for dz in -5..=1 {
                    let is_wall = dx == -5 || dx == 5 || dz == -5 || dz == 1;
                    let is_corner = (dx == -5 || dx == 5) && (dz == -5 || dz == 1);
                    let is_door = dz == 1 && (dx == 0 || dx == 1) && dy <= 2;
                    let is_win = (dz == 1 && (dx == -3 || dx == -2 || dx == 3) && (dy == 2 || dy == 3))
                        || (dx == -5 && (dz == -2 || dz == -1) && (dy == 2 || dy == 3));
                    let pos = IVec3::new(cx + dx, cy + dy, cz + dz);
                    if dy == 5 {
                        set_block(&mut wm, pos, BlockId::SPRUCE_PLANKS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_door {
                        set_block(&mut wm, pos, BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                    } else if is_corner {
                        set_block(&mut wm, pos, BlockId::SPRUCE_LOG, 0, &renderer.device, &renderer.pipeline);
                    } else if is_win {
                        set_block(&mut wm, pos, BlockId::GLASS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_wall {
                        let mat = if dy == 1 { BlockId::BRICKS } else { BlockId::PLANKS };
                        set_block(&mut wm, pos, mat, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }

        // Interior warm lights
        set_block(&mut wm, IVec3::new(cx, cy + 4, cz - 2), BlockId::ELECTRIC_LAMP_LIT, 0, &renderer.device, &renderer.pipeline);

        // Rooftop Solar Panels & Advanced Solar Panels
        for dx in -4..=-1 {
            set_block(&mut wm, IVec3::new(cx + dx, cy + 6, cz - 3), BlockId::ADVANCED_SOLAR_PANEL, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + dx, cy + 6, cz - 1), BlockId::SOLAR_PANEL, 0, &renderer.device, &renderer.pipeline);
        }

        // Two tall 4-block animated Wind Turbines
        let t1 = IVec3::new(cx + 4, cy + 6, cz - 3);
        let t2 = IVec3::new(cx + 7, cy + 5, cz - 2);
        for dy in 1..=4 {
            set_block(&mut wm, IVec3::new(cx + 7, cy + dy, cz - 2), BlockId::COBBLESTONE, 0, &renderer.device, &renderer.pipeline);
        }
        set_block(&mut wm, t1, BlockId::WIND_TURBINE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, t2, BlockId::WIND_TURBINE, 0, &renderer.device, &renderer.pipeline);

        // Outdoor Power Array + Storage Terminal Network
        set_block(&mut wm, IVec3::new(cx - 5, cy + 1, cz + 3), BlockId::COAL_GENERATOR, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 4, cy + 1, cz + 3), BlockId::COPPER_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz + 3), BlockId::BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 2, cy + 1, cz + 3), BlockId::ADVANCED_BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 1, cy + 1, cz + 3), BlockId::QUANTUM_BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx, cy + 1, cz + 3), BlockId::GOLD_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 1, cy + 1, cz + 3), BlockId::ELECTRIC_FURNACE, 2, &renderer.device, &renderer.pipeline);

        set_block(&mut wm, IVec3::new(cx + 3, cy + 1, cz + 3), BlockId::STORAGE_TERMINAL, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 1, cz + 3), BlockId::STORAGE_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + 3), BlockId::DIAMOND_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 6, cy + 1, cz + 3), BlockId::GOLD_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 7, cy + 1, cz + 3), BlockId::IRON_CHEST, 2, &renderer.device, &renderer.pipeline);

        // Glowing Electric Lamp posts
        for lx in [-6, 7] {
            set_block(&mut wm, IVec3::new(cx + lx, cy + 1, cz + 5), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + lx, cy + 2, cz + 5), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + lx, cy + 3, cz + 5), BlockId::ELECTRIC_LAMP_LIT, 0, &renderer.device, &renderer.pipeline);
        }

        let turbines = vec![(t1, 45.0, 0), (t2, 42.0, 0)];
        let eye = Vec3::new(cx as f32 - 8.5, cy as f32 + 6.2, cz as f32 + 12.2);
        let target = Vec3::new(cx as f32 + 1.0, cy as f32 + 3.2, cz as f32 - 0.5);
        let cam = look_at_camera(eye, target);
        let mut dn = DayNightCycle::new();
        dn.set_time(12100.0); // Warm dusk light

        renderer.render_and_save(
            &wm,
            &cam,
            &dn,
            &[],
            &turbines,
            &[],
            &item_registry,
            &out_dir.join("promo_power.png"),
        )?;
    }

    // =========================================================================
    // SHOT 3: Homestead, Irrigated Farmland, Crops, Cooking Pot & Animals
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 9, 8, &renderer.device, &renderer.pipeline);

        // Cozy Homestead Cabin
        for dy in 1..=4 {
            for dx in -7..=-1 {
                for dz in -6..=-1 {
                    let is_wall = dx == -7 || dx == -1 || dz == -6 || dz == -1;
                    let is_corner = (dx == -7 || dx == -1) && (dz == -6 || dz == -1);
                    let is_door = dz == -1 && dx == -4 && dy <= 2;
                    let is_win = dz == -1 && (dx == -6 || dx == -2) && dy == 2;
                    let pos = IVec3::new(cx + dx, cy + dy, cz + dz);
                    if is_door {
                        set_block(&mut wm, pos, BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                    } else if is_corner {
                        set_block(&mut wm, pos, BlockId::WOOD_LOG, 0, &renderer.device, &renderer.pipeline);
                    } else if is_win {
                        set_block(&mut wm, pos, BlockId::GLASS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_wall {
                        let mat = if dy == 1 { BlockId::COBBLESTONE } else { BlockId::BIRCH_PLANKS };
                        set_block(&mut wm, pos, mat, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }
        // Pitched spruce roof
        for step in 0..=3 {
            for dx in (-8 + step)..=(0 - step) {
                for dz in -7..=0 {
                    set_block(
                        &mut wm,
                        IVec3::new(cx + dx, cy + 5 + step, cz + dz),
                        BlockId::SPRUCE_PLANKS,
                        0,
                        &renderer.device,
                        &renderer.pipeline,
                    );
                }
            }
        }

        // Outdoor Hearth with lit Cooking Pot, Crafting Table, Chest & Torch
        set_block(&mut wm, IVec3::new(cx - 5, cy + 1, cz + 1), BlockId::COOKING_POT_LIT, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz + 1), BlockId::CRAFTING_TABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 2, cy + 1, cz + 1), BlockId::CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 6, cy + 1, cz + 1), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);

        // Irrigated Farmland Rows with central water channel (now cleanly replaces grass!)
        for dz in -4..=4 {
            set_block(&mut wm, IVec3::new(cx + 4, cy, cz + dz), BlockId::WATER, 0, &renderer.device, &renderer.pipeline);
            if dz == -4 || dz == -3 {
                set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + dz), BlockId::SUGAR_CANE, 0, &renderer.device, &renderer.pipeline);
                set_block(&mut wm, IVec3::new(cx + 5, cy + 2, cz + dz), BlockId::SUGAR_CANE, 0, &renderer.device, &renderer.pipeline);
            }
        }
        for dz in -3..=3 {
            set_block(&mut wm, IVec3::new(cx + 2, cy, cz + dz), BlockId::FARMLAND_BONE_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 2, cy + 1, cz + dz), BlockId::WHEAT_3, 0, &renderer.device, &renderer.pipeline);

            set_block(&mut wm, IVec3::new(cx + 3, cy, cz + dz), BlockId::FARMLAND_COMPOST_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 3, cy + 1, cz + dz), BlockId::TOMATOES_3, 0, &renderer.device, &renderer.pipeline);

            set_block(&mut wm, IVec3::new(cx + 5, cy, cz + dz), BlockId::FARMLAND_MOIST, 0, &renderer.device, &renderer.pipeline);
            if dz >= -1 {
                set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + dz), BlockId::CARROTS_3, 0, &renderer.device, &renderer.pipeline);
            }

            set_block(&mut wm, IVec3::new(cx + 6, cy, cz + dz), BlockId::FARMLAND_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 6, cy + 1, cz + dz), BlockId::POTATOES_3, 0, &renderer.device, &renderer.pipeline);
        }
        // Pumpkins & Melons
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz - 2), BlockId::PUMPKIN, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz), BlockId::PUMPKIN, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz + 2), BlockId::MELON, 0, &renderer.device, &renderer.pipeline);

        // Passive Animals grazing on the pasture
        let animals = vec![
            AnimalEntity::new(1, AnimalKind::Cow, Vec3::new(cx as f32 - 4.5, cy as f32 + 1.0, cz as f32 + 4.8)),
            AnimalEntity::new(2, AnimalKind::Sheep, Vec3::new(cx as f32 - 2.0, cy as f32 + 1.0, cz as f32 + 5.5)),
            AnimalEntity::new(3, AnimalKind::Pig, Vec3::new(cx as f32 + 0.8, cy as f32 + 1.0, cz as f32 + 5.6)),
            AnimalEntity::new(4, AnimalKind::Chicken, Vec3::new(cx as f32 - 0.5, cy as f32 + 1.0, cz as f32 + 4.2)),
        ];

        let eye = Vec3::new(cx as f32 + 1.8, cy as f32 + 5.5, cz as f32 + 11.5);
        let target = Vec3::new(cx as f32 + 0.5, cy as f32 + 1.8, cz as f32 - 0.5);
        let cam = look_at_camera(eye, target);
        let mut dn = DayNightCycle::new();
        dn.set_time(4200.0);

        renderer.render_and_save(
            &wm,
            &cam,
            &dn,
            &animals,
            &[],
            &[],
            &item_registry,
            &out_dir.join("promo_homestead.png"),
        )?;
    }

    // =========================================================================
    // SHOT 4: Natural Shoreline Fishing Dock over Real Coral/Ocean Water
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 7, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let (shore, dir) = find_shoreline(&wm, 48)
            .unwrap_or((IVec3::new(0, 62, 0), IVec3::new(0, 0, -1)));
        let perp = IVec3::new(-dir.z, 0, dir.x);

        // Build wooden fishing dock extending organically from the real shoreline over water
        for step in -2..=7 {
            for side in -1..=1 {
                let p = shore + dir * step + perp * side;
                set_block(&mut wm, IVec3::new(p.x, shore.y + 1, p.z), BlockId::PLANKS, 0, &renderer.device, &renderer.pipeline);
                // Clear air above dock
                for ay in 2..=6 {
                    set_block(&mut wm, IVec3::new(p.x, shore.y + ay, p.z), BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                }
                if side.abs() == 1 && (step == 0 || step == 4 || step == 7) {
                    for py in (shore.y - 3)..=shore.y {
                        set_block(&mut wm, IVec3::new(p.x, py, p.z), BlockId::WOOD_LOG, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }
        // Lantern post & tackle chest on dock
        let tip_left = shore + dir * 7 - perp;
        set_block(&mut wm, IVec3::new(tip_left.x, shore.y + 2, tip_left.z), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(tip_left.x, shore.y + 3, tip_left.z), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);
        let mid_right = shore + dir * 3 + perp;
        set_block(&mut wm, IVec3::new(mid_right.x, shore.y + 2, mid_right.z), BlockId::CHEST, 0, &renderer.device, &renderer.pipeline);

        let eye_v = shore - dir * 5 - perp * 5;
        let tgt_v = shore + dir * 8 + perp;
        let eye = Vec3::new(eye_v.x as f32 + 0.5, shore.y as f32 + 4.5, eye_v.z as f32 + 0.5);
        let target = Vec3::new(tgt_v.x as f32 + 0.5, shore.y as f32 + 1.2, tgt_v.z as f32 + 0.5);
        let cam = look_at_camera(eye, target);
        let mut dn = DayNightCycle::new();
        dn.set_time(2000.0);

        renderer.render_and_save(
            &wm,
            &cam,
            &dn,
            &[],
            &[],
            &[],
            &item_registry,
            &out_dir.join("promo_lakeside.png"),
        )?;
    }

    // =========================================================================
    // SHOT 5: Stylized 3D Characters & Co-op Adventurers in Natural Meadow
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 5, 5, &renderer.device, &renderer.pipeline);

        // Camp backdrop: timber-and-stone wall, lit furnace, crafting table, diamond chest
        for dx in -4..=4 {
            set_block(&mut wm, IVec3::new(cx + dx, cy + 1, cz - 3), BlockId::COBBLESTONE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + dx, cy + 2, cz - 3), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
        }
        set_block(&mut wm, IVec3::new(cx - 4, cy + 1, cz - 2), BlockId::CRAFTING_TABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz - 2), BlockId::FURNACE_LIT, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 1, cz - 2), BlockId::DIAMOND_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 4, cy + 3, cz - 3), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 3, cz - 3), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);

        let mut players = Vec::new();
        let positions = [
            (Vec3::new(cx as f32 - 1.6, cy as f32 + 1.0, cz as f32 + 0.2), 1.35_f32, ItemId::DIAMOND_PICKAXE),
            (Vec3::new(cx as f32 + 0.0, cy as f32 + 1.0, cz as f32 + 0.7), 1.57_f32, ItemId::IRON_SWORD),
            (Vec3::new(cx as f32 + 1.6, cy as f32 + 1.0, cz as f32 + 0.2), 1.80_f32, ItemId::TORCH),
        ];
        for (i, (pos, yaw, item)) in positions.into_iter().enumerate() {
            let appearance = presets
                .get(i % presets.len().max(1))
                .map(|p| p.appearance.clone())
                .unwrap_or_default();
            players.push(PlayerModelState {
                player_id: (i + 1) as u32,
                username: format!("Explorer{}", i + 1),
                prefix: String::new(),
                position: pos,
                velocity: Vec3::ZERO,
                yaw,
                pitch: 0.0,
                held_item: item,
                walk_anim: 0.0,
                swing_anim: 0.0,
                hurt_timer: 0.0,
                is_sneaking: false,
                appearance,
                health: 20.0,
                max_health: 20.0,
            });
        }

        let eye = Vec3::new(cx as f32 + 0.0, cy as f32 + 2.5, cz as f32 + 5.2);
        let target = Vec3::new(cx as f32 + 0.0, cy as f32 + 1.8, cz as f32);
        let cam = look_at_camera(eye, target);
        let mut dn = DayNightCycle::new();
        dn.set_time(3200.0);

        renderer.render_and_save(
            &wm,
            &cam,
            &dn,
            &[],
            &[],
            &players,
            &item_registry,
            &out_dir.join("promo_characters.png"),
        )?;
    }

    Ok(())
}

fn render_clip_with_ffmpeg<F>(
    renderer: &SceneRenderer,
    output_path: &Path,
    fps: u32,
    num_frames: u32,
    mut render_frame: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(u32, f32) -> Result<Vec<u8>, Box<dyn std::error::Error>>,
{
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{}x{}", renderer.width, renderer.height),
            "-r",
            &fps.to_string(),
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            output_path.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().ok_or("Failed to open ffmpeg stdin")?;
    for f in 0..num_frames {
        let t = f as f32 / (num_frames.saturating_sub(1).max(1)) as f32;
        let pixels = render_frame(f, t)?;
        stdin.write_all(&pixels)?;
    }
    drop(stdin);
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("ffmpeg failed with status {}", status).into());
    }
    log::info!("Saved trailer clip: {}", output_path.display());
    Ok(())
}

pub async fn render_trailer_clips(
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = Path::new(output_dir);
    std::fs::create_dir_all(out_dir)?;

    let width = 1280;
    let height = 720;
    let fps = 24;
    let renderer = SceneRenderer::new(width, height).await?;
    let registry = Arc::new(BlockRegistry::new());
    let item_registry = Arc::new(ItemRegistry::new());
    let presets = starter_presets();

    // =========================================================================
    // CLIP 1: Sweeping Highland & Glacial Sea Flyover (6.5s = 156 frames)
    // =========================================================================
    {
        let mut wm = WorldManager::new(1337, 8, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let mut best_pos = IVec3::new(0, 72, 0);
        let mut best_y = 0;
        for x in (-48..=48).step_by(6) {
            for z in (-48..=48).step_by(6) {
                let (y, b) = surface_block_and_y(&wm, x, z);
                if !b.is_water() && y > best_y {
                    best_y = y;
                    best_pos = IVec3::new(x, y, z);
                }
            }
        }
        let mut low_pos = IVec3::new(-best_pos.x, 62, -best_pos.z);
        let mut low_y = 255;
        for x in (-64..=64).step_by(8) {
            for z in (-64..=64).step_by(8) {
                let d2 = (x - best_pos.x).pow(2) + (z - best_pos.z).pow(2);
                if d2 > 36 * 36 {
                    let (y, _) = surface_block_and_y(&wm, x, z);
                    if y < low_y {
                        low_y = y;
                        low_pos = IVec3::new(x, y, z);
                    }
                }
            }
        }

        let start_eye = Vec3::new(best_pos.x as f32 - 8.0, best_pos.y as f32 + 6.5, best_pos.z as f32 - 6.0);
        let end_eye = Vec3::new(best_pos.x as f32 + 12.0, best_pos.y as f32 + 10.5, best_pos.z as f32 + 8.0);
        let target = Vec3::new(
            low_pos.x as f32 + 0.5,
            (low_pos.y as f32 + 2.0).min(best_pos.y as f32 - 2.0),
            low_pos.z as f32 + 0.5,
        );

        render_clip_with_ffmpeg(&renderer, &out_dir.join("clip_1_world.mp4"), fps, 156, |f, t| {
            let eye = start_eye.lerp(end_eye, t);
            let cam = look_at_camera(eye, target);
            let mut dn = DayNightCycle::new();
            dn.set_time(2100.0 + t * 800.0);
            renderer.render_frame_rgba(&wm, &cam, &dn, &[], &[], &[], &item_registry, f as f32 / fps as f32)
        })?;
    }

    // =========================================================================
    // CLIP 2: Dusk Power Workshop with Actively Spinning Wind Turbines (8.0s = 192 frames)
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 8, 8, &renderer.device, &renderer.pipeline);

        for dx in -6..=6 {
            for dz in -4..=6 {
                set_block(&mut wm, IVec3::new(cx + dx, cy, cz + dz), BlockId::COBBLESTONE, 0, &renderer.device, &renderer.pipeline);
            }
        }
        for dy in 1..=5 {
            for dx in -5..=5 {
                for dz in -5..=1 {
                    let is_wall = dx == -5 || dx == 5 || dz == -5 || dz == 1;
                    let is_corner = (dx == -5 || dx == 5) && (dz == -5 || dz == 1);
                    let is_door = dz == 1 && (dx == 0 || dx == 1) && dy <= 2;
                    let is_win = (dz == 1 && (dx == -3 || dx == -2 || dx == 3) && (dy == 2 || dy == 3))
                        || (dx == -5 && (dz == -2 || dz == -1) && (dy == 2 || dy == 3));
                    let pos = IVec3::new(cx + dx, cy + dy, cz + dz);
                    if dy == 5 {
                        set_block(&mut wm, pos, BlockId::SPRUCE_PLANKS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_door {
                        set_block(&mut wm, pos, BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                    } else if is_corner {
                        set_block(&mut wm, pos, BlockId::SPRUCE_LOG, 0, &renderer.device, &renderer.pipeline);
                    } else if is_win {
                        set_block(&mut wm, pos, BlockId::GLASS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_wall {
                        let mat = if dy == 1 { BlockId::BRICKS } else { BlockId::PLANKS };
                        set_block(&mut wm, pos, mat, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }
        set_block(&mut wm, IVec3::new(cx, cy + 4, cz - 2), BlockId::ELECTRIC_LAMP_LIT, 0, &renderer.device, &renderer.pipeline);
        for dx in -4..=-1 {
            set_block(&mut wm, IVec3::new(cx + dx, cy + 6, cz - 3), BlockId::ADVANCED_SOLAR_PANEL, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + dx, cy + 6, cz - 1), BlockId::SOLAR_PANEL, 0, &renderer.device, &renderer.pipeline);
        }
        let t1 = IVec3::new(cx + 4, cy + 6, cz - 3);
        let t2 = IVec3::new(cx + 7, cy + 5, cz - 2);
        for dy in 1..=4 {
            set_block(&mut wm, IVec3::new(cx + 7, cy + dy, cz - 2), BlockId::COBBLESTONE, 0, &renderer.device, &renderer.pipeline);
        }
        set_block(&mut wm, t1, BlockId::WIND_TURBINE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, t2, BlockId::WIND_TURBINE, 0, &renderer.device, &renderer.pipeline);

        set_block(&mut wm, IVec3::new(cx - 5, cy + 1, cz + 3), BlockId::COAL_GENERATOR, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 4, cy + 1, cz + 3), BlockId::COPPER_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz + 3), BlockId::BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 2, cy + 1, cz + 3), BlockId::ADVANCED_BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 1, cy + 1, cz + 3), BlockId::QUANTUM_BATTERY_BOX, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx, cy + 1, cz + 3), BlockId::GOLD_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 1, cy + 1, cz + 3), BlockId::ELECTRIC_FURNACE, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 3, cy + 1, cz + 3), BlockId::STORAGE_TERMINAL, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 1, cz + 3), BlockId::STORAGE_CABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + 3), BlockId::DIAMOND_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 6, cy + 1, cz + 3), BlockId::GOLD_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 7, cy + 1, cz + 3), BlockId::IRON_CHEST, 2, &renderer.device, &renderer.pipeline);
        for lx in [-6, 7] {
            set_block(&mut wm, IVec3::new(cx + lx, cy + 1, cz + 5), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + lx, cy + 2, cz + 5), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + lx, cy + 3, cz + 5), BlockId::ELECTRIC_LAMP_LIT, 0, &renderer.device, &renderer.pipeline);
        }

        let turbines = vec![(t1, 45.0, 0), (t2, 42.0, 0)];
        let center = Vec3::new(cx as f32 + 1.0, cy as f32 + 3.4, cz as f32 - 0.5);

        render_clip_with_ffmpeg(&renderer, &out_dir.join("clip_2_power.mp4"), fps, 192, |f, t| {
            // Smooth orbital sweep from left to right as dusk deepens and turbines spin!
            let angle = -0.68 + t * 0.92;
            let radius = 14.5 - t * 1.4;
            let eye = Vec3::new(
                center.x + angle.sin() * radius,
                cy as f32 + 5.2 + t * 2.4,
                center.z + angle.cos() * radius,
            );
            let cam = look_at_camera(eye, center);
            let mut dn = DayNightCycle::new();
            dn.set_time(11850.0 + t * 550.0);
            renderer.render_frame_rgba(&wm, &cam, &dn, &[], &turbines, &[], &item_registry, f as f32 / fps as f32)
        })?;
    }

    // =========================================================================
    // CLIP 3: Sunlit Homestead, Irrigated Crops, Cooking Pot & Animals (7.5s = 180 frames)
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 9, 8, &renderer.device, &renderer.pipeline);

        for dy in 1..=4 {
            for dx in -7..=-1 {
                for dz in -6..=-1 {
                    let is_wall = dx == -7 || dx == -1 || dz == -6 || dz == -1;
                    let is_corner = (dx == -7 || dx == -1) && (dz == -6 || dz == -1);
                    let is_door = dz == -1 && dx == -4 && dy <= 2;
                    let is_win = dz == -1 && (dx == -6 || dx == -2) && dy == 2;
                    let pos = IVec3::new(cx + dx, cy + dy, cz + dz);
                    if is_door {
                        set_block(&mut wm, pos, BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                    } else if is_corner {
                        set_block(&mut wm, pos, BlockId::WOOD_LOG, 0, &renderer.device, &renderer.pipeline);
                    } else if is_win {
                        set_block(&mut wm, pos, BlockId::GLASS, 0, &renderer.device, &renderer.pipeline);
                    } else if is_wall {
                        let mat = if dy == 1 { BlockId::COBBLESTONE } else { BlockId::BIRCH_PLANKS };
                        set_block(&mut wm, pos, mat, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }
        for step in 0..=3 {
            for dx in (-8 + step)..=(0 - step) {
                for dz in -7..=0 {
                    set_block(&mut wm, IVec3::new(cx + dx, cy + 5 + step, cz + dz), BlockId::SPRUCE_PLANKS, 0, &renderer.device, &renderer.pipeline);
                }
            }
        }
        set_block(&mut wm, IVec3::new(cx - 5, cy + 1, cz + 1), BlockId::COOKING_POT_LIT, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz + 1), BlockId::CRAFTING_TABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 2, cy + 1, cz + 1), BlockId::CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 6, cy + 1, cz + 1), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);

        for dz in -4..=4 {
            set_block(&mut wm, IVec3::new(cx + 4, cy, cz + dz), BlockId::WATER, 0, &renderer.device, &renderer.pipeline);
            if dz == -4 || dz == -3 {
                set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + dz), BlockId::SUGAR_CANE, 0, &renderer.device, &renderer.pipeline);
                set_block(&mut wm, IVec3::new(cx + 5, cy + 2, cz + dz), BlockId::SUGAR_CANE, 0, &renderer.device, &renderer.pipeline);
            }
        }
        for dz in -3..=3 {
            set_block(&mut wm, IVec3::new(cx + 2, cy, cz + dz), BlockId::FARMLAND_BONE_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 2, cy + 1, cz + dz), BlockId::WHEAT_3, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 3, cy, cz + dz), BlockId::FARMLAND_COMPOST_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 3, cy + 1, cz + dz), BlockId::TOMATOES_3, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 5, cy, cz + dz), BlockId::FARMLAND_MOIST, 0, &renderer.device, &renderer.pipeline);
            if dz >= -1 {
                set_block(&mut wm, IVec3::new(cx + 5, cy + 1, cz + dz), BlockId::CARROTS_3, 0, &renderer.device, &renderer.pipeline);
            }
            set_block(&mut wm, IVec3::new(cx + 6, cy, cz + dz), BlockId::FARMLAND_MOIST, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + 6, cy + 1, cz + dz), BlockId::POTATOES_3, 0, &renderer.device, &renderer.pipeline);
        }
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz - 2), BlockId::PUMPKIN, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz), BlockId::PUMPKIN, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 8, cy + 1, cz + 2), BlockId::MELON, 0, &renderer.device, &renderer.pipeline);

        let animals = vec![
            AnimalEntity::new(1, AnimalKind::Cow, Vec3::new(cx as f32 - 4.5, cy as f32 + 1.0, cz as f32 + 4.8)),
            AnimalEntity::new(2, AnimalKind::Sheep, Vec3::new(cx as f32 - 2.0, cy as f32 + 1.0, cz as f32 + 5.5)),
            AnimalEntity::new(3, AnimalKind::Pig, Vec3::new(cx as f32 + 0.8, cy as f32 + 1.0, cz as f32 + 5.6)),
            AnimalEntity::new(4, AnimalKind::Chicken, Vec3::new(cx as f32 - 0.5, cy as f32 + 1.0, cz as f32 + 4.2)),
        ];

        let start_eye = Vec3::new(cx as f32 - 6.0, cy as f32 + 4.2, cz as f32 + 10.5);
        let end_eye = Vec3::new(cx as f32 + 7.5, cy as f32 + 6.0, cz as f32 + 10.0);
        let target = Vec3::new(cx as f32 + 0.5, cy as f32 + 2.0, cz as f32 - 0.5);

        render_clip_with_ffmpeg(&renderer, &out_dir.join("clip_3_homestead.mp4"), fps, 180, |f, t| {
            let eye = start_eye.lerp(end_eye, t);
            let cam = look_at_camera(eye, target);
            let mut dn = DayNightCycle::new();
            dn.set_time(4200.0);
            renderer.render_frame_rgba(&wm, &cam, &dn, &animals, &[], &[], &item_registry, f as f32 / fps as f32)
        })?;
    }

    // =========================================================================
    // CLIP 4: Shoreline Fishing Dock & Translucent Water Glide (6.5s = 156 frames)
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 7, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let (shore, dir) = find_shoreline(&wm, 48)
            .unwrap_or((IVec3::new(0, 62, 0), IVec3::new(0, 0, -1)));
        let perp = IVec3::new(-dir.z, 0, dir.x);

        for step in -2..=7 {
            for side in -1..=1 {
                let p = shore + dir * step + perp * side;
                set_block(&mut wm, IVec3::new(p.x, shore.y + 1, p.z), BlockId::PLANKS, 0, &renderer.device, &renderer.pipeline);
                for ay in 2..=6 {
                    set_block(&mut wm, IVec3::new(p.x, shore.y + ay, p.z), BlockId::AIR, 0, &renderer.device, &renderer.pipeline);
                }
                if side.abs() == 1 && (step == 0 || step == 4 || step == 7) {
                    for py in (shore.y - 3)..=shore.y {
                        set_block(&mut wm, IVec3::new(p.x, py, p.z), BlockId::WOOD_LOG, 0, &renderer.device, &renderer.pipeline);
                    }
                }
            }
        }
        let tip_left = shore + dir * 7 - perp;
        set_block(&mut wm, IVec3::new(tip_left.x, shore.y + 2, tip_left.z), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(tip_left.x, shore.y + 3, tip_left.z), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);
        let mid_right = shore + dir * 3 + perp;
        set_block(&mut wm, IVec3::new(mid_right.x, shore.y + 2, mid_right.z), BlockId::CHEST, 0, &renderer.device, &renderer.pipeline);

        let e0 = shore - dir * 6 - perp * 5;
        let e1 = shore + dir * 4 - perp * 4;
        let tv = shore + dir * 10 + perp;
        let start_eye = Vec3::new(e0.x as f32 + 0.5, shore.y as f32 + 4.8, e0.z as f32 + 0.5);
        let end_eye = Vec3::new(e1.x as f32 + 0.5, shore.y as f32 + 3.4, e1.z as f32 + 0.5);
        let target = Vec3::new(tv.x as f32 + 0.5, shore.y as f32 + 1.2, tv.z as f32 + 0.5);

        render_clip_with_ffmpeg(&renderer, &out_dir.join("clip_4_lakeside.mp4"), fps, 156, |f, t| {
            let eye = start_eye.lerp(end_eye, t);
            let cam = look_at_camera(eye, target);
            let mut dn = DayNightCycle::new();
            dn.set_time(1800.0 + t * 500.0);
            renderer.render_frame_rgba(&wm, &cam, &dn, &[], &[], &[], &item_registry, f as f32 / fps as f32)
        })?;
    }

    // =========================================================================
    // CLIP 5: Stylized 3D Characters & Co-Op Camp Arc (5.5s = 132 frames)
    // =========================================================================
    {
        let mut wm = WorldManager::new(4242, 6, registry.clone());
        wm.init_spawn_area(&renderer.device, &renderer.pipeline);

        let site = find_dry_clearing(&wm, 32);
        let (cx, cy, cz) = (site.x, site.y, site.z);
        grade_yard(&mut wm, cx, cz, cy, 5, 5, &renderer.device, &renderer.pipeline);

        for dx in -4..=4 {
            set_block(&mut wm, IVec3::new(cx + dx, cy + 1, cz - 3), BlockId::COBBLESTONE, 0, &renderer.device, &renderer.pipeline);
            set_block(&mut wm, IVec3::new(cx + dx, cy + 2, cz - 3), BlockId::OAK_FENCE, 0, &renderer.device, &renderer.pipeline);
        }
        set_block(&mut wm, IVec3::new(cx - 4, cy + 1, cz - 2), BlockId::CRAFTING_TABLE, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 3, cy + 1, cz - 2), BlockId::FURNACE_LIT, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 1, cz - 2), BlockId::DIAMOND_CHEST, 2, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx - 4, cy + 3, cz - 3), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);
        set_block(&mut wm, IVec3::new(cx + 4, cy + 3, cz - 3), BlockId::TORCH, 0, &renderer.device, &renderer.pipeline);

        let mut base_players = Vec::new();
        let positions = [
            (Vec3::new(cx as f32 - 1.6, cy as f32 + 1.0, cz as f32 + 0.2), 1.35_f32, ItemId::DIAMOND_PICKAXE),
            (Vec3::new(cx as f32 + 0.0, cy as f32 + 1.0, cz as f32 + 0.7), 1.57_f32, ItemId::IRON_SWORD),
            (Vec3::new(cx as f32 + 1.6, cy as f32 + 1.0, cz as f32 + 0.2), 1.80_f32, ItemId::TORCH),
        ];
        for (i, (pos, yaw, item)) in positions.into_iter().enumerate() {
            let appearance = presets
                .get(i % presets.len().max(1))
                .map(|p| p.appearance.clone())
                .unwrap_or_default();
            base_players.push(PlayerModelState {
                player_id: (i + 1) as u32,
                username: format!("Explorer{}", i + 1),
                prefix: String::new(),
                position: pos,
                velocity: Vec3::ZERO,
                yaw,
                pitch: 0.0,
                held_item: item,
                walk_anim: 0.0,
                swing_anim: 0.0,
                hurt_timer: 0.0,
                is_sneaking: false,
                appearance,
                health: 20.0,
                max_health: 20.0,
            });
        }

        let target = Vec3::new(cx as f32, cy as f32 + 1.9, cz as f32);
        render_clip_with_ffmpeg(&renderer, &out_dir.join("clip_5_characters.mp4"), fps, 132, |f, t| {
            let angle = -0.32 + t * 0.64;
            let dist = 5.6 - t * 0.7;
            let eye = Vec3::new(
                target.x + angle.sin() * dist,
                cy as f32 + 2.4 + t * 0.3,
                target.z + angle.cos() * dist,
            );
            let cam = look_at_camera(eye, target);
            let mut dn = DayNightCycle::new();
            dn.set_time(3200.0);
            renderer.render_frame_rgba(&wm, &cam, &dn, &[], &[], &base_players, &item_registry, f as f32 / fps as f32)
        })?;
    }

    Ok(())
}
