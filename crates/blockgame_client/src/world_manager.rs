use std::collections::{HashMap, HashSet};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use blockgame_core::{
    animal::{AnimalEntity, AnimalKind},
    block::{
        break_unsupported_attached, can_block_survive, find_decaying_leaves_around_pos,
        get_leaf_decay_drops, is_farmland_hydrated, is_leaf_supported, try_grow_cactus, BlockId,
        BlockRegistry,
    },
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, VERTICAL_CHUNKS},
    direction::Direction,
    entity::{FallingBlock, FallingBlockStatus, ItemDrop, ItemDropStatus},
    item::{ItemId, ItemSlot},
    monster::{can_monster_spawn_at, ArrowProjectile, MonsterEntity, MonsterKind},
    physics::AABB,
    world::{world_to_chunk_coords, World},
};
use blockgame_gen::{Biome, TerrainGenerator};
use blockgame_render::{ChunkMesh, ChunkMesher, GpuMeshBuffers, VoxelPipeline};
use glam::{IVec2, IVec3, Vec3};
use rayon::prelude::*;

/// Meshes a single chunk using loaded neighbor chunks when available and returning `None`
/// for out-of-bounds coordinates in currently unloaded chunks so phantom water chunk-border
/// walls are not emitted before neighbor chunks finish streaming.
fn mesh_chunk_from_world(
    world: &World,
    registry: &BlockRegistry,
    chunk: &Chunk,
) -> ChunkMesh {
    mesh_chunk_from_world_with_options(world, registry, chunk, false)
}

fn mesh_chunk_from_world_with_options(
    world: &World,
    registry: &BlockRegistry,
    chunk: &Chunk,
    fast_leaves: bool,
) -> ChunkMesh {
    if chunk.is_all_air() {
        return ChunkMesh::new();
    }
    let mesher = ChunkMesher::new(registry).with_fast_leaves(fast_leaves);
    let origin = chunk.pos.to_world_origin();
    let sources = world.get_light_sources_near(origin, 15);
    mesher.mesh_chunk_with_lighting_and_meta_opt(
        chunk,
        &sources,
        |lx, ly, lz| {
            let wp = origin + IVec3::new(lx, ly, lz);
            let (ncpos, nlx, nly, nlz) = world_to_chunk_coords(wp);
            if ncpos.0.y < 0 || ncpos.0.y >= VERTICAL_CHUNKS {
                Some(BlockId::AIR)
            } else {
                world
                    .get_chunk(ncpos)
                    .map(|c| c.get_unchecked(nlx, nly, nlz))
            }
        },
        |lx, ly, lz| {
            let wp = origin + IVec3::new(lx, ly, lz);
            let (ncpos, nlx, nly, nlz) = world_to_chunk_coords(wp);
            if ncpos.0.y < 0 || ncpos.0.y >= VERTICAL_CHUNKS {
                0
            } else {
                world
                    .get_chunk(ncpos)
                    .map_or(0, |c| c.get_facing_unchecked(nlx, nly, nlz) & 0x0F)
            }
        },
    )
}

