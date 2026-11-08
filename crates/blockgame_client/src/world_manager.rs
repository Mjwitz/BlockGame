use std::collections::{HashMap, HashSet};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use blockgame_core::{
    block::BlockRegistry,
    chunk::{Chunk, ChunkPos},
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

    pub fn total_visible_faces(&self) -> usize {
        self.gpu_chunks.values().map(|g| (g.index_count / 6) as usize).sum()
    }
}
