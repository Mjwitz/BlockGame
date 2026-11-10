use std::collections::{HashMap, HashSet};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use blockgame_core::{
    animal::{AnimalEntity, AnimalKind},
    block::{BlockId, BlockRegistry},
    chunk::{Chunk, ChunkPos},
    entity::{ItemDrop, ItemDropStatus},
    item::ItemSlot,
    physics::AABB,
    world::{world_to_chunk_coords, World},
};
use blockgame_gen::TerrainGenerator;
use blockgame_render::{ChunkMesh, ChunkMesher, GpuMeshBuffers, VoxelPipeline};
use glam::{IVec3, Vec3};
use rayon::prelude::*;

pub struct WorldManager {
    world: Arc<Mutex<World>>,
    generator: Arc<TerrainGenerator>,
    registry: Arc<BlockRegistry>,
    render_distance: i32,
    loaded_chunks_set: HashSet<ChunkPos>,
    queued_generation: HashSet<ChunkPos>,
    queued_meshing: HashSet<ChunkPos>,
    pub gpu_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    mesh_rx: Receiver<(ChunkPos, ChunkMesh)>,
    mesh_tx: Sender<(ChunkPos, ChunkMesh)>,
    last_player_chunk: ChunkPos,
    pub item_drops: Vec<ItemDrop>,
    pub animals: Vec<AnimalEntity>,
    pub next_animal_id: u32,
    pub planted_crops: Vec<(IVec3, f32)>,
}

impl WorldManager {
    pub fn new(seed: u32, render_distance: i32, registry: Arc<BlockRegistry>) -> Self {
        let (mesh_tx, mesh_rx) = mpsc::channel();

        Self {
            world: Arc::new(Mutex::new(World::new())),
            generator: Arc::new(TerrainGenerator::new(seed)),
            registry,
            render_distance,
            loaded_chunks_set: HashSet::new(),
            queued_generation: HashSet::new(),
            queued_meshing: HashSet::new(),
            gpu_chunks: HashMap::new(),
            mesh_rx,
            mesh_tx,
            last_player_chunk: ChunkPos::new(i32::MAX, i32::MAX, i32::MAX),
            item_drops: Vec::new(),
            animals: Vec::new(),
            next_animal_id: 1,
            planted_crops: Vec::new(),
        }
    }

    /// Synchronously pre-generates and meshes the initial area around the spawn point.
    pub fn init_spawn_area(&mut self, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        log::info!(
            "Generating initial spawn chunks (radius {})...",
            self.render_distance
        );

        let mut chunks_to_generate = Vec::new();
        let r = self.render_distance;

        // Vertical chunk range: y from 0 to 2 (world y from 0 to 48)
        for cy in 0..=2 {
            for cz in -r..=r {
                for cx in -r..=r {
                    if cx * cx + cz * cz <= (r + 1) * (r + 1) {
                        chunks_to_generate.push(ChunkPos::new(cx, cy, cz));
                    }
                }
            }
        }

        // Parallel chunk terrain generation using Rayon
        let generator = self.generator.clone();
        let generated_chunks: Vec<Chunk> = chunks_to_generate
            .par_iter()
            .map(|&pos| {
                let mut chunk = generator.generate_chunk(pos);
                generator.populate_chunk_features(&mut chunk);
                chunk
            })
            .collect();

        {
            let mut world = self.world.lock().unwrap();
            for chunk in generated_chunks {
                self.loaded_chunks_set.insert(chunk.pos);
                world.insert_chunk(chunk);
            }
        }

        // Parallel chunk meshing
        let world_guard = self.world.lock().unwrap();
        let mesher = ChunkMesher::new(&self.registry);

        let meshes: Vec<(ChunkPos, ChunkMesh)> = chunks_to_generate
            .par_iter()
            .filter_map(|&pos| {
                world_guard.get_chunk(pos).map(|chunk| {
                    let origin = pos.to_world_origin();
                    let sources = world_guard.get_light_sources_near(origin, 15);
                    let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |lx, ly, lz| {
                        world_guard.get_block(origin + IVec3::new(lx, ly, lz))
                    });
                    (pos, mesh)
                })
            })
            .collect();
        drop(world_guard);