/// Meshes a batch of chunks in parallel across all Rayon worker threads without holding
/// the `World` mutex during lighting or mesh construction.
fn mesh_batch_parallel(
    world_arc: &Arc<Mutex<World>>,
    registry: &Arc<BlockRegistry>,
    batch: &[(ChunkPos, u64)],
    fast_leaves: bool,
    tx: &Sender<(ChunkPos, u64, ChunkMesh, Vec<(IVec3, bool)>)>,
) {
    let mut jobs: Vec<(ChunkPos, u64, Vec<(IVec3, u8)>)> = Vec::with_capacity(batch.len());
    let mut snapshot: HashMap<ChunkPos, Chunk> = HashMap::new();

    {
        let world = world_arc.lock().unwrap();
        for &(pos, epoch) in batch {
            let Some(center) = world.get_chunk(pos) else {
                continue;
            };
            if center.is_all_air() {
                let _ = tx.send((pos, epoch, ChunkMesh::new(), Vec::new()));
                continue;
            }
            let origin = pos.to_world_origin();
            let sources = world.get_light_sources_near(origin, 15);
            jobs.push((pos, epoch, sources));

            // Snapshot deduplicated 3x3 neighbor columns (including sky-light columns above)
            let min_y = (pos.0.y - 1).max(0);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    for cy in min_y..VERTICAL_CHUNKS {
                        let npos = ChunkPos::new(pos.0.x + dx, cy, pos.0.z + dz);
                        if !snapshot.contains_key(&npos) {
                            if let Some(c) = world.get_chunk(npos) {
                                snapshot.insert(npos, c.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // World mutex is now dropped: mesh and scan exposed fluids concurrently across all CPU cores!
    jobs.into_par_iter().for_each(|(pos, epoch, sources)| {
        let Some(chunk) = snapshot.get(&pos) else {
            return;
        };
        let mesher = ChunkMesher::new(registry).with_fast_leaves(fast_leaves);
        let origin = pos.to_world_origin();
        let mesh = mesher.mesh_chunk_with_lighting_and_meta_opt(
            chunk,
            &sources,
            |lx, ly, lz| {
                let wp = origin + IVec3::new(lx, ly, lz);
                let (ncpos, nlx, nly, nlz) = world_to_chunk_coords(wp);
                if ncpos.0.y < 0 || ncpos.0.y >= VERTICAL_CHUNKS {
                    Some(BlockId::AIR)
                } else {
                    snapshot
                        .get(&ncpos)
                        .map(|c| c.get_unchecked(nlx, nly, nlz))
                }
            },
            |lx, ly, lz| {
                let wp = origin + IVec3::new(lx, ly, lz);
                let (ncpos, nlx, nly, nlz) = world_to_chunk_coords(wp);
                if ncpos.0.y < 0 || ncpos.0.y >= VERTICAL_CHUNKS {
                    0
                } else {
                    snapshot
                        .get(&ncpos)
                        .map_or(0, |c| c.get_facing_unchecked(nlx, nly, nlz) & 0x0F)
                }
            },
        );
        let exposed_fluids = blockgame_core::fluid::find_exposed_fluids_in_chunk(
            chunk,
            snapshot.get(&ChunkPos::new(pos.0.x - 1, pos.0.y, pos.0.z)),
            snapshot.get(&ChunkPos::new(pos.0.x + 1, pos.0.y, pos.0.z)),
            snapshot.get(&ChunkPos::new(pos.0.x, pos.0.y, pos.0.z - 1)),
            snapshot.get(&ChunkPos::new(pos.0.x, pos.0.y, pos.0.z + 1)),
            snapshot.get(&ChunkPos::new(pos.0.x, pos.0.y - 1, pos.0.z)),
            snapshot.get(&ChunkPos::new(pos.0.x, pos.0.y + 1, pos.0.z)),
        );
        let _ = tx.send((pos, epoch, mesh, exposed_fluids));
    });
}

/// Checks whether inserting `new_chunk` adjacent to `neighbor_chunk` (at `new_chunk.pos + dir`)
/// can alter `neighbor_chunk`'s visible boundary faces or water boundaries.
fn chunks_share_visible_boundary(
    new_chunk: &Chunk,
    neighbor_chunk: &Chunk,
    dir: Direction,
) -> bool {
    match dir {
        Direction::PosX | Direction::NegX => {
            let (new_x, nb_x) = if dir == Direction::PosX {
                (CHUNK_SIZE_X - 1, 0)
            } else {
                (0, CHUNK_SIZE_X - 1)
            };
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let new_b = new_chunk.get_unchecked(new_x, y, z);
                    let nb_b = neighbor_chunk.get_unchecked(nb_x, y, z);
                    if (!nb_b.is_air() && !new_b.is_air())
                        || ((nb_b.is_water() || nb_b == BlockId::ICE) && new_b != nb_b)
                    {
                        return true;
                    }
                }
            }
            false
        }
        Direction::PosY | Direction::NegY => {
            let (new_y, nb_y) = if dir == Direction::PosY {
                (CHUNK_SIZE_Y - 1, 0)
            } else {
                (0, CHUNK_SIZE_Y - 1)
            };
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let new_b = new_chunk.get_unchecked(x, new_y, z);
                    let nb_b = neighbor_chunk.get_unchecked(x, nb_y, z);
                    if (!nb_b.is_air() && !new_b.is_air())
                        || ((nb_b.is_water() || nb_b == BlockId::ICE) && new_b != nb_b)
                    {
                        return true;
                    }
                }
            }
            false
        }
        Direction::PosZ | Direction::NegZ => {
            let (new_z, nb_z) = if dir == Direction::PosZ {
                (CHUNK_SIZE_Z - 1, 0)
            } else {
                (0, CHUNK_SIZE_Z - 1)
            };
            for y in 0..CHUNK_SIZE_Y {
                for x in 0..CHUNK_SIZE_X {
                    let new_b = new_chunk.get_unchecked(x, y, new_z);
                    let nb_b = neighbor_chunk.get_unchecked(x, y, nb_z);
                    if (!nb_b.is_air() && !new_b.is_air())
                        || ((nb_b.is_water() || nb_b == BlockId::ICE) && new_b != nb_b)
                    {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Finds already-loaded neighbor chunks touching `newly_inserted` chunks that should remesh
/// to update face culling, water boundary faces, or cross-chunk lighting.
fn collect_affected_neighbors(world: &World, newly_inserted: &[ChunkPos]) -> Vec<ChunkPos> {
    let inserted_set: HashSet<ChunkPos> = newly_inserted.iter().copied().collect();
    let mut affected = HashSet::new();

    for &pos in newly_inserted {
        let Some(new_chunk) = world.get_chunk(pos) else {
            continue;
        };
        let has_near_lights = !world.get_light_sources_near(pos.to_world_origin(), 15).is_empty();

        for dir in Direction::ALL {
            let normal = dir.normal_ivec();
            let n_pos = ChunkPos::new(pos.0.x + normal.x, pos.0.y + normal.y, pos.0.z + normal.z);
            if inserted_set.contains(&n_pos) || affected.contains(&n_pos) {
                continue;
            }
            let Some(neighbor_chunk) = world.get_chunk(n_pos) else {
                continue;
            };
            if has_near_lights || chunks_share_visible_boundary(new_chunk, neighbor_chunk, dir) {
                affected.insert(n_pos);
            }
        }
    }

    affected.into_iter().collect()
}

/// Priority key for chunk streaming & GPU uploads: strongly prefers horizontally closest columns
/// around `player_chunk` while prioritizing the player's current altitude band inside each ring.
#[inline]
fn chunk_streaming_priority(pos: ChunkPos, player_chunk: ChunkPos) -> (i32, i32, i32) {
    let dx = pos.0.x - player_chunk.0.x;
    let dz = pos.0.z - player_chunk.0.z;
    let dy_abs = (pos.0.y - player_chunk.0.y).abs();
    let horiz_sq = dx * dx + dz * dz;
    let vert_penalty = if dy_abs <= 1 {
        0
    } else if dy_abs <= 3 {
        1
    } else {
        4
    };
    (horiz_sq + vert_penalty, horiz_sq, dy_abs)
}

pub struct WorldManager {
    world: Arc<Mutex<World>>,
    pub generator: Arc<TerrainGenerator>,
    registry: Arc<BlockRegistry>,
    render_distance: i32,
    loaded_chunks_set: HashSet<ChunkPos>,
    queued_generation: HashSet<ChunkPos>,
    queued_meshing: HashSet<ChunkPos>,
    dirty_remesh: HashSet<ChunkPos>,
    chunk_mesh_epoch: HashMap<ChunkPos, u64>,
    next_mesh_epoch: u64,
    has_pending_chunks: bool,
    pub gpu_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    pub gpu_cutout_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    pub gpu_foliage_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    pub gpu_water_chunks: HashMap<ChunkPos, GpuMeshBuffers>,
    fast_graphics: bool,
    gpu_chunks_dark_underground: HashSet<ChunkPos>,
    pending_gpu_uploads: HashMap<ChunkPos, ChunkMesh>,
    mesh_rx: Receiver<(ChunkPos, u64, ChunkMesh, Vec<(IVec3, bool)>)>,
    mesh_tx: Sender<(ChunkPos, u64, ChunkMesh, Vec<(IVec3, bool)>)>,
    gen_done_rx: Receiver<Vec<ChunkPos>>,
    gen_done_tx: Sender<Vec<ChunkPos>>,
    last_player_chunk: ChunkPos,
    last_village_scan_chunk: Option<ChunkPos>,
    pub item_drops: Vec<ItemDrop>,
    pub falling_blocks: Vec<FallingBlock>,
    pub animals: Vec<AnimalEntity>,
    pub next_animal_id: u32,
    pub monsters: Vec<MonsterEntity>,
    pub arrows: Vec<ArrowProjectile>,
    pub next_monster_id: u32,
    pub next_arrow_id: u32,
    monster_spawn_timer: f32,
    monster_rng_state: u64,
    pub farming_sim: blockgame_core::FarmingSimulation,
    pub fluid_sim: blockgame_core::FluidSimulation,
    fluid_scanned_chunks: HashSet<ChunkPos>,
    pub is_multiplayer: bool,
    pub growing_cacti: Vec<(IVec3, f32)>,
    pub decaying_leaves: Vec<(IVec3, f32)>,
    pub animal_spawned_columns: HashSet<IVec2>,
    // Biome Boss Temples & Instanced Arenas (Issue #9)
    pub temples: blockgame_core::temple::TempleManager,
    pub mp_temple_monsters: Vec<MonsterEntity>,
    pub mp_temple_arrows: Vec<ArrowProjectile>,
    pub mp_boss_hud: Option<blockgame_core::temple::BossHudState>,
    pub pending_temple_messages: Vec<String>,
    spawned_overworld_temples: HashSet<u64>,
    // NPC Villages & Trading (Issue #11)
    pub villages: blockgame_core::village::VillageManager,
    pub mp_village_npcs: Vec<blockgame_core::SyncedVillageNpcState>,
    announced_mp_villages: HashSet<u64>,
    pub pending_village_blueprints: Vec<blockgame_core::VillageSpawnBlueprint>,
    pub pending_mp_npc_arrow_hits: Vec<(u32, f32, Vec3)>,
    pub pending_arrow_killed_monsters: Vec<blockgame_core::MonsterKind>,
}

impl WorldManager {
    pub fn new(seed: u32, render_distance: i32, registry: Arc<BlockRegistry>) -> Self {
        let (mesh_tx, mesh_rx) = mpsc::channel();
        let (gen_done_tx, gen_done_rx) = mpsc::channel();
        let mut monster_rng_state = (seed as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ 0x853c49e6748fea9b;
        if monster_rng_state == 0 {
            monster_rng_state = 0x853c49e6748fea9b;
        }

        Self {
            world: Arc::new(Mutex::new(World::new())),
            generator: Arc::new(TerrainGenerator::new(seed)),
            registry,
            render_distance,
            loaded_chunks_set: HashSet::new(),
            queued_generation: HashSet::new(),
            queued_meshing: HashSet::new(),
            dirty_remesh: HashSet::new(),
            chunk_mesh_epoch: HashMap::new(),
            next_mesh_epoch: 1,
            has_pending_chunks: true,
            gpu_chunks: HashMap::new(),
            gpu_cutout_chunks: HashMap::new(),
            gpu_foliage_chunks: HashMap::new(),
            gpu_water_chunks: HashMap::new(),
            fast_graphics: false,
            gpu_chunks_dark_underground: HashSet::new(),
            pending_gpu_uploads: HashMap::new(),
            mesh_rx,
            mesh_tx,
            gen_done_rx,
            gen_done_tx,
            last_player_chunk: ChunkPos::new(i32::MAX, i32::MAX, i32::MAX),
            last_village_scan_chunk: None,
            item_drops: Vec::new(),
            falling_blocks: Vec::new(),
            animals: Vec::new(),
            next_animal_id: 1,
            monsters: Vec::new(),
            arrows: Vec::new(),
            next_monster_id: 1,
            next_arrow_id: 1,
            monster_spawn_timer: 4.0,
            monster_rng_state,
            farming_sim: blockgame_core::FarmingSimulation::new(seed as u64 ^ 0xFA4A_1234),
            fluid_sim: blockgame_core::FluidSimulation::new(),
            fluid_scanned_chunks: HashSet::new(),
            is_multiplayer: false,
            growing_cacti: Vec::new(),
            decaying_leaves: Vec::new(),
            animal_spawned_columns: HashSet::new(),
            temples: blockgame_core::temple::TempleManager::new(),
            mp_temple_monsters: Vec::new(),
            mp_temple_arrows: Vec::new(),
            mp_boss_hud: None,
            pending_temple_messages: Vec::new(),
            spawned_overworld_temples: HashSet::new(),
            villages: blockgame_core::village::VillageManager::new(),
            mp_village_npcs: Vec::new(),
            announced_mp_villages: HashSet::new(),
            pending_village_blueprints: Vec::new(),
            pending_mp_npc_arrow_hits: Vec::new(),
            pending_arrow_killed_monsters: Vec::new(),
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
            if pos.0.y <= 1 && !mesh.vertices.iter().any(|v| v.light[0] > 0.05) {
                self.gpu_chunks_dark_underground.insert(pos);
            } else {
                self.gpu_chunks_dark_underground.remove(&pos);
            }
        } else {
            self.gpu_chunks.remove(&pos);
            self.gpu_chunks_dark_underground.remove(&pos);
        }
        if let Some(gpu_cutout) = pipeline.upload_cutout_mesh(device, mesh) {
            self.gpu_cutout_chunks.insert(pos, gpu_cutout);
        } else {
            self.gpu_cutout_chunks.remove(&pos);
        }
        if let Some(gpu_foliage) = pipeline.upload_foliage_mesh(device, mesh) {
            self.gpu_foliage_chunks.insert(pos, gpu_foliage);
        } else {
            self.gpu_foliage_chunks.remove(&pos);
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
    /// Updates the active chunk render distance (`2..=12`).
    pub fn set_render_distance(&mut self, render_distance: i32) {
        self.render_distance = render_distance.clamp(2, 12);
        self.has_pending_chunks = true;
    }

    /// Updates `Fast` vs. `Fancy` Graphics Quality mode and schedules background remeshing of
    /// loaded cutout chunks when leaf-to-leaf culling changes.
    pub fn set_fast_graphics(&mut self, fast: bool) {
        if self.fast_graphics == fast {
            return;
        }
        self.fast_graphics = fast;
        for &pos in self.gpu_cutout_chunks.keys() {
            self.dirty_remesh.insert(pos);
        }
        if !self.dirty_remesh.is_empty() {
            self.has_pending_chunks = true;
        }
    }

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
        if !self.is_multiplayer {
            for &pos in &area_chunks {
                if self.fluid_scanned_chunks.insert(pos) {
                    self.fluid_sim.schedule_exposed_fluids_in_chunk(pos, &world_guard);
                }
            }
        }
        let meshes: Vec<(ChunkPos, ChunkMesh)> = area_chunks
            .par_iter()
            .filter_map(|&pos| {
                world_guard
                    .get_chunk(pos)
                    .map(|chunk| {
                        (
                            pos,
                            mesh_chunk_from_world_with_options(
                                &world_guard,
                                &self.registry,
                                chunk,
                                self.fast_graphics,
                            ),
                        )
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
        self.has_pending_chunks = false;

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
        self.farming_sim = blockgame_core::FarmingSimulation::from_save_data(data);
        self.growing_cacti = data.growing_cacti.clone();
        self.decaying_leaves = data.decaying_leaves.clone();
        self.animal_spawned_columns = data.animal_spawned_columns.clone();
        self.temples = data.temples.clone();
        self.villages = data.villages.clone();
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
        self.farming_sim.export_to_save_data(&mut data);
        data.growing_cacti = self.growing_cacti.clone();
        data.decaying_leaves = self.decaying_leaves.clone();
        data.animal_spawned_columns = self.animal_spawned_columns.clone();
        data.temples = self.temples.clone();
        data.villages = self.villages.clone();
        data
    }

    /// Evaluates biome-aware animal spawning in a 16x16 chunk column (cx, cz).
    pub fn try_spawn_animals_in_column(&mut self, cx: i32, cz: i32, world: &World) {
        if self.is_multiplayer {
            return;
        }
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
        // - River: 20% spawn chance (riverbank chickens/pigs)
        let (spawn_chance, kind_weights, min_herd, max_herd) = match biome {
            Biome::Desert
            | Biome::RedDesert
            | Biome::Glacier
            | Biome::Ocean
            | Biome::Badlands
            | Biome::CoralReef => return,
            Biome::Plains => (0.65, [(AnimalKind::Cow, 0.30), (AnimalKind::Pig, 0.25), (AnimalKind::Chicken, 0.15), (AnimalKind::Sheep, 0.30)], 2, 4),
            Biome::FlowerMeadow => (0.70, [(AnimalKind::Sheep, 0.35), (AnimalKind::Cow, 0.30), (AnimalKind::Pig, 0.20), (AnimalKind::Chicken, 0.15)], 2, 4),
            Biome::Forest => (0.50, [(AnimalKind::Pig, 0.35), (AnimalKind::Chicken, 0.25), (AnimalKind::Cow, 0.15), (AnimalKind::Sheep, 0.25)], 2, 3),
            Biome::BirchForest => (0.55, [(AnimalKind::Pig, 0.30), (AnimalKind::Sheep, 0.30), (AnimalKind::Chicken, 0.25), (AnimalKind::Cow, 0.15)], 2, 4),
            Biome::DarkForest => (0.50, [(AnimalKind::Pig, 0.35), (AnimalKind::Chicken, 0.25), (AnimalKind::Cow, 0.15), (AnimalKind::Sheep, 0.25)], 2, 4),
            Biome::Mountains => (0.25, [(AnimalKind::Sheep, 0.45), (AnimalKind::Chicken, 0.25), (AnimalKind::Cow, 0.20), (AnimalKind::Pig, 0.10)], 1, 3),
            Biome::Savanna => (0.60, [(AnimalKind::Cow, 0.45), (AnimalKind::Sheep, 0.25), (AnimalKind::Pig, 0.15), (AnimalKind::Chicken, 0.15)], 2, 4),
            Biome::Swamp => (0.55, [(AnimalKind::Pig, 0.55), (AnimalKind::Chicken, 0.25), (AnimalKind::Cow, 0.10), (AnimalKind::Sheep, 0.10)], 2, 4),
            Biome::SnowyTundra => (0.25, [(AnimalKind::Sheep, 0.40), (AnimalKind::Chicken, 0.30), (AnimalKind::Cow, 0.20), (AnimalKind::Pig, 0.10)], 1, 3),
            Biome::Jungle => (0.65, [(AnimalKind::Chicken, 0.45), (AnimalKind::Pig, 0.30), (AnimalKind::Cow, 0.15), (AnimalKind::Sheep, 0.10)], 2, 5),
            Biome::Beach => (0.30, [(AnimalKind::Chicken, 0.65), (AnimalKind::Pig, 0.15), (AnimalKind::Cow, 0.10), (AnimalKind::Sheep, 0.10)], 1, 3),
            Biome::StonyShore => (0.15, [(AnimalKind::Chicken, 0.70), (AnimalKind::Sheep, 0.30), (AnimalKind::Pig, 0.0), (AnimalKind::Cow, 0.0)], 1, 2),
            Biome::Taiga => (0.55, [(AnimalKind::Cow, 0.30), (AnimalKind::Sheep, 0.30), (AnimalKind::Pig, 0.20), (AnimalKind::Chicken, 0.20)], 2, 4),
            Biome::River => (0.20, [(AnimalKind::Chicken, 0.55), (AnimalKind::Pig, 0.25), (AnimalKind::Cow, 0.10), (AnimalKind::Sheep, 0.10)], 1, 2),
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
                if b == BlockId::GRASS
                    || b == BlockId::DIRT
                    || b == BlockId::SNOW_BLOCK
                    || b == BlockId::CLAY
                    || b == BlockId::SAND
                    || b == BlockId::GRAVEL
                    || b == BlockId::PODZOL
                    || b == BlockId::MYCELIUM
                {
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

    /// Removes GPU buffers and streaming state for chunks beyond `render_distance + 2`.
    fn unload_distant_chunks(&mut self, current_chunk: ChunkPos) {
        let unload_radius_sq = (self.render_distance + 2) * (self.render_distance + 2);
        let pcx = current_chunk.0.x;
        let pcz = current_chunk.0.z;

        let mut to_remove = HashSet::new();
        for &pos in self
            .loaded_chunks_set
            .iter()
            .chain(self.gpu_chunks.keys())
            .chain(self.gpu_cutout_chunks.keys())
            .chain(self.gpu_foliage_chunks.keys())
            .chain(self.gpu_water_chunks.keys())
            .chain(self.pending_gpu_uploads.keys())
            .chain(self.dirty_remesh.iter())
        {
            let dx = pos.0.x - pcx;
            let dz = pos.0.z - pcz;
            if dx * dx + dz * dz > unload_radius_sq {
                to_remove.insert(pos);
            }
        }

        for pos in to_remove {
            self.gpu_chunks.remove(&pos);
            self.gpu_cutout_chunks.remove(&pos);
            self.gpu_foliage_chunks.remove(&pos);
            self.gpu_water_chunks.remove(&pos);
            self.gpu_chunks_dark_underground.remove(&pos);
            self.pending_gpu_uploads.remove(&pos);
            self.loaded_chunks_set.remove(&pos);
            self.fluid_scanned_chunks.remove(&pos);
            self.queued_meshing.remove(&pos);
            self.queued_generation.remove(&pos);
            self.dirty_remesh.remove(&pos);
            self.chunk_mesh_epoch.remove(&pos);
        }
    }

    /// Collects and prioritizes missing or dirty-neighbor chunks within render distance using a unified
    /// player-proximity queue so nearby chunks always load before distant chunks regardless of whether
    /// they need terrain generation or only meshing.
    fn collect_streaming_batch(
        &mut self,
        current_chunk: ChunkPos,
        max_in_flight: usize,
    ) -> (Vec<(ChunkPos, u64)>, Vec<(ChunkPos, u64)>) {
        let pcx = current_chunk.0.x;
        let pcz = current_chunk.0.z;
        let r = self.render_distance;
        let max_radius_sq = (r + 1) * (r + 1);

        let in_flight = self.queued_generation.len() + self.queued_meshing.len();
        let remaining_capacity = max_in_flight.saturating_sub(in_flight);
        if remaining_capacity == 0 {
            return (Vec::new(), Vec::new());
        }

        let mut candidates: Vec<(ChunkPos, bool)> = Vec::new();
        let mut seen = HashSet::new();
        let mut stale_dirty = Vec::new();

        {
            let world_guard = self.world.lock().unwrap();
            for cy in 0..VERTICAL_CHUNKS {
                for dz in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dz * dz <= max_radius_sq {
                            let pos = ChunkPos::new(pcx + dx, cy, pcz + dz);
                            if !self.loaded_chunks_set.contains(&pos)
                                && !self.queued_meshing.contains(&pos)
                                && !self.queued_generation.contains(&pos)
                            {
                                let needs_gen = !world_guard.has_chunk(pos);
                                candidates.push((pos, needs_gen));
                                seen.insert(pos);
                            }
                        }
                    }
                }
            }

            for &pos in &self.dirty_remesh {
                let dx = pos.0.x - pcx;
                let dz = pos.0.z - pcz;
                if dx * dx + dz * dz > max_radius_sq || !world_guard.has_chunk(pos) {
                    stale_dirty.push(pos);
                    continue;
                }
                if !self.queued_meshing.contains(&pos)
                    && !self.queued_generation.contains(&pos)
                    && seen.insert(pos)
                {
                    candidates.push((pos, false));
                }
            }
        }

        for pos in stale_dirty {
            self.dirty_remesh.remove(&pos);
        }

        candidates.sort_by_key(|&(pos, needs_gen)| {
            (chunk_streaming_priority(pos, current_chunk), needs_gen)
        });

        let total_candidates = candidates.len();
        let mut batch_gen = Vec::new();
        let mut batch_mesh = Vec::new();

        for (pos, needs_gen) in candidates.into_iter().take(remaining_capacity) {
            self.dirty_remesh.remove(&pos);
            let epoch = self.next_mesh_epoch;
            self.next_mesh_epoch += 1;
            self.chunk_mesh_epoch.insert(pos, epoch);
            if needs_gen {
                self.queued_generation.insert(pos);
                batch_gen.push((pos, epoch));
            } else {
                self.queued_meshing.insert(pos);
                batch_mesh.push((pos, epoch));
            }
        }

        self.has_pending_chunks = total_candidates > remaining_capacity;
        (batch_gen, batch_mesh)
    }

    /// Periodic update called every frame: streams chunks and uploads completed background meshes.
    pub fn update(&mut self, player_pos: Vec3, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        // 1. Identify player's current chunk coordinates and unload far chunks when moving across borders
        let (current_chunk, _, _, _) = world_to_chunk_coords(IVec3::new(
            player_pos.x.floor() as i32,
            player_pos.y.floor() as i32,
            player_pos.z.floor() as i32,
        ));

        if current_chunk != self.last_player_chunk {
            self.last_player_chunk = current_chunk;
            self.has_pending_chunks = true;
            self.unload_distant_chunks(current_chunk);
        }

        // 2. Queue neighbor remeshes triggered by newly generated chunks touching loaded chunks
        while let Ok(neighbors) = self.gen_done_rx.try_recv() {
            for n_pos in neighbors {
                if self.loaded_chunks_set.contains(&n_pos)
                    || self.queued_meshing.contains(&n_pos)
                    || self.queued_generation.contains(&n_pos)
                    || self.pending_gpu_uploads.contains_key(&n_pos)
                {
                    self.dirty_remesh.insert(n_pos);
                    self.has_pending_chunks = true;
                }
            }
        }

        // 3. Drain completed background mesh results without letting empty chunks consume GPU upload budget
        let unload_radius_sq = (self.render_distance + 2) * (self.render_distance + 2);
        let mut completed_surface_chunks = Vec::new();
        while let Ok((pos, epoch, mesh, exposed_fluids)) = self.mesh_rx.try_recv() {
            let latest_epoch = self.chunk_mesh_epoch.get(&pos).copied().unwrap_or(0);
            if epoch < latest_epoch {
                continue;
            }
            self.queued_meshing.remove(&pos);
            self.queued_generation.remove(&pos);

            let dx = pos.0.x - current_chunk.0.x;
            let dz = pos.0.z - current_chunk.0.z;
            if dx * dx + dz * dz > unload_radius_sq {
                continue;
            }

            self.loaded_chunks_set.insert(pos);
            if !self.is_multiplayer && self.fluid_scanned_chunks.insert(pos) && !exposed_fluids.is_empty() {
                self.fluid_sim.enqueue_exposed(&exposed_fluids);
            }
            if pos.0.y == 0 {
                completed_surface_chunks.push((pos.0.x, pos.0.z));
            }

            if mesh.is_empty() {
                self.pending_gpu_uploads.remove(&pos);
                self.store_mesh(pos, &mesh, device, pipeline);
            } else {
                self.pending_gpu_uploads.insert(pos, mesh);
            }
        }

        // Upload closest non-empty meshes first within a per-frame GPU buffer budget
        const MAX_GPU_UPLOADS_PER_FRAME: usize = 16;
        if !self.pending_gpu_uploads.is_empty() {
            let mut ready_positions: Vec<ChunkPos> =
                self.pending_gpu_uploads.keys().copied().collect();
            ready_positions.sort_by_key(|&pos| chunk_streaming_priority(pos, current_chunk));
            for pos in ready_positions.into_iter().take(MAX_GPU_UPLOADS_PER_FRAME) {
                if let Some(mesh) = self.pending_gpu_uploads.remove(&pos) {
                    self.store_mesh(pos, &mesh, device, pipeline);
                }
            }
        }

        if !completed_surface_chunks.is_empty() {
            let world_arc = self.world.clone();
            let world_guard = world_arc.lock().unwrap();
            for (cx, cz) in completed_surface_chunks {
                self.try_spawn_animals_in_column(cx, cz, &world_guard);
            }
        }

        // 4. Scan render distance when player crosses a chunk boundary or there are pending/dirty chunks.
        // Scale in-flight generation + meshing with available CPU cores for snappy multithreaded streaming.
        let max_in_flight = (rayon::current_num_threads() * 8).clamp(48, 128);
        let in_flight = self.queued_generation.len() + self.queued_meshing.len();

        if (self.has_pending_chunks || !self.dirty_remesh.is_empty()) && in_flight < max_in_flight {
            let (batch_gen, batch_mesh) =
                self.collect_streaming_batch(current_chunk, max_in_flight);

            // 5. Dispatch terrain generation + parallel lock-free meshing asynchronously
            let fast_leaves = self.fast_graphics;
            if !batch_gen.is_empty() {
                let generator = self.generator.clone();
                let world_arc = self.world.clone();
                let registry = self.registry.clone();
                let tx = self.mesh_tx.clone();
                let gen_tx = self.gen_done_tx.clone();

                rayon::spawn(move || {
                    let positions: Vec<ChunkPos> = batch_gen.iter().map(|&(pos, _)| pos).collect();
                    let new_chunks: Vec<Chunk> = positions
                        .par_iter()
                        .map(|&pos| {
                            let mut chunk = generator.generate_chunk(pos);
                            generator.populate_chunk_features(&mut chunk);
                            chunk
                        })
                        .collect();

                    // Insert generated chunks in a quick, isolated lock scope and notify affected neighbors
                    let affected_neighbors = {
                        let mut world = world_arc.lock().unwrap();
                        for chunk in new_chunks {
                            if !world.has_chunk(chunk.pos) {
                                world.insert_chunk(chunk);
                            }
                        }
                        collect_affected_neighbors(&world, &positions)
                    };

                    if !affected_neighbors.is_empty() {
                        let _ = gen_tx.send(affected_neighbors);
                    }

                    mesh_batch_parallel(&world_arc, &registry, &batch_gen, fast_leaves, &tx);
                });
            }

            // 6. Dispatch parallel lock-free meshing for already-generated chunks needing GPU buffers or remeshes
            if !batch_mesh.is_empty() {
                let world_arc = self.world.clone();
                let registry = self.registry.clone();
                let tx = self.mesh_tx.clone();

                rayon::spawn(move || {
                    mesh_batch_parallel(&world_arc, &registry, &batch_mesh, fast_leaves, &tx);
                });
            }
        }
    }

    /// Renders loaded GPU chunk meshes with optional camera view frustum culling.
    pub fn render_with_frustum<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
    ) {
        self.render_with_frustum_and_camera(render_pass, frustum, None);
    }

    /// Renders loaded GPU opaque chunk meshes front-to-back with frustum culling and
    /// surface-to-underground occlusion culling so hardware Early-Z rejects occluded terrain.
    pub fn render_with_frustum_and_camera<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
        camera_pos: Option<Vec3>,
    ) {
        let cull_underground = camera_pos.filter(|cam| cam.y >= 34.0);
        let mut visible: Vec<(f32, &'a GpuMeshBuffers)> =
            Vec::with_capacity(self.gpu_chunks.len());
        for (&pos, gpu_mesh) in &self.gpu_chunks {
            if gpu_mesh.index_count == 0 {
                continue;
            }

            let min = Vec3::new(
                pos.0.x as f32 * 16.0,
                pos.0.y as f32 * 16.0,
                pos.0.z as f32 * 16.0,
            );
            if let Some(cam) = cull_underground {
                if self.gpu_chunks_dark_underground.contains(&pos) {
                    let dx = (min.x + 8.0) - cam.x;
                    let dz = (min.z + 8.0) - cam.z;
                    if dx * dx + dz * dz > 48.0 * 48.0 {
                        continue;
                    }
                }
            }

            if let Some(f) = frustum {
                let max = min + Vec3::splat(16.0);
                if !f.is_box_visible(min, max) {
                    continue;
                }
            }

            let dist_sq = if let Some(cam) = camera_pos {
                (min + Vec3::splat(8.0)).distance_squared(cam)
            } else {
                0.0
            };
            visible.push((dist_sq, gpu_mesh));
        }

        if camera_pos.is_some() {
            // Front-to-back (nearest chunk first) to maximize hardware Early-Z rejection
            visible.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        for (_, gpu_mesh) in visible {
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
        }
    }

    /// Renders alpha-cutout chunk meshes (leaves, glass, lily pads, dripstone, glow crystals,
    /// seagrass) and ground foliage cross-quads (flowers, tall grass, crops, sugar cane)
    /// front-to-back. In `Fast` Graphics Quality mode, ground foliage chunks beyond 48 blocks
    /// horizontally are skipped immediately without remeshing.
    pub fn render_cutout_with_frustum_and_camera<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
        camera_pos: Option<Vec3>,
    ) {
        const FAST_FOLIAGE_MAX_DIST_SQ: f32 = 48.0 * 48.0;
        let cull_underground = camera_pos.filter(|cam| cam.y >= 34.0);
        let mut visible: Vec<(f32, &'a GpuMeshBuffers)> =
            Vec::with_capacity(self.gpu_cutout_chunks.len() + self.gpu_foliage_chunks.len());

        for (&pos, gpu_mesh) in &self.gpu_cutout_chunks {
            if gpu_mesh.index_count == 0 {
                continue;
            }
            let min = Vec3::new(
                pos.0.x as f32 * 16.0,
                pos.0.y as f32 * 16.0,
                pos.0.z as f32 * 16.0,
            );
            if let Some(cam) = cull_underground {
                if self.gpu_chunks_dark_underground.contains(&pos) {
                    let dx = (min.x + 8.0) - cam.x;
                    let dz = (min.z + 8.0) - cam.z;
                    if dx * dx + dz * dz > 48.0 * 48.0 {
                        continue;
                    }
                }
            }
            if let Some(f) = frustum {
                if !f.is_box_visible(min, min + Vec3::splat(16.0)) {
                    continue;
                }
            }
            let dist_sq = camera_pos
                .map(|cam| (min + Vec3::splat(8.0)).distance_squared(cam))
                .unwrap_or(0.0);
            visible.push((dist_sq, gpu_mesh));
        }

        for (&pos, gpu_mesh) in &self.gpu_foliage_chunks {
            if gpu_mesh.index_count == 0 {
                continue;
            }
            let min = Vec3::new(
                pos.0.x as f32 * 16.0,
                pos.0.y as f32 * 16.0,
                pos.0.z as f32 * 16.0,
            );
            if let Some(cam) = camera_pos {
                if self.fast_graphics {
                    let dx = (min.x + 8.0) - cam.x;
                    let dz = (min.z + 8.0) - cam.z;
                    if dx * dx + dz * dz > FAST_FOLIAGE_MAX_DIST_SQ {
                        continue;
                    }
                }
            }
            if let Some(f) = frustum {
                if !f.is_box_visible(min, min + Vec3::splat(16.0)) {
                    continue;
                }
            }
            let dist_sq = camera_pos
                .map(|cam| (min + Vec3::splat(8.0)).distance_squared(cam))
                .unwrap_or(0.0);
            visible.push((dist_sq, gpu_mesh));
        }

        if camera_pos.is_some() {
            visible.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        for (_, gpu_mesh) in visible {
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

    /// Renders loaded GPU water/ice translucent chunk meshes with optional camera frustum culling
    /// and back-to-front sorting so closer ice always blends cleanly over farther water.
    pub fn render_water_with_frustum<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        frustum: Option<&blockgame_render::Frustum>,
        camera_pos: Option<Vec3>,
    ) {
        let cull_underground = camera_pos.filter(|cam| cam.y >= 34.0);
        let mut visible: Vec<(f32, &'a GpuMeshBuffers)> = Vec::new();
        for (&pos, gpu_mesh) in &self.gpu_water_chunks {
            if gpu_mesh.index_count == 0 {
                continue;
            }

            let min = Vec3::new(
                pos.0.x as f32 * 16.0,
                pos.0.y as f32 * 16.0,
                pos.0.z as f32 * 16.0,
            );
            if let Some(cam) = cull_underground {
                if self.gpu_chunks_dark_underground.contains(&pos) {
                    let dx = (min.x + 8.0) - cam.x;
                    let dz = (min.z + 8.0) - cam.z;
                    if dx * dx + dz * dz > 48.0 * 48.0 {
                        continue;
                    }
                }
            }
            let max = min + Vec3::splat(16.0);
            if let Some(f) = frustum {
                if !f.is_box_visible(min, max) {
                    continue;
                }
            }

            let dist_sq = if let Some(cam) = camera_pos {
                let center = min + Vec3::splat(8.0);
                center.distance_squared(cam)
            } else {
                0.0
            };
            visible.push((dist_sq, gpu_mesh));
        }

        if camera_pos.is_some() {
            // Back-to-front (farthest chunk first) for correct translucent alpha blending
            visible.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        for (_, gpu_mesh) in visible {
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
        }
    }

    /// Renders all currently loaded GPU water chunk meshes without frustum culling.
    pub fn render_water<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.render_water_with_frustum(render_pass, None, None);
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
            let dropped_chest_items = if current.is_chest() {
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
            } else if current == BlockId::COAL_GENERATOR {
                if let Some(g) = world.remove_coal_generator(pos) {
                    if !g.fuel.is_empty() {
                        vec![g.fuel]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            } else if current.is_battery_box() {
                world.remove_battery(pos);
                Vec::new()
            } else if current == BlockId::ELECTRIC_FURNACE {
                if let Some(ef) = world.remove_electric_furnace(pos) {
                    let mut items = Vec::new();
                    if !ef.input.is_empty() {
                        items.push(ef.input);
                    }
                    if !ef.output.is_empty() {
                        items.push(ef.output);
                    }
                    items
                } else {
                    Vec::new()
                }
            } else if current.is_cooking_pot() {
                if let Some(pot) = world.remove_cooking_pot(pos) {
                    let mut items = Vec::new();
                    for slot in pot.ingredients {
                        if !slot.is_empty() {
                            items.push(slot);
                        }
                    }
                    if !pot.fuel.is_empty() {
                        items.push(pot.fuel);
                    }
                    if !pot.output.is_empty() {
                        items.push(pot.output);
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
            let facing_before_break = world.get_block_facing(pos);
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

            // If breaking one half of a 2-block-long bed, also remove the other half cleanly
            if current.is_bed() {
                let offset = BlockId::bed_head_offset(facing_before_break);
                let pair_pos = if current.is_bed_head() {
                    pos - offset
                } else {
                    pos + offset
                };
                if world.get_block(pair_pos).is_bed() {
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

            // Schedule nearby Water/Lava to flow dynamically into the newly opened space
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);

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

            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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

        // Clean up planted crops if a crop was removed
        {
            let world = self.world.lock().unwrap();
            self.farming_sim
                .planted_crops
                .retain(|c| world.get_block(c.pos).is_farmland_crop());
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

    /// Woodcutting Lv 250+ Tree Feller: fells up to `max_logs` connected log blocks above/adjacent to `root_pos`.
    pub fn fell_connected_logs(
        &mut self,
        root_pos: IVec3,
        max_logs: usize,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> Vec<(IVec3, BlockId)> {
        use std::collections::{HashSet, VecDeque};
        if max_logs == 0 {
            return Vec::new();
        }

        let mut felled = Vec::new();
        let mut chunks_to_rebuild = Vec::new();
        let new_meshes: Vec<(ChunkPos, ChunkMesh)> = {
            let mut world = self.world.lock().unwrap();
            let mut visited: HashSet<IVec3> = HashSet::new();
            let mut queue: VecDeque<IVec3> = VecDeque::new();
            visited.insert(root_pos);
            queue.push_back(root_pos);

            while let Some(cur) = queue.pop_front() {
                if felled.len() >= max_logs {
                    break;
                }
                for dy in 0..=1 {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            if dx == 0 && dy == 0 && dz == 0 {
                                continue;
                            }
                            let npos = cur + IVec3::new(dx, dy, dz);
                            if !visited.insert(npos) {
                                continue;
                            }
                            let bid = world.get_block(npos);
                            if bid.is_log() {
                                world.set_block(npos, BlockId::AIR);
                                Self::add_chunk_and_neighbors(npos, &mut chunks_to_rebuild);
                                felled.push((npos, bid));
                                queue.push_back(npos);
                                if felled.len() >= max_logs {
                                    break;
                                }
                            }
                        }
                        if felled.len() >= max_logs {
                            break;
                        }
                    }
                    if felled.len() >= max_logs {
                        break;
                    }
                }
            }

            if felled.is_empty() {
                return felled;
            }

            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
                })
                .collect()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }

        if let Some(&(last_pos, _)) = felled.last() {
            let world = self.world.lock().unwrap();
            let decaying = find_decaying_leaves_around_pos(last_pos, 5, |p| world.get_block(p));
            for (i, lp) in decaying.into_iter().enumerate() {
                if !self.decaying_leaves.iter().any(|(p, _)| *p == lp) {
                    let delay = 0.1 + (i as f32 * 0.05);
                    self.decaying_leaves.push((lp, delay));
                }
            }
        }

        felled
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
        self.place_block_with_facing(pos, block_id, 0, player_aabb, device, pipeline)
    }

    pub fn place_block_with_facing(
        &mut self,
        pos: IVec3,
        block_id: BlockId,
        facing: u8,
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
            } else if current == BlockId::WATER || current == BlockId::LAVA {
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

            // 2-block-long Bed requires empty space at head_pos (and no player collision)
            if block_id == BlockId::BED {
                let head_pos = pos + BlockId::bed_head_offset(facing);
                if !world.get_block(head_pos).is_air() || player_aabb.intersects(&AABB::from_block(head_pos)) {
                    return false;
                }
            }

            let is_light_source = block_id.is_emissive() || world.get_block(pos).is_emissive();
            world.set_block(pos, block_id);
            world.set_block_facing(pos, facing);
            if block_id == BlockId::OAK_DOOR_CLOSED {
                let top_pos = pos + IVec3::Y;
                world.set_block(top_pos, BlockId::OAK_DOOR_TOP_CLOSED);
                world.set_block_facing(top_pos, facing);
                Self::add_chunk_and_neighbors(top_pos, &mut chunks_to_rebuild);
            }
            if block_id == BlockId::BED {
                let head_pos = pos + BlockId::bed_head_offset(facing);
                world.set_block(head_pos, BlockId::BED_HEAD);
                world.set_block_facing(head_pos, facing);
                Self::add_chunk_and_neighbors(head_pos, &mut chunks_to_rebuild);
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
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);

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

            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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
        } else if block_id == BlockId::SUGAR_CANE {
            self.farming_sim.track_sugar_cane(pos);
        }

        true
    }

    /// Registers a farmland block for moisture and drying updates.
    pub fn register_farmland(&mut self, pos: IVec3) {
        self.farming_sim.track_farmland(pos);
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
            if !current.is_water() || world.get_fluid_level(pos) != blockgame_core::FLUID_SOURCE {
                return false;
            }

            world.set_block(pos, BlockId::AIR);
            world.set_fluid_level(pos, 0);
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);

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

            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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

    /// Places a Water Source Block at `pos` from a water bucket and schedules cellular fluid simulation.
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
            let is_flowing_water =
                current.is_water() && world.get_fluid_level(pos) != blockgame_core::FLUID_SOURCE;
            if !current.is_air() && !current.is_fragile() && !is_flowing_water {
                return false;
            }

            let mut extra_drops = Vec::new();
            if current.is_fragile() {
                if let Some(item_id) = self.registry.get(current).and_then(|def| def.drop_item) {
                    extra_drops.push((
                        pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                        ItemSlot::new(item_id, 1),
                    ));
                }
            }

            world.set_block(pos, BlockId::WATER);
            world.set_fluid_level(pos, blockgame_core::FLUID_SOURCE);
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);

            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();

            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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

    fn add_light_neighborhood(pos: IVec3, chunks: &mut Vec<ChunkPos>) {
        Self::add_chunk_and_neighbors(pos, chunks);
        let (chunk_pos, _, _, _) = world_to_chunk_coords(pos);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.push(ChunkPos::new(
                        chunk_pos.0.x + dx,
                        chunk_pos.0.y + dy,
                        chunk_pos.0.z + dz,
                    ));
                }
            }
        }
    }

    fn rebuild_chunks_and_store(
        &mut self,
        mut chunks_to_rebuild: Vec<ChunkPos>,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
        chunks_to_rebuild.dedup();

        let new_meshes: Vec<(ChunkPos, ChunkMesh)> = {
            let world = self.world.lock().unwrap();
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
                })
                .collect()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

    /// Scoops a Lava Source Block at `pos` with an empty bucket, replacing it with AIR and scheduling neighbor draining.
    pub fn scoop_lava(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        let mut chunks_to_rebuild = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            if !world.get_block(pos).is_lava()
                || world.get_fluid_level(pos) != blockgame_core::FLUID_SOURCE
            {
                return false;
            }
            world.set_block(pos, BlockId::AIR);
            world.set_fluid_level(pos, 0);
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);
        }
        Self::add_light_neighborhood(pos, &mut chunks_to_rebuild);
        self.rebuild_chunks_and_store(chunks_to_rebuild, device, pipeline);
        true
    }

    /// Places a Lava Source Block at `pos` from a lava bucket and schedules cellular fluid simulation.
    pub fn place_lava(
        &mut self,
        pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> bool {
        let mut chunks_to_rebuild = Vec::new();
        {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            let is_flowing_lava =
                current.is_lava() && world.get_fluid_level(pos) != blockgame_core::FLUID_SOURCE;
            if !current.is_air() && !current.is_fragile() && !is_flowing_lava {
                return false;
            }
            world.set_block(pos, BlockId::LAVA);
            world.set_fluid_level(pos, blockgame_core::FLUID_SOURCE);
            self.fluid_sim.schedule_block_and_neighbors(pos, &world);
        }
        Self::add_light_neighborhood(pos, &mut chunks_to_rebuild);
        self.rebuild_chunks_and_store(chunks_to_rebuild, device, pipeline);
        true
    }

    /// Advances cellular Water and Lava flow ticks in singleplayer, rebuilding affected chunk meshes and spawning washed drops.
    pub fn tick_fluids(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if self.is_multiplayer || !self.fluid_sim.has_pending() {
            return;
        }
        let had_lava_pending = self.fluid_sim.has_pending_lava();
        let mut chunks_to_rebuild = Vec::new();
        let (modified, washed_drops) = {
            let mut world = self.world.lock().unwrap();
            let (modified, drops) = self.fluid_sim.tick(dt, &mut world, &self.registry);
            for &mpos in &modified {
                let b = world.get_block(mpos);
                if b.is_lava()
                    || b == BlockId::OBSIDIAN
                    || b == BlockId::COBBLESTONE
                    || b == BlockId::STONE
                    || (had_lava_pending && b.is_air())
                {
                    Self::add_light_neighborhood(mpos, &mut chunks_to_rebuild);
                } else {
                    Self::add_chunk_and_neighbors(mpos, &mut chunks_to_rebuild);
                }
            }
            (modified, drops)
        };
        if !modified.is_empty() {
            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();
            for cpos in chunks_to_rebuild {
                if self.loaded_chunks_set.contains(&cpos) {
                    self.dirty_remesh.insert(cpos);
                }
            }
        }
        let _ = (device, pipeline);
        for (dpos, item) in washed_drops {
            if !item.is_empty() {
                self.spawn_item_drop(dpos.as_vec3() + Vec3::new(0.5, 0.5, 0.5), item);
            }
        }
    }

    /// Performs raycast attack against closest animal within reach.
    /// Returns `(animal_id, kind, pos)` for audio and multiplayer `AttackAnimal` sync.
    pub fn attack_animal(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        reach: f32,
        damage: f32,
    ) -> Option<(u32, blockgame_core::animal::AnimalKind, Vec3)> {
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
            if !self.is_multiplayer {
                self.animals[idx].take_damage(damage, knockback);
            }
            Some((
                self.animals[idx].id,
                self.animals[idx].kind,
                self.animals[idx].position,
            ))
        } else {
            None
        }
    }

    #[inline]
    fn next_monster_rng_f32(&mut self) -> f32 {
        self.monster_rng_state ^= self.monster_rng_state << 13;
        self.monster_rng_state ^= self.monster_rng_state >> 7;
        self.monster_rng_state ^= self.monster_rng_state << 17;
        if self.monster_rng_state == 0 {
            self.monster_rng_state = 0x853c49e6748fea9b;
        }
        ((self.monster_rng_state >> 32) as u32 as f32) / (u32::MAX as f32)
    }

    /// Performs raycast attack against closest monster within reach.
    /// Returns `(kind, position, killed)` on hit.
    pub fn attack_monster(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        reach: f32,
        damage: f32,
    ) -> Option<(blockgame_core::monster::MonsterKind, Vec3, bool)> {
        let mut closest_dist = reach;
        let mut hit_idx = None;

        for (idx, monster) in self.monsters.iter().enumerate() {
            if monster.dead {
                continue;
            }
            let bbox = monster.aabb();
            if let Some(dist) = bbox.intersect_ray(ray_origin, ray_dir, reach) {
                if dist < closest_dist {
                    closest_dist = dist;
                    hit_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = hit_idx {
            if self.monsters[idx].invuln_timer > 0.0 {
                return None;
            }
            let knockback = ray_dir.normalize_or_zero();
            self.monsters[idx].take_damage(damage, knockback);
            Some((
                self.monsters[idx].kind,
                self.monsters[idx].position,
                self.monsters[idx].dead,
            ))
        } else {
            None
        }
    }

    /// Spawns a player-fired arrow projectile.
    pub fn spawn_player_arrow(&mut self, position: Vec3, velocity: Vec3, damage: f32) {
        let id = self.next_arrow_id;
        self.next_arrow_id = self.next_arrow_id.wrapping_add(1).max(1);
        self.arrows
            .push(ArrowProjectile::new(id, position, velocity, true, damage));
    }

    /// Spawns, updates, and despawns monsters. Returns `(melee_damage_to_player, drops)`.
    pub fn update_monsters(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        sun_intensity: f32,
        is_multiplayer: bool,
    ) -> (f32, Vec<(Vec3, Vec<ItemSlot>)>) {
        let mut total_damage = 0.0;
        let mut drops = Vec::new();

        self.temples.tick_cooldowns(dt);

        const MAX_AMBIENT_MONSTERS: usize = 10;
        const MONSTER_SPAWN_INTERVAL_SECS: f32 = 5.0;
        const MONSTER_SPAWN_ATTEMPTS_PER_WAVE: usize = 2;

        // Suppress normal night-monster spawning inside instanced boss arenas
        let in_arena_band = blockgame_core::temple::is_in_arena_band(player_pos);
        self.monster_spawn_timer -= dt;
        if !in_arena_band && self.monster_spawn_timer <= 0.0 {
            self.monster_spawn_timer = MONSTER_SPAWN_INTERVAL_SECS;
            if self.monsters.len() < MAX_AMBIENT_MONSTERS {
                let mut candidates = Vec::new();
                for _ in 0..MONSTER_SPAWN_ATTEMPTS_PER_WAVE {
                    let angle = self.next_monster_rng_f32() * std::f32::consts::TAU;
                    let dist = 16.0 + self.next_monster_rng_f32() * 22.0;
                    let wx = (player_pos.x + angle.cos() * dist).floor() as i32;
                    let wz = (player_pos.z + angle.sin() * dist).floor() as i32;
                    let kind_roll = self.next_monster_rng_f32();
                    let kind = if kind_roll < 0.45 {
                        MonsterKind::Zombie
                    } else if kind_roll < 0.80 {
                        MonsterKind::Skeleton
                    } else {
                        MonsterKind::Spider
                    };
                    candidates.push((wx, wz, kind));
                }

                let world_guard = self.world.lock().unwrap();
                let min_y = (player_pos.y as i32 - 18).max(2);
                let max_y = (player_pos.y as i32 + 16).min(120);
                for (wx, wz, kind) in candidates {
                    if self.monsters.len() >= MAX_AMBIENT_MONSTERS {
                        break;
                    }
                    for wy in (min_y..=max_y).rev() {
                        let pos = IVec3::new(wx, wy, wz);
                        if can_monster_spawn_at(pos, kind, sun_intensity, &world_guard, &self.registry) {
                            let id = self.next_monster_id;
                            self.next_monster_id = self.next_monster_id.wrapping_add(1).max(1);
                            self.monsters.push(MonsterEntity::new(
                                id,
                                kind,
                                Vec3::new(wx as f32 + 0.5, wy as f32, wz as f32 + 0.5),
                            ));
                            break;
                        }
                    }
                }
            }
        }

        let mut spawned_arrows: Vec<(Vec3, Vec3, f32)> = Vec::new();
        let mut newly_summoned: Vec<(MonsterKind, Vec3, u64)> = Vec::new();
        let npc_targets = if !is_multiplayer {
            self.villages.living_npc_targets()
        } else {
            Vec::new()
        };
        let mut npc_damage_events: Vec<(u32, f32, Vec3)> = Vec::new();
        {
            let world_guard = self.world.lock().unwrap();
            for monster in self.monsters.iter_mut() {
                let outcome = monster.update_with_npc_targets(
                    dt,
                    player_pos,
                    &npc_targets,
                    sun_intensity,
                    &world_guard,
                    &self.registry,
                );
                total_damage += outcome.damage_to_player;
                if let Some(ev) = outcome.damage_to_npc {
                    npc_damage_events.push(ev);
                }
                if let Some((apos, avel)) = outcome.spawned_arrow {
                    spawned_arrows.push((apos, avel, MonsterKind::Skeleton.attack_damage()));
                }
                for (apos, avel, dmg) in outcome.extra_arrows {
                    spawned_arrows.push((apos, avel, dmg));
                }
                if let Some((center, radius, aoe_dmg)) = outcome.aoe_blast {
                    if player_pos.distance(center) <= radius {
                        total_damage += aoe_dmg;
                    }
                }
                if let Some(new_phase) = outcome.phase_changed {
                    self.pending_temple_messages.push(
                        blockgame_core::temple::phase_barrier_message(
                            monster.kind.name(),
                            new_phase,
                        ),
                    );
                }
                if outcome.barrier_broken {
                    self.pending_temple_messages.push(
                        blockgame_core::temple::barrier_shatter_message(monster.kind.name()),
                    );
                }
                for (mkind, spos) in outcome.summon_minions {
                    if let Some(tid) = monster.temple_id {
                        newly_summoned.push((mkind, spos, tid));
                    }
                }
            }
        }

        for (npc_id, dmg, kb) in npc_damage_events {
            let _ = self.villages.damage_npc(npc_id, dmg, kb, None, 0);
        }

        for (apos, avel, dmg) in spawned_arrows {
            let id = self.next_arrow_id;
            self.next_arrow_id = self.next_arrow_id.wrapping_add(1).max(1);
            self.arrows
                .push(ArrowProjectile::new(id, apos, avel, false, dmg));
        }

        for (mkind, spos, tid) in newly_summoned {
            let count_in_temple = self
                .monsters
                .iter()
                .filter(|m| !m.dead && m.temple_id == Some(tid) && !m.kind.is_boss())
                .count();
            if count_in_temple < blockgame_core::temple::MAX_MINIONS_PER_TEMPLE {
                let id = self.next_monster_id;
                self.next_monster_id = self.next_monster_id.wrapping_add(1).max(1);
                self.monsters
                    .push(MonsterEntity::new_temple_mob(id, mkind, spos, tid));
            }
        }

        let mut defeated_temples: Vec<(u64, &'static str)> = Vec::new();
        self.monsters.retain(|monster| {
            if monster.dead {
                drops.push((monster.position, monster.generate_drops()));
                if monster.kind.is_boss() {
                    if let Some(tid) = monster.temple_id {
                        defeated_temples.push((tid, monster.kind.name()));
                    }
                }
                false
            } else if monster.kind.is_boss() {
                // Keep bosses alive within 520 blocks (so the 500-block boss HUD stays consistent)
                (monster.position - player_pos).length_squared() <= 520.0 * 520.0
            } else {
                (monster.position - player_pos).length_squared() <= 72.0 * 72.0
            }
        });

        for (tid, boss_name) in defeated_temples {
            if let Some(tkind) = self.temples.mark_boss_defeated(tid) {
                if let Some(state_snapshot) = self.temples.temples.get(&tid).cloned() {
                    let mut w = self.world.lock().unwrap();
                    blockgame_core::temple::populate_arena_treasure_chest(&mut w, &state_snapshot);
                }
                self.pending_temple_messages.push(
                    blockgame_core::temple::boss_victory_message(boss_name, tkind.name()),
                );
            }
        }

        // Only after processing boss deaths & marking defeated temples, ensure that
        // an undefeated arena (e.g., loaded from disk) has its boss spawned in singleplayer.
        // In multiplayer, arena bosses are server-authoritative via `mp_temple_monsters`.
        if !is_multiplayer && in_arena_band {
            let p_voxel = IVec3::new(
                player_pos.x.floor() as i32,
                player_pos.y.floor() as i32,
                player_pos.z.floor() as i32,
            );
            if let Some(tid) = self.temples.find_temple_by_arena_pos(p_voxel) {
                let needs_boss = self.temples.temples.get(&tid).map_or(false, |t| !t.boss_defeated)
                    && !self
                        .monsters
                        .iter()
                        .any(|m| !m.dead && m.kind.is_boss() && m.temple_id == Some(tid));
                if needs_boss {
                    if let Some(state_snapshot) = self.temples.temples.get(&tid).cloned() {
                        self.monsters.retain(|m| m.temple_id != Some(tid));
                        let spawned = blockgame_core::temple::spawn_arena_encounter(
                            &state_snapshot,
                            &mut self.next_monster_id,
                        );
                        let boss_id = spawned.first().map(|m| m.id);
                        self.monsters.extend(spawned);
                        if let Some(t) = self.temples.temples.get_mut(&tid) {
                            t.active_boss_id = boss_id;
                        }
                    }
                }
            }
        } else if !is_multiplayer && !in_arena_band {
            // Spawn 2 biome-themed overworld guardians outside undefeated temples on approach
            let (player_cpos, _, _, _) = blockgame_core::world_to_chunk_coords(IVec3::new(
                player_pos.x.floor() as i32,
                player_pos.y.floor() as i32,
                player_pos.z.floor() as i32,
            ));
            for placement in self.generator.find_temples_for_chunk(player_cpos) {
                if self.spawned_overworld_temples.contains(&placement.temple_id) {
                    continue;
                }
                let defeated = self
                    .temples
                    .temples
                    .get(&placement.temple_id)
                    .map_or(false, |t| t.boss_defeated);
                if defeated {
                    self.spawned_overworld_temples.insert(placement.temple_id);
                    continue;
                }
                let dist_sq = (placement.origin.as_vec3() - player_pos).length_squared();
                if dist_sq <= 40.0 * 40.0 {
                    self.spawned_overworld_temples.insert(placement.temple_id);
                    let guardians = blockgame_core::temple::spawn_overworld_temple_guardians(
                        placement.temple_id,
                        placement.kind,
                        placement.origin,
                        &mut self.next_monster_id,
                    );
                    self.monsters.extend(guardians);
                }
            }
        }

        (total_damage, drops)
    }

    /// Discovers nearby deterministic Villages, ticks Village Shopkeepers & Guards in singleplayer,
    /// and queues `VillageSpawnBlueprint`s for server registration in multiplayer.
    /// Returns any retaliation damage dealt by Village Guards to the local player in singleplayer.
    pub fn update_villages(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        day_night_time: f32,
        is_multiplayer: bool,
    ) -> f32 {
        let world_ticks = self.villages.advance_clock(day_night_time);
        let p_cpos = ChunkPos::new(
            (player_pos.x.floor() as i32).div_euclid(16),
            (player_pos.y.floor() as i32).div_euclid(16),
            (player_pos.z.floor() as i32).div_euclid(16),
        );
        if self.last_village_scan_chunk != Some(p_cpos) {
            self.last_village_scan_chunk = Some(p_cpos);
            for dcx in -2..=2 {
                for dcz in -2..=2 {
                    let cpos = ChunkPos::new(p_cpos.0.x + dcx, p_cpos.0.y, p_cpos.0.z + dcz);
                    for placement in self.generator.find_villages_for_chunk(cpos) {
                        if is_multiplayer {
                            if self.announced_mp_villages.insert(placement.village_id) {
                                self.pending_village_blueprints
                                    .push(placement.to_spawn_blueprint());
                            }
                        } else {
                            self.villages.ensure_village_spawned(
                                &placement.to_spawn_blueprint(),
                                world_ticks,
                            );
                        }
                    }
                }
            }
        }

        if is_multiplayer {
            return 0.0;
        }

        let world_guard = self.world.lock().unwrap();
        let players = vec![("singleplayer".to_string(), player_pos)];
        let out = self.villages.tick(
            dt,
            world_ticks,
            &players,
            &mut self.monsters,
            &world_guard,
            &self.registry,
        );
        out.guard_damage_to_players
            .into_iter()
            .map(|(_, dmg, _)| dmg)
            .sum()
    }

    /// Raycasts against living Village NPCs (both SP and MP).
    /// Returns `Some((npc_id, is_guard, display_label, dist))`.
    pub fn raycast_village_npc(
        &self,
        eye_pos: Vec3,
        look_dir: Vec3,
        max_dist: f32,
    ) -> Option<(u32, bool, String, f32)> {
        let dir = look_dir.normalize_or_zero();
        if dir.length_squared() < 0.001 {
            return None;
        }
        let mut best: Option<(u32, bool, String, f32)> = None;

        let mut test_npc = |id: u32, is_guard: bool, name: &str, pos: Vec3, half_w: f32, h: f32| {
            let min = Vec3::new(pos.x - half_w, pos.y, pos.z - half_w);
            let max = Vec3::new(pos.x + half_w, pos.y + h, pos.z + half_w);
            let inv = Vec3::new(
                if dir.x.abs() > 1e-6 { 1.0 / dir.x } else { f32::INFINITY },
                if dir.y.abs() > 1e-6 { 1.0 / dir.y } else { f32::INFINITY },
                if dir.z.abs() > 1e-6 { 1.0 / dir.z } else { f32::INFINITY },
            );
            let t1 = (min - eye_pos) * inv;
            let t2 = (max - eye_pos) * inv;
            let tmin = t1.min(t2).max_element();
            let tmax = t1.max(t2).min_element();
            if tmax >= 0.0 && tmin <= tmax && tmin <= max_dist {
                let d = tmin.max(0.0);
                if best.as_ref().map_or(true, |b| d < b.3) {
                    best = Some((id, is_guard, name.to_string(), d));
                }
            }
        };

        if self.is_multiplayer {
            for n in &self.mp_village_npcs {
                test_npc(n.id, n.is_guard, &n.name, n.position, 0.38, 1.85);
            }
        } else {
            for sk in &self.villages.shopkeepers {
                if !sk.dead {
                    test_npc(sk.id, false, &sk.name, sk.position, 0.38, 1.85);
                }
            }
            for g in &self.villages.guards {
                if !g.dead {
                    test_npc(g.id, true, &g.name, g.position, 0.40, 1.90);
                }
            }
        }

        best
    }

    /// Projects all living Village Shopkeepers & Village Guards into `PlayerModelState`
    /// so they render seamlessly through the existing `CharacterPipeline`.
    pub fn collect_village_npc_render_models(&self) -> Vec<blockgame_core::PlayerModelState> {
        let mut out = Vec::new();
        if self.is_multiplayer {
            for n in &self.mp_village_npcs {
                out.push(blockgame_core::PlayerModelState {
                    player_id: 0x8000_0000 | n.id,
                    username: n.name.clone(),
                    prefix: String::new(),
                    position: n.position,
                    velocity: Vec3::ZERO,
                    yaw: n.yaw,
                    pitch: 0.0,
                    held_item: n.held_item,
                    walk_anim: n.walk_anim,
                    swing_anim: n.swing_anim,
                    hurt_timer: n.hurt_timer,
                    is_sneaking: false,
                    appearance: n.appearance,
                    health: n.health,
                    max_health: n.max_health,
                });
            }
        } else {
            for sk in &self.villages.shopkeepers {
                if sk.dead {
                    continue;
                }
                out.push(blockgame_core::PlayerModelState {
                    player_id: 0x8000_0000 | sk.id,
                    username: sk.name.clone(),
                    prefix: String::new(),
                    position: sk.position,
                    velocity: sk.velocity,
                    yaw: sk.yaw,
                    pitch: 0.0,
                    held_item: sk.profession.held_item(),
                    walk_anim: sk.walk_anim,
                    swing_anim: 0.0,
                    hurt_timer: sk.hurt_timer,
                    is_sneaking: false,
                    appearance: sk.appearance,
                    health: sk.health,
                    max_health: sk.max_health,
                });
            }
            for g in &self.villages.guards {
                if g.dead {
                    continue;
                }
                out.push(blockgame_core::PlayerModelState {
                    player_id: 0x8000_0000 | g.id,
                    username: g.name.clone(),
                    prefix: String::new(),
                    position: g.position,
                    velocity: g.velocity,
                    yaw: g.yaw,
                    pitch: 0.0,
                    held_item: ItemId::IRON_SWORD,
                    walk_anim: g.walk_anim,
                    swing_anim: g.swing_anim,
                    hurt_timer: g.hurt_timer,
                    is_sneaking: false,
                    appearance: g.appearance,
                    health: g.health,
                    max_health: g.max_health,
                });
            }
        }
        out
    }

    /// Returns `true` if opening a chest at `pos` is forbidden because the temple's boss is still alive.
    pub fn is_chest_locked(&self, pos: IVec3) -> bool {
        if self.temples.is_chest_locked(pos) {
            return true;
        }
        let (cpos, _, _, _) = blockgame_core::world_to_chunk_coords(pos);
        for placement in self.generator.find_temples_for_chunk(cpos) {
            let defeated = self
                .temples
                .temples
                .get(&placement.temple_id)
                .map_or(false, |t| t.boss_defeated);
            if !defeated {
                let bounds = blockgame_core::temple::TempleBounds {
                    min: placement.origin + IVec3::new(-12, -4, -12),
                    max: placement.origin + IVec3::new(12, 16, 12),
                };
                if bounds.contains(pos) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns `true` if breaking or placing at `pos` is forbidden due to boss temple/arena protection.
    pub fn is_block_protected(&self, pos: IVec3) -> bool {
        let existing_block = {
            let world_guard = self.world.lock().unwrap();
            world_guard.get_block(pos)
        };
        if self.temples.is_block_protected(pos, existing_block) {
            return true;
        }
        // Also enforce protection on undiscovered overworld temples before the player enters their portal
        let (cpos, _, _, _) = blockgame_core::world_to_chunk_coords(pos);
        for placement in self.generator.find_temples_for_chunk(cpos) {
            let defeated = self
                .temples
                .temples
                .get(&placement.temple_id)
                .map_or(false, |t| t.boss_defeated);
            if !defeated {
                let bounds = blockgame_core::temple::TempleBounds {
                    min: placement.origin + IVec3::new(-12, -4, -12),
                    max: placement.origin + IVec3::new(12, 16, 12),
                };
                if bounds.contains(pos) {
                    return true;
                }
            }
        }
        false
    }

    /// Looks up the deterministic overworld temple placement corresponding to a `TEMPLE_PORTAL` block.
    pub fn find_temple_placement_at_portal(&self, pos: IVec3) -> Option<blockgame_gen::TemplePlacement> {
        let (cpos, _, _, _) = blockgame_core::world_to_chunk_coords(pos);
        self.generator
            .find_temples_for_chunk(cpos)
            .into_iter()
            .find(|t| {
                let d = (t.portal_pos - pos).abs();
                d.x <= 2 && d.y <= 3 && d.z <= 2
            })
    }

    /// Handles singleplayer right-click on `TEMPLE_PORTAL`, `ARENA_RETURN_PORTAL`, or `BOSS_ALTAR`.
    /// Returns `Some(teleport_dest)` if the player should be teleported.
    pub fn interact_temple_block_sp(&mut self, pos: IVec3, player_pos: Vec3) -> Option<Vec3> {
        let block = {
            let w = self.world.lock().unwrap();
            w.get_block(pos)
        };

        if block == BlockId::TEMPLE_PORTAL {
            let placement = self.find_temple_placement_at_portal(pos)?;
            self.temples
                .player_return_positions
                .insert("local".to_string(), player_pos);

            let state_snapshot = {
                let entry = self.temples.register_temple(
                    placement.temple_id,
                    placement.kind,
                    placement.origin,
                    placement.portal_pos,
                );
                entry.clone()
            };

            let floor_missing = {
                let w = self.world.lock().unwrap();
                w.get_block(state_snapshot.arena_origin) == BlockId::AIR
            };
            if !state_snapshot.arena_built || floor_missing {
                {
                    let mut w = self.world.lock().unwrap();
                    blockgame_core::temple::build_arena_into_world(&mut w, &state_snapshot);
                    if state_snapshot.boss_defeated {
                        blockgame_core::temple::populate_arena_treasure_chest(&mut w, &state_snapshot);
                    }
                }
                let min_c = blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.min).0;
                let max_c = blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.max).0;
                for cx in min_c.0.x..=max_c.0.x {
                    for cy in min_c.0.y..=max_c.0.y {
                        for cz in min_c.0.z..=max_c.0.z {
                            let acp = ChunkPos::new(cx, cy, cz);
                            self.loaded_chunks_set.insert(acp);
                            self.dirty_remesh.insert(acp);
                        }
                    }
                }
                self.has_pending_chunks = true;
                if let Some(t) = self.temples.temples.get_mut(&placement.temple_id) {
                    t.arena_built = true;
                }
            }

            // Always ensure the boss & guardians are present if this temple is not yet defeated
            let has_live_boss = self.monsters.iter().any(|m| {
                !m.dead && m.kind.is_boss() && m.temple_id == Some(placement.temple_id)
            });
            if !state_snapshot.boss_defeated && !has_live_boss {
                self.monsters
                    .retain(|m| m.temple_id != Some(placement.temple_id));
                let spawned = blockgame_core::temple::spawn_arena_encounter(
                    &state_snapshot,
                    &mut self.next_monster_id,
                );
                let boss_id = spawned.first().map(|m| m.id);
                self.monsters.extend(spawned);
                if let Some(t) = self.temples.temples.get_mut(&placement.temple_id) {
                    t.active_boss_id = boss_id;
                }
            }

            self.pending_temple_messages.push(format!(
                "&6Entered {}! Defeat {} to unlock the temple!",
                placement.kind.name(),
                placement.kind.boss_kind().name()
            ));
            return Some(state_snapshot.arena_entry_spawn());
        } else if block == BlockId::ARENA_RETURN_PORTAL {
            let dest = self
                .temples
                .player_return_positions
                .get("local")
                .copied()
                .or_else(|| {
                    self.temples
                        .find_temple_by_arena_pos(pos)
                        .and_then(|tid| self.temples.temples.get(&tid))
                        .map(|t| t.overworld_return_spawn())
                });
            if dest.is_some() {
                self.pending_temple_messages
                    .push("&aReturned to the overworld temple.".to_string());
            }
            return dest;
        } else if block == BlockId::BOSS_ALTAR {
            if let Some(tid) = self.temples.find_temple_by_arena_pos(pos) {
                let info = self.temples.temples.get(&tid).map(|t| {
                    (t.boss_defeated, t.rechallenge_cooldown, t.clone())
                });
                if let Some((defeated, cd, state_snapshot)) = info {
                    if !defeated {
                        self.pending_temple_messages
                            .push("&eThe boss is already awake in this arena!".to_string());
                    } else if cd > 0.0 {
                        self.pending_temple_messages.push(format!(
                            "&eBoss Altar recharging... {:.0}s remaining.",
                            cd.ceil()
                        ));
                    } else {
                        self.monsters.retain(|m| m.temple_id != Some(tid));
                        {
                            let mut w = self.world.lock().unwrap();
                            blockgame_core::temple::build_arena_into_world(&mut w, &state_snapshot);
                        }
                        let min_c =
                            blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.min).0;
                        let max_c =
                            blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.max).0;
                        for cx in min_c.0.x..=max_c.0.x {
                            for cy in min_c.0.y..=max_c.0.y {
                                for cz in min_c.0.z..=max_c.0.z {
                                    self.dirty_remesh.insert(ChunkPos::new(cx, cy, cz));
                                }
                            }
                        }
                        let spawned = blockgame_core::temple::spawn_arena_encounter(
                            &state_snapshot,
                            &mut self.next_monster_id,
                        );
                        let boss_id = spawned.first().map(|m| m.id);
                        self.monsters.extend(spawned);
                        if let Some(t) = self.temples.temples.get_mut(&tid) {
                            t.boss_defeated = false;
                            t.active_boss_id = boss_id;
                        }
                        self.pending_temple_messages.push(format!(
                            "&c{} has been re-summoned at {}!",
                            state_snapshot.kind.boss_kind().name(),
                            state_snapshot.kind.name()
                        ));
                    }
                }
            }
        }
        None
    }

    /// Updates the synced multiplayer temple monster list from `ServerPacket::TempleMonstersSync`.
    pub fn apply_mp_temple_monsters(&mut self, synced: &[blockgame_core::SyncedMonsterState]) {
        self.monsters.retain(|m| !m.kind.is_temple_mob());
        self.mp_temple_monsters = synced
            .iter()
            .map(|s| {
                let mut m = MonsterEntity::new(s.id, s.kind, s.position);
                m.yaw = s.yaw;
                m.health = s.health;
                m.max_health = s.max_health;
                m.hurt_timer = s.hurt_timer;
                m.walk_anim_timer = s.walk_anim_timer;
                m.phase = s.phase;
                m.in_combat = s.in_combat;
                m.invuln_timer = if s.invulnerable { 1.0 } else { 0.0 };
                m
            })
            .collect();
    }

    /// Updates the synced multiplayer temple arrow list from `ServerPacket::TempleArrowsSync`.
    pub fn apply_mp_temple_arrows(&mut self, synced: &[blockgame_core::SyncedArrowState]) {
        self.mp_temple_arrows = synced
            .iter()
            .map(|s| {
                let mut a = ArrowProjectile::new(s.id, s.position, s.velocity, false, 0.0);
                a.stuck = s.stuck;
                a
            })
            .collect();
    }

    /// Raycast hit test against synced multiplayer boss/temple monsters.
    /// Returns `(id, kind, position, invulnerable)`.
    pub fn hit_test_mp_temple_monster(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        reach: f32,
    ) -> Option<(u32, MonsterKind, Vec3, bool)> {
        let mut closest_dist = reach;
        let mut hit = None;
        for m in &self.mp_temple_monsters {
            if m.dead {
                continue;
            }
            if let Some(dist) = m.aabb().intersect_ray(ray_origin, ray_dir, reach) {
                if dist < closest_dist {
                    closest_dist = dist;
                    hit = Some((m.id, m.kind, m.position, m.invuln_timer > 0.0));
                }
            }
        }
        hit
    }

    /// Returns the active top-of-screen Boss HUD state (within 500 blocks and in combat).
    pub fn active_boss_hud(
        &self,
        player_pos: Vec3,
        is_multiplayer: bool,
    ) -> Option<blockgame_core::temple::BossHudState> {
        if is_multiplayer {
            self.mp_boss_hud.clone()
        } else {
            blockgame_core::temple::compute_boss_hud(&self.monsters, player_pos)
        }
    }

    /// Updates all active arrow projectiles and applies hits to monsters, MP temple monsters,
    /// animals, or the player.
    /// Returns `(player_damage_taken, monster_hits, monster_kills, mp_temple_hits)` where each
    /// MP temple hit is `(monster_id, damage, knockback)`.
    pub fn update_arrows(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        player_half_width: f32,
        player_height: f32,
    ) -> (f32, usize, usize, Vec<(u32, f32, Vec3)>) {
        let mut total_player_damage = 0.0;
        let mut monster_hits = 0;
        let mut monster_kills = 0;
        let mut mp_temple_hits: Vec<(u32, f32, Vec3)> = Vec::new();
        let player_box = AABB::from_player(player_pos, player_half_width, player_height);
        let world_guard = self.world.lock().unwrap();

        for arrow in self.arrows.iter_mut() {
            arrow.update(dt, &world_guard, &self.registry);
            if arrow.dead || arrow.stuck {
                continue;
            }

            if arrow.from_player {
                let p = arrow.position;
                let mut hit = false;
                for monster in self.monsters.iter_mut() {
                    if monster.dead {
                        continue;
                    }
                    let b = monster.aabb();
                    if p.x >= b.min.x - 0.15
                        && p.x <= b.max.x + 0.15
                        && p.y >= b.min.y - 0.15
                        && p.y <= b.max.y + 0.15
                        && p.z >= b.min.z - 0.15
                        && p.z <= b.max.z + 0.15
                    {
                        if monster.invuln_timer <= 0.0 {
                            monster.take_damage(arrow.damage, arrow.velocity.normalize_or_zero());
                            monster_hits += 1;
                            if monster.dead {
                                monster_kills += 1;
                                self.pending_arrow_killed_monsters.push(monster.kind);
                            }
                        }
                        arrow.dead = true;
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    for monster in &self.mp_temple_monsters {
                        if monster.dead {
                            continue;
                        }
                        let b = monster.aabb();
                        if p.x >= b.min.x - 0.15
                            && p.x <= b.max.x + 0.15
                            && p.y >= b.min.y - 0.15
                            && p.y <= b.max.y + 0.15
                            && p.z >= b.min.z - 0.15
                            && p.z <= b.max.z + 0.15
                        {
                            if monster.invuln_timer <= 0.0 {
                                mp_temple_hits.push((
                                    monster.id,
                                    arrow.damage,
                                    arrow.velocity.normalize_or_zero(),
                                ));
                                monster_hits += 1;
                            }
                            arrow.dead = true;
                            hit = true;
                            break;
                        }
                    }
                }
                if !hit {
                    for animal in self.animals.iter_mut() {
                        if animal.dead {
                            continue;
                        }
                        let b = animal.aabb();
                        if p.x >= b.min.x - 0.15
                            && p.x <= b.max.x + 0.15
                            && p.y >= b.min.y - 0.15
                            && p.y <= b.max.y + 0.15
                            && p.z >= b.min.z - 0.15
                            && p.z <= b.max.z + 0.15
                        {
                            animal.take_damage(arrow.damage, arrow.velocity.normalize_or_zero());
                            arrow.dead = true;
                            hit = true;
                            break;
                        }
                    }
                }
                if !hit {
                    let kb = arrow.velocity.normalize_or_zero();
                    if self.is_multiplayer {
                        for n in &self.mp_village_npcs {
                            let pos = n.position;
                            if p.x >= pos.x - 0.5
                                && p.x <= pos.x + 0.5
                                && p.y >= pos.y - 0.1
                                && p.y <= pos.y + 1.95
                                && p.z >= pos.z - 0.5
                                && p.z <= pos.z + 0.5
                            {
                                self.pending_mp_npc_arrow_hits.push((n.id, arrow.damage, kb));
                                arrow.dead = true;
                                break;
                            }
                        }
                    } else {
                        let hit_npc_id = self
                            .villages
                            .living_npc_targets()
                            .into_iter()
                            .find(|(_, pos)| {
                                p.x >= pos.x - 0.5
                                    && p.x <= pos.x + 0.5
                                    && p.y >= pos.y - 0.1
                                    && p.y <= pos.y + 1.95
                                    && p.z >= pos.z - 0.5
                                    && p.z <= pos.z + 0.5
                            })
                            .map(|(id, _)| id);
                        if let Some(npc_id) = hit_npc_id {
                            let wt = self.villages.total_world_ticks;
                            let _ = self.villages.damage_npc(
                                npc_id,
                                arrow.damage,
                                kb,
                                Some("singleplayer"),
                                wt,
                            );
                            arrow.dead = true;
                        }
                    }
                }
            } else {
                let p = arrow.position;
                if p.x >= player_box.min.x - 0.1
                    && p.x <= player_box.max.x + 0.1
                    && p.y >= player_box.min.y - 0.1
                    && p.y <= player_box.max.y + 0.1
                    && p.z >= player_box.min.z - 0.1
                    && p.z <= player_box.max.z + 0.1
                {
                    total_player_damage += arrow.damage;
                    arrow.dead = true;
                } else if !self.is_multiplayer {
                    let hit_npc_id = self
                        .villages
                        .living_npc_targets()
                        .into_iter()
                        .find(|(_, pos)| {
                            p.x >= pos.x - 0.5
                                && p.x <= pos.x + 0.5
                                && p.y >= pos.y - 0.1
                                && p.y <= pos.y + 1.95
                                && p.z >= pos.z - 0.5
                                && p.z <= pos.z + 0.5
                        })
                        .map(|(id, _)| id);
                    if let Some(npc_id) = hit_npc_id {
                        let wt = self.villages.total_world_ticks;
                        let _ = self.villages.damage_npc(
                            npc_id,
                            arrow.damage,
                            arrow.velocity.normalize_or_zero(),
                            None,
                            wt,
                        );
                        arrow.dead = true;
                    }
                }
            }
        }

        self.arrows.retain(|a| !a.dead);
        (total_player_damage, monster_hits, monster_kills, mp_temple_hits)
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

            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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
        self.farming_sim.plant_crop(pos);
        self.register_farmland(pos - IVec3::Y);
    }

    /// Rebuilds and uploads GPU meshes for each chunk in `chunks`.
    fn remesh_chunks(
        &mut self,
        chunks: HashSet<ChunkPos>,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        if chunks.is_empty() {
            return;
        }
        let registry = Arc::clone(&self.registry);
        let mut meshes = Vec::with_capacity(chunks.len());
        {
            let world = self.world.lock().unwrap();
            for cpos in chunks {
                if let Some(chunk) = world.get_chunk(cpos) {
                    meshes.push((cpos, mesh_chunk_from_world_with_options(&world, &registry, chunk, self.fast_graphics)));
                }
            }
        }
        for (cpos, mesh) in meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

    /// Applies a passive fertilizer tier to farmland at `pos` and remeshes if upgraded.
    pub fn apply_fertilizer(
        &mut self,
        pos: IVec3,
        tier: blockgame_core::FertilizerTier,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> Option<BlockId> {
        let updated = {
            let mut world = self.world.lock().unwrap();
            self.farming_sim.apply_fertilizer(pos, tier, &mut world)
        };
        if updated.is_some() {
            let (cpos, _, _, _) = world_to_chunk_coords(pos);
            let mut chunks = HashSet::with_capacity(1);
            chunks.insert(cpos);
            self.remesh_chunks(chunks, device, pipeline);
        }
        updated
    }

    /// Consumes passive fertilizer on the farmland under `crop_pos` when a mature crop is harvested.
    pub fn consume_fertilizer_on_harvest(
        &mut self,
        crop_pos: IVec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) -> (blockgame_core::FertilizerTier, Option<BlockId>) {
        let (tier, reset_block) = {
            let mut world = self.world.lock().unwrap();
            self.farming_sim
                .consume_fertilizer_on_harvest(crop_pos, &mut world)
        };
        if reset_block.is_some() {
            let farm_pos = crop_pos - IVec3::Y;
            let (cpos, _, _, _) = world_to_chunk_coords(farm_pos);
            let mut chunks = HashSet::with_capacity(1);
            chunks.insert(cpos);
            self.remesh_chunks(chunks, device, pipeline);
        }
        (tier, reset_block)
    }

    /// Ticks shared `FarmingSimulation` (crops, stems, sugar cane, farmland hydration & reversion)
    /// for singleplayer worlds, spawning popped drops and remeshing modified chunks.
    pub fn update_farming(
        &mut self,
        dt: f32,
        sun_intensity: f32,
        player_pos: Vec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let events = {
            let mut world = self.world.lock().unwrap();
            self.farming_sim
                .tick(dt, sun_intensity, &[player_pos], &mut world, &self.registry)
        };
        if events.is_empty() {
            return;
        }
        let mut changed_chunks = HashSet::new();
        for ev in events {
            match ev {
                blockgame_core::FarmingEvent::BlockChanged { pos, .. } => {
                    let (cp, _, _, _) = world_to_chunk_coords(pos);
                    changed_chunks.insert(cp);
                }
                blockgame_core::FarmingEvent::SpawnDrop { pos, item } => {
                    self.spawn_item_drop(pos.as_vec3() + Vec3::new(0.5, 0.35, 0.5), item);
                }
            }
        }
        self.remesh_chunks(changed_chunks, device, pipeline);
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
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        meshes.push((cpos, mesh_chunk_from_world_with_options(&world, &registry, chunk, self.fast_graphics)));
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
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        meshes.push((cpos, mesh_chunk_from_world_with_options(&world, &registry, chunk, self.fast_graphics)));
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
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        meshes.push((cpos, mesh_chunk_from_world_with_options(&world, &registry, chunk, self.fast_graphics)));
                    }
                }
            }
            for (cpos, mesh) in meshes {
                self.store_mesh(cpos, &mesh, device, pipeline);
            }
        }
    }

    /// Ticks all Cooking Pots in singleplayer and remeshes chunks when lit state changes.
    pub fn update_cooking_pots(
        &mut self,
        dt: f32,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let mut changed_chunks = HashSet::new();
        {
            let mut world = self.world.lock().unwrap();
            let mut lit_changes = Vec::new();
            for (&pos, pot) in world.cooking_pots.iter_mut() {
                if pot.update(dt) {
                    lit_changes.push((pos, pot.is_burning()));
                }
            }
            for (pos, is_burning) in lit_changes {
                let cur = world.get_block(pos);
                if is_burning && cur == BlockId::COOKING_POT {
                    world.set_block(pos, BlockId::COOKING_POT_LIT);
                    let (cp, _, _, _) = world_to_chunk_coords(pos);
                    changed_chunks.insert(cp);
                } else if !is_burning && cur == BlockId::COOKING_POT_LIT {
                    world.set_block(pos, BlockId::COOKING_POT);
                    let (cp, _, _, _) = world_to_chunk_coords(pos);
                    changed_chunks.insert(cp);
                }
            }
        }
        self.remesh_chunks(changed_chunks, device, pipeline);
    }

    /// Ticks passive animals in singleplayer using the shared `blockgame_core::animal::tick_animals`
    /// simulation (herding toward Breeding Feed, breeding, passive Chicken eggs, death drops).
    pub fn update_animals_with_lure(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        held_item: ItemId,
    ) -> Vec<(Vec3, ItemSlot)> {
        let world = self.world.lock().unwrap();
        blockgame_core::animal::tick_animals(
            &mut self.animals,
            &mut self.next_animal_id,
            dt,
            &[(player_pos, held_item)],
            &world,
            &self.registry,
        )
    }

    /// Replaces/updates client-side animal entities from a server-authoritative `AnimalsSync` packet.
    pub fn apply_synced_animals(&mut self, synced: &[blockgame_core::SyncedAnimalState]) {
        self.animals.clear();
        for s in synced {
            let mut a = AnimalEntity::new(s.id, s.kind, s.position);
            a.velocity = s.velocity;
            a.yaw = s.yaw;
            a.health = s.health;
            a.max_health = s.max_health;
            a.hurt_timer = s.hurt_timer;
            a.love_timer = s.love_timer;
            a.breed_cooldown = s.breed_cooldown;
            a.baby_timer = s.baby_timer;
            a.is_moving = s.is_moving;
            a.walk_anim_timer = s.walk_anim_timer;
            self.animals.push(a);
        }
    }

    /// Ticks all electrical networks (generators, copper cables, batteries, electric furnaces,
    /// and electric lamps) and remeshes chunks affected by lighting state changes.
    pub fn update_power_networks(
        &mut self,
        dt: f32,
        is_daytime: bool,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let changed_blocks = {
            let mut world = self.world.lock().unwrap();
            blockgame_core::power::tick_power_networks(&mut world, dt, is_daytime)
        };

        if !changed_blocks.is_empty() {
            let mut changed_chunks = HashSet::new();
            for pos in changed_blocks {
                let (cp, _, _, _) = world_to_chunk_coords(pos);
                changed_chunks.insert(cp);
                // Light level 15 can illuminate adjacent chunks too
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            changed_chunks.insert(blockgame_core::chunk::ChunkPos::new(
                                cp.0.x + dx,
                                cp.0.y + dy,
                                cp.0.z + dz,
                            ));
                        }
                    }
                }
            }

            let registry = Arc::clone(&self.registry);
            let mut meshes = Vec::new();
            {
                let world = self.world.lock().unwrap();
                for cpos in changed_chunks {
                    if let Some(chunk) = world.get_chunk(cpos) {
                        meshes.push((cpos, mesh_chunk_from_world_with_options(&world, &registry, chunk, self.fast_graphics)));
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

                let meshes: Vec<(ChunkPos, ChunkMesh)> = chunks_to_rebuild
                    .into_iter()
                    .filter_map(|cpos| {
                        world
                            .get_chunk(cpos)
                            .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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

            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
                })
                .collect::<Vec<_>>()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

    /// Applies an authoritative `ServerPacket::FluidUpdate` (block + 4-bit fluid level) from the
    /// multiplayer server and remeshes affected chunks.
    pub fn apply_remote_fluid_update(
        &mut self,
        pos: IVec3,
        block_id: BlockId,
        level: u8,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let mut chunks_to_rebuild = Vec::new();
        Self::add_chunk_and_neighbors(pos, &mut chunks_to_rebuild);

        let new_meshes = {
            let mut world = self.world.lock().unwrap();
            let old_block = world.get_block(pos);
            let old_level = world.get_fluid_level(pos);
            let level = level & 0x0F;
            if old_block == block_id && old_level == level {
                return;
            }
            let is_light_change = old_block.is_emissive() || block_id.is_emissive();
            world.set_block(pos, block_id);
            world.set_fluid_level(pos, level);

            if is_light_change {
                Self::add_light_neighborhood(pos, &mut chunks_to_rebuild);
            }

            chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
            chunks_to_rebuild.dedup();

            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
                })
                .collect::<Vec<_>>()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

    /// Applies an initial `WorldDeltaSync` packet from the multiplayer server and remeshes modified chunks.
    /// Builds (if needed) and synchronously meshes/uploads a temple's instanced arena chunks so
    /// the arena floor is 100% solid and rendered immediately upon teleporting in (SP & MP).
    pub fn ensure_arena_built_local(
        &mut self,
        placement: blockgame_gen::TemplePlacement,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        let state_snapshot = {
            let entry = self.temples.register_temple(
                placement.temple_id,
                placement.kind,
                placement.origin,
                placement.portal_pos,
            );
            entry.clone()
        };

        let floor_missing = {
            let w = self.world.lock().unwrap();
            w.get_block(state_snapshot.arena_origin) == BlockId::AIR
        };
        if !state_snapshot.arena_built || floor_missing {
            let mut w = self.world.lock().unwrap();
            blockgame_core::temple::build_arena_into_world(&mut w, &state_snapshot);
            if state_snapshot.boss_defeated {
                blockgame_core::temple::populate_arena_treasure_chest(&mut w, &state_snapshot);
            }
        }

        let min_c = blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.min).0;
        let max_c = blockgame_core::world_to_chunk_coords(state_snapshot.arena_bounds.max).0;
        let mut arena_cpos = Vec::new();
        for cx in min_c.0.x..=max_c.0.x {
            for cy in min_c.0.y..=max_c.0.y {
                for cz in min_c.0.z..=max_c.0.z {
                    let acp = ChunkPos::new(cx, cy, cz);
                    self.loaded_chunks_set.insert(acp);
                    self.dirty_remesh.remove(&acp);
                    arena_cpos.push(acp);
                }
            }
        }

        let new_meshes = {
            let world = self.world.lock().unwrap();
            arena_cpos
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
                })
                .collect::<Vec<_>>()
        };

        for (cpos, mesh) in new_meshes {
            self.store_mesh(cpos, &mesh, device, pipeline);
        }
    }

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
                let rle_sum: usize = blocks.iter().map(|&(count, _)| count as usize).sum();
                if rle_sum == blockgame_core::CHUNK_VOLUME {
                    let chunk = blockgame_core::save::decode_chunk_rle(*cpos, blocks);
                    world.insert_chunk(chunk);
                } else {
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

        for &cpos in &chunks_to_rebuild {
            self.loaded_chunks_set.insert(cpos);
        }

        self.item_drops.clear();
        for (drop_id, pos, vel, item) in item_drops {
            self.item_drops.push(ItemDrop::with_id_and_velocity(*drop_id, *pos, *vel, *item));
        }

        chunks_to_rebuild.sort_by_key(|c| (c.0.x, c.0.y, c.0.z));
        chunks_to_rebuild.dedup();

        let new_meshes = {
            let world = self.world.lock().unwrap();
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world
                        .get_chunk(cpos)
                        .map(|chunk| (cpos, mesh_chunk_from_world_with_options(&world, &self.registry, chunk, self.fast_graphics)))
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
        wm.farming_sim
            .active_farmland
            .push(blockgame_core::farming::FarmlandEntry {
                pos: farm_pos,
                dry_timer: 14.0,
            });
        wm.farming_sim
            .planted_crops
            .push(blockgame_core::farming::CropEntry {
                pos: crop_pos,
                timer: 7.5,
                target_secs: 120.0,
            });
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
            homes: std::collections::HashMap::new(),
            skills: blockgame_core::PlayerSkills::new(),
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
        assert_eq!(wm_loaded.farming_sim.active_farmland.len(), 1);
        assert_eq!(wm_loaded.farming_sim.active_farmland[0].pos, farm_pos);
        assert_eq!(wm_loaded.farming_sim.active_farmland[0].dry_timer, 14.0);
        assert_eq!(wm_loaded.farming_sim.planted_crops.len(), 1);
        assert_eq!(wm_loaded.farming_sim.planted_crops[0].pos, crop_pos);
        assert_eq!(wm_loaded.farming_sim.planted_crops[0].timer, 7.5);
        assert_eq!(wm_loaded.item_drops.len(), 1);
        assert_eq!(wm_loaded.item_drops[0].item.item_id, ItemId::APPLE);
    }

    #[test]
    fn test_world_manager_chunk_streaming_budget_and_pending_flags() {
        let registry = Arc::new(BlockRegistry::default());
        let mut wm = WorldManager::new(4242, 2, registry);
        assert!(wm.has_pending_chunks, "New WorldManager should have pending chunks");

        wm.has_pending_chunks = false;
        wm.set_render_distance(4);
        assert!(wm.has_pending_chunks, "Setting render distance must flag pending chunks");

        assert!(wm.queued_generation.is_empty());
        assert!(wm.queued_meshing.is_empty());
    }

    #[test]
    fn test_world_manager_prioritizes_near_chunks_and_skips_loaded_empty_chunks() {
        let registry = Arc::new(BlockRegistry::default());
        let mut wm = WorldManager::new(4242, 4, registry);
        let player_chunk = ChunkPos::new(0, 4, 0);

        // Pre-populate a nearby chunk in `world` that still needs meshing (without gpu_chunks entry),
        // and mark an empty sky chunk above the player as already completed in `loaded_chunks_set`.
        let near_to_mesh = ChunkPos::new(1, 4, 0);
        let already_loaded_empty = ChunkPos::new(0, 5, 0);
        {
            let mut world = wm.world().lock().unwrap();
            world.insert_chunk(Chunk::new(near_to_mesh));
            world.insert_chunk(Chunk::new(already_loaded_empty));
        }
        wm.loaded_chunks_set.insert(already_loaded_empty);

        let (batch_gen, batch_mesh) = wm.collect_streaming_batch(player_chunk, 12);

        // Already-loaded empty chunk must NOT be re-queued into batch_mesh
        assert!(
            batch_mesh.iter().all(|&(p, _)| p != already_loaded_empty),
            "Loaded empty chunks must never be re-queued into meshing"
        );
        // Nearby `to_mesh` chunk must NOT be starved behind distant `to_generate` chunks
        assert!(
            batch_mesh.iter().any(|&(p, _)| p == near_to_mesh),
            "Nearby already-generated chunk must be included in first streaming batch instead of starving behind far generation"
        );
        // Player's own column must be in the very first batch and no distant edge chunk (dist >= 3) should jump ahead
        assert!(batch_gen.iter().any(|&(p, _)| p == player_chunk));
        for &(p, _) in batch_gen.iter().chain(batch_mesh.iter()) {
            let horiz_sq = p.0.x * p.0.x + p.0.z * p.0.z;
            assert!(
                horiz_sq <= 4,
                "Expected only close chunks (horiz_sq <= 4) in first batch, got {:?} (horiz_sq={})",
                p,
                horiz_sq
            );
        }
    }

    #[test]
    fn test_world_manager_neighbor_remesh_and_water_border_suppression() {
        let registry = Arc::new(BlockRegistry::default());
        let mut world = World::new();

        let c_left = ChunkPos::new(0, 2, 0);
        let c_right = ChunkPos::new(1, 2, 0);
        let c_air = ChunkPos::new(-1, 2, 0);

        // Create two adjacent ocean chunks with water meeting at x=15 | x=0
        let mut left_chunk = Chunk::new(c_left);
        let mut right_chunk = Chunk::new(c_right);
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                for y in 0..=6 {
                    left_chunk.set(x, y, z, BlockId::WATER);
                    right_chunk.set(x, y, z, BlockId::WATER);
                }
            }
        }
        world.insert_chunk(left_chunk);
        world.insert_chunk(right_chunk);
        // Note: `c_air` (-1, 2, 0) and `ChunkPos(0, 2, 1)` are still unloaded!

        let mesh_left_before = mesh_chunk_from_world(&world, &registry, world.get_chunk(c_left).unwrap());
        // Must have NO internal water wall on +X (shared with c_right) and NO water wall on -X or +Z (unloaded)
        assert!(
            mesh_left_before
                .water_vertices
                .iter()
                .all(|v| v.normal[0].abs() < 0.1 && v.normal[2] < 0.1),
            "Water chunk must not emit vertical border walls toward loaded water or unloaded neighbors"
        );

        // Now insert `c_air` (-1, 2, 0) containing AIR; collect_affected_neighbors must flag `c_left` for remesh
        world.insert_chunk(Chunk::new(c_air));
        let affected = collect_affected_neighbors(&world, &[c_air]);
        assert!(
            affected.contains(&c_left),
            "Inserting a loaded neighbor touching water must schedule affected neighbor for remesh"
        );

        // After remeshing `c_left` with `c_air` loaded, the -X water-to-air wall must now be emitted cleanly
        let mesh_left_after = mesh_chunk_from_world(&world, &registry, world.get_chunk(c_left).unwrap());
        assert!(
            mesh_left_after.water_vertices.iter().any(|v| v.normal[0] < -0.5),
            "Remeshed water chunk must emit -X face once neighboring AIR chunk is loaded"
        );
    }

    #[test]
    fn test_issue_26_biome_meshing_and_village_scan_guard() {
        let registry = Arc::new(BlockRegistry::default());
        let mut wm = WorldManager::new(2577988685, 4, registry.clone());

        // 1. Verify update_villages records last_village_scan_chunk so stationary frames skip rescanning
        let pos = Vec3::new(1711.5, 59.0, -1129.5);
        assert_eq!(wm.last_village_scan_chunk, None);
        wm.update_villages(0.016, pos, 6000.0, false);
        let expected_cpos = ChunkPos::new(
            (1711_i32).div_euclid(16),
            (59_i32).div_euclid(16),
            (-1129_i32).div_euclid(16),
        );
        assert_eq!(wm.last_village_scan_chunk, Some(expected_cpos));
        // Second call in the same chunk must keep the guard intact and run instantly
        wm.update_villages(0.016, pos + Vec3::new(0.2, 0.0, 0.2), 6001.0, false);
        assert_eq!(wm.last_village_scan_chunk, Some(expected_cpos));

        // 2. Verify Fast vs Fancy leaf geometry reduction on real generated chunks around (1711, 59, -1129)
        let mut world = World::new();
        let mut positions = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                let cpos = ChunkPos::new(106 + dx, 3, -71 + dz);
                let mut chunk = wm.generator.generate_chunk(cpos);
                wm.generator.populate_chunk_features(&mut chunk);
                world.insert_chunk(chunk);
                positions.push(cpos);
            }
        }
        let mut fancy_cutout = 0usize;
        let mut fast_cutout = 0usize;
        let mut foliage_verts = 0usize;
        for cpos in positions {
            let chunk = world.get_chunk(cpos).unwrap();
            let fancy = mesh_chunk_from_world_with_options(&world, &registry, chunk, false);
            let fast = mesh_chunk_from_world_with_options(&world, &registry, chunk, true);
            fancy_cutout += fancy.cutout_vertices.len();
            fast_cutout += fast.cutout_vertices.len();
            foliage_verts += fancy.foliage_vertices.len();
        }
        assert!(foliage_verts > 0, "Expected Savanna/Jungle foliage around (1711, 59, -1129)");
        assert!(
            fast_cutout < fancy_cutout,
            "Fast Graphics Quality should cull internal leaf faces (fast={} vs fancy={})",
            fast_cutout,
            fancy_cutout
        );
    }
}

