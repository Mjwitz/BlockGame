use std::collections::{HashMap, HashSet};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use blockgame_core::{
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
}

impl WorldManager {
    pub fn new(
        seed: u32,
        render_distance: i32,
        registry: Arc<BlockRegistry>,
    ) -> Self {
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
        }
    }

    /// Synchronously pre-generates and meshes the initial area around the spawn point.
    pub fn init_spawn_area(&mut self, device: &wgpu::Device, pipeline: &VoxelPipeline) {
        log::info!("Generating initial spawn chunks (radius {})...", self.render_distance);

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
                    let mesh = mesher.mesh_chunk_with_neighbors(chunk, |lx, ly, lz| {
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
    }

    /// Periodic update called every frame: streams chunks and uploads completed background meshes.
    pub fn update(
        &mut self,
        player_pos: Vec3,
        device: &wgpu::Device,
        pipeline: &VoxelPipeline,
    ) {
        // 1. Ingest background mesh results (up to budget per frame to avoid stutters)
        let mut upload_count = 0;
        while let Ok((pos, mesh)) = self.mesh_rx.try_recv() {
            self.queued_meshing.remove(&pos);
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(pos, gpu_mesh);
            }
            upload_count += 1;
            if upload_count >= 8 {
                break;
            }
        }

        // 2. Check if player entered a new chunk
        let (current_chunk, _, _, _) = world_to_chunk_coords(IVec3::new(
            player_pos.x.floor() as i32,
            player_pos.y.floor() as i32,
            player_pos.z.floor() as i32,
        ));

        if current_chunk == self.last_player_chunk {
            return;
        }
        self.last_player_chunk = current_chunk;

        let pcx = current_chunk.0.x;
        let pcz = current_chunk.0.z;
        let r = self.render_distance;

        // 3. Identify chunks that need generation
        let mut to_generate = Vec::new();
        for cy in 0..=2 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dz * dz <= (r + 1) * (r + 1) {
                        let pos = ChunkPos::new(pcx + dx, cy, pcz + dz);
                        if !self.loaded_chunks_set.contains(&pos)
                            && !self.queued_generation.contains(&pos)
                        {
                            self.queued_generation.insert(pos);
                            to_generate.push(pos);
                        }
                    }
                }
            }
        }

        // 4. Dispatch generation asynchronously if needed
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

                // Mesh newly loaded chunks that have neighbors
                let mesher = ChunkMesher::new(&registry);
                for &pos in &to_generate {
                    if let Some(chunk) = world.get_chunk(pos) {
                        let origin = pos.to_world_origin();
                        let mesh = mesher.mesh_chunk_with_neighbors(chunk, |lx, ly, lz| {
                            world.get_block(origin + IVec3::new(lx, ly, lz))
                        });
                        let _ = tx.send((pos, mesh));
                    }
                }
            });
        }

        // 5. Unload distant chunks outside radius + 2
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
        }
    }

    /// Renders all currently loaded GPU chunk meshes.
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        for gpu_mesh in self.gpu_chunks.values() {
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                gpu_mesh.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..gpu_mesh.index_count, 0, 0..1);
        }
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
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x - 1, chunk_pos.0.y, chunk_pos.0.z));
        } else if lx == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x + 1, chunk_pos.0.y, chunk_pos.0.z));
        }
        if ly == 0 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y - 1, chunk_pos.0.z));
        } else if ly == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y + 1, chunk_pos.0.z));
        }
        if lz == 0 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y, chunk_pos.0.z - 1));
        } else if lz == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y, chunk_pos.0.z + 1));
        }

        let (broken_block, new_meshes): (BlockId, Vec<(ChunkPos, ChunkMesh)>) = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if current.is_air() {
                return None;
            }
            world.set_block(pos, BlockId::AIR);

            let mesher = ChunkMesher::new(&self.registry);
            let meshes = chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world.get_chunk(cpos).map(|chunk| {
                        let origin = cpos.to_world_origin();
                        let mesh = mesher.mesh_chunk_with_neighbors(chunk, |x, y, z| {
                            world.get_block(origin + IVec3::new(x, y, z))
                        });
                        (cpos, mesh)
                    })
                })
                .collect();
            (current, meshes)
        };

        for (cpos, mesh) in new_meshes {
            if let Some(gpu_mesh) = pipeline.upload_mesh(device, &mesh) {
                self.gpu_chunks.insert(cpos, gpu_mesh);
            } else {
                self.gpu_chunks.remove(&cpos);
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
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x - 1, chunk_pos.0.y, chunk_pos.0.z));
        } else if lx == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x + 1, chunk_pos.0.y, chunk_pos.0.z));
        }
        if ly == 0 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y - 1, chunk_pos.0.z));
        } else if ly == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y + 1, chunk_pos.0.z));
        }
        if lz == 0 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y, chunk_pos.0.z - 1));
        } else if lz == 15 {
            chunks_to_rebuild.push(ChunkPos::new(chunk_pos.0.x, chunk_pos.0.y, chunk_pos.0.z + 1));
        }

        let new_meshes: Vec<(ChunkPos, ChunkMesh)> = {
            let mut world = self.world.lock().unwrap();
            let current = world.get_block(pos);
            if !current.is_air() {
                return false;
            }
            world.set_block(pos, block_id);

            let mesher = ChunkMesher::new(&self.registry);
            chunks_to_rebuild
                .into_iter()
                .filter_map(|cpos| {
                    world.get_chunk(cpos).map(|chunk| {
                        let origin = cpos.to_world_origin();
                        let mesh = mesher.mesh_chunk_with_neighbors(chunk, |x, y, z| {
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

    #[allow(dead_code)]
    pub fn total_visible_faces(&self) -> usize {
        self.gpu_chunks.values().map(|g| (g.index_count / 6) as usize).sum()
    }
}