        // Upload to GPU
        let mut face_count = 0;
        for (pos, mesh) in meshes {
            face_count += mesh.face_count();
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(pos, gpu_mesh);
            }
        }

        log::info!(
            "Spawn area ready: {} chunks loaded, {} visible faces rendered",
            self.gpu_chunks.len(),
            face_count
        );

        // Spawn initial passive animals on the surface near spawn
        {
            let world_guard = self.world.lock().unwrap();
            let mut spawn_animal_at = |kind: AnimalKind, x: i32, z: i32| {
                let mut y = 50;
                while y > 1 {
                    let b = world_guard.get_block(IVec3::new(x, y, z));
                    if b == BlockId::GRASS || b == BlockId::DIRT {
                        break;
                    }
                    y -= 1;
                }
                if y > 1 {
                    self.animals.push(AnimalEntity::new(
                        self.next_animal_id,
                        kind,
                        Vec3::new(x as f32 + 0.5, (y + 1) as f32, z as f32 + 0.5),
                    ));
                    self.next_animal_id += 1;
                }
            };

            spawn_animal_at(AnimalKind::Cow, 6, 6);
            spawn_animal_at(AnimalKind::Cow, 8, 4);
            spawn_animal_at(AnimalKind::Pig, -6, 5);
            spawn_animal_at(AnimalKind::Pig, -8, 7);
            spawn_animal_at(AnimalKind::Chicken, 4, -6);
            spawn_animal_at(AnimalKind::Chicken, 6, -8);
        }
    }

    /// Periodic update called every frame: streams chunks and uploads completed background meshes.
    pub fn update(&mut self, player_pos: Vec3, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        // 1. Ingest background mesh results (all completed meshes uploaded)
        while let Ok((pos, mesh)) = self.mesh_rx.try_recv() {
            self.queued_meshing.remove(&pos);
            self.queued_generation.remove(&pos);
            self.loaded_chunks_set.insert(pos);
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(pos, gpu_mesh);
            }
        }

        // 2. Identify player's current chunk coordinates
        let (current_chunk, _, _, _) = world_to_chunk_coords(IVec3::new(
            player_pos.x.floor() as i32,
            player_pos.y.floor() as i32,
            player_pos.z.floor() as i32,
        ));

        let player_chunk_changed = current_chunk != self.last_player_chunk;
        let pcx = current_chunk.0.x;
        let pcz = current_chunk.0.z;
        let r = self.render_distance;

        // 3. Scan render distance only when player enters a new chunk or there is pending generation/meshing
        if player_chunk_changed
            || !self.queued_generation.is_empty()
            || !self.queued_meshing.is_empty()
        {
            self.last_player_chunk = current_chunk;

            let mut to_generate = Vec::new();
            let mut to_mesh = Vec::new();

            {
                let world_guard = self.world.lock().unwrap();
                for cy in 0..=2 {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx * dx + dz * dz <= (r + 1) * (r + 1) {
                                let pos = ChunkPos::new(pcx + dx, cy, pcz + dz);
                                if !self.gpu_chunks.contains_key(&pos)
                                    && !self.queued_meshing.contains(&pos)
                                    && !self.queued_generation.contains(&pos)
                                {
                                    if world_guard.has_chunk(pos) {
                                        self.queued_meshing.insert(pos);
                                        to_mesh.push(pos);
                                    } else {
                                        self.queued_generation.insert(pos);
                                        to_generate.push(pos);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 4. Dispatch terrain generation + meshing asynchronously
            if !to_generate.is_empty() {
                let generator = self.generator.clone();
                let world_arc = self.world.clone();
                let registry = self.registry.clone();
                let tx = self.mesh_tx.clone();

                rayon::spawn(move || {
                    let new_chunks: Vec<Chunk> = to_generate
                        .par_iter()
                        .map(|&pos| {
                            let mut chunk = generator.generate_chunk(pos);
                            generator.populate_chunk_features(&mut chunk);
                            chunk
                        })
                        .collect();

                    let mut world = world_arc.lock().unwrap();
                    for chunk in new_chunks {
                        world.insert_chunk(chunk);
                    }

                    let mesher = ChunkMesher::new(&registry);
                    for &pos in &to_generate {
                        if let Some(chunk) = world.get_chunk(pos) {
                            let origin = pos.to_world_origin();
                            let sources = world.get_light_sources_near(origin, 15);
                            let mesh =
                                mesher.mesh_chunk_with_lighting(chunk, &sources, |lx, ly, lz| {
                                    world.get_block(origin + IVec3::new(lx, ly, lz))
                                });
                            let _ = tx.send((pos, mesh));
                        }
                    }
                });
            }

            // 5. Dispatch meshing for already-generated chunks needing GPU buffers
            if !to_mesh.is_empty() {
                let world_arc = self.world.clone();
                let registry = self.registry.clone();
                let tx = self.mesh_tx.clone();

                rayon::spawn(move || {
                    let world = world_arc.lock().unwrap();
                    let mesher = ChunkMesher::new(&registry);
                    for pos in to_mesh {
                        if let Some(chunk) = world.get_chunk(pos) {
                            let origin = pos.to_world_origin();
                            let sources = world.get_light_sources_near(origin, 15);
                            let mesh =
                                mesher.mesh_chunk_with_lighting(chunk, &sources, |lx, ly, lz| {
                                    world.get_block(origin + IVec3::new(lx, ly, lz))
                                });
                            let _ = tx.send((pos, mesh));
                        }
                    }
                });
            }

            // 6. Unload distant chunks outside radius + 2 to free VRAM
            let unload_radius_sq = (r + 2) * (r + 2);
            let mut to_remove = Vec::new();
            for &pos in self.gpu_chunks.keys() {
                let dx = pos.0.x - pcx;
                let dz = pos.0.z - pcz;
                if dx * dx + dz * dz > unload_radius_sq {
                    to_remove.push(pos);
                }
            }

            for pos in to_remove {
                self.gpu_chunks.remove(&pos);
                self.loaded_chunks_set.remove(&pos);
                self.queued_meshing.remove(&pos);
                self.queued_generation.remove(&pos);
            }
        }
    }

    /// Renders loaded GPU chunk meshes with optional camera view frustum culling.
    pub fn render_with_frustum<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
    ) {
        for (&pos, gpu_mesh) in &self.gpu_chunks {
            if gpu_mesh.index_count == 0 {
                continue;
            }

            if let Some(f) = frustum {
                let min = Vec3::new(
                    pos.0.x as f32 * 16.0,
                    pos.0.y as f32 * 16.0,
                    pos.0.z as f32 * 16.0,
                );
                let max = min + Vec3::splat(16.0);
                if !f.is_box_visible(min, max) {
                    continue;
                }
            }

            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
        }
    }

    /// Renders all currently loaded GPU chunk meshes without frustum culling.
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.render_with_frustum(render_pass, None);
    }

    pub fn world(&self) -> &Arc<Mutex<World>> {
        &self.world
    }

    pub fn break_block(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> Option<BlockId> {
        let (chunk_pos, lx, ly, lz) = world_to_chunk_coords(pos);
        let mut chunks_to_rebuild = vec![chunk_pos];

        if lx == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x - 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        } else if lx == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x + 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        }
        if ly == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y - 1,
                chunk_pos.0.z,
            ));
        } else if ly == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y + 1,
                chunk_pos.0.z,
            ));
        }
        if lz == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z - 1,
            ));
        } else if lz == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z + 1,
            ));
        }

        let (broken_block, new_meshes, dropped_chest_items): (
            BlockId,
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<ItemSlot>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if current.is_air() {
                return None;
            }
            let is_torch = current == BlockId::TORCH || current == BlockId::FURNACE_LIT;
            let dropped_chest_items = if current == BlockId::CHEST {
                world.remove_chest(pos).unwrap_or_default()
            } else if current == BlockId::FURNACE || current == BlockId::FURNACE_LIT {
                if let Some(f) = world.remove_furnace(pos) {
                    let mut items = Vec::new();
                    if !f.input.is_empty() {
                        items.push(f.input);
                    }
                    if !f.fuel.is_empty() {
                        items.push(f.fuel);
                    }
                    if !f.output.is_empty() {
                        items.push(f.output);
                    }
                    items
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };
            world.set_block(pos, BlockId::AIR);

            if is_torch {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            chunks_to_rebuild.push(ChunkPos::new(
                                chunk_pos.0.x + dx,
                                chunk_pos.0.y + dy,
                                chunk_pos.0.z + dz,
                            ));
                        }
                    }
                }
            }
            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();

            let mesher = ChunkMesher::new(&self.registry);
            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world.get_chunk(cpos).map(|chunk| {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        (cpos, mesh)
                    })
                })
                .collect();
            (current, meshes, dropped_chest_items)
        };

        for (cpos, mesh) in new_meshes {
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(cpos, gpu_mesh);
            } else {
                self.gpu_chunks.remove(&cpos);
            }
        }

        let drop_pos = pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
        for item in dropped_chest_items {
            if !item.is_empty() {
                self.spawn_item_drop(drop_pos, item);
            }
        }

        Some(broken_block)
    }

    /// Spawns a floating item drop at the given world position
    pub fn spawn_item_drop(&mut self, pos: Vec3, item: ItemSlot) {
        if !item.is_empty() {
            self.item_drops.push(ItemDrop::new(pos, item));
        }
    }

    /// Updates item drops with gravity, collision, player magnetism, and returns any collected item slots
    pub fn update_item_drops(&mut self, dt: f32, player_pos: Vec3) -> Vec<ItemSlot> {
        let mut collected = Vec::new();
        let world_guard = self.world.lock().unwrap();

        self.item_drops.retain_mut(|drop| {
            match drop.update(dt, &world_guard, &self.registry, player_pos) {
                ItemDropStatus::Alive => true,
                ItemDropStatus::Collected(item) => {
                    collected.push(item);
                    false
                }
                ItemDropStatus::Despawned => false,
            }
        });

        collected
    }

    pub fn place_block(
        &mut self,
        pos: IVec3,
        block_id: BlockId,
        player_aabb: &AABB,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        // Prevent placing inside player collider
        let block_box = AABB::from_block(pos);
        if player_aabb.intersects(&block_box) {
            return false;
        }

        let (chunk_pos, lx, ly, lz) = world_to_chunk_coords(pos);
        let mut chunks_to_rebuild = vec![chunk_pos];

        if lx == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x - 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        } else if lx == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x + 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        }
        if ly == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y - 1,
                chunk_pos.0.z,
            ));
        } else if ly == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y + 1,
                chunk_pos.0.z,
            ));
        }
        if lz == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z - 1,
            ));
        } else if lz == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z + 1,
            ));
        }

        let new_meshes: Vec<(ChunkPos, ChunkMesh)> = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if !current.is_air() {
                return false;
            }
            let is_torch = block_id == BlockId::TORCH || block_id == BlockId::FURNACE_LIT;
            world.set_block(pos, block_id);
            if block_id == BlockId::FURNACE || block_id == BlockId::FURNACE_LIT {
                world.get_or_create_furnace(pos);
            }

            if is_torch {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            chunks_to_rebuild.push(ChunkPos::new(
                                chunk_pos.0.x + dx,
                                chunk_pos.0.y + dy,
                                chunk_pos.0.z + dz,
                            ));
                        }
                    }
                }
            }
            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();

            let mesher = ChunkMesher::new(&self.registry);
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world.get_chunk(cpos).map(|chunk| {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        (cpos, mesh)
                    })
                })
                .collect()
        };

        for (cpos, mesh) in new_meshes {
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(cpos, gpu_mesh);
            } else {
                self.gpu_chunks.remove(&cpos);
            }
        }

        true
    }

    /// Updates all active animals, applies physics and AI, and generates drops for any that died.
    pub fn update_animals(&mut self, dt: f32) -> Vec<(Vec3, Vec<ItemSlot>)> {
        let mut drops = Vec::new();
        let world_guard = self.world.lock().unwrap();

        for animal in self.animals.iter_mut() {
            animal.update(dt, &world_guard, &self.registry);
        }

        self.animals.retain(|animal| {
            if animal.dead {
                let animal_drops = animal.generate_drops();
                drops.push((animal.position, animal_drops));
                false
            } else {
                true
            }
        });

        drops
    }

    /// Performs raycast attack against closest animal within reach.
    pub fn attack_animal(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        reach: f32,
        damage: f32,
    ) -> bool {
        let mut closest_dist = reach;
        let mut hit_idx = None;

        for (idx, animal) in self.animals.iter().enumerate() {
            if animal.dead {
                continue;
            }
            let bbox = animal.aabb();
            if let Some(dist) = bbox.intersect_ray(ray_origin, ray_dir, reach) {
                if dist < closest_dist {
                    closest_dist = dist;
                    hit_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = hit_idx {
            let knockback = ray_dir.normalize_or_zero();
            self.animals[idx].take_damage(damage, knockback);
            true
        } else {
            false
        }
    }

    /// Tills Grass or Dirt into Farmland using a Hoe.
    pub fn till_block(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        let (chunk_pos, lx, ly, lz) = world_to_chunk_coords(pos);
        let mut chunks_to_rebuild = vec![chunk_pos];

        if lx == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x - 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        } else if lx == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x + 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        }
        if ly == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y - 1,
                chunk_pos.0.z,
            ));
        } else if ly == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y + 1,
                chunk_pos.0.z,
            ));
        }
        if lz == 0 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z - 1,
            ));
        } else if lz == 15 {
            chunks_to_rebuild.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z + 1,
            ));
        }

        let new_meshes: Vec<(ChunkPos, ChunkMesh)> = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            let above = world.get_block(pos + IVec3::Y);
            if (current != BlockId::GRASS && current != BlockId::DIRT) || !above.is_air() {
                return false;
            }

            world.set_block(pos, BlockId::FARMLAND);

            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();

            let mesher = ChunkMesher::new(&self.registry);
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world.get_chunk(cpos).map(|chunk| {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        (cpos, mesh)
                    })
                })
                .collect()
        };

        for (cpos, mesh) in new_meshes {
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(cpos, gpu_mesh);
            } else {
                self.gpu_chunks.remove(&cpos);
            }
        }

        true
    }

    /// Registers a planted crop for growth updates.
    pub fn plant_crop(&mut self, pos: IVec3) {
        self.planted_crops.push((pos, 0.0));
    }

    /// Periodically grows planted crops to their next stage.
    pub fn update_crops(&mut self, dt: f32, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        let mut changed_chunks = HashSet::new();
        {
            let mut world = self.world.lock().unwrap();
            for (pos, timer) in self.planted_crops.iter_mut() {
                *timer += dt;
                // Grow to next stage every 8 seconds
                if *timer >= 8.0 {
                    *timer = 0.0;
                    let current = world.get_block(*pos);
                    let next = match current {
                        BlockId::WHEAT_0 => Some(BlockId::WHEAT_1),
                        BlockId::WHEAT_1 => Some(BlockId::WHEAT_2),
                        BlockId::WHEAT_2 => Some(BlockId::WHEAT_3),
                        _ => None,
                    };
                    if let Some(next_block) = next {
                        world.set_block(*pos, next_block);
                        let (cp, _, _, _) = world_to_chunk_coords(*pos);
                        changed_chunks.insert(cp);
                    }
                }
            }
        }

        // Retain only growing crops
        {
            let world = self.world.lock().unwrap();
            self.planted_crops.retain(|(pos, _)| {
                let b = world.get_block(*pos);
                b == BlockId::WHEAT_0 || b == BlockId::WHEAT_1 || b == BlockId::WHEAT_2
            });
        }

        if !changed_chunks.is_empty() {
            let world = self.world.lock().unwrap();
            let mesher = ChunkMesher::new(&self.registry);
            for cpos in changed_chunks {
                if let Some(chunk) = world.get_chunk(cpos) {
                    let origin = cpos.to_world_origin();
                    let sources = world.get_light_sources_near(origin, 15);
                    let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                        world.get_block(origin + IVec3::new(x, y, z))
                    });
                    if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                        self.gpu_chunks.insert(cpos, gpu_mesh);
                    }
                }
            }
        }
    }

    /// Periodically updates all active furnaces, cooking smeltable items, burning fuels,
    /// and updating world block states between FURNACE and FURNACE_LIT.
    pub fn update_furnaces(&mut self, dt: f32, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        let mut changed_chunks = HashSet::new();
        {
            let mut world = self.world.lock().unwrap();
            let mut lit_changes = Vec::new();

            for (&pos, furnace) in world.furnaces.iter_mut() {
                let state_changed = furnace.update(dt);
                if state_changed {
                    let is_burning = furnace.is_burning();
                    lit_changes.push((pos, is_burning));
                }
            }

            for (pos, is_burning) in lit_changes {
                let current_block = world.get_block(pos);
                if is_burning && current_block == BlockId::FURNACE {
                    world.set_block(pos, BlockId::FURNACE_LIT);
                    let (cp, _, _, _) = world_to_chunk_coords(pos);
                    changed_chunks.insert(cp);
                } else if !is_burning && current_block == BlockId::FURNACE_LIT {
                    world.set_block(pos, BlockId::FURNACE);
                    let (cp, _, _, _) = world_to_chunk_coords(pos);
                    changed_chunks.insert(cp);
                }
            }
        }

        if !changed_chunks.is_empty() {
            let world = self.world.lock().unwrap();
            let mesher = ChunkMesher::new(&self.registry);
            for cpos in changed_chunks {
                if let Some(chunk) = world.get_chunk(cpos) {
                    let origin = cpos.to_world_origin();
                    let sources = world.get_light_sources_near(origin, 15);
                    let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                        world.get_block(origin + IVec3::new(x, y, z))
                    });
                    if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                        self.gpu_chunks.insert(cpos, gpu_mesh);
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn total_visible_faces(&self) -> usize {
        self.gpu_chunks
            .values()
            .map(|g| (g.index_count / 6) as usize)
            .sum()
    }
}
