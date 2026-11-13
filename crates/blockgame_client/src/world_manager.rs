use std::collections::{HashMap, HashSet};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use blockgame_core::{
    animal::{AnimalEntity, AnimalKind},
    block::{
        apply_water_fill, break_unsupported_attached, can_block_survive,
        find_decaying_leaves_around_pos, get_leaf_decay_drops, is_farmland_hydrated,
        is_leaf_supported, place_water_source, try_grow_cactus, BlockId, BlockRegistry,
    },
    chunk::{Chunk, ChunkPos, VERTICAL_CHUNKS},
    entity::{FallingBlock, FallingBlockStatus, ItemDrop, ItemDropStatus},
    item::{ItemId, ItemSlot},
    physics::AABB,
    world::{world_to_chunk_coords, World},
};
use blockgame_gen::{Biome, TerrainGenerator};
use blockgame_render::{ChunkMesh, ChunkMesher, GpuMeshBuffers, VoxelPipeline};
use glam::{IVec2, IVec3, Vec3};
use rayon::prelude::*;

pub struct WorldManager {
    world: Arc<Mutex<World>>,
    pub generator: Arc<TerrainGenerator>,
    registry: Arc<BlockRegistry>,
    render_distance: i32,
    loaded_chunks_set: HashSet<ChunkPos>,
    queued_generation: HashSet<ChunkPos>,
    queued_meshing: HashSet<ChunkPos>,
    pub gpu_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    pub gpu_water_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    mesh_rx: Receiver<(ChunkPos, ChunkMesh)>,
    mesh_tx: Sender<(ChunkPos, ChunkMesh)>,
    last_player_chunk: ChunkPos,
    pub item_drops: Vec<ItemDrop>,
    pub falling_blocks: Vec<FallingBlock>,
    pub animals: Vec<AnimalEntity>,
    pub next_animal_id: u32,
    pub planted_crops: Vec<(IVec3, f32)>,
    pub active_farmland: Vec<(IVec3, f32)>,
    pub growing_cacti: Vec<(IVec3, f32)>,
    pub decaying_leaves: Vec<(IVec3, f32)>,
    pub animal_spawned_columns: HashSet<IVec2>,
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
            gpu_water_chunks: HashMap::new(),
            mesh_rx,
            mesh_tx,
            last_player_chunk: ChunkPos::new(i32::MAX, i32::MAX, i32::MAX),
            item_drops: Vec::new(),
            falling_blocks: Vec::new(),
            animals: Vec::new(),
            next_animal_id: 1,
            planted_crops: Vec::new(),
            active_farmland: Vec::new(),
            growing_cacti: Vec::new(),
            decaying_leaves: Vec::new(),
            animal_spawned_columns: HashSet::new(),
        }
    }

    pub fn store_mesh(
        &mut self,
        pos: ChunkPos,
        mesh: &ChunkMesh,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if let Some(gpu_mesh) = pipeline.upload_mesh(device, mesh) {
            self.gpu_chunks.insert(pos, gpu_mesh);
        } else {
            self.gpu_chunks.remove(&pos);
        }
        if let Some(gpu_water) = pipeline.upload_water_mesh(device, mesh) {
            self.gpu_water_chunks.insert(pos, gpu_water);
        } else {
            self.gpu_water_chunks.remove(&pos);
        }
    }

    /// Synchronously pre-generates and meshes the initial area around the spawn point (0, 0, 0).
    pub fn init_spawn_area(&mut self, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        self.init_area_around(Vec3::ZERO, device, pipeline);
    }

    /// Synchronously pre-generates (skipping chunks already loaded from save) and meshes the area
    /// around `center_pos`.
    pub fn init_area_around(
        &mut self,
        center_pos: Vec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        log::info!(
            "Initializing chunks around ({:.1}, {:.1}) (radius {})...",
            center_pos.x,
            center_pos.z,
            self.render_distance
        );

        let (center_chunk, _, _, _) = world_to_chunk_coords(center_pos.floor().as_ivec3());
        let ccx = center_chunk.0.x;
        let ccz = center_chunk.0.z;
        let r = self.render_distance;

        let mut area_chunks = Vec::new();
        let mut missing_chunks = Vec::new();

        {
            let world = self.world.lock().unwrap();
            for cy in 0..VERTICAL_CHUNKS {
                for dz in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dz * dz <= (r + 1) * (r + 1) {
                            let pos = ChunkPos::new(ccx + dx, cy, ccz + dz);
                            area_chunks.push(pos);
                            if world.get_chunk(pos).is_none() {
                                missing_chunks.push(pos);
                            }
                        }
                    }
                }
            }
        }

        // Parallel chunk terrain generation using Rayon for any chunks not already loaded from save
        if !missing_chunks.is_empty() {
            let generator = self.generator.clone();
            let generated_chunks: Vec<Chunk> = missing_chunks
                .par_iter()
                .map(|&pos| {
                    let mut chunk = generator.generate_chunk(pos);
                    generator.populate_chunk_features(&mut chunk);
                    chunk
                })
                .collect();

            let mut world = self.world.lock().unwrap();
            for chunk in generated_chunks {
                world.insert_chunk(chunk);
            }
        }

        for &pos in &area_chunks {
            self.loaded_chunks_set.insert(pos);
        }

        // Parallel chunk meshing for all chunks in the area
        let world_guard = self.world.lock().unwrap();
        let mesher = ChunkMesher::new(&self.registry);

        let meshes: Vec<(ChunkPos, ChunkMesh)> = area_chunks
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
            self.store_mesh(pos, &mesh, device, pipeline);
        }

        self.last_player_chunk = center_chunk;

        log::info!(
            "Area ready: {} chunks loaded, {} visible faces rendered",
            self.gpu_chunks.len(),
            face_count
        );

        // Biome-aware initial animal herd spawning across area columns
        {
            let world_arc = self.world.clone();
            let world_guard = world_arc.lock().unwrap();
            for dz in -r..=r {
                for dx in -r..=r {
                    self.try_spawn_animals_in_column(ccx + dx, ccz + dz, &world_guard);
                }
            }
        }
    }

    /// Restores saved world chunks, containers, entities, and simulation timers into this `WorldManager`.
    #[allow(dead_code)]
    pub fn apply_save_data(&mut self, data: &blockgame_core::save::WorldSaveData) {
        {
            let mut world = self.world.lock().unwrap();
            data.apply_to_world(&mut world);
        }
        self.item_drops = data.item_drops.clone();
        self.falling_blocks = data.falling_blocks.clone();
        self.animals = data.animals.clone();
        self.next_animal_id = data.next_animal_id;
        self.planted_crops = data.planted_crops.clone();
        self.active_farmland = data.active_farmland.clone();
        self.growing_cacti = data.growing_cacti.clone();
        self.decaying_leaves = data.decaying_leaves.clone();
        self.animal_spawned_columns = data.animal_spawned_columns.clone();
    }

    /// Captures all modified chunks, containers, entities, and simulation timers from this `WorldManager`
    /// into a `WorldSaveData` ready to be written to disk.
    #[allow(dead_code)]
    pub fn capture_save_data(
        &self,
        metadata: blockgame_core::save::WorldMetadata,
        player: blockgame_core::save::SavedPlayerState,
    ) -> blockgame_core::save::WorldSaveData {
        let mut data = blockgame_core::save::WorldSaveData::new(metadata, player);
        {
            let world = self.world.lock().unwrap();
            data.capture_from_world(&world);
        }
        data.item_drops = self.item_drops.clone();
        data.falling_blocks = self.falling_blocks.clone();
        data.animals = self.animals.clone();
        data.next_animal_id = self.next_animal_id;
        data.planted_crops = self.planted_crops.clone();
        data.active_farmland = self.active_farmland.clone();
        data.growing_cacti = self.growing_cacti.clone();
        data.decaying_leaves = self.decaying_leaves.clone();
        data.animal_spawned_columns = self.animal_spawned_columns.clone();
        data
    }

    /// Evaluates biome-aware animal spawning in a 16x16 chunk column (cx, cz).
    pub fn try_spawn_animals_in_column(&mut self, cx: i32, cz: i32, world: &World) {
        // Prevent re-spawning in the same column
        if !self.animal_spawned_columns.insert(IVec2::new(cx, cz)) {
            return;
        }

        // Global animal cap to protect performance and maintain balanced density
        if self.animals.len() >= 35 {
            return;
        }

        let center_x = cx * 16 + 8;
        let center_z = cz * 16 + 8;
        let surface_y = self.generator.surface_height(center_x, center_z);
        let biome = self.generator.get_biome(center_x, center_z, surface_y);

        // Biome spawn probabilities & herd compositions:
        // - Desert & Ocean: 0% spawn chance
        // - Plains: 65% spawn chance (pasture grazing: cows, pigs, chickens)
        // - Forest: 50% spawn chance (woodland foraging: pigs, chickens, occasional cows)
        // - Mountains: 20% spawn chance (high-altitude chickens, occasional cows/pigs)
        // - Savanna: 60% spawn chance (grazing cows and pigs)
        // - Swamp: 55% spawn chance (mud wallowing pigs and chickens)
        // - SnowyTundra: 25% spawn chance (cold adapted chickens, cows, pigs)
        // - Taiga: 55% spawn chance (coniferous foraging: cows, pigs, chickens)
        // - MushroomFields: 70% spawn chance (peaceful fungal sanctuary: cows)
        // - River: 20% spawn chance (riverbank chickens/pigs)
        let (spawn_chance, kind_weights, min_herd, max_herd) = match biome {
            Biome::Desert | Biome::Ocean | Biome::Badlands | Biome::CoralReef => return,
            Biome::Plains => (0.65, [(AnimalKind::Cow, 0.40), (AnimalKind::Pig, 0.35), (AnimalKind::Chicken, 0.25)], 2, 4),
            Biome::Forest => (0.50, [(AnimalKind::Pig, 0.45), (AnimalKind::Chicken, 0.35), (AnimalKind::Cow, 0.20)], 2, 3),
            Biome::DarkForest => (0.50, [(AnimalKind::Pig, 0.45), (AnimalKind::Chicken, 0.35), (AnimalKind::Cow, 0.20)], 2, 4),
            Biome::Mountains => (0.20, [(AnimalKind::Chicken, 0.50), (AnimalKind::Cow, 0.30), (AnimalKind::Pig, 0.20)], 1, 2),
            Biome::Savanna => (0.60, [(AnimalKind::Cow, 0.55), (AnimalKind::Pig, 0.25), (AnimalKind::Chicken, 0.20)], 2, 4),
            Biome::Swamp => (0.55, [(AnimalKind::Pig, 0.60), (AnimalKind::Chicken, 0.30), (AnimalKind::Cow, 0.10)], 2, 4),
            Biome::SnowyTundra => (0.25, [(AnimalKind::Chicken, 0.45), (AnimalKind::Cow, 0.35), (AnimalKind::Pig, 0.20)], 1, 3),
            Biome::Jungle => (0.65, [(AnimalKind::Chicken, 0.50), (AnimalKind::Pig, 0.35), (AnimalKind::Cow, 0.15)], 2, 5),
            Biome::Beach => (0.30, [(AnimalKind::Chicken, 0.70), (AnimalKind::Pig, 0.20), (AnimalKind::Cow, 0.10)], 1, 3),
            Biome::Taiga => (0.55, [(AnimalKind::Cow, 0.40), (AnimalKind::Pig, 0.35), (AnimalKind::Chicken, 0.25)], 2, 4),
            Biome::MushroomFields => (0.70, [(AnimalKind::Cow, 1.00), (AnimalKind::Pig, 0.0), (AnimalKind::Chicken, 0.0)], 2, 4),
            Biome::River => (0.20, [(AnimalKind::Chicken, 0.60), (AnimalKind::Pig, 0.30), (AnimalKind::Cow, 0.10)], 1, 2),
        };

        // Deterministic pseudo-random seed per chunk column
        let mut rng = ((cx as u64).wrapping_mul(73856093)
            ^ (cz as u64).wrapping_mul(19349663)
            ^ (self.generator.seed as u64))
            ^ 0x9e3779b97f4a7c15;

        #[inline]
        fn next_f32(rng: &mut u64) -> f32 {
            *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((*rng >> 32) as u32 as f32) / (u32::MAX as f32)
        }

        if next_f32(&mut rng) > spawn_chance {
            return;
        }

        // Select animal kind for this herd
        let roll = next_f32(&mut rng);
        let mut accum = 0.0;
        let mut selected_kind = kind_weights[0].0;
        for (kind, weight) in kind_weights {
            accum += weight;
            if roll <= accum {
                selected_kind = kind;
                break;
            }
        }

        let herd_size = min_herd + ((next_f32(&mut rng) * (max_herd - min_herd + 1) as f32).floor() as usize).min(max_herd - min_herd);

        for _ in 0..herd_size {
            if self.animals.len() >= 35 {
                break;
            }
            let dx = ((next_f32(&mut rng) * 8.0) - 4.0).round() as i32;
            let dz = ((next_f32(&mut rng) * 8.0) - 4.0).round() as i32;
            let ax = center_x + dx;
            let az = center_z + dz;

            // Find top solid ground block
            let mut ay = (VERTICAL_CHUNKS * 16) - 2;
            while ay > 1 {
                let b = world.get_block(IVec3::new(ax, ay, az));
                if b == BlockId::GRASS || b == BlockId::DIRT || b == BlockId::SNOW_BLOCK || b == BlockId::CLAY || b == BlockId::SAND || b == BlockId::PODZOL || b == BlockId::MYCELIUM {
                    break;
                }
                ay -= 1;
            }

            if ay > 1 {
                let above = world.get_block(IVec3::new(ax, ay + 1, az));
                if above.is_air() || above == BlockId::TALL_GRASS {
                    let mut entity = AnimalEntity::new(
                        self.next_animal_id,
                        selected_kind,
                        Vec3::new(ax as f32 + 0.5, (ay + 1) as f32, az as f32 + 0.5),
                    );
                    entity.rng_state = rng;
                    self.animals.push(entity);
                    self.next_animal_id += 1;
                }
            }
        }
    }

    /// Periodic update called every frame: streams chunks and uploads completed background meshes.
    pub fn update(&mut self, player_pos: Vec3, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        // 1. Ingest background mesh results (all completed meshes uploaded)
        let mut completed_surface_chunks = Vec::new();
        while let Ok((pos, mesh)) = self.mesh_rx.try_recv() {
            self.queued_meshing.remove(&pos);
            self.queued_generation.remove(&pos);
            self.loaded_chunks_set.insert(pos);
            if pos.0.y == 0 {
                completed_surface_chunks.push((pos.0.x, pos.0.z));
            }
            self.store_mesh(pos, &mesh, device, pipeline);
        }

        if !completed_surface_chunks.is_empty() {
            let world_arc = self.world.clone();
            let world_guard = world_arc.lock().unwrap();
            for (cx, cz) in completed_surface_chunks {
                self.try_spawn_animals_in_column(cx, cz, &world_guard);
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
                for cy in 0..VERTICAL_CHUNKS {
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
                self.gpu_water_chunks.remove(&pos);
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

    /// Renders loaded GPU water chunk meshes with optional camera view frustum culling.
    pub fn render_water_with_frustum<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
    ) {
        for (&pos, gpu_mesh) in &self.gpu_water_chunks {
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

    /// Renders all currently loaded GPU water chunk meshes without frustum culling.
    pub fn render_water<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.render_water_with_frustum(render_pass, None);
    }

    pub fn world(&self) -> &Arc<Mutex<World>> {
        &self.world
    }

    pub fn registry(&self) -> &Arc<BlockRegistry> {
        &self.registry
    }

    #[inline]
    fn add_chunk_and_neighbors(pos: IVec3, chunks: &mut Vec<ChunkPos>) {
        let (chunk_pos, lx, ly, lz) = world_to_chunk_coords(pos);
        chunks.push(chunk_pos);
        if lx == 0 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x - 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        } else if lx == 15 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x + 1,
                chunk_pos.0.y,
                chunk_pos.0.z,
            ));
        }
        if ly == 0 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y - 1,
                chunk_pos.0.z,
            ));
        } else if ly == 15 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y + 1,
                chunk_pos.0.z,
            ));
        }
        if lz == 0 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z - 1,
            ));
        } else if lz == 15 {
            chunks.push(ChunkPos::new(
                chunk_pos.0.x,
                chunk_pos.0.y,
                chunk_pos.0.z + 1,
            ));
        }
    }

    pub fn break_block(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> Option<BlockId> {
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let (broken_block, new_meshes, dropped_chest_items, extra_drops, spawned_falling): (
            BlockId,
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<ItemSlot>,
            Vec<(Vec3, ItemSlot)>,
            Vec<(BlockId, IVec3)>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if current.is_air() {
                return None;
            }
            let is_light_source = current.is_emissive();
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
            let replacement = if current == BlockId::SEAGRASS {
                BlockId::WATER
            } else {
                BlockId::AIR
            };
            world.set_block(pos, replacement);

            // If breaking one half of a 2-block-tall door, also remove the other half cleanly
            if current.is_door() {
                let pair_pos = if current.is_door_top() {
                    pos - IVec3::Y
                } else {
                    pos + IVec3::Y
                };
                if world.get_block(pair_pos).is_door() {
                    world.set_block(pair_pos, BlockId::AIR);
                    Self::add_chunk_and_neighbors(pair_pos, &mut chunks_to_rebuild);
                }
            }

            // Check unsupported attached blocks that depend on pos
            let (unsupported_drops, modified_positions, spawned_falling) =
                break_unsupported_attached(pos, &mut world, &self.registry);

            for mpos in modified_positions {
                Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
            }
            let mut extra_drops = Vec::new();
            for (mpos, drop) in unsupported_drops {
                let drop_pos = mpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
                extra_drops.push((drop_pos, drop));
            }

            // If broken block was air and has adjacent water, water flows to fill exposed space
            let (water_filled, water_drops) = apply_water_fill(pos, &mut world, &self.registry);
            for wpos in water_filled {
                Self::add_chunk_and_neighbors(wpos, &mut chunks_to_rebuild);
            }
            for (wpos, drop) in water_drops {
                extra_drops.push((wpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5), drop));
            }

            if is_light_source {
                let (chunk_pos, _, _, _) = world_to_chunk_coords(pos);
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
            (current, meshes, dropped_chest_items, extra_drops, spawned_falling)
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        let drop_pos = pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
        for item in dropped_chest_items {
            if !item.is_empty() {
                self.spawn_item_drop(drop_pos, item);
            }
        }
        for (extra_pos, item) in extra_drops {
            if !item.is_empty() {
                self.spawn_item_drop(extra_pos, item);
            }
        }
        for (block_id, fpos) in spawned_falling {
            self.falling_blocks.push(FallingBlock::new(block_id, fpos));
        }

        // Clean up planted crops if wheat was removed
        {
            let world = self.world.lock().unwrap();
            self.planted_crops.retain(|(p, _)| {
                let b = world.get_block(*p);
                b == BlockId::WHEAT_0 || b == BlockId::WHEAT_1 || b == BlockId::WHEAT_2
            });
        }

        // If broken block was a tree log or leaves, check for leaves that have lost all log support
        if broken_block.is_log() || broken_block.is_leaves() {
            let world = self.world.lock().unwrap();
            let decaying = find_decaying_leaves_around_pos(pos, 4, |p| world.get_block(p));
            for (i, lp) in decaying.into_iter().enumerate() {
                if !self.decaying_leaves.iter().any(|(p, _)| *p == lp) {
                    let delay = 0.1 + (i as f32 * 0.05);
                    self.decaying_leaves.push((lp, delay));
                }
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

    /// Spawns a floating item drop with an initial velocity vector
    pub fn spawn_item_drop_with_velocity(&mut self, pos: Vec3, vel: Vec3, item: ItemSlot) {
        if !item.is_empty() {
            self.item_drops.push(ItemDrop::with_velocity(pos, vel, item));
        }
    }

    /// Spawns a floating item drop with an explicit network drop ID and velocity
    pub fn spawn_item_drop_with_id_and_velocity(
        &mut self,
        drop_id: u64,
        pos: Vec3,
        vel: Vec3,
        item: ItemSlot,
    ) {
        if !item.is_empty() && !self.item_drops.iter().any(|d| d.drop_id == drop_id) {
            self.item_drops
                .push(ItemDrop::with_id_and_velocity(drop_id, pos, vel, item));
        }
    }

    /// Removes an item drop by drop ID (e.g. when collected on server)
    pub fn remove_item_drop(&mut self, drop_id: u64) -> Option<ItemSlot> {
        if let Some(idx) = self.item_drops.iter().position(|d| d.drop_id == drop_id) {
            Some(self.item_drops.remove(idx).item)
        } else {
            None
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

    /// Updates item drops in multiplayer: runs physics, collision, and magnetism toward the player,
    /// returning drop IDs of items within pickup range whose collection has been initiated.
    pub fn update_item_drops_multiplayer(&mut self, dt: f32, player_pos: Vec3) -> Vec<u64> {
        let mut pickable = Vec::new();
        let world_guard = self.world.lock().unwrap();

        self.item_drops.retain_mut(|drop| {
            let alive = drop.update_physics_only(dt, &world_guard, &self.registry, player_pos);
            if !alive {
                return false;
            }
            if drop.can_be_collected_by(player_pos) {
                drop.pickup_pending = true;
                pickable.push(drop.drop_id);
            }
            true
        });

        pickable
    }

    pub fn place_block(
        &mut self,
        pos: IVec3,
        block_id: BlockId,
        player_aabb: &AABB,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        // Prevent placing inside player collider if block is solid
        if self.registry.is_solid(block_id) {
            let block_box = AABB::from_block(pos);
            if player_aabb.intersects(&block_box) {
                return false;
            }
        }

        // If gravity block is placed and the block below is not solid, spawn as falling block entity
        if block_id.has_gravity() {
            let world_guard = self.world.lock().unwrap();
            let current = world_guard.get_block(pos);
            let can_place = current.is_air() || current == BlockId::WATER;
            if can_place {
                let below = world_guard.get_block(pos - IVec3::Y);
                if !self.registry.is_solid(below) {
                    drop(world_guard);
                    self.falling_blocks.push(FallingBlock::new(block_id, pos));
                    return true;
                }
            }
        }

        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let (new_meshes, extra_drops, spawned_falling): (
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<(Vec3, ItemSlot)>,
            Vec<(BlockId, IVec3)>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            let can_place = if current.is_air() {
                true
            } else if current == BlockId::WATER {
                block_id.can_exist_in_water() || block_id.has_gravity() || self.registry.is_solid(block_id)
            } else {
                false
            };
            if !can_place {
                return false;
            }

            // Check if block can survive at pos
            if block_id.requires_support() {
                let can_survive = can_block_survive(
                    block_id,
                    pos,
                    &self.registry,
                    |p| if p == pos { block_id } else { world.get_block(p) },
                );
                if !can_survive {
                    return false;
                }
            }

            // 2-block-tall Oak Door requires empty space at pos + Y (and no player collision)
            if block_id == BlockId::OAK_DOOR_CLOSED {
                let top_pos = pos + IVec3::Y;
                if !world.get_block(top_pos).is_air() || player_aabb.intersects(&AABB::from_block(top_pos)) {
                    return false;
                }
            }

            let is_light_source = block_id.is_emissive();
            world.set_block(pos, block_id);
            if block_id == BlockId::OAK_DOOR_CLOSED {
                let top_pos = pos + IVec3::Y;
                world.set_block(top_pos, BlockId::OAK_DOOR_TOP_CLOSED);
                Self::add_chunk_and_neighbors(top_pos, &mut chunks_to_rebuild);
            }
            if block_id == BlockId::FURNACE || block_id == BlockId::FURNACE_LIT {
                world.get_or_create_furnace(pos);
            }

            // If a solid block is placed, check if any horizontal cactus neighbors are invalidated
            let mut extra_drops = Vec::new();
            let mut spawned_falling = Vec::new();
            if self.registry.is_solid(block_id) {
                let (unsupported_drops, modified_positions, fall_blocks) =
                    break_unsupported_attached(pos, &mut world, &self.registry);

                for mpos in modified_positions {
                    Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
                }
                for (mpos, drop) in unsupported_drops {
                    let drop_pos = mpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
                    extra_drops.push((drop_pos, drop));
                }
                spawned_falling = fall_blocks;
            }

            if is_light_source {
                let (chunk_pos, _, _, _) = world_to_chunk_coords(pos);
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
            (meshes, extra_drops, spawned_falling)
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        for (extra_pos, item) in extra_drops {
            if !item.is_empty() {
                self.spawn_item_drop(extra_pos, item);
            }
        }
        for (block_id, fpos) in spawned_falling {
            self.falling_blocks.push(FallingBlock::new(block_id, fpos));
        }

        if block_id == BlockId::CACTUS {
            self.register_cactus(pos);
        }
        if block_id.is_farmland() {
            self.register_farmland(pos);
        }

        true
    }

    /// Registers a farmland block for moisture and drying updates.
    pub fn register_farmland(&mut self, pos: IVec3) {
        if !self.active_farmland.iter().any(|(p, _)| *p == pos) {
            self.active_farmland.push((pos, 0.0));
        }
    }

    /// Scoops a water block at `pos` with an empty bucket, replacing it with AIR and updating meshes.
    pub fn scoop_water(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let (scooped, new_meshes, extra_drops, spawned_falling): (
            bool,
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<(Vec3, ItemSlot)>,
            Vec<(BlockId, IVec3)>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if !current.is_water() {
                return false;
            }

            world.set_block(pos, BlockId::AIR);

            // Break any blocks above or adjacent that required support or water (e.g. lily pads, seagrass)
            let (unsupported_drops, modified_positions, spawned_falling) =
                break_unsupported_attached(pos, &mut world, &self.registry);
            for p in modified_positions {
                Self::add_chunk_and_neighbors(p, &mut chunks_to_rebuild);
            }
            let mut extra_drops = Vec::new();
            for (p, drop) in unsupported_drops {
                let drop_pos = p.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
                extra_drops.push((drop_pos, drop));
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

            (true, meshes, extra_drops, spawned_falling)
        };

        if !scooped {
            return false;
        }

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        for (extra_pos, item) in extra_drops {
            if !item.is_empty() {
                self.spawn_item_drop(extra_pos, item);
            }
        }
        for (block_id, fpos) in spawned_falling {
            self.falling_blocks.push(FallingBlock::new(block_id, fpos));
        }

        true
    }

    /// Places a water block at `pos` from a water bucket, cascading downward and washing fragile blocks.
    pub fn place_water(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let (placed, new_meshes, extra_drops): (
            bool,
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<(Vec3, ItemSlot)>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if !current.is_air() && !current.is_fragile() {
                return false;
            }

            let (modified_positions, water_drops) = place_water_source(pos, &mut world);
            for mpos in modified_positions {
                Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
            }
            let mut extra_drops = Vec::new();
            for (p, drop) in water_drops {
                extra_drops.push((p.as_vec3() + Vec3::new(0.5, 0.5, 0.5), drop));
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

            (true, meshes, extra_drops)
        };

        if !placed {
            return false;
        }

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        for (extra_pos, item) in extra_drops {
            if !item.is_empty() {
                self.spawn_item_drop(extra_pos, item);
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
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let (new_meshes, extra_drops, spawned_falling): (
            Vec<(ChunkPos, ChunkMesh)>,
            Vec<(Vec3, ItemSlot)>,
            Vec<(BlockId, IVec3)>,
        ) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            let above = world.get_block(pos + IVec3::Y);
            if (current != BlockId::GRASS && current != BlockId::DIRT)
                || (!above.is_air() && above != BlockId::TALL_GRASS)
            {
                return false;
            }

            let is_hydrated = is_farmland_hydrated(pos, |p| world.get_block(p));
            let target_block = if is_hydrated {
                BlockId::FARMLAND_MOIST
            } else {
                BlockId::FARMLAND
            };
            world.set_block(pos, target_block);

            let (unsupported_drops, modified_positions, spawned_falling) =
                break_unsupported_attached(pos, &mut world, &self.registry);

            for mpos in modified_positions {
                Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
            }
            let mut extra_drops = Vec::new();
            for (mpos, drop) in unsupported_drops {
                let drop_pos = mpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
                extra_drops.push((drop_pos, drop));
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
            (meshes, extra_drops, spawned_falling)
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        for (extra_pos, item) in extra_drops {
            if !item.is_empty() {
                self.spawn_item_drop(extra_pos, item);
            }
        }
        for (block_id, fpos) in spawned_falling {
            self.falling_blocks.push(FallingBlock::new(block_id, fpos));
        }

        self.register_farmland(pos);

        true
    }

    /// Registers a planted crop for growth updates.
    pub fn plant_crop(&mut self, pos: IVec3) {
        self.planted_crops.push((pos, 0.0));
        self.register_farmland(pos - IVec3::Y);
    }

    /// Periodically grows planted crops to their next stage, requiring nearby water to stay hydrated.
    pub fn update_crops(&mut self, dt: f32, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        let mut changed_chunks = HashSet::new();
        let mut extra_drops = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            for (pos, timer) in self.planted_crops.iter_mut() {
                let below_pos = *pos - IVec3::Y;
                let below = world.get_block(below_pos);
                if !below.is_farmland() {
                    // Farmland missing -> pop crop as item drop
                    let current = world.get_block(*pos);
                    world.set_block(*pos, BlockId::AIR);
                    let (cp, _, _, _) = world_to_chunk_coords(*pos);
                    changed_chunks.insert(cp);
                    let drop_item = if current == BlockId::WHEAT_3 {
                        ItemSlot::new(ItemId::WHEAT, 1)
                    } else {
                        ItemSlot::new(ItemId::WHEAT_SEEDS, 1)
                    };
                    extra_drops.push((pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5), drop_item));
                    continue;
                }

                // Farmland requires water nearby to stay hydrated and grow crops
                let is_hydrated = is_farmland_hydrated(below_pos, |p| world.get_block(p));
                if !is_hydrated {
                    // Crop does NOT grow without water. Drying into dirt is handled in update_farmland.
                    continue;
                }

                // Farmland is hydrated: grow crop stage every 8 seconds
                *timer += dt;
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

        // Retain only growing crops with farmland beneath
        {
            let world = self.world.lock().unwrap();
            self.planted_crops.retain(|(pos, _)| {
                let b = world.get_block(*pos);
                let below = world.get_block(*pos - IVec3::Y);
                (b == BlockId::WHEAT_0 || b == BlockId::WHEAT_1 || b == BlockId::WHEAT_2)
                    && below.is_farmland()
            });
        }

        for (drop_pos, slot) in extra_drops {
            self.spawn_item_drop(drop_pos, slot);
        }

        if !changed_chunks.is_empty() {
            let registry = Arc::clone(&self.registry);
            let mesher = ChunkMesher::new(&registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        meshes.push((cpos, mesh));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
        }
    }

    /// Updates all active farmland blocks, transitioning between dry (FARMLAND) and moist (FARMLAND_MOIST)
    /// based on nearby water hydration, and reverting unhydrated farmland back to DIRT after a timer.
    pub fn update_farmland(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if self.active_farmland.is_empty() {
            return;
        }

        let mut changed_chunks = HashSet::new();
        let mut extra_drops = Vec::new();

        {
            let mut world = self.world.lock().unwrap();
            for (pos, timer) in self.active_farmland.iter_mut() {
                let current = world.get_block(*pos);
                if !current.is_farmland() {
                    continue;
                }

                let hydrated = is_farmland_hydrated(*pos, |p| world.get_block(p));
                if hydrated {
                    *timer = 0.0;
                    if current == BlockId::FARMLAND {
                        // Became hydrated: turn into MOIST farmland!
                        world.set_block(*pos, BlockId::FARMLAND_MOIST);
                        let (cp, _, _, _) = world_to_chunk_coords(*pos);
                        changed_chunks.insert(cp);
                    }
                } else {
                    if current == BlockId::FARMLAND_MOIST {
                        // Lost hydration: turn into dry farmland!
                        world.set_block(*pos, BlockId::FARMLAND);
                        let (cp, _, _, _) = world_to_chunk_coords(*pos);
                        changed_chunks.insert(cp);
                    }

                    *timer += dt;
                    let above_pos = *pos + IVec3::Y;
                    let above = world.get_block(above_pos);
                    let has_crop = matches!(
                        above,
                        BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 | BlockId::WHEAT_3
                    );

                    // Revert time: 5s if empty farmland, 10s if crop on top
                    let revert_time = if has_crop { 10.0 } else { 5.0 };
                    if *timer >= revert_time {
                        // Farmland reverts back to regular dirt!
                        world.set_block(*pos, BlockId::DIRT);
                        let (cp1, _, _, _) = world_to_chunk_coords(*pos);
                        changed_chunks.insert(cp1);

                        if has_crop {
                            world.set_block(above_pos, BlockId::AIR);
                            let (cp2, _, _, _) = world_to_chunk_coords(above_pos);
                            changed_chunks.insert(cp2);
                            let drop_item = if above == BlockId::WHEAT_3 {
                                ItemSlot::new(ItemId::WHEAT, 1)
                            } else {
                                ItemSlot::new(ItemId::WHEAT_SEEDS, 1)
                            };
                            extra_drops.push((
                                above_pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                drop_item,
                            ));
                        }
                    }
                }
            }
        }

        // Retain only blocks that are still farmland
        {
            let world = self.world.lock().unwrap();
            self.active_farmland.retain(|(pos, _)| {
                world.get_block(*pos).is_farmland()
            });
        }

        for (drop_pos, slot) in extra_drops {
            self.spawn_item_drop(drop_pos, slot);
        }

        if !changed_chunks.is_empty() {
            let registry = Arc::clone(&self.registry);
            let mesher = ChunkMesher::new(&registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        meshes.push((cpos, mesh));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
        }
    }

    /// Registers a cactus block for height growth updates.
    pub fn register_cactus(&mut self, pos: IVec3) {
        if !self.growing_cacti.iter().any(|(p, _)| *p == pos) {
            self.growing_cacti.push((pos, 0.0));
        }
    }

    /// Periodically grows cacti up to a maximum height of 3 blocks.
    pub fn update_cacti(&mut self, dt: f32, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        let mut changed_chunks = HashSet::new();
        let mut new_cacti = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            for (pos, timer) in self.growing_cacti.iter_mut() {
                *timer += dt;
                // Check growth every 10 seconds
                if *timer >= 10.0 {
                    *timer = 0.0;
                    if let Some(grow_pos) =
                        try_grow_cactus(*pos, &self.registry, |p| world.get_block(p))
                    {
                        world.set_block(grow_pos, BlockId::CACTUS);
                        let (cp, _, _, _) = world_to_chunk_coords(grow_pos);
                        changed_chunks.insert(cp);
                        new_cacti.push(grow_pos);
                    }
                }
            }
        }

        for nc in new_cacti {
            self.register_cactus(nc);
        }

        // Retain only cacti that still exist and can grow (column height < 3)
        {
            let world = self.world.lock().unwrap();
            self.growing_cacti.retain(|(pos, _)| {
                world.get_block(*pos) == BlockId::CACTUS
                    && try_grow_cactus(*pos, &self.registry, |p| world.get_block(p)).is_some()
            });
        }

        if !changed_chunks.is_empty() {
            let registry = Arc::clone(&self.registry);
            let mesher = ChunkMesher::new(&registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        meshes.push((cpos, mesh));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
        }
    }

    /// Periodically updates and removes decaying leaves that have lost tree log connectivity.
    pub fn update_decaying_leaves(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if self.decaying_leaves.is_empty() {
            return;
        }

        let mut decayed_positions = Vec::new();
        for (pos, timer) in self.decaying_leaves.iter_mut() {
            *timer -= dt;
            if *timer <= 0.0 {
                decayed_positions.push(*pos);
            }
        }

        if decayed_positions.is_empty() {
            return;
        }

        let mut changed_chunks = HashSet::new();
        let mut extra_drops = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            for pos in &decayed_positions {
                let block = world.get_block(*pos);
                if block.is_leaves() {
                    // Check one last time if it's still unsupported
                    if !is_leaf_supported(*pos, 4, |p| world.get_block(p)) {
                        world.set_block(*pos, BlockId::AIR);
                        let (cp, _, _, _) = world_to_chunk_coords(*pos);
                        changed_chunks.insert(cp);
                        let drops = get_leaf_decay_drops(block, *pos);
                        for d in drops {
                            extra_drops.push((pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5), d));
                        }
                    }
                }
            }
        }

        self.decaying_leaves
            .retain(|(p, _)| !decayed_positions.contains(p));

        for (p, drop) in extra_drops {
            self.spawn_item_drop(p, drop);
        }

        if !changed_chunks.is_empty() {
            let registry = Arc::clone(&self.registry);
            let mesher = ChunkMesher::new(&registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        meshes.push((cpos, mesh));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
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
            let registry = Arc::clone(&self.registry);
            let mesher = ChunkMesher::new(&registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        let origin = cpos.to_world_origin();
                        let sources = world.get_light_sources_near(origin, 15);
                        let mesh = mesher.mesh_chunk_with_lighting(chunk, &sources, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        meshes.push((cpos, mesh));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
        }
    }

    /// Updates all active falling gravity blocks (Sand, Red Sand, Gravel), moves them with gravity,
    /// solidifies them when they hit ground, displaces water, drops fragile blocks, and remeshes affected chunks.
    pub fn update_falling_blocks(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if self.falling_blocks.is_empty() {
            return;
        }

        let mut landed_blocks: Vec<(BlockId, IVec3)> = Vec::new();
        let mut dropped_items: Vec<(Vec3, ItemSlot)> = Vec::new();

        {
            let mut world = self.world.lock().unwrap();
            self.falling_blocks.retain_mut(|fb| {
                match fb.update(dt, &world, &self.registry) {
                    FallingBlockStatus::Falling => true,
                    FallingBlockStatus::Landed(land_pos) => {
                        let old_block = world.get_block(land_pos);
                        // If old_block is fragile (wheat, tall grass, torch, lily pad), drop it
                        if old_block.is_fragile() {
                            let drop_item = match old_block {
                                BlockId::TORCH => {
                                    ItemSlot::new(blockgame_core::item::ItemId::TORCH, 1)
                                }
                                BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 => {
                                    ItemSlot::new(blockgame_core::item::ItemId::WHEAT_SEEDS, 1)
                                }
                                BlockId::WHEAT_3 => {
                                    ItemSlot::new(blockgame_core::item::ItemId::WHEAT, 1)
                                }
                                BlockId::TALL_GRASS => {
                                    ItemSlot::new(blockgame_core::item::ItemId::WHEAT_SEEDS, 1)
                                }
                                BlockId::LILY_PAD => {
                                    ItemSlot::new(blockgame_core::item::ItemId::LILY_PAD, 1)
                                }
                                _ => ItemSlot::EMPTY,
                            };
                            if !drop_item.is_empty() {
                                dropped_items.push((
                                    land_pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                                    drop_item,
                                ));
                            }
                        }
                        // Solidify at land_pos (displacing air, water, or crushed fragile block)
                        world.set_block(land_pos, fb.block_id);
                        landed_blocks.push((fb.block_id, land_pos));
                        false
                    }
                    FallingBlockStatus::Despawned => false,
                }
            });
        }

        for (pos, item) in dropped_items {
            self.spawn_item_drop(pos, item);
        }

        if !landed_blocks.is_empty() {
            let mut chunks_to_rebuild = Vec::new();
            let (new_meshes, extra_drops, more_falling) = {
                let mut world = self.world.lock().unwrap();
                let mut extra_drops = Vec::new();
                let mut more_falling = Vec::new();

                for (_, land_pos) in &landed_blocks {
                    Self::add_chunk_and_neighbors(*land_pos, &mut chunks_to_rebuild);
                    let (u_drops, mod_pos, fall_blocks) =
                        break_unsupported_attached(*land_pos, &mut world, &self.registry);
                    for mpos in mod_pos {
                        Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
                    }
                    for (mpos, drop) in u_drops {
                        extra_drops.push((mpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5), drop));
                    }
                    more_falling.extend(fall_blocks);
                }

                chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
                chunks_to_rebuild.dedup();

                let mesher = ChunkMesher::new(&self.registry);
                let meshes: Vec<(ChunkPos, ChunkMesh)> = chunks_to_rebuild
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
                (meshes, extra_drops, more_falling)
            };

            for (cpos, mesh) in new_meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
            for (pos, item) in extra_drops {
                self.spawn_item_drop(pos, item);
            }
            for (block_id, fpos) in more_falling {
                self.falling_blocks.push(FallingBlock::new(block_id, fpos));
            }
        }
    }

    /// Applies a single block update received from a multiplayer server and remeshes affected chunks.
    pub fn apply_remote_block_update(
        &mut self,
        pos: IVec3,
        block_id: BlockId,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let new_meshes = {
            let mut world = self.world.lock().unwrap();
            let old_block = world.get_block(pos);
            if old_block == block_id {
                return;
            }
            let is_light_change = old_block.is_emissive() || block_id.is_emissive();
            world.set_block(pos, block_id);

            if is_light_change {
                let (chunk_pos, _, _, _) = world_to_chunk_coords(pos);
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
                .collect::<Vec<_>>()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

    /// Applies an initial `WorldDeltaSync` packet from the multiplayer server and remeshes modified chunks.
    pub fn apply_world_delta_sync(
        &mut self,
        modified_chunks: &[(ChunkPos, Vec<(u16, BlockId)>)],
        chests: &[(IVec3, Vec<ItemSlot>)],
        furnaces: &[(IVec3, blockgame_core::world::FurnaceState)],
        item_drops: &[(u64, Vec3, Vec3, ItemSlot)],
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let mut chunks_to_rebuild = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            for (cpos, blocks) in modified_chunks {
                if !world.has_chunk(*cpos) {
                    let mut chunk = self.generator.generate_chunk(*cpos);
                    self.generator.populate_chunk_features(&mut chunk);
                    world.insert_chunk(chunk);
                }
                let origin = cpos.to_world_origin();
                for &(idx, block_id) in blocks {
                    let lx = (idx as usize % 16) as i32;
                    let lz = ((idx as usize / 16) % 16) as i32;
                    let ly = (idx as usize / 256) as i32;
                    world.set_block(origin + IVec3::new(lx, ly, lz), block_id);
                }
                chunks_to_rebuild.push(*cpos);
            }

            world.chest_containers.clear();
            for (pos, slots) in chests {
                world.chest_containers.insert(*pos, slots.clone());
            }

            world.furnaces.clear();
            for (pos, state) in furnaces {
                world.furnaces.insert(*pos, state.clone());
            }
        }

        self.item_drops.clear();
        for (drop_id, pos, vel, item) in item_drops {
            self.item_drops.push(ItemDrop::with_id_and_velocity(*drop_id, *pos, *vel, *item));
        }

        chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
        chunks_to_rebuild.dedup();

        let new_meshes = {
            let world = self.world.lock().unwrap();
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
                .collect::<Vec<_>>()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
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

#[cfg(test)]
mod tests {
    use super::*;
    use blockgame_core::item::{Inventory, ItemId, ItemSlot};
    use blockgame_core::player::PlayerStats;
    use blockgame_core::save::{SavedPlayerState, WorldMetadata};

    #[test]
    fn test_world_manager_save_and_restore_state() {
        let registry = Arc::new(BlockRegistry::new());
        let mut wm = WorldManager::new(1337, 2, registry.clone());

        // Modify a block and add a chest & furnace in the world
        let block_pos = IVec3::new(4, 48, 4);
        let chest_pos = IVec3::new(5, 48, 4);
        let crop_pos = IVec3::new(6, 49, 4);
        let farm_pos = IVec3::new(6, 48, 4);
        {
            let mut world = wm.world().lock().unwrap();
            world.insert_chunk(Chunk::new(ChunkPos::new(0, 3, 0)));
            world.set_block(block_pos, BlockId::DIAMOND_ORE);
            world.set_block(chest_pos, BlockId::CHEST);
            let chest = world.get_or_create_chest(chest_pos);
            chest[0] = ItemSlot::new(ItemId::DIAMOND, 12);
            world.set_block(farm_pos, BlockId::FARMLAND_MOIST);
            world.set_block(crop_pos, BlockId::WHEAT_2);
        }
        wm.active_farmland.push((farm_pos, 14.0));
        wm.planted_crops.push((crop_pos, 7.5));
        wm.spawn_item_drop(Vec3::new(4.5, 50.0, 4.5), ItemSlot::new(ItemId::APPLE, 3));

        let meta = WorldMetadata::new("Test World", 1337);
        let player_state = SavedPlayerState {
            position: Vec3::new(4.5, 50.0, 4.5),
            velocity: Vec3::ZERO,
            yaw: 1.2,
            pitch: -0.3,
            stats: PlayerStats::new(),
            inventory: Inventory::starter_inventory(),
            active_hotbar_slot: 2,
            rank: "default".to_string(),
        };

        let save_data = wm.capture_save_data(meta, player_state);
        assert_eq!(save_data.modified_chunks.len(), 1);

        let mut wm_loaded = WorldManager::new(1337, 2, registry);
        wm_loaded.apply_save_data(&save_data);

        {
            let world = wm_loaded.world().lock().unwrap();
            assert_eq!(world.get_block(block_pos), BlockId::DIAMOND_ORE);
            assert_eq!(world.get_block(farm_pos), BlockId::FARMLAND_MOIST);
            assert_eq!(world.get_block(crop_pos), BlockId::WHEAT_2);
            let chest = world.get_chest(chest_pos).expect("Chest should be restored");
            assert_eq!(chest[0], ItemSlot::new(ItemId::DIAMOND, 12));
        }
        assert_eq!(wm_loaded.active_farmland, vec![(farm_pos, 14.0)]);
        assert_eq!(wm_loaded.planted_crops, vec![(crop_pos, 7.5)]);
        assert_eq!(wm_loaded.item_drops.len(), 1);
        assert_eq!(wm_loaded.item_drops[0].item.item_id, ItemId::APPLE);
    }
}

