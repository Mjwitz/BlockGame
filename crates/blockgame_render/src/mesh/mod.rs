pub mod vertex;

use blockgame_core::{
    animal::{AnimalEntity, AnimalKind},
    block::{BlockId, BlockRegistry},
    chunk::{Chunk, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    direction::Direction,
    entity::FallingBlock,
};
pub use vertex::VoxelVertex;

use crate::atlas::TextureAtlas;

pub struct ChunkMesh {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
    pub water_vertices: Vec<VoxelVertex>,
    pub water_indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            water_vertices: Vec::new(),
            water_indices: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty() && self.water_indices.is_empty()
    }

    pub fn face_count(&self) -> usize {
        (self.indices.len() + self.water_indices.len()) / 6
    }
}

/// Maps a world-space face direction to the block's local texture direction
/// given horizontal facing (`0 = PosZ`, `1 = NegZ`, `2 = PosX`, `3 = NegX`).
#[inline]
fn rotate_horizontal_dir(dir: Direction, facing: u8) -> Direction {
    match (facing, dir) {
        (1, Direction::PosZ) => Direction::NegZ,
        (1, Direction::NegZ) => Direction::PosZ,
        (1, Direction::PosX) => Direction::NegX,
        (1, Direction::NegX) => Direction::PosX,

        (2, Direction::PosX) => Direction::PosZ,
        (2, Direction::NegX) => Direction::NegZ,
        (2, Direction::NegZ) => Direction::PosX,
        (2, Direction::PosZ) => Direction::NegX,

        (3, Direction::NegX) => Direction::PosZ,
        (3, Direction::PosX) => Direction::NegZ,
        (3, Direction::PosZ) => Direction::PosX,
        (3, Direction::NegZ) => Direction::NegX,

        _ => dir,
    }
}

pub struct ChunkMesher<'a> {
    registry: &'a BlockRegistry,
}

impl<'a> ChunkMesher<'a> {
    pub fn new(registry: &'a BlockRegistry) -> Self {
        Self { registry }
    }

    /// Meshes a single standalone chunk (out-of-bounds voxels are treated as Air).
    /// Meshes a single standalone chunk (out-of-bounds voxels are treated as Air).
    pub fn mesh_chunk(&self, chunk: &Chunk) -> ChunkMesh {
        self.mesh_chunk_with_neighbors(chunk, |_, _, _| BlockId::AIR)
    }

    /// Meshes a chunk with a neighbor lookup callback for out-of-bounds coordinates.
    pub fn mesh_chunk_with_neighbors<F>(&self, chunk: &Chunk, get_neighbor: F) -> ChunkMesh
    where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        self.mesh_chunk_with_lighting(chunk, &[], get_neighbor)
    }

    /// Meshes a chunk with external light sources for cross-chunk flood fill lighting.
    pub fn mesh_chunk_with_lighting<F>(
        &self,
        chunk: &Chunk,
        light_sources_rel: &[(glam::IVec3, u8)],
        get_neighbor: F,
    ) -> ChunkMesh
    where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        self.mesh_chunk_with_lighting_opt(chunk, light_sources_rel, |x, y, z| {
            Some(get_neighbor(x, y, z))
        })
    }

    /// Meshes a chunk with external light sources and an optional neighbor lookup callback
    /// returning `None` for out-of-bounds coordinates belonging to currently unloaded chunks.
    pub fn mesh_chunk_with_lighting_opt<F>(
        &self,
        chunk: &Chunk,
        light_sources_rel: &[(glam::IVec3, u8)],
        get_neighbor_opt: F,
    ) -> ChunkMesh
    where
        F: Fn(i32, i32, i32) -> Option<BlockId>,
    {
        let mut mesh = ChunkMesh::new();

        let query_block_opt = |x: i32, y: i32, z: i32| -> Option<BlockId> {
            if x >= 0
                && x < CHUNK_SIZE_X as i32
                && y >= 0
                && y < CHUNK_SIZE_Y as i32
                && z >= 0
                && z < CHUNK_SIZE_Z as i32
            {
                Some(chunk.get_unchecked(x as usize, y as usize, z as usize))
            } else {
                get_neighbor_opt(x, y, z)
            }
        };

        let query_block = |x: i32, y: i32, z: i32| -> BlockId {
            query_block_opt(x, y, z).unwrap_or(BlockId::AIR)
        };

        let light_map = blockgame_core::lighting::calculate_chunk_lighting_with_sources(
            chunk,
            self.registry,
            light_sources_rel,
            &query_block,
        );

        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let block_id = chunk.get_unchecked(x, y, z);
                    if block_id.is_air() {
                        continue;
                    }

                    let ix = x as i32;
                    let iy = y as i32;
                    let iz = z as i32;

                    if block_id == BlockId::TORCH {
                        self.build_torch(&mut mesh, chunk.pos, ix, iy, iz);
                        continue;
                    }

                    if block_id == BlockId::WHEAT_0
                        || block_id == BlockId::WHEAT_1
                        || block_id == BlockId::WHEAT_2
                        || block_id == BlockId::WHEAT_3
                        || block_id == BlockId::TALL_GRASS
                        || block_id.is_flower()
                    {
                        self.build_crop(&mut mesh, chunk.pos, ix, iy, iz, block_id, &light_map);
                        continue;
                    }

                    if block_id == BlockId::LILY_PAD {
                        self.build_lily_pad(&mut mesh, chunk.pos, ix, iy, iz, &light_map);
                        continue;
                    }

                    let facing = chunk.get_facing_unchecked(x, y, z);

                    if block_id.is_stairs() {
                        self.build_stairs(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            block_id,
                            facing,
                            &light_map,
                        );
                        continue;
                    }

                    if block_id == BlockId::OAK_FENCE {
                        self.build_fence(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            &light_map,
                            &query_block,
                        );
                        continue;
                    }

                    if block_id.is_cable() {
                        self.build_copper_cable(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            block_id,
                            &light_map,
                            &query_block,
                        );
                        continue;
                    }

                    if block_id.is_solar_panel() {
                        self.build_solar_panel(&mut mesh, chunk.pos, ix, iy, iz, block_id, &light_map);
                        continue;
                    }

                    if block_id == BlockId::WIND_TURBINE {
                        self.build_wind_turbine(&mut mesh, chunk.pos, ix, iy, iz, facing, &light_map);
                        continue;
                    }

                    if block_id.is_door() {
                        self.build_door(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            block_id.is_door_open(),
                            block_id.is_door_top(),
                            facing,
                            &light_map,
                        );
                        continue;
                    }

                    if block_id.is_bed() {
                        let facing = chunk.get_facing_unchecked(ix as usize, iy as usize, iz as usize);
                        self.build_bed(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            block_id.is_bed_head(),
                            facing,
                            &light_map,
                        );
                        continue;
                    }

                    if block_id == BlockId::SEAGRASS {
                        // 1. Build crossed diagonal plant quads for seagrass
                        self.build_crop(&mut mesh, chunk.pos, ix, iy, iz, block_id, &light_map);

                        // 2. Seagrass is waterlogged! Generate water boundary faces for any loaded direction
                        // not adjacent to water or another waterlogged block.
                        for dir in Direction::ALL {
                            let normal = dir.normal_ivec();
                            let nx = ix + normal.x;
                            let ny = iy + normal.y;
                            let nz = iz + normal.z;

                            let Some(neighbor_id) = query_block_opt(nx, ny, nz) else {
                                // Neighbor chunk not loaded yet; avoid emitting phantom water walls at chunk borders
                                continue;
                            };

                            if neighbor_id.is_water() || neighbor_id == BlockId::ICE {
                                continue;
                            }

                            if self.registry.is_transparent(neighbor_id) {
                                let tile_id = self.registry.get_texture(BlockId::WATER, dir);
                                let uv = TextureAtlas::get_uv_bounds(tile_id);

                                let light = if nx >= 0
                                    && nx < CHUNK_SIZE_X as i32
                                    && ny >= 0
                                    && ny < CHUNK_SIZE_Y as i32
                                    && nz >= 0
                                    && nz < CHUNK_SIZE_Z as i32
                                {
                                    let lv = light_map.get(nx as usize, ny as usize, nz as usize);
                                    [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0]
                                } else {
                                    let blk = light_map.get_block_rel(nx, ny, nz);
                                    [1.0, blk as f32 / 15.0]
                                };

                                self.build_water_face(
                                    &mut mesh,
                                    chunk.pos,
                                    ix,
                                    iy,
                                    iz,
                                    dir,
                                    uv,
                                    light,
                                );
                            }
                        }
                        continue;
                    }

                    if block_id == BlockId::WATER {
                        for dir in Direction::ALL {
                            let normal = dir.normal_ivec();
                            let nx = ix + normal.x;
                            let ny = iy + normal.y;
                            let nz = iz + normal.z;

                            let Some(neighbor_id) = query_block_opt(nx, ny, nz) else {
                                // Neighbor chunk not loaded yet; avoid emitting phantom water walls at chunk borders
                                continue;
                            };

                            // Adjacent water blocks, waterlogged seagrass, and ice handle the shared boundary cleanly
                            if neighbor_id.is_water() || neighbor_id == BlockId::ICE {
                                continue;
                            }

                            // Only generate water face if adjacent block is transparent (air, tall grass, glass, etc.)
                            if self.registry.is_transparent(neighbor_id) {
                                let tile_id = self.registry.get_texture(block_id, dir);
                                let uv = TextureAtlas::get_uv_bounds(tile_id);

                                let light = if nx >= 0
                                    && nx < CHUNK_SIZE_X as i32
                                    && ny >= 0
                                    && ny < CHUNK_SIZE_Y as i32
                                    && nz >= 0
                                    && nz < CHUNK_SIZE_Z as i32
                                {
                                    let lv = light_map.get(nx as usize, ny as usize, nz as usize);
                                    [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0]
                                } else {
                                    let blk = light_map.get_block_rel(nx, ny, nz);
                                    [1.0, blk as f32 / 15.0]
                                };

                                self.build_water_face(
                                    &mut mesh,
                                    chunk.pos,
                                    ix,
                                    iy,
                                    iz,
                                    dir,
                                    uv,
                                    light,
                                );
                            }
                        }
                        continue;
                    }

                    if block_id == BlockId::ICE {
                        // Ice uses a semi-transparent texture and must render in the translucent pass
                        // (`water_pipeline`, depth_write_enabled = false) after all opaque geometry so
                        // looking through ice never culls terrain behind it.
                        for dir in Direction::ALL {
                            let normal = dir.normal_ivec();
                            let nx = ix + normal.x;
                            let ny = iy + normal.y;
                            let nz = iz + normal.z;

                            let Some(neighbor_id) = query_block_opt(nx, ny, nz) else {
                                continue;
                            };

                            // Adjacent ice blocks should not render internal faces between them
                            if neighbor_id == BlockId::ICE {
                                continue;
                            }

                            if self.registry.is_transparent(neighbor_id) {
                                let tex_dir = rotate_horizontal_dir(dir, facing);
                                let tile_id = self.registry.get_texture(block_id, tex_dir);
                                let uv = TextureAtlas::get_uv_bounds(tile_id);

                                let light = if nx >= 0
                                    && nx < CHUNK_SIZE_X as i32
                                    && ny >= 0
                                    && ny < CHUNK_SIZE_Y as i32
                                    && nz >= 0
                                    && nz < CHUNK_SIZE_Z as i32
                                {
                                    let lv = light_map.get(nx as usize, ny as usize, nz as usize);
                                    [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0]
                                } else {
                                    let blk = light_map.get_block_rel(nx, ny, nz);
                                    [1.0, blk as f32 / 15.0]
                                };

                                self.build_water_face(
                                    &mut mesh,
                                    chunk.pos,
                                    ix,
                                    iy,
                                    iz,
                                    dir,
                                    uv,
                                    light,
                                );
                            }
                        }
                        continue;
                    }

                    for dir in Direction::ALL {
                        let normal = dir.normal_ivec();
                        let nx = ix + normal.x;
                        let ny = iy + normal.y;
                        let nz = iz + normal.z;

                        let neighbor_id = query_block(nx, ny, nz);

                        // Adjacent glass blocks should not render the internal face between them
                        if block_id == BlockId::GLASS && neighbor_id == BlockId::GLASS {
                            continue;
                        }

                        // Only generate face if the neighbor block is transparent
                        if self.registry.is_transparent(neighbor_id) {
                            let tex_dir = rotate_horizontal_dir(dir, facing);
                            let tile_id = self.registry.get_texture(block_id, tex_dir);
                            let uv = TextureAtlas::get_uv_bounds(tile_id);

                            // Sample light from adjacent air block (supports boundary voxels from [-1..16])
                            let light = if nx >= 0
                                && nx < CHUNK_SIZE_X as i32
                                && ny >= 0
                                && ny < CHUNK_SIZE_Y as i32
                                && nz >= 0
                                && nz < CHUNK_SIZE_Z as i32
                            {
                                let lv = light_map.get(nx as usize, ny as usize, nz as usize);
                                [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0]
                            } else {
                                let blk = light_map.get_block_rel(nx, ny, nz);
                                [1.0, blk as f32 / 15.0]
                            };

                            self.build_face(
                                &mut mesh,
                                chunk.pos,
                                ix,
                                iy,
                                iz,
                                dir,
                                uv,
                                light,
                                &query_block,
                            );
                        }
                    }
                }
            }
        }

        mesh
    }

    fn build_face<F>(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        dir: Direction,
        uv_bounds: [f32; 4],
        light: [f32; 2],
        query: &F,
    ) where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        let [u_min, v_min, u_max, v_max] = uv_bounds;
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;
        let normal = dir.normal_f32();

        // 4 corner positions for the quad
        let positions = match dir {
            Direction::PosY => [
                [fx, fy + 1.0, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz],
                [fx, fy + 1.0, fz],
            ],
            Direction::NegY => [
                [fx, fy, fz],
                [fx + 1.0, fy, fz],
                [fx + 1.0, fy, fz + 1.0],
                [fx, fy, fz + 1.0],
            ],
            Direction::PosZ => [
                [fx, fy, fz + 1.0],
                [fx + 1.0, fy, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz + 1.0],
                [fx, fy + 1.0, fz + 1.0],
            ],
            Direction::NegZ => [
                [fx + 1.0, fy, fz],
                [fx, fy, fz],
                [fx, fy + 1.0, fz],
                [fx + 1.0, fy + 1.0, fz],
            ],
            Direction::PosX => [
                [fx + 1.0, fy, fz + 1.0],
                [fx + 1.0, fy, fz],
                [fx + 1.0, fy + 1.0, fz],
                [fx + 1.0, fy + 1.0, fz + 1.0],
            ],
            Direction::NegX => [
                [fx, fy, fz],
                [fx, fy, fz + 1.0],
                [fx, fy + 1.0, fz + 1.0],
                [fx, fy + 1.0, fz],
            ],
        };

        let uvs = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];

        let ao = self.calculate_face_ao(x, y, z, dir, query);

        let start_idx = mesh.vertices.len() as u32;

        for i in 0..4 {
            mesh.vertices.push(VoxelVertex {
                position: positions[i],
                uv: uvs[i],
                normal,
                ao: ao[i],
                light,
            });
        }

        // Quad triangulation:
        // Flip diagonal if ao[0] + ao[2] < ao[1] + ao[3] to prevent anisotropy
        if ao[0] + ao[2] < ao[1] + ao[3] {
            mesh.indices.extend_from_slice(&[
                start_idx + 1,
                start_idx + 2,
                start_idx + 3,
                start_idx + 3,
                start_idx + 0,
                start_idx + 1,
            ]);
        } else {
            mesh.indices.extend_from_slice(&[
                start_idx + 0,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx + 0,
            ]);
        }
    }

    fn build_water_face(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        dir: Direction,
        uv_bounds: [f32; 4],
        light: [f32; 2],
    ) {
        let [u_min, v_min, u_max, v_max] = uv_bounds;
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;
        let normal = dir.normal_f32();

        let positions = match dir {
            Direction::PosY => [
                [fx, fy + 1.0, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz],
                [fx, fy + 1.0, fz],
            ],
            Direction::NegY => [
                [fx, fy, fz],
                [fx + 1.0, fy, fz],
                [fx + 1.0, fy, fz + 1.0],
                [fx, fy, fz + 1.0],
            ],
            Direction::PosZ => [
                [fx, fy, fz + 1.0],
                [fx + 1.0, fy, fz + 1.0],
                [fx + 1.0, fy + 1.0, fz + 1.0],
                [fx, fy + 1.0, fz + 1.0],
            ],
            Direction::NegZ => [
                [fx + 1.0, fy, fz],
                [fx, fy, fz],
                [fx, fy + 1.0, fz],
                [fx + 1.0, fy + 1.0, fz],
            ],
            Direction::PosX => [
                [fx + 1.0, fy, fz + 1.0],
                [fx + 1.0, fy, fz],
                [fx + 1.0, fy + 1.0, fz],
                [fx + 1.0, fy + 1.0, fz + 1.0],
            ],
            Direction::NegX => [
                [fx, fy, fz],
                [fx, fy, fz + 1.0],
                [fx, fy + 1.0, fz + 1.0],
                [fx, fy + 1.0, fz],
            ],
        };

        let uvs = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];

        let start_idx = mesh.water_vertices.len() as u32;

        for i in 0..4 {
            mesh.water_vertices.push(VoxelVertex {
                position: positions[i],
                uv: uvs[i],
                normal,
                ao: 1.0,
                light,
            });
        }

        mesh.water_indices.extend_from_slice(&[
            start_idx + 0,
            start_idx + 1,
            start_idx + 2,
            start_idx + 2,
            start_idx + 3,
            start_idx + 0,
        ]);
    }

    /// Generates slender 3D post mesh for torches centered in the block.
    fn build_torch(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let x0 = fx + 7.0 / 16.0;
        let x1 = fx + 9.0 / 16.0;
        let z0 = fz + 7.0 / 16.0;
        let z1 = fz + 9.0 / 16.0;
        let y0 = fy;
        let y1 = fy + 10.0 / 16.0;

        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(13);
        let u_width = u_max - u_min;
        let v_height = v_max - v_min;

        let u_side_left = u_min + u_width * (7.0 / 16.0);
        let u_side_right = u_min + u_width * (9.0 / 16.0);
        let v_side_top = v_min + v_height * (6.0 / 16.0);
        let v_side_bottom = v_min + v_height * (16.0 / 16.0);

        let u_top_left = u_min + u_width * (7.0 / 16.0);
        let u_top_right = u_min + u_width * (9.0 / 16.0);
        let v_top_min = v_min + v_height * (6.0 / 16.0);
        let v_top_max = v_min + v_height * (8.0 / 16.0);

        let v_bottom_min = v_min + v_height * (14.0 / 16.0);
        let v_bottom_max = v_min + v_height * (16.0 / 16.0);

        let light = [1.0, 1.0]; // Self-illuminating warm torch glow
        let ao = 1.0;

        let mut add_quad = |positions: [[f32; 3]; 4], uvs: [[f32; 2]; 4], normal: [f32; 3]| {
            let start_idx = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: positions[i],
                    uv: uvs[i],
                    normal,
                    ao,
                    light,
                });
            }
            mesh.indices.extend_from_slice(&[
                start_idx + 0,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx + 0,
            ]);
        };

        // 1. South Face (+Z)
        add_quad(
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            [
                [u_side_left, v_side_bottom],
                [u_side_right, v_side_bottom],
                [u_side_right, v_side_top],
                [u_side_left, v_side_top],
            ],
            [0.0, 0.0, 1.0],
        );

        // 2. North Face (-Z)
        add_quad(
            [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            [
                [u_side_right, v_side_bottom],
                [u_side_left, v_side_bottom],
                [u_side_left, v_side_top],
                [u_side_right, v_side_top],
            ],
            [0.0, 0.0, -1.0],
        );

        // 3. East Face (+X)
        add_quad(
            [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            [
                [u_side_left, v_side_bottom],
                [u_side_right, v_side_bottom],
                [u_side_right, v_side_top],
                [u_side_left, v_side_top],
            ],
            [1.0, 0.0, 0.0],
        );

        // 4. West Face (-X)
        add_quad(
            [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            [
                [u_side_left, v_side_bottom],
                [u_side_right, v_side_bottom],
                [u_side_right, v_side_top],
                [u_side_left, v_side_top],
            ],
            [-1.0, 0.0, 0.0],
        );

        // 5. Top Face (+Y)
        add_quad(
            [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
            [
                [u_top_left, v_top_max],
                [u_top_right, v_top_max],
                [u_top_right, v_top_min],
                [u_top_left, v_top_min],
            ],
            [0.0, 1.0, 0.0],
        );

        // 6. Bottom Face (-Y)
        add_quad(
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            [
                [u_top_left, v_bottom_min],
                [u_top_right, v_bottom_min],
                [u_top_right, v_bottom_max],
                [u_top_left, v_bottom_max],
            ],
            [0.0, -1.0, 0.0],
        );
    }

    /// Generates crossed diagonal plant quads for wheat crops.
    fn build_crop(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        block_id: BlockId,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile_id = self.registry.get_texture(block_id, Direction::PosY);
        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);

        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];
        let ao = 1.0;

        let mut add_quad = |positions: [[f32; 3]; 4], uvs: [[f32; 2]; 4], normal: [f32; 3]| {
            let start_idx = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: positions[i],
                    uv: uvs[i],
                    normal,
                    ao,
                    light,
                });
            }
            mesh.indices.extend_from_slice(&[
                start_idx + 0,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx + 0,
            ]);
        };

        // Crossed diagonal 1 (from (fx, fz) to (fx+1, fz+1))
        let diag1_front = [
            [fx, fy, fz],
            [fx + 1.0, fy, fz + 1.0],
            [fx + 1.0, fy + 1.0, fz + 1.0],
            [fx, fy + 1.0, fz],
        ];
        let diag1_back = [
            [fx + 1.0, fy, fz + 1.0],
            [fx, fy, fz],
            [fx, fy + 1.0, fz],
            [fx + 1.0, fy + 1.0, fz + 1.0],
        ];
        let uv_front = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];
        let uv_back = [
            [u_max, v_max],
            [u_min, v_max],
            [u_min, v_min],
            [u_max, v_min],
        ];

        add_quad(diag1_front, uv_front, [0.707, 0.0, -0.707]);
        add_quad(diag1_back, uv_back, [-0.707, 0.0, 0.707]);

        // Crossed diagonal 2 (from (fx+1, fz) to (fx, fz+1))
        let diag2_front = [
            [fx, fy, fz + 1.0],
            [fx + 1.0, fy, fz],
            [fx + 1.0, fy + 1.0, fz],
            [fx, fy + 1.0, fz + 1.0],
        ];
        let diag2_back = [
            [fx + 1.0, fy, fz],
            [fx, fy, fz + 1.0],
            [fx, fy + 1.0, fz + 1.0],
            [fx + 1.0, fy + 1.0, fz],
        ];
        add_quad(diag2_front, uv_front, [0.707, 0.0, 0.707]);
        add_quad(diag2_back, uv_back, [-0.707, 0.0, -0.707]);
    }

    /// Generates flat horizontal two-sided water surface quad for lily pads.
    fn build_lily_pad(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32 + 0.04;
        let fz = (origin.z + z) as f32;

        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(206);
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        let base_index = mesh.vertices.len() as u32;
        let positions = [
            [fx, fy, fz + 1.0],
            [fx + 1.0, fy, fz + 1.0],
            [fx + 1.0, fy, fz],
            [fx, fy, fz],
        ];
        let uvs = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];

        for i in 0..4 {
            mesh.vertices.push(VoxelVertex {
                position: positions[i],
                uv: uvs[i],
                normal: [0.0, 1.0, 0.0],
                ao: 1.0,
                light,
            });
        }

        mesh.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
            // Back face
            base_index + 2,
            base_index + 1,
            base_index,
            base_index + 3,
            base_index + 2,
            base_index,
        ]);
    }

    #[allow(clippy::too_many_arguments)]
    fn add_box(
        &self,
        mesh: &mut ChunkMesh,
        x0: f32,
        y0: f32,
        z0: f32,
        x1: f32,
        y1: f32,
        z1: f32,
        tiles: [u16; 6], // [South +Z, North -Z, East +X, West -X, Top +Y, Bottom -Y]
        light: [f32; 2],
    ) {
        let mut add_quad =
            |positions: [[f32; 3]; 4], normal: [f32; 3], tile_id: u16, ao: f32| {
                let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);
                let uvs = [
                    [u_min, v_max],
                    [u_max, v_max],
                    [u_max, v_min],
                    [u_min, v_min],
                ];
                let start_idx = mesh.vertices.len() as u32;
                for i in 0..4 {
                    mesh.vertices.push(VoxelVertex {
                        position: positions[i],
                        uv: uvs[i],
                        normal,
                        ao,
                        light,
                    });
                }
                mesh.indices.extend_from_slice(&[
                    start_idx,
                    start_idx + 1,
                    start_idx + 2,
                    start_idx + 2,
                    start_idx + 3,
                    start_idx,
                ]);
            };

        // South (+Z)
        add_quad(
            [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
            [0.0, 0.0, 1.0],
            tiles[0],
            0.92,
        );
        // North (-Z)
        add_quad(
            [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            [0.0, 0.0, -1.0],
            tiles[1],
            0.92,
        );
        // East (+X)
        add_quad(
            [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            [1.0, 0.0, 0.0],
            tiles[2],
            0.85,
        );
        // West (-X)
        add_quad(
            [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            [-1.0, 0.0, 0.0],
            tiles[3],
            0.85,
        );
        // Top (+Y)
        add_quad(
            [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
            [0.0, 1.0, 0.0],
            tiles[4],
            1.0,
        );
        // Bottom (-Y)
        add_quad(
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            [0.0, -1.0, 0.0],
            tiles[5],
            0.75,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn build_stairs(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        block_id: BlockId,
        facing: u8,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile = self.registry.get_texture(block_id, Direction::PosY);
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // Bottom half slab
        self.add_box(
            mesh,
            fx,
            fy,
            fz,
            fx + 1.0,
            fy + 0.5,
            fz + 1.0,
            [tile; 6],
            light,
        );
        // Upper back step (opposite to facing so the stairs face toward the player)
        let (ux0, uz0, ux1, uz1) = match facing {
            1 => (0.0, 0.5, 1.0, 1.0), // Facing NegZ -> back at +Z
            2 => (0.0, 0.0, 0.5, 1.0), // Facing PosX -> back at -X
            3 => (0.5, 0.0, 1.0, 1.0), // Facing NegX -> back at +X
            _ => (0.0, 0.0, 1.0, 0.5), // Facing PosZ (default) -> back at -Z
        };
        self.add_box(
            mesh,
            fx + ux0,
            fy + 0.5,
            fz + uz0,
            fx + ux1,
            fy + 1.0,
            fz + uz1,
            [tile; 6],
            light,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn build_fence<F>(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        light_map: &blockgame_core::lighting::ChunkLightMap,
        query_block: &F,
    ) where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile = self.registry.get_texture(BlockId::OAK_FENCE, Direction::PosY);
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // Center post (4/16 = 0.25 wide, centered at 0.375..0.625)
        self.add_box(
            mesh,
            fx + 0.375,
            fy,
            fz + 0.375,
            fx + 0.625,
            fy + 1.0,
            fz + 0.625,
            [tile; 6],
            light,
        );

        let connects = |nx: i32, ny: i32, nz: i32| -> bool {
            let id = query_block(nx, ny, nz);
            id == BlockId::OAK_FENCE || self.registry.is_solid(id)
        };

        // +X rail
        if connects(x + 1, y, z) {
            self.add_box(mesh, fx + 0.625, fy + 0.75, fz + 0.4375, fx + 1.0, fy + 0.9375, fz + 0.5625, [tile; 6], light);
            self.add_box(mesh, fx + 0.625, fy + 0.375, fz + 0.4375, fx + 1.0, fy + 0.5625, fz + 0.5625, [tile; 6], light);
        }
        // -X rail
        if connects(x - 1, y, z) {
            self.add_box(mesh, fx, fy + 0.75, fz + 0.4375, fx + 0.375, fy + 0.9375, fz + 0.5625, [tile; 6], light);
            self.add_box(mesh, fx, fy + 0.375, fz + 0.4375, fx + 0.375, fy + 0.5625, fz + 0.5625, [tile; 6], light);
        }
        // +Z rail
        if connects(x, y, z + 1) {
            self.add_box(mesh, fx + 0.4375, fy + 0.75, fz + 0.625, fx + 0.5625, fy + 0.9375, fz + 1.0, [tile; 6], light);
            self.add_box(mesh, fx + 0.4375, fy + 0.375, fz + 0.625, fx + 0.5625, fy + 0.5625, fz + 1.0, [tile; 6], light);
        }
        // -Z rail
        if connects(x, y, z - 1) {
            self.add_box(mesh, fx + 0.4375, fy + 0.75, fz, fx + 0.5625, fy + 0.9375, fz + 0.375, [tile; 6], light);
            self.add_box(mesh, fx + 0.4375, fy + 0.375, fz, fx + 0.5625, fy + 0.5625, fz + 0.375, [tile; 6], light);
        }
    }

    fn build_copper_cable<F>(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        block_id: BlockId,
        light_map: &blockgame_core::lighting::ChunkLightMap,
        query_block: &F,
    ) where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile = match block_id {
            BlockId::GOLD_CABLE => 434u16,
            BlockId::DIAMOND_CABLE => 435u16,
            _ => 410u16,
        };
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // Central junction hub (0.36..0.64)
        self.add_box(mesh, fx + 0.36, fy + 0.36, fz + 0.36, fx + 0.64, fy + 0.64, fz + 0.64, [tile; 6], light);

        let connects = |nx: i32, ny: i32, nz: i32| -> bool {
            query_block(nx, ny, nz).is_power_block()
        };

        // 6-way automatic conduit arms (0.40..0.60)
        if connects(x + 1, y, z) {
            self.add_box(mesh, fx + 0.64, fy + 0.40, fz + 0.40, fx + 1.0, fy + 0.60, fz + 0.60, [tile; 6], light);
        }
        if connects(x - 1, y, z) {
            self.add_box(mesh, fx, fy + 0.40, fz + 0.40, fx + 0.36, fy + 0.60, fz + 0.60, [tile; 6], light);
        }
        if connects(x, y + 1, z) {
            self.add_box(mesh, fx + 0.40, fy + 0.64, fz + 0.40, fx + 0.60, fy + 1.0, fz + 0.60, [tile; 6], light);
        }
        if connects(x, y - 1, z) {
            self.add_box(mesh, fx + 0.40, fy, fz + 0.40, fx + 0.60, fy + 0.36, fz + 0.60, [tile; 6], light);
        }
        if connects(x, y, z + 1) {
            self.add_box(mesh, fx + 0.40, fy + 0.40, fz + 0.64, fx + 0.60, fy + 0.60, fz + 1.0, [tile; 6], light);
        }
        if connects(x, y, z - 1) {
            self.add_box(mesh, fx + 0.40, fy + 0.40, fz, fx + 0.60, fy + 0.60, fz + 0.36, [tile; 6], light);
        }
    }

    fn build_solar_panel(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        block_id: BlockId,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        let (top_tile, side_tile) = if block_id == BlockId::ADVANCED_SOLAR_PANEL {
            (428u16, 429u16)
        } else {
            (413u16, 414u16)
        };

        // add_box face order: [South +Z, North -Z, East +X, West -X, Top +Y, Bottom -Y]
        // Support pedestal base
        self.add_box(
            mesh,
            fx + 0.30,
            fy,
            fz + 0.30,
            fx + 0.70,
            fy + 0.22,
            fz + 0.70,
            [side_tile, side_tile, side_tile, side_tile, 425, 425],
            light,
        );
        // Slim photovoltaic panel slab on top (solar cell grid on Top +Y!)
        self.add_box(
            mesh,
            fx + 0.03,
            fy + 0.22,
            fz + 0.03,
            fx + 0.97,
            fy + 0.38,
            fz + 0.97,
            [side_tile, side_tile, side_tile, side_tile, top_tile, 425],
            light,
        );
    }

    fn build_wind_turbine(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        facing: u8,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // add_box face order: [South +Z, North -Z, East +X, West -X, Top +Y, Bottom -Y]
        // 1. Bottom power port base plinth (copper power terminal on Bottom -Y and side ports)
        self.add_box(
            mesh,
            fx + 0.16,
            fy,
            fz + 0.16,
            fx + 0.84,
            fy + 0.34,
            fz + 0.84,
            [414, 414, 414, 414, 425, 410],
            light,
        );
        // Copper power output collar right at the bottom center
        self.add_box(
            mesh,
            fx + 0.34,
            fy,
            fz + 0.34,
            fx + 0.66,
            fy + 0.48,
            fz + 0.66,
            [410, 410, 410, 410, 425, 410],
            light,
        );
        // 2. Tall tapered 4-block steel mast (lower shaft + upper shaft)
        self.add_box(
            mesh,
            fx + 0.36,
            fy + 0.48,
            fz + 0.36,
            fx + 0.64,
            fy + 2.05,
            fz + 0.64,
            [416, 416, 416, 416, 416, 425],
            light,
        );
        self.add_box(
            mesh,
            fx + 0.40,
            fy + 2.05,
            fz + 0.40,
            fx + 0.60,
            fy + 3.52,
            fz + 0.60,
            [416, 416, 416, 416, 416, 425],
            light,
        );
        // 3. Top aerodynamic nacelle housing oriented to match block facing (0=+Z, 1=-Z, 2=+X, 3=-X)
        let (nx0, nz0, nx1, nz1, nac_tiles) = match facing & 0x03 {
            1 => (0.28, 0.16, 0.72, 0.86, [416, 415, 416, 416, 416, 425]), // North (-Z)
            2 => (0.14, 0.28, 0.84, 0.72, [416, 416, 415, 416, 416, 425]), // East (+X)
            3 => (0.16, 0.28, 0.86, 0.72, [416, 416, 416, 415, 416, 425]), // West (-X)
            _ => (0.28, 0.14, 0.72, 0.84, [415, 416, 416, 416, 416, 425]), // South (+Z)
        };
        self.add_box(
            mesh,
            fx + nx0,
            fy + 3.46,
            fz + nz0,
            fx + nx1,
            fy + 3.98,
            fz + nz1,
            nac_tiles,
            light,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn build_door(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        is_open: bool,
        is_top: bool,
        facing: u8,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile = if is_top { 333 } else { 334 };
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        let (x0, z0, x1, z1) = match (facing, is_open) {
            // Facing PosZ (+Z) or NegZ (-Z): closed spans X, open swings onto X edge
            (0, false) => (0.0, 0.8125, 1.0, 1.0),
            (1, false) => (0.0, 0.0, 1.0, 0.1875),
            (0, true) | (1, true) => (0.0, 0.0, 0.1875, 1.0),
            // Facing PosX (+X) or NegX (-X): closed spans Z, open swings onto Z edge
            (2, false) => (0.8125, 0.0, 1.0, 1.0),
            (3, false) => (0.0, 0.0, 0.1875, 1.0),
            (2, true) | (3, true) => (0.0, 0.0, 1.0, 0.1875),
            _ => (0.0, 0.8125, 1.0, 1.0),
        };
        self.add_box(
            mesh,
            fx + x0,
            fy,
            fz + z0,
            fx + x1,
            fy + 1.0,
            fz + z1,
            [tile; 6],
            light,
        );
    }

    fn build_bed(
        &self,
        mesh: &mut ChunkMesh,
        chunk_pos: blockgame_core::chunk::ChunkPos,
        x: i32,
        y: i32,
        z: i32,
        is_head: bool,
        facing: u8,
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // Mattress & frame bounds: flush at the middle seam between foot and head
        // facing: 0=PosZ (foot +Z, head -Z), 1=NegZ (foot -Z, head +Z), 2=PosX (foot +X, head -X), 3=NegX (foot -X, head +X)
        let (x0, x1, z0, z1) = match (facing, is_head) {
            (0, false) => (0.05, 0.95, 0.0, 0.95),
            (0, true) => (0.05, 0.95, 0.05, 1.0),
            (1, false) => (0.05, 0.95, 0.05, 1.0),
            (1, true) => (0.05, 0.95, 0.0, 0.95),
            (2, false) => (0.0, 0.95, 0.05, 0.95),
            (2, true) => (0.05, 1.0, 0.05, 0.95),
            (3, false) => (0.05, 1.0, 0.05, 0.95),
            (3, true) => (0.0, 0.95, 0.05, 0.95),
            _ => (0.05, 0.95, 0.0, 0.95),
        };

        // Seam face direction to cull (direction pointing to the other half of the bed)
        // facing: 0=PosZ (foot +Z, head -Z), 1=NegZ (foot -Z, head +Z), 2=PosX (foot +X, head -X), 3=NegX (foot -X, head +X)
        let seam_dir = match (facing, is_head) {
            (0, false) => Direction::NegZ,
            (0, true) => Direction::PosZ,
            (1, false) => Direction::PosZ,
            (1, true) => Direction::NegZ,
            (2, false) => Direction::NegX,
            (2, true) => Direction::PosX,
            (3, false) => Direction::PosX,
            (3, true) => Direction::NegX,
            _ => Direction::NegZ,
        };

        let bx0 = fx + x0;
        let by0 = fy + 0.1875;
        let bz0 = fz + z0;
        let bx1 = fx + x1;
        let by1 = fy + 0.5625;
        let bz1 = fz + z1;

        let mut add_quad = |positions: [[f32; 3]; 4], normal: [f32; 3], uvs: [[f32; 2]; 4], ao: f32| {
            let start_idx = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: positions[i],
                    uv: uvs[i],
                    normal,
                    ao,
                    light,
                });
            }
            mesh.indices.extend_from_slice(&[
                start_idx,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx,
            ]);
        };

        let get_default_uvs = |tile_id: u16| {
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);
            [
                [u_min, v_max],
                [u_max, v_max],
                [u_max, v_min],
                [u_min, v_min],
            ]
        };

        // South (+Z)
        if seam_dir != Direction::PosZ {
            add_quad(
                [[bx0, by0, bz1], [bx1, by0, bz1], [bx1, by1, bz1], [bx0, by1, bz1]],
                [0.0, 0.0, 1.0],
                get_default_uvs(318),
                0.92,
            );
        }
        // North (-Z)
        if seam_dir != Direction::NegZ {
            add_quad(
                [[bx1, by0, bz0], [bx0, by0, bz0], [bx0, by1, bz0], [bx1, by1, bz0]],
                [0.0, 0.0, -1.0],
                get_default_uvs(318),
                0.92,
            );
        }
        // East (+X)
        if seam_dir != Direction::PosX {
            add_quad(
                [[bx1, by0, bz1], [bx1, by0, bz0], [bx1, by1, bz0], [bx1, by1, bz1]],
                [1.0, 0.0, 0.0],
                get_default_uvs(318),
                0.85,
            );
        }
        // West (-X)
        if seam_dir != Direction::NegX {
            add_quad(
                [[bx0, by0, bz0], [bx0, by0, bz1], [bx0, by1, bz1], [bx0, by1, bz0]],
                [-1.0, 0.0, 0.0],
                get_default_uvs(318),
                0.85,
            );
        }
        // Bottom (-Y): Oak Planks
        add_quad(
            [[bx0, by0, bz0], [bx1, by0, bz0], [bx1, by0, bz1], [bx0, by0, bz1]],
            [0.0, -1.0, 0.0],
            get_default_uvs(10),
            0.75,
        );
        // Top (+Y): Head (342) or Foot (343)
        let top_tile = if is_head { 342 } else { 343 };
        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(top_tile);
        // Orient pillow (v_min in tile 342) towards the head of the bed
        let top_uvs = match facing {
            1 => [
                [u_min, v_min],
                [u_max, v_min],
                [u_max, v_max],
                [u_min, v_max],
            ],
            2 => [
                [u_max, v_min],
                [u_max, v_max],
                [u_min, v_max],
                [u_min, v_min],
            ],
            3 => [
                [u_min, v_max],
                [u_min, v_min],
                [u_max, v_min],
                [u_max, v_max],
            ],
            _ => [
                [u_min, v_max],
                [u_max, v_max],
                [u_max, v_min],
                [u_min, v_min],
            ],
        };
        add_quad(
            [[bx0, by1, bz1], [bx1, by1, bz1], [bx1, by1, bz0], [bx0, by1, bz0]],
            [0.0, 1.0, 0.0],
            top_uvs,
            1.0,
        );

        // 2 outer corner oak legs on this half (using Oak Planks tile 10)
        let leg_coords: [(f32, f32, f32, f32); 2] = match (facing, is_head) {
            (0, false) | (1, true) => [(0.05, 0.22, 0.78, 0.95), (0.78, 0.95, 0.78, 0.95)],
            (0, true) | (1, false) => [(0.05, 0.22, 0.05, 0.22), (0.78, 0.95, 0.05, 0.22)],
            (2, false) | (3, true) => [(0.78, 0.95, 0.05, 0.22), (0.78, 0.95, 0.78, 0.95)],
            _ => [(0.05, 0.22, 0.05, 0.22), (0.05, 0.22, 0.78, 0.95)],
        };
        for (lx0, lx1, lz0, lz1) in leg_coords {
            self.add_box(
                mesh,
                fx + lx0,
                fy,
                fz + lz0,
                fx + lx1,
                fy + 0.1875,
                fz + lz1,
                [10; 6],
                light,
            );
        }
    }

    fn calculate_face_ao<F>(&self, x: i32, y: i32, z: i32, dir: Direction, query: &F) -> [f32; 4]
    where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        // Query neighbor occlusion: true if block is solid/opaque
        let is_occ = |dx: i32, dy: i32, dz: i32| -> bool {
            let id = query(x + dx, y + dy, z + dz);
            self.registry.is_solid(id)
        };

        // For each direction, corner neighbors (side1, side2, corner) for vertices 0..3
        let ao_values = match dir {
            Direction::PosY => [
                vertex_ao(is_occ(-1, 1, 0), is_occ(0, 1, 1), is_occ(-1, 1, 1)),
                vertex_ao(is_occ(1, 1, 0), is_occ(0, 1, 1), is_occ(1, 1, 1)),
                vertex_ao(is_occ(1, 1, 0), is_occ(0, 1, -1), is_occ(1, 1, -1)),
                vertex_ao(is_occ(-1, 1, 0), is_occ(0, 1, -1), is_occ(-1, 1, -1)),
            ],
            Direction::NegY => [
                vertex_ao(is_occ(-1, -1, 0), is_occ(0, -1, -1), is_occ(-1, -1, -1)),
                vertex_ao(is_occ(1, -1, 0), is_occ(0, -1, -1), is_occ(1, -1, -1)),
                vertex_ao(is_occ(1, -1, 0), is_occ(0, -1, 1), is_occ(1, -1, 1)),
                vertex_ao(is_occ(-1, -1, 0), is_occ(0, -1, 1), is_occ(-1, -1, 1)),
            ],
            Direction::PosZ => [
                vertex_ao(is_occ(-1, 0, 1), is_occ(0, -1, 1), is_occ(-1, -1, 1)),
                vertex_ao(is_occ(1, 0, 1), is_occ(0, -1, 1), is_occ(1, -1, 1)),
                vertex_ao(is_occ(1, 0, 1), is_occ(0, 1, 1), is_occ(1, 1, 1)),
                vertex_ao(is_occ(-1, 0, 1), is_occ(0, 1, 1), is_occ(-1, 1, 1)),
            ],
            Direction::NegZ => [
                vertex_ao(is_occ(1, 0, -1), is_occ(0, -1, -1), is_occ(1, -1, -1)),
                vertex_ao(is_occ(-1, 0, -1), is_occ(0, -1, -1), is_occ(-1, -1, -1)),
                vertex_ao(is_occ(-1, 0, -1), is_occ(0, 1, -1), is_occ(-1, 1, -1)),
                vertex_ao(is_occ(1, 0, -1), is_occ(0, 1, -1), is_occ(1, 1, -1)),
            ],
            Direction::PosX => [
                vertex_ao(is_occ(1, 0, 1), is_occ(1, -1, 0), is_occ(1, -1, 1)),
                vertex_ao(is_occ(1, 0, -1), is_occ(1, -1, 0), is_occ(1, -1, -1)),
                vertex_ao(is_occ(1, 0, -1), is_occ(1, 1, 0), is_occ(1, 1, -1)),
                vertex_ao(is_occ(1, 0, 1), is_occ(1, 1, 0), is_occ(1, 1, 1)),
            ],
            Direction::NegX => [
                vertex_ao(is_occ(-1, 0, -1), is_occ(-1, -1, 0), is_occ(-1, -1, -1)),
                vertex_ao(is_occ(-1, 0, 1), is_occ(-1, -1, 0), is_occ(-1, -1, 1)),
                vertex_ao(is_occ(-1, 0, 1), is_occ(-1, 1, 0), is_occ(-1, 1, 1)),
                vertex_ao(is_occ(-1, 0, -1), is_occ(-1, 1, 0), is_occ(-1, 1, -1)),
            ],
        };

        [
            ao_factor(ao_values[0]),
            ao_factor(ao_values[1]),
            ao_factor(ao_values[2]),
            ao_factor(ao_values[3]),
        ]
    }
}

#[inline]
fn vertex_ao(side1: bool, side2: bool, corner: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

#[inline]
fn ao_factor(ao_level: u8) -> f32 {
    match ao_level {
        0 => 0.45,
        1 => 0.65,
        2 => 0.85,
        _ => 1.00,
    }
}

/// Builds a dynamic 3D mesh for dropped items floating and spinning in the world.
pub fn build_item_drops_mesh(
    item_drops: &[blockgame_core::entity::ItemDrop],
    item_registry: &blockgame_core::item::ItemRegistry,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();

    for drop in item_drops {
        let def = match item_registry.get(drop.item.item_id) {
            Some(d) => d,
            None => continue,
        };

        let center = drop.position + glam::Vec3::new(0.0, 0.15 + drop.visual_bob_y(), 0.0);
        let s = 0.13; // half size -> 0.26 block size
        let angle = drop.age * 2.5;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Rotate (dx, dz) around Y
        let rotate_point = |dx: f32, dy: f32, dz: f32| -> [f32; 3] {
            let rx = dx * cos_a - dz * sin_a;
            let rz = dx * sin_a + dz * cos_a;
            [center.x + rx, center.y + dy, center.z + rz]
        };

        let rotate_normal = |nx: f32, ny: f32, nz: f32| -> [f32; 3] {
            let rx = nx * cos_a - nz * sin_a;
            let rz = nx * sin_a + nz * cos_a;
            [rx, ny, rz]
        };

        if drop.item.item_id == blockgame_core::item::ItemId::TORCH {
            // Slender 3D Torch Model
            let rw = 0.035; // width radius
            let rh = 0.12; // height radius (total height ~0.24)
            let light = [1.0, 1.0]; // Self-illuminated warm torch glow
            let ao = 1.0;

            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(13);
            let u_width = u_max - u_min;
            let v_height = v_max - v_min;

            let u_side_left = u_min + u_width * (7.0 / 16.0);
            let u_side_right = u_min + u_width * (9.0 / 16.0);
            let v_side_top = v_min + v_height * (6.0 / 16.0);
            let v_side_bottom = v_min + v_height * (16.0 / 16.0);

            let side_uvs = [
                [u_side_left, v_side_bottom],
                [u_side_right, v_side_bottom],
                [u_side_right, v_side_top],
                [u_side_left, v_side_top],
            ];

            let u_top_left = u_min + u_width * (7.0 / 16.0);
            let u_top_right = u_min + u_width * (9.0 / 16.0);
            let v_top_min = v_min + v_height * (6.0 / 16.0);
            let v_top_max = v_min + v_height * (8.0 / 16.0);

            let top_uvs = [
                [u_top_left, v_top_max],
                [u_top_right, v_top_max],
                [u_top_right, v_top_min],
                [u_top_left, v_top_min],
            ];

            let mut add_torch_quad = |p: [[f32; 3]; 4], uvs: [[f32; 2]; 4], n: [f32; 3]| {
                let start_idx = mesh.vertices.len() as u32;
                for i in 0..4 {
                    mesh.vertices.push(VoxelVertex {
                        position: p[i],
                        uv: uvs[i],
                        normal: n,
                        ao,
                        light,
                    });
                }
                mesh.indices.extend_from_slice(&[
                    start_idx + 0,
                    start_idx + 1,
                    start_idx + 2,
                    start_idx + 2,
                    start_idx + 3,
                    start_idx + 0,
                ]);
            };

            // South (+Z)
            add_torch_quad(
                [
                    rotate_point(-rw, -rh, rw),
                    rotate_point(rw, -rh, rw),
                    rotate_point(rw, rh, rw),
                    rotate_point(-rw, rh, rw),
                ],
                side_uvs,
                rotate_normal(0.0, 0.0, 1.0),
            );
            // North (-Z)
            add_torch_quad(
                [
                    rotate_point(rw, -rh, -rw),
                    rotate_point(-rw, -rh, -rw),
                    rotate_point(-rw, rh, -rw),
                    rotate_point(rw, rh, -rw),
                ],
                side_uvs,
                rotate_normal(0.0, 0.0, -1.0),
            );
            // East (+X)
            add_torch_quad(
                [
                    rotate_point(rw, -rh, rw),
                    rotate_point(rw, -rh, -rw),
                    rotate_point(rw, rh, -rw),
                    rotate_point(rw, rh, rw),
                ],
                side_uvs,
                rotate_normal(1.0, 0.0, 0.0),
            );
            // West (-X)
            add_torch_quad(
                [
                    rotate_point(-rw, -rh, -rw),
                    rotate_point(-rw, -rh, rw),
                    rotate_point(-rw, rh, rw),
                    rotate_point(-rw, rh, -rw),
                ],
                side_uvs,
                rotate_normal(-1.0, 0.0, 0.0),
            );
            // Top (+Y)
            add_torch_quad(
                [
                    rotate_point(-rw, rh, rw),
                    rotate_point(rw, rh, rw),
                    rotate_point(rw, rh, -rw),
                    rotate_point(-rw, rh, -rw),
                ],
                top_uvs,
                rotate_normal(0.0, 1.0, 0.0),
            );
            // Bottom (-Y)
            add_torch_quad(
                [
                    rotate_point(-rw, -rh, -rw),
                    rotate_point(rw, -rh, -rw),
                    rotate_point(rw, -rh, rw),
                    rotate_point(-rw, -rh, rw),
                ],
                top_uvs,
                rotate_normal(0.0, -1.0, 0.0),
            );
            continue;
        }

        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(def.icon_tile as u16);
        let uvs = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];

        let light = [1.0, 0.6]; // Brightly lit dropped item
        let ao = 1.0;

        // Non-block items (Coal, Raw Iron, Raw Copper, Ingots, Diamonds, Sticks, Tools)
        // render as a flat 2D floating item sprite spinning around the Y-axis (like Minecraft items).
        if def.placeable_block.is_none() {
            let iw = 0.15;
            let ih = 0.15;

            let mut add_flat_quad = |p: [[f32; 3]; 4], quad_uvs: [[f32; 2]; 4], n: [f32; 3]| {
                let start_idx = mesh.vertices.len() as u32;
                for i in 0..4 {
                    mesh.vertices.push(VoxelVertex {
                        position: p[i],
                        uv: quad_uvs[i],
                        normal: n,
                        ao,
                        light,
                    });
                }
                mesh.indices.extend_from_slice(&[
                    start_idx + 0,
                    start_idx + 1,
                    start_idx + 2,
                    start_idx + 2,
                    start_idx + 3,
                    start_idx + 0,
                ]);
            };

            // Front side (+Z)
            add_flat_quad(
                [
                    rotate_point(-iw, -ih, 0.0),
                    rotate_point(iw, -ih, 0.0),
                    rotate_point(iw, ih, 0.0),
                    rotate_point(-iw, ih, 0.0),
                ],
                uvs,
                rotate_normal(0.0, 0.0, 1.0),
            );

            // Back side (-Z, reversed winding and flipped horizontal UV so it's not mirrored)
            let back_uvs = [
                [u_max, v_max],
                [u_min, v_max],
                [u_min, v_min],
                [u_max, v_min],
            ];
            add_flat_quad(
                [
                    rotate_point(iw, -ih, 0.0),
                    rotate_point(-iw, -ih, 0.0),
                    rotate_point(-iw, ih, 0.0),
                    rotate_point(iw, ih, 0.0),
                ],
                back_uvs,
                rotate_normal(0.0, 0.0, -1.0),
            );

            continue;
        }

        let mut add_face = |p: [[f32; 3]; 4], n: [f32; 3]| {
            let start_idx = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: p[i],
                    uv: uvs[i],
                    normal: n,
                    ao,
                    light,
                });
            }
            mesh.indices.extend_from_slice(&[
                start_idx + 0,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx + 0,
            ]);
        };

        // PosZ
        add_face(
            [
                rotate_point(-s, -s, s),
                rotate_point(s, -s, s),
                rotate_point(s, s, s),
                rotate_point(-s, s, s),
            ],
            rotate_normal(0.0, 0.0, 1.0),
        );
        // NegZ
        add_face(
            [
                rotate_point(s, -s, -s),
                rotate_point(-s, -s, -s),
                rotate_point(-s, s, -s),
                rotate_point(s, s, -s),
            ],
            rotate_normal(0.0, 0.0, -1.0),
        );
        // PosX
        add_face(
            [
                rotate_point(s, -s, s),
                rotate_point(s, -s, -s),
                rotate_point(s, s, -s),
                rotate_point(s, s, s),
            ],
            rotate_normal(1.0, 0.0, 0.0),
        );
        // NegX
        add_face(
            [
                rotate_point(-s, -s, -s),
                rotate_point(-s, -s, s),
                rotate_point(-s, s, s),
                rotate_point(-s, s, -s),
            ],
            rotate_normal(-1.0, 0.0, 0.0),
        );
        // PosY
        add_face(
            [
                rotate_point(-s, s, s),
                rotate_point(s, s, s),
                rotate_point(s, s, -s),
                rotate_point(-s, s, -s),
            ],
            rotate_normal(0.0, 1.0, 0.0),
        );
        // NegY
        add_face(
            [
                rotate_point(-s, -s, -s),
                rotate_point(s, -s, -s),
                rotate_point(s, -s, s),
                rotate_point(-s, -s, s),
            ],
            rotate_normal(0.0, -1.0, 0.0),
        );
    }

    mesh
}

#[inline]
fn add_animal_face(
    mesh: &mut ChunkMesh,
    p: [[f32; 3]; 4],
    n: [f32; 3],
    uvs: &[[f32; 2]; 4],
    ao: f32,
    light: [f32; 2],
) {
    let start_idx = mesh.vertices.len() as u32;
    for i in 0..4 {
        mesh.vertices.push(VoxelVertex {
            position: p[i],
            uv: uvs[i],
            normal: n,
            ao,
            light,
        });
    }
    mesh.indices.extend_from_slice(&[
        start_idx + 0,
        start_idx + 1,
        start_idx + 2,
        start_idx + 2,
        start_idx + 3,
        start_idx + 0,
    ]);
}

fn add_animal_box(
    mesh: &mut ChunkMesh,
    min: [f32; 3],
    max: [f32; 3],
    uvs: &[[f32; 2]; 4],
    uvs_front: Option<&[[f32; 2]; 4]>,
    ao: f32,
    light: [f32; 2],
    pos: glam::Vec3,
    cos_y: f32,
    sin_y: f32,
) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;

    let to_world = |lx: f32, ly: f32, lz: f32| -> [f32; 3] {
        [
            pos.x + lx * cos_y + lz * sin_y,
            pos.y + ly,
            pos.z - lx * sin_y + lz * cos_y,
        ]
    };

    let to_world_normal = |nx: f32, ny: f32, nz: f32| -> [f32; 3] {
        [nx * cos_y + nz * sin_y, ny, -nx * sin_y + nz * cos_y]
    };

    // PosZ
    add_animal_face(
        mesh,
        [
            to_world(x0, y0, z1),
            to_world(x1, y0, z1),
            to_world(x1, y1, z1),
            to_world(x0, y1, z1),
        ],
        to_world_normal(0.0, 0.0, 1.0),
        uvs_front.unwrap_or(uvs),
        ao,
        light,
    );
    // NegZ
    add_animal_face(
        mesh,
        [
            to_world(x1, y0, z0),
            to_world(x0, y0, z0),
            to_world(x0, y1, z0),
            to_world(x1, y1, z0),
        ],
        to_world_normal(0.0, 0.0, -1.0),
        uvs,
        ao,
        light,
    );
    // PosX
    add_animal_face(
        mesh,
        [
            to_world(x1, y0, z1),
            to_world(x1, y0, z0),
            to_world(x1, y1, z0),
            to_world(x1, y1, z1),
        ],
        to_world_normal(1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegX
    add_animal_face(
        mesh,
        [
            to_world(x0, y0, z0),
            to_world(x0, y0, z1),
            to_world(x0, y1, z1),
            to_world(x0, y1, z0),
        ],
        to_world_normal(-1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // PosY
    add_animal_face(
        mesh,
        [
            to_world(x0, y1, z1),
            to_world(x1, y1, z1),
            to_world(x1, y1, z0),
            to_world(x0, y1, z0),
        ],
        to_world_normal(0.0, 1.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegY
    add_animal_face(
        mesh,
        [
            to_world(x0, y0, z0),
            to_world(x1, y0, z0),
            to_world(x1, y0, z1),
            to_world(x0, y0, z1),
        ],
        to_world_normal(0.0, -1.0, 0.0),
        uvs,
        ao,
        light,
    );
}

fn add_animal_leg_box(
    mesh: &mut ChunkMesh,
    min: [f32; 3],
    max: [f32; 3],
    hip_y: f32,
    hip_z: f32,
    angle: f32,
    uvs: &[[f32; 2]; 4],
    ao: f32,
    light: [f32; 2],
    pos: glam::Vec3,
    cos_y: f32,
    sin_y: f32,
) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    let leg_point = |lx: f32, ly: f32, lz: f32| -> [f32; 3] {
        let rel_y = ly - hip_y;
        let rel_z = lz - hip_z;
        let rot_z = rel_z * cos_a - rel_y * sin_a;
        let rot_y = rel_z * sin_a + rel_y * cos_a;
        [
            pos.x + lx * cos_y + (hip_z + rot_z) * sin_y,
            pos.y + (hip_y + rot_y),
            pos.z - lx * sin_y + (hip_z + rot_z) * cos_y,
        ]
    };

    let leg_normal = |nx: f32, ny: f32, nz: f32| -> [f32; 3] {
        let rot_z = nz * cos_a - ny * sin_a;
        let rot_y = nz * sin_a + ny * cos_a;
        [
            nx * cos_y + rot_z * sin_y,
            rot_y,
            -nx * sin_y + rot_z * cos_y,
        ]
    };

    // PosZ
    add_animal_face(
        mesh,
        [
            leg_point(x0, y0, z1),
            leg_point(x1, y0, z1),
            leg_point(x1, y1, z1),
            leg_point(x0, y1, z1),
        ],
        leg_normal(0.0, 0.0, 1.0),
        uvs,
        ao,
        light,
    );
    // NegZ
    add_animal_face(
        mesh,
        [
            leg_point(x1, y0, z0),
            leg_point(x0, y0, z0),
            leg_point(x0, y1, z0),
            leg_point(x1, y1, z0),
        ],
        leg_normal(0.0, 0.0, -1.0),
        uvs,
        ao,
        light,
    );
    // PosX
    add_animal_face(
        mesh,
        [
            leg_point(x1, y0, z1),
            leg_point(x1, y0, z0),
            leg_point(x1, y1, z0),
            leg_point(x1, y1, z1),
        ],
        leg_normal(1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegX
    add_animal_face(
        mesh,
        [
            leg_point(x0, y0, z0),
            leg_point(x0, y0, z1),
            leg_point(x0, y1, z1),
            leg_point(x0, y1, z0),
        ],
        leg_normal(-1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // PosY
    add_animal_face(
        mesh,
        [
            leg_point(x0, y1, z1),
            leg_point(x1, y1, z1),
            leg_point(x1, y1, z0),
            leg_point(x0, y1, z0),
        ],
        leg_normal(0.0, 1.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegY
    add_animal_face(
        mesh,
        [
            leg_point(x0, y0, z0),
            leg_point(x1, y0, z0),
            leg_point(x1, y0, z1),
            leg_point(x0, y0, z1),
        ],
        leg_normal(0.0, -1.0, 0.0),
        uvs,
        ao,
        light,
    );
}

#[inline]
fn get_animal_uvs(tile_id: u16) -> [[f32; 2]; 4] {
    let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);
    [
        [u_min, v_max],
        [u_max, v_max],
        [u_max, v_min],
        [u_min, v_min],
    ]
}

/// Builds 3D composite blocky models for active animals with walking leg animations.
pub fn build_animals_mesh(animals: &[AnimalEntity]) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();

    for animal in animals {
        if animal.dead {
            continue;
        }
        let start_v = mesh.vertices.len();

        // Red/white damage flash when hurt
        let (light, ao) = if animal.hurt_timer > 0.0 {
            ([1.0, 1.0], 0.25)
        } else {
            ([1.0, 0.0], 1.0)
        };

        let cos_y = animal.yaw.cos();
        let sin_y = animal.yaw.sin();
        let pos = animal.position;
        let leg_swing = (animal.walk_anim_timer).sin() * 0.45;

        match animal.kind {
            AnimalKind::Cow => {
                let uvs_body = get_animal_uvs(170);
                let uvs_head = get_animal_uvs(185);
                let uvs_legs = get_animal_uvs(186);
                let uvs_snout = get_animal_uvs(240);
                let uvs_horns = get_animal_uvs(241);

                // Body
                add_animal_box(
                    &mut mesh,
                    [-0.35, 0.45, -0.55],
                    [0.35, 1.1, 0.55],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Head (with front-facing eyes and markings)
                add_animal_box(
                    &mut mesh,
                    [-0.22, 0.85, 0.55],
                    [0.22, 1.3, 1.0],
                    &uvs_body,
                    Some(&uvs_head),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Snout
                add_animal_box(
                    &mut mesh,
                    [-0.15, 0.85, 1.0],
                    [0.15, 1.08, 1.16],
                    &uvs_snout,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Horns
                add_animal_box(
                    &mut mesh,
                    [-0.25, 1.3, 0.7],
                    [-0.18, 1.45, 0.76],
                    &uvs_horns,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_box(
                    &mut mesh,
                    [0.18, 1.3, 0.7],
                    [0.25, 1.45, 0.76],
                    &uvs_horns,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // 4 Legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.32, 0.0, 0.3],
                    [-0.14, 0.45, 0.48],
                    0.45,
                    0.39,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.14, 0.0, 0.3],
                    [0.32, 0.45, 0.48],
                    0.45,
                    0.39,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [-0.32, 0.0, -0.48],
                    [-0.14, 0.45, -0.3],
                    0.45,
                    -0.39,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.14, 0.0, -0.48],
                    [0.32, 0.45, -0.3],
                    0.45,
                    -0.39,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
            AnimalKind::Pig => {
                let uvs_body = get_animal_uvs(171);
                let uvs_head = get_animal_uvs(187);
                let uvs_legs = get_animal_uvs(188);

                // Body
                add_animal_box(
                    &mut mesh,
                    [-0.3, 0.32, -0.45],
                    [0.3, 0.82, 0.45],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Head (with front-facing pig face)
                add_animal_box(
                    &mut mesh,
                    [-0.2, 0.5, 0.45],
                    [0.2, 0.9, 0.85],
                    &uvs_body,
                    Some(&uvs_head),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Snout
                add_animal_box(
                    &mut mesh,
                    [-0.12, 0.52, 0.85],
                    [0.12, 0.72, 0.98],
                    &uvs_legs,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // 4 Legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.28, 0.0, 0.22],
                    [-0.12, 0.32, 0.38],
                    0.32,
                    0.3,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.12, 0.0, 0.22],
                    [0.28, 0.32, 0.38],
                    0.32,
                    0.3,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [-0.28, 0.0, -0.38],
                    [-0.12, 0.32, -0.22],
                    0.32,
                    -0.3,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.12, 0.0, -0.38],
                    [0.28, 0.32, -0.22],
                    0.32,
                    -0.3,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
            AnimalKind::Chicken => {
                let uvs_body = get_animal_uvs(172);
                let uvs_head = get_animal_uvs(189);
                let uvs_legs = get_animal_uvs(190);

                // Body
                add_animal_box(
                    &mut mesh,
                    [-0.18, 0.25, -0.22],
                    [0.18, 0.55, 0.22],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Head (with front-facing eyes and comb)
                add_animal_box(
                    &mut mesh,
                    [-0.1, 0.45, 0.15],
                    [0.1, 0.75, 0.35],
                    &uvs_body,
                    Some(&uvs_head),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Beak
                add_animal_box(
                    &mut mesh,
                    [-0.06, 0.52, 0.35],
                    [0.06, 0.62, 0.48],
                    &uvs_legs,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Wattle
                add_animal_box(
                    &mut mesh,
                    [-0.03, 0.44, 0.26],
                    [0.03, 0.52, 0.34],
                    &uvs_head,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Wings
                add_animal_box(
                    &mut mesh,
                    [-0.22, 0.3, -0.18],
                    [-0.18, 0.52, 0.18],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_box(
                    &mut mesh,
                    [0.18, 0.3, -0.18],
                    [0.22, 0.52, 0.18],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // 2 Legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.12, 0.0, -0.05],
                    [-0.04, 0.25, 0.05],
                    0.25,
                    0.0,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.04, 0.0, -0.05],
                    [0.12, 0.25, 0.05],
                    0.25,
                    0.0,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
            AnimalKind::Sheep => {
                let uvs_wool = get_animal_uvs(339);
                let uvs_face = get_animal_uvs(340);
                let uvs_legs = get_animal_uvs(341);

                // Fluffy wool body
                add_animal_box(
                    &mut mesh,
                    [-0.36, 0.42, -0.52],
                    [0.36, 1.08, 0.52],
                    &uvs_wool,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Head (wool top/sides/back, pale face with eyes on front)
                add_animal_box(
                    &mut mesh,
                    [-0.22, 0.82, 0.52],
                    [0.22, 1.25, 0.96],
                    &uvs_wool,
                    Some(&uvs_face),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // 4 Legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.30, 0.0, 0.28],
                    [-0.14, 0.42, 0.44],
                    0.42,
                    0.36,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.14, 0.0, 0.28],
                    [0.30, 0.42, 0.44],
                    0.42,
                    0.36,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [-0.30, 0.0, -0.44],
                    [-0.14, 0.42, -0.28],
                    0.42,
                    -0.36,
                    -leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.14, 0.0, -0.44],
                    [0.30, 0.42, -0.28],
                    0.42,
                    -0.36,
                    leg_swing,
                    &uvs_legs,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
        }

        if animal.is_baby() {
            for v in &mut mesh.vertices[start_v..] {
                v.position[0] = pos.x + (v.position[0] - pos.x) * 0.55;
                v.position[1] = pos.y + (v.position[1] - pos.y) * 0.55;
                v.position[2] = pos.z + (v.position[2] - pos.z) * 0.55;
            }
        }

        if animal.love_timer > 0.0 {
            let (_, h) = animal.kind.dimensions();
            let heart_y = pos.y + (if animal.is_baby() { h * 0.55 } else { h }) + 0.35;
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(26);
            let s = 0.22_f32;
            let planes = [
                ([-s, -s, 0.0], [s, -s, 0.0], [0.0, 0.0, 1.0]),
                ([0.0, -s, -s], [0.0, -s, s], [1.0, 0.0, 0.0]),
            ];
            for (a, b, n) in planes {
                let base_idx = mesh.vertices.len() as u32;
                let p0 = [pos.x + a[0], heart_y - s, pos.z + a[2]];
                let p1 = [pos.x + b[0], heart_y - s, pos.z + b[2]];
                let p2 = [pos.x + b[0], heart_y + s, pos.z + b[2]];
                let p3 = [pos.x + a[0], heart_y + s, pos.z + a[2]];
                for (p, uv) in [
                    (p0, [u_min, v_max]),
                    (p1, [u_max, v_max]),
                    (p2, [u_max, v_min]),
                    (p3, [u_min, v_min]),
                ] {
                    mesh.vertices.push(VoxelVertex {
                        position: p,
                        uv,
                        normal: n,
                        ao: 1.4,
                        light: [1.0, 1.0],
                    });
                }
                mesh.indices.extend_from_slice(&[
                    base_idx,
                    base_idx + 1,
                    base_idx + 2,
                    base_idx,
                    base_idx + 2,
                    base_idx + 3,
                    base_idx,
                    base_idx + 2,
                    base_idx + 1,
                    base_idx,
                    base_idx + 3,
                    base_idx + 2,
                ]);
            }
        }
    }

    mesh
}

/// Builds dynamic 3D meshes for active monsters (Zombies, Skeletons, Spiders).
pub fn build_monsters_mesh(monsters: &[blockgame_core::monster::MonsterEntity]) -> ChunkMesh {
    build_monsters_mesh_animated(monsters, 0.0)
}

pub fn build_monsters_mesh_animated(
    monsters: &[blockgame_core::monster::MonsterEntity],
    time: f32,
) -> ChunkMesh {
    use blockgame_core::monster::MonsterKind;
    let mut mesh = ChunkMesh::new();

    for m in monsters {
        if m.dead {
            continue;
        }
        let cos_y = m.yaw.cos();
        let sin_y = m.yaw.sin();
        let ao = if m.hurt_timer > 0.0 || m.burning_timer > 0.0 {
            1.45
        } else {
            1.0
        };
        let light = [0.95, if m.burning_timer > 0.0 { 1.0 } else { 0.4 }];
        let pos = m.position;
        let leg_swing = m.walk_anim_timer.sin() * 0.45;

        match m.kind {
            MonsterKind::Zombie => {
                let uvs_head = get_animal_uvs(349);
                let uvs_shirt = get_animal_uvs(350);
                let uvs_pants = get_animal_uvs(351);

                // Head
                add_animal_box(
                    &mut mesh,
                    [-0.24, 1.45, -0.24],
                    [0.24, 1.93, 0.24],
                    &uvs_shirt,
                    Some(&uvs_head),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Torso
                add_animal_box(
                    &mut mesh,
                    [-0.24, 0.72, -0.14],
                    [0.24, 1.45, 0.14],
                    &uvs_shirt,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Outstretched Zombie Arms
                add_animal_box(
                    &mut mesh,
                    [-0.44, 1.18, 0.0],
                    [-0.24, 1.42, 0.68],
                    &uvs_shirt,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_box(
                    &mut mesh,
                    [0.24, 1.18, 0.0],
                    [0.44, 1.42, 0.68],
                    &uvs_shirt,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.22, 0.0, -0.11],
                    [-0.02, 0.72, 0.11],
                    0.72,
                    0.0,
                    leg_swing,
                    &uvs_pants,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.02, 0.0, -0.11],
                    [0.22, 0.72, 0.11],
                    0.72,
                    0.0,
                    -leg_swing,
                    &uvs_pants,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
            MonsterKind::Skeleton => {
                let uvs_skull = get_animal_uvs(352);
                let uvs_bones = get_animal_uvs(353);

                // Skull
                add_animal_box(
                    &mut mesh,
                    [-0.23, 1.45, -0.23],
                    [0.23, 1.91, 0.23],
                    &uvs_bones,
                    Some(&uvs_skull),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Ribcage torso
                add_animal_box(
                    &mut mesh,
                    [-0.22, 0.72, -0.12],
                    [0.22, 1.45, 0.12],
                    &uvs_bones,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Slim arms aiming forward
                add_animal_box(
                    &mut mesh,
                    [-0.36, 1.22, 0.0],
                    [-0.22, 1.38, 0.62],
                    &uvs_bones,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_box(
                    &mut mesh,
                    [0.22, 1.22, 0.0],
                    [0.36, 1.38, 0.62],
                    &uvs_bones,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Slim bone legs
                add_animal_leg_box(
                    &mut mesh,
                    [-0.18, 0.0, -0.07],
                    [-0.04, 0.72, 0.07],
                    0.72,
                    0.0,
                    leg_swing,
                    &uvs_bones,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                add_animal_leg_box(
                    &mut mesh,
                    [0.04, 0.0, -0.07],
                    [0.18, 0.72, 0.07],
                    0.72,
                    0.0,
                    -leg_swing,
                    &uvs_bones,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
            MonsterKind::Spider => {
                let uvs_face = get_animal_uvs(354);
                let uvs_body = get_animal_uvs(355);

                // Abdomen
                add_animal_box(
                    &mut mesh,
                    [-0.35, 0.25, -0.62],
                    [0.35, 0.78, 0.08],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Thorax
                add_animal_box(
                    &mut mesh,
                    [-0.22, 0.28, 0.08],
                    [0.22, 0.64, 0.34],
                    &uvs_body,
                    None,
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // Head with glowing red eyes
                add_animal_box(
                    &mut mesh,
                    [-0.26, 0.26, 0.34],
                    [0.26, 0.68, 0.72],
                    &uvs_body,
                    Some(&uvs_face),
                    ao,
                    light,
                    pos,
                    cos_y,
                    sin_y,
                );
                // 8 Spider legs (4 per side)
                for i in 0..4 {
                    let lz = -0.25 + (i as f32) * 0.22;
                    let phase = if i % 2 == 0 { leg_swing } else { -leg_swing };
                    add_animal_leg_box(
                        &mut mesh,
                        [-0.78, 0.0, lz - 0.05],
                        [-0.26, 0.38, lz + 0.05],
                        0.38,
                        lz,
                        phase,
                        &uvs_body,
                        ao,
                        light,
                        pos,
                        cos_y,
                        sin_y,
                    );
                    add_animal_leg_box(
                        &mut mesh,
                        [0.26, 0.0, lz - 0.05],
                        [0.78, 0.38, lz + 0.05],
                        0.38,
                        lz,
                        -phase,
                        &uvs_body,
                        ao,
                        light,
                        pos,
                        cos_y,
                        sin_y,
                    );
                }
            }
        }

        // Animated burning flames wrapping around the mob when on fire
        if m.burning_timer > 0.0 {
            let frame = ((time * 12.0 + m.id as f32 * 0.7).floor() as usize) % 3;
            let flame_tile = 375 + frame as u16;
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(flame_tile);

            let (hw, h) = m.kind.dimensions();
            let radius = hw + 0.14;
            let flicker = 1.0 + (time * 16.0 + m.id as f32 * 1.3).sin() * 0.08;
            let flame_h = (h * 0.95) * flicker;
            let y0 = pos.y + 0.05;
            let y1 = pos.y + flame_h;
            let flame_ao = 1.55;
            let flame_light = [1.0_f32, 1.0_f32];

            let sides: [([f32; 3], [f32; 3], [f32; 3]); 4] = [
                ([-radius, 0.0, radius], [radius, 0.0, radius], [0.0, 0.0, 1.0]),
                ([radius, 0.0, -radius], [-radius, 0.0, -radius], [0.0, 0.0, -1.0]),
                ([radius, 0.0, radius], [radius, 0.0, -radius], [1.0, 0.0, 0.0]),
                ([-radius, 0.0, -radius], [-radius, 0.0, radius], [-1.0, 0.0, 0.0]),
            ];

            for (a, b, n) in sides {
                let base_idx = mesh.vertices.len() as u32;
                let p0 = [pos.x + a[0], y0, pos.z + a[2]];
                let p1 = [pos.x + b[0], y0, pos.z + b[2]];
                let p2 = [pos.x + b[0], y1, pos.z + b[2]];
                let p3 = [pos.x + a[0], y1, pos.z + a[2]];
                for (p, uv) in [
                    (p0, [u_min, v_max]),
                    (p1, [u_max, v_max]),
                    (p2, [u_max, v_min]),
                    (p3, [u_min, v_min]),
                ] {
                    mesh.vertices.push(VoxelVertex {
                        position: p,
                        uv,
                        normal: n,
                        ao: flame_ao,
                        light: flame_light,
                    });
                }
                // Double-sided so flames are visible from inside and outside
                mesh.indices.extend_from_slice(&[
                    base_idx,
                    base_idx + 1,
                    base_idx + 2,
                    base_idx,
                    base_idx + 2,
                    base_idx + 3,
                    base_idx,
                    base_idx + 2,
                    base_idx + 1,
                    base_idx,
                    base_idx + 3,
                    base_idx + 2,
                ]);
            }
        }
    }

    mesh
}

/// Builds dynamic 3D meshes for flying/stuck Arrow projectiles.
pub fn build_arrows_mesh(arrows: &[blockgame_core::monster::ArrowProjectile]) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();
    let uvs_shaft = get_animal_uvs(10); // Oak planks wood shaft

    for arrow in arrows {
        if arrow.dead {
            continue;
        }
        let dir = arrow.velocity.normalize_or_zero();
        let yaw = dir.x.atan2(dir.z);
        let cos_y = yaw.cos();
        let sin_y = yaw.sin();
        add_animal_box(
            &mut mesh,
            [-0.035, -0.035, -0.28],
            [0.035, 0.035, 0.28],
            &uvs_shaft,
            None,
            1.0,
            [1.0, 0.4],
            arrow.position,
            cos_y,
            sin_y,
        );
    }

    mesh
}

/// Builds a 3D mesh for a cast fishing bobber (red/white float with pin) and a continuous
/// 3D-oriented catenary fishing line connecting `rod_tip` to the bobber's top pin.
pub fn build_fishing_bobber_mesh(
    rod_tip: glam::Vec3,
    bobber_pos: glam::Vec3,
    is_biting: bool,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();
    let white_uvs = get_animal_uvs(289); // Solid white wool
    let red_uvs = get_animal_uvs(290);   // Solid red wool
    let dark_uvs = get_animal_uvs(301);  // Solid charcoal/black wool
    let wood_uvs = get_animal_uvs(4);    // Wood planks for top pin

    let s = if is_biting { 1.15 } else { 1.0 };

    // 1. White bottom half of the 3D bobber
    add_animal_box(
        &mut mesh,
        [-0.075 * s, -0.07 * s, -0.075 * s],
        [0.075 * s, 0.0, 0.075 * s],
        &white_uvs,
        None,
        1.0,
        [1.0, 0.35],
        bobber_pos,
        1.0,
        0.0,
    );

    // 2. Red top half of the 3D bobber
    add_animal_box(
        &mut mesh,
        [-0.075 * s, 0.0, -0.075 * s],
        [0.075 * s, 0.07 * s, 0.075 * s],
        &red_uvs,
        None,
        1.0,
        [1.0, 0.35],
        bobber_pos,
        1.0,
        0.0,
    );

    // 3. Top pin (where the line ties onto the bobber)
    add_animal_box(
        &mut mesh,
        [-0.018 * s, 0.07 * s, -0.018 * s],
        [0.018 * s, 0.14 * s, 0.018 * s],
        &wood_uvs,
        None,
        1.0,
        [1.0, 0.35],
        bobber_pos,
        1.0,
        0.0,
    );

    // 4. Small underwater sinker/hook below the bobber
    add_animal_box(
        &mut mesh,
        [-0.018 * s, -0.13 * s, -0.018 * s],
        [0.018 * s, -0.07 * s, 0.018 * s],
        &dark_uvs,
        None,
        1.0,
        [1.0, 0.35],
        bobber_pos,
        1.0,
        0.0,
    );

    // 5. Continuous 3D-oriented catenary fishing line from rod_tip to bobber pin
    let attach_pos = bobber_pos + glam::Vec3::new(0.0, 0.14 * s, 0.0);
    let segments = 16;
    let delta = attach_pos - rod_tip;
    let total_len = delta.length();
    if total_len > 0.1 {
        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(301);
        let radius = 0.008;

        let mut points = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let sag = if is_biting { 0.04 } else { 0.38 } * (t * (1.0 - t) * 4.0);
            points.push(rod_tip + delta * t - glam::Vec3::new(0.0, sag, 0.0));
        }

        for i in 0..segments {
            let p_start = points[i];
            let p_end = points[i + 1];
            let seg = p_end - p_start;
            let len = seg.length();
            if len < 1e-4 {
                continue;
            }
            let dir = seg / len;
            let ref_up = if dir.y.abs() < 0.95 {
                glam::Vec3::Y
            } else {
                glam::Vec3::X
            };
            let right = dir.cross(ref_up).normalize() * radius;
            let up = right.cross(dir).normalize() * radius;

            let p_ring = [
                p_start - right - up,
                p_start + right - up,
                p_start + right + up,
                p_start - right + up,
            ];
            let q_ring = [
                p_end - right - up,
                p_end + right - up,
                p_end + right + up,
                p_end - right + up,
            ];
            let side_normals = [
                -up.normalize(),
                right.normalize(),
                up.normalize(),
                -right.normalize(),
            ];

            for side in 0..4 {
                let next_side = (side + 1) % 4;
                let base_idx = mesh.vertices.len() as u32;
                let n = side_normals[side].to_array();
                let corners = [
                    p_ring[side],
                    p_ring[next_side],
                    q_ring[next_side],
                    q_ring[side],
                ];
                let uvs = [
                    [u_min, v_min],
                    [u_max, v_min],
                    [u_max, v_max],
                    [u_min, v_max],
                ];
                for k in 0..4 {
                    mesh.vertices.push(VoxelVertex {
                        position: corners[k].to_array(),
                        normal: n,
                        uv: uvs[k],
                        ao: 1.0,
                        light: [1.0, 0.25],
                    });
                }
                mesh.indices.extend_from_slice(&[
                    base_idx,
                    base_idx + 1,
                    base_idx + 2,
                    base_idx,
                    base_idx + 2,
                    base_idx + 3,
                ]);
            }
        }
    }

    mesh
}

/// Builds dynamic 3D meshes for active falling gravity blocks (Sand, Red Sand, Gravel).
pub fn build_falling_blocks_mesh(
    falling_blocks: &[FallingBlock],
    registry: &BlockRegistry,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();

    for fb in falling_blocks {
        let fx = fb.position.x;
        let fy = fb.position.y;
        let fz = fb.position.z;
        let light = [1.0, 0.0];
        let ao = 1.0;

        for dir in Direction::ALL {
            let tile_id = registry.get_texture(fb.block_id, dir);
            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);
            let normal = dir.normal_f32();
            let positions = match dir {
                Direction::PosY => [
                    [fx, fy + 1.0, fz + 1.0],
                    [fx + 1.0, fy + 1.0, fz + 1.0],
                    [fx + 1.0, fy + 1.0, fz],
                    [fx, fy + 1.0, fz],
                ],
                Direction::NegY => [
                    [fx, fy, fz],
                    [fx + 1.0, fy, fz],
                    [fx + 1.0, fy, fz + 1.0],
                    [fx, fy, fz + 1.0],
                ],
                Direction::PosZ => [
                    [fx, fy, fz + 1.0],
                    [fx + 1.0, fy, fz + 1.0],
                    [fx + 1.0, fy + 1.0, fz + 1.0],
                    [fx, fy + 1.0, fz + 1.0],
                ],
                Direction::NegZ => [
                    [fx + 1.0, fy, fz],
                    [fx, fy, fz],
                    [fx, fy + 1.0, fz],
                    [fx + 1.0, fy + 1.0, fz],
                ],
                Direction::PosX => [
                    [fx + 1.0, fy, fz + 1.0],
                    [fx + 1.0, fy, fz],
                    [fx + 1.0, fy + 1.0, fz],
                    [fx + 1.0, fy + 1.0, fz + 1.0],
                ],
                Direction::NegX => [
                    [fx, fy, fz],
                    [fx, fy, fz + 1.0],
                    [fx, fy + 1.0, fz + 1.0],
                    [fx, fy + 1.0, fz],
                ],
            };

            let uvs = [
                [u_min, v_max],
                [u_max, v_max],
                [u_max, v_min],
                [u_min, v_min],
            ];

            let start_idx = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: positions[i],
                    uv: uvs[i],
                    normal,
                    ao,
                    light,
                });
            }
            mesh.indices.extend_from_slice(&[
                start_idx + 0,
                start_idx + 1,
                start_idx + 2,
                start_idx + 2,
                start_idx + 3,
                start_idx + 0,
            ]);
        }
    }

    mesh
}

fn add_pivoted_box_with_front(
    mesh: &mut ChunkMesh,
    min: [f32; 3],
    max: [f32; 3],
    pivot_y: f32,
    pivot_z: f32,
    angle: f32,
    uvs: &[[f32; 2]; 4],
    uvs_front: Option<&[[f32; 2]; 4]>,
    ao: f32,
    light: [f32; 2],
    pos: glam::Vec3,
    cos_y: f32,
    sin_y: f32,
) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    let pt = |lx: f32, ly: f32, lz: f32| -> [f32; 3] {
        let rel_y = ly - pivot_y;
        let rel_z = lz - pivot_z;
        let rot_z = rel_z * cos_a - rel_y * sin_a;
        let rot_y = rel_z * sin_a + rel_y * cos_a;
        [
            pos.x + lx * cos_y + (pivot_z + rot_z) * sin_y,
            pos.y + (pivot_y + rot_y),
            pos.z - lx * sin_y + (pivot_z + rot_z) * cos_y,
        ]
    };

    let nrm = |nx: f32, ny: f32, nz: f32| -> [f32; 3] {
        let rot_z = nz * cos_a - ny * sin_a;
        let rot_y = nz * sin_a + ny * cos_a;
        [
            nx * cos_y + rot_z * sin_y,
            rot_y,
            -nx * sin_y + rot_z * cos_y,
        ]
    };

    // PosZ (Front)
    add_animal_face(
        mesh,
        [
            pt(x0, y0, z1),
            pt(x1, y0, z1),
            pt(x1, y1, z1),
            pt(x0, y1, z1),
        ],
        nrm(0.0, 0.0, 1.0),
        uvs_front.unwrap_or(uvs),
        ao,
        light,
    );
    // NegZ (Back)
    add_animal_face(
        mesh,
        [
            pt(x1, y0, z0),
            pt(x0, y0, z0),
            pt(x0, y1, z0),
            pt(x1, y1, z0),
        ],
        nrm(0.0, 0.0, -1.0),
        uvs,
        ao,
        light,
    );
    // PosX (Left)
    add_animal_face(
        mesh,
        [
            pt(x1, y0, z1),
            pt(x1, y0, z0),
            pt(x1, y1, z0),
            pt(x1, y1, z1),
        ],
        nrm(1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegX (Right)
    add_animal_face(
        mesh,
        [
            pt(x0, y0, z0),
            pt(x0, y0, z1),
            pt(x0, y1, z1),
            pt(x0, y1, z0),
        ],
        nrm(-1.0, 0.0, 0.0),
        uvs,
        ao,
        light,
    );
    // PosY (Top)
    add_animal_face(
        mesh,
        [
            pt(x0, y1, z1),
            pt(x1, y1, z1),
            pt(x1, y1, z0),
            pt(x0, y1, z0),
        ],
        nrm(0.0, 1.0, 0.0),
        uvs,
        ao,
        light,
    );
    // NegY (Bottom)
    add_animal_face(
        mesh,
        [
            pt(x0, y0, z0),
            pt(x1, y0, z0),
            pt(x1, y0, z1),
            pt(x0, y0, z1),
        ],
        nrm(0.0, -1.0, 0.0),
        uvs,
        ao,
        light,
    );
}

/// Builds 3D articulated humanoid player models with head pitch/yaw, walking limb swing,
/// mining/attack right-arm swing, held 3D item, and skin textures.
pub fn build_players_mesh(
    players: &[blockgame_core::net::PlayerModelState],
    item_registry: &blockgame_core::item::ItemRegistry,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();

    for player in players {
        if player.health <= 0.0 {
            continue;
        }

        let (light, ao) = if player.hurt_timer > 0.0 {
            ([1.0, 1.0], 0.25)
        } else {
            ([1.0, 0.0], 1.0)
        };

        // Align local +Z with Camera::forward (cos(yaw), 0, sin(yaw))
        let sin_y = player.yaw.cos();
        let cos_y = player.yaw.sin();
        let pos = player.position;

        let skin_base = 272 + ((player.skin_id as u16) % 4) * 4;
        let uvs_head_front = get_animal_uvs(skin_base);
        let uvs_head_side = get_animal_uvs(skin_base + 1);
        let uvs_torso = get_animal_uvs(skin_base + 2);
        let uvs_legs = get_animal_uvs(skin_base + 3);

        let crouch_y = if player.is_sneaking { -0.14 } else { 0.0 };
        let leg_swing = (player.walk_anim).sin() * 0.55;
        let swing_boost = if player.swing_anim > 0.0 {
            (player.swing_anim * std::f32::consts::PI).sin() * 1.15
        } else {
            0.0
        };
        let hold_pose = if player.held_item != blockgame_core::item::ItemId::EMPTY {
            0.25
        } else {
            0.0
        };

        // 1. Left & Right Legs (pivoted at hip y = 0.70)
        add_animal_leg_box(
            &mut mesh,
            [0.01, 0.0, -0.12],
            [0.24, 0.70, 0.12],
            0.70,
            0.0,
            leg_swing,
            &uvs_legs,
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );
        add_animal_leg_box(
            &mut mesh,
            [-0.24, 0.0, -0.12],
            [-0.01, 0.70, 0.12],
            0.70,
            0.0,
            -leg_swing,
            &uvs_legs,
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );

        // 2. Torso
        add_animal_box(
            &mut mesh,
            [-0.24, 0.70 + crouch_y, -0.13],
            [0.24, 1.40 + crouch_y, 0.13],
            &uvs_torso,
            None,
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );

        // 3. Head (pivoted at neck y = 1.40 + crouch_y, rotating with pitch)
        let pitch_clamped = player.pitch.clamp(-1.1, 1.1);
        add_pivoted_box_with_front(
            &mut mesh,
            [-0.24, 1.40 + crouch_y, -0.24],
            [0.24, 1.88 + crouch_y, 0.24],
            1.40 + crouch_y,
            0.0,
            pitch_clamped,
            &uvs_head_side,
            Some(&uvs_head_front),
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );

        // 4. Left Arm (pivoted at shoulder y = 1.35 + crouch_y)
        add_animal_leg_box(
            &mut mesh,
            [0.24, 0.70 + crouch_y, -0.12],
            [0.46, 1.38 + crouch_y, 0.12],
            1.35 + crouch_y,
            0.0,
            -leg_swing * 0.85,
            &uvs_torso,
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );

        // 5. Right Arm (pivoted at shoulder y = 1.35 + crouch_y, with mining/attack swing & held item pose)
        let right_arm_angle = leg_swing * 0.85 + hold_pose + swing_boost;
        add_animal_leg_box(
            &mut mesh,
            [-0.46, 0.70 + crouch_y, -0.12],
            [-0.24, 1.38 + crouch_y, 0.12],
            1.35 + crouch_y,
            0.0,
            right_arm_angle,
            &uvs_torso,
            ao,
            light,
            pos,
            cos_y,
            sin_y,
        );

        // 6. Held 3D Item in Right Hand
        if player.held_item != blockgame_core::item::ItemId::EMPTY {
            if let Some(def) = item_registry.get(player.held_item) {
                let item_uvs = get_animal_uvs(def.icon_tile as u16);
                let item_light = if player.held_item == blockgame_core::item::ItemId::TORCH {
                    [1.0, 1.0]
                } else {
                    light
                };
                add_animal_leg_box(
                    &mut mesh,
                    [-0.43, 0.62 + crouch_y, 0.06],
                    [-0.25, 0.80 + crouch_y, 0.28],
                    1.35 + crouch_y,
                    0.0,
                    right_arm_angle,
                    &item_uvs,
                    ao,
                    item_light,
                    pos,
                    cos_y,
                    sin_y,
                );
            }
        }
    }

    mesh
}

/// Builds billboarded Sun (tile 373) and Moon (tile 374) quads at distance 250 around `camera_pos`
/// along `sun_dir` and `-sun_dir` so the player can track the time of day across the sky.
pub fn build_celestial_mesh(camera_pos: glam::Vec3, sun_dir: glam::Vec3) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();
    let dir = sun_dir.normalize_or_zero();
    if dir.length_squared() < 0.01 {
        return mesh;
    }

    let distance = 250.0;
    let bodies = [
        (dir, 22.0_f32, 373_u16),   // Sun
        (-dir, 18.0_f32, 374_u16), // Moon (directly opposite the Sun)
    ];

    for (body_dir, half_size, tile_id) in bodies {
        let center = camera_pos + body_dir * distance;
        let forward = -body_dir;
        // Pick a stable tangent basis (sun_dir moves in the X-Y plane with a small Z tilt)
        let ref_axis = if forward.z.abs() < 0.9 {
            glam::Vec3::Z
        } else {
            glam::Vec3::X
        };
        let right = ref_axis.cross(forward).normalize_or_zero() * half_size;
        let up = forward.cross(right).normalize_or_zero() * half_size;

        let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(tile_id);
        let base_idx = mesh.vertices.len() as u32;
        let normal = [forward.x, forward.y, forward.z];

        let corners = [
            (center - right - up, [u_min, v_max]),
            (center + right - up, [u_max, v_max]),
            (center + right + up, [u_max, v_min]),
            (center - right + up, [u_min, v_min]),
        ];

        for (pos, uv) in corners {
            mesh.vertices.push(VoxelVertex {
                position: [pos.x, pos.y, pos.z],
                uv,
                normal,
                ao: 1.0,
                light: [1.0, 1.0],
            });
        }
        mesh.indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    mesh
}

/// Builds a dynamic 3D mesh for all animated spinning 3-blade Wind Turbine rotors.
/// Each entry is `(base_block_pos, live_output_eu_s, facing)`.
pub fn build_wind_turbine_rotors_mesh(
    turbines: &[(glam::IVec3, f32, u8)],
    total_time: f32,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();
    if turbines.is_empty() {
        return mesh;
    }

    let blade_uv = TextureAtlas::get_uv_bounds(416);
    let hub_uv = TextureAtlas::get_uv_bounds(415);

    let add_rot_box = |mesh: &mut ChunkMesh,
                       cx: f32,
                       cy: f32,
                       cz: f32,
                       facing: u8,
                       angle: f32,
                       lx0: f32,
                       ly0: f32,
                       lz0: f32,
                       lx1: f32,
                       ly1: f32,
                       lz1: f32,
                       uv_bounds: [f32; 4]| {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let orient = |rx: f32, ry: f32, rz: f32| -> [f32; 3] {
            match facing & 0x03 {
                1 => [-rx, ry, -rz], // North (-Z)
                2 => [rz, ry, -rx],  // East (+X)
                3 => [-rz, ry, rx],  // West (-X)
                _ => [rx, ry, rz],   // South (+Z)
            }
        };
        let xform = |lx: f32, ly: f32, lz: f32| -> [f32; 3] {
            let rx = lx * cos_a - ly * sin_a;
            let ry = lx * sin_a + ly * cos_a;
            let [ox, oy, oz] = orient(rx, ry, lz);
            [cx + ox, cy + oy, cz + oz]
        };
        let xform_n = |nx: f32, ny: f32, nz: f32| -> [f32; 3] {
            let rx = nx * cos_a - ny * sin_a;
            let ry = nx * sin_a + ny * cos_a;
            orient(rx, ry, nz)
        };

        let [u_min, v_min, u_max, v_max] = uv_bounds;
        let uvs = [
            [u_min, v_max],
            [u_max, v_max],
            [u_max, v_min],
            [u_min, v_min],
        ];
        let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
            // Front (+Z)
            (
                xform_n(0.0, 0.0, 1.0),
                [
                    xform(lx0, ly0, lz1),
                    xform(lx1, ly0, lz1),
                    xform(lx1, ly1, lz1),
                    xform(lx0, ly1, lz1),
                ],
            ),
            // Back (-Z)
            (
                xform_n(0.0, 0.0, -1.0),
                [
                    xform(lx1, ly0, lz0),
                    xform(lx0, ly0, lz0),
                    xform(lx0, ly1, lz0),
                    xform(lx1, ly1, lz0),
                ],
            ),
            // Right (+X)
            (
                xform_n(1.0, 0.0, 0.0),
                [
                    xform(lx1, ly0, lz1),
                    xform(lx1, ly0, lz0),
                    xform(lx1, ly1, lz0),
                    xform(lx1, ly1, lz1),
                ],
            ),
            // Left (-X)
            (
                xform_n(-1.0, 0.0, 0.0),
                [
                    xform(lx0, ly0, lz0),
                    xform(lx0, ly0, lz1),
                    xform(lx0, ly1, lz1),
                    xform(lx0, ly1, lz0),
                ],
            ),
            // Top (+Y)
            (
                xform_n(0.0, 1.0, 0.0),
                [
                    xform(lx0, ly1, lz1),
                    xform(lx1, ly1, lz1),
                    xform(lx1, ly1, lz0),
                    xform(lx0, ly1, lz0),
                ],
            ),
            // Bottom (-Y)
            (
                xform_n(0.0, -1.0, 0.0),
                [
                    xform(lx0, ly0, lz0),
                    xform(lx1, ly0, lz0),
                    xform(lx1, ly0, lz1),
                    xform(lx0, ly0, lz1),
                ],
            ),
        ];

        for (normal, verts) in faces {
            let base = mesh.vertices.len() as u32;
            for i in 0..4 {
                mesh.vertices.push(VoxelVertex {
                    position: verts[i],
                    uv: uvs[i],
                    normal,
                    ao: 1.0,
                    light: [1.0, 0.0],
                });
            }
            mesh.indices.extend_from_slice(&[
                base,
                base + 1,
                base + 2,
                base + 2,
                base + 3,
                base,
            ]);
        }
    };

    for &(pos, output_eu_s, facing) in turbines {
        let (hx, hz) = match facing & 0x03 {
            1 => (0.5, 0.10), // North (-Z)
            2 => (0.90, 0.5), // East (+X)
            3 => (0.10, 0.5), // West (-X)
            _ => (0.5, 0.90), // South (+Z)
        };
        let cx = pos.x as f32 + hx;
        let cy = pos.y as f32 + 3.72;
        let cz = pos.z as f32 + hz;
        let phase = ((pos.x.wrapping_mul(31) ^ pos.z.wrapping_mul(17)) as f32) * 0.7;
        let spin_speed = if output_eu_s > 0.0 {
            1.6 + output_eu_s * 0.075
        } else {
            0.0
        };
        let base_angle = total_time * spin_speed + phase;

        // Rotating central nose spinner hub
        add_rot_box(
            &mut mesh,
            cx,
            cy,
            cz,
            facing,
            base_angle,
            -0.14,
            -0.14,
            -0.06,
            0.14,
            0.14,
            0.12,
            hub_uv,
        );

        // 3 long aerodynamic blades spaced 120 degrees apart
        for b in 0..3 {
            let angle = base_angle + (b as f32) * (std::f32::consts::TAU / 3.0);
            // Inner root section
            add_rot_box(
                &mut mesh,
                cx,
                cy,
                cz,
                facing,
                angle,
                -0.10,
                0.12,
                -0.04,
                0.10,
                1.05,
                0.04,
                blade_uv,
            );
            // Tapered outer tip section
            add_rot_box(
                &mut mesh,
                cx,
                cy,
                cz,
                facing,
                angle,
                -0.07,
                1.05,
                -0.03,
                0.07,
                1.85,
                0.03,
                blade_uv,
            );
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockgame_core::chunk::ChunkPos;

    #[test]
    fn test_single_block_faces() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        chunk.set(8, 8, 8, BlockId::STONE);
        let mesh = mesher.mesh_chunk(&chunk);

        // A single block surrounded by air must have 6 faces (24 vertices, 36 indices)
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_face_culling_between_adjacent_blocks() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Place two adjacent stone blocks along X axis
        chunk.set(8, 8, 8, BlockId::STONE);
        chunk.set(9, 8, 8, BlockId::STONE);
        let mesh = mesher.mesh_chunk(&chunk);

        // 2 isolated blocks would have 12 faces, but the 2 touching faces are culled -> 10 faces!
        assert_eq!(mesh.face_count(), 10);
    }

    #[test]
    fn test_solid_chunk_culls_all_internal_faces() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Fill entire 16x16x16 chunk with stone
        chunk.fill(BlockId::STONE);
        let mesh = mesher.mesh_chunk(&chunk);

        // Only the outer 6 boundary faces should exist:
        // 6 sides * 16 * 16 = 1536 faces
        // Naive meshing would have 4096 * 6 = 24576 faces!
        assert_eq!(mesh.face_count(), 1536);
        assert_eq!(mesh.vertices.len(), 1536 * 4);
        assert_eq!(mesh.indices.len(), 1536 * 6);
    }

    #[test]
    fn test_torch_slender_geometry() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        chunk.set(4, 4, 4, BlockId::TORCH);
        let mesh = mesher.mesh_chunk(&chunk);

        // Torch has 6 quads (24 vertices, 36 indices)
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);

        // Verify vertices are slender and centered around (4.5, 4.5), not reaching unit cube borders (4.0 or 5.0) on X or Z
        for v in &mesh.vertices {
            assert!(v.position[0] >= 4.4 && v.position[0] <= 4.6);
            assert!(v.position[2] >= 4.4 && v.position[2] <= 4.6);
            assert!(v.position[1] >= 4.0 && v.position[1] <= 4.7);
        }

        // Verify top face (+Y normal) has valid non-transparent flame UV coordinates (rows 12..16 / 32, i.e. 6.0/16.0 to 8.0/16.0)
        let top_verts: Vec<_> = mesh.vertices.iter().filter(|v| v.normal[1] > 0.9).collect();
        assert_eq!(top_verts.len(), 4, "Torch must have 4 top face vertices");
        let [_u_min, v_min, _u_max, v_max] = crate::TextureAtlas::get_uv_bounds(13);
        let v_h = v_max - v_min;
        for tv in &top_verts {
            assert!(
                tv.uv[1] >= v_min + v_h * (5.9 / 16.0) && tv.uv[1] <= v_min + v_h * (8.1 / 16.0),
                "Top face UV V must sample flame head rows, got {}",
                tv.uv[1]
            );
        }
    }

    #[test]
    fn test_build_item_drops_mesh() {
        let item_registry = blockgame_core::item::ItemRegistry::new();
        let drops = vec![blockgame_core::entity::ItemDrop::new(
            glam::Vec3::new(10.0, 5.0, 10.0),
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 3),
        )];

        let mesh = build_item_drops_mesh(&drops, &item_registry);
        // Mini cube has 6 faces (24 vertices, 36 indices)
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);

        // Torch drop
        let torch_drops = vec![blockgame_core::entity::ItemDrop::new(
            glam::Vec3::new(10.0, 5.0, 10.0),
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TORCH, 1),
        )];
        let torch_mesh = build_item_drops_mesh(&torch_drops, &item_registry);
        assert_eq!(torch_mesh.face_count(), 6);
        assert_eq!(torch_mesh.vertices.len(), 24);
        assert_eq!(torch_mesh.indices.len(), 36);
        // Torch drop is self-illuminated: light == [1.0, 1.0]
        assert_eq!(torch_mesh.vertices[0].light, [1.0, 1.0]);

        // Flat item drops (e.g. Coal, Raw Iron) have 2 faces (front & back: 8 vertices, 12 indices)
        let coal_drops = vec![blockgame_core::entity::ItemDrop::new(
            glam::Vec3::new(10.0, 5.0, 10.0),
            blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::COAL, 1),
        )];
        let coal_mesh = build_item_drops_mesh(&coal_drops, &item_registry);
        assert_eq!(coal_mesh.face_count(), 2);
        assert_eq!(coal_mesh.vertices.len(), 8);
        assert_eq!(coal_mesh.indices.len(), 12);
    }

    #[test]
    fn test_crop_meshing() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        chunk.set(4, 4, 4, BlockId::WHEAT_3);
        let mesh = mesher.mesh_chunk(&chunk);

        // 2 double-sided quads = 4 faces (16 vertices, 24 indices)
        assert_eq!(mesh.face_count(), 4);
        assert_eq!(mesh.vertices.len(), 16);
        assert_eq!(mesh.indices.len(), 24);
    }

    #[test]
    fn test_build_animals_mesh() {
        let animals = vec![
            AnimalEntity::new(1, AnimalKind::Cow, glam::Vec3::new(5.0, 64.0, 5.0)),
            AnimalEntity::new(2, AnimalKind::Pig, glam::Vec3::new(10.0, 64.0, 10.0)),
            AnimalEntity::new(3, AnimalKind::Chicken, glam::Vec3::new(15.0, 64.0, 15.0)),
        ];

        let mesh = build_animals_mesh(&animals);
        assert!(!mesh.is_empty());
        assert!(mesh.vertices.len() > 100);
        assert!(mesh.indices.len() > 100);
    }

    #[test]
    fn test_water_mesh_separation() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        chunk.set(2, 2, 2, BlockId::STONE);
        chunk.set(8, 8, 8, BlockId::WATER);
        let mesh = mesher.mesh_chunk(&chunk);

        // Stone block generates 6 opaque faces (24 vertices, 36 indices)
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);

        // Water block generates 6 translucent faces (24 vertices, 36 indices) in water buffers
        assert_eq!(mesh.water_vertices.len(), 24);
        assert_eq!(mesh.water_indices.len(), 36);
        assert_eq!(mesh.face_count(), 12);
    }

    #[test]
    fn test_seagrass_waterlogged_and_crossed_quads() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Isolated seagrass in air: crossed quads (16 vertices, 24 indices)
        // and waterlogged water faces (24 vertices, 36 indices)
        chunk.set(4, 4, 4, BlockId::SEAGRASS);
        let mesh = mesher.mesh_chunk(&chunk);

        assert_eq!(mesh.vertices.len(), 16);
        assert_eq!(mesh.indices.len(), 24);
        assert_eq!(mesh.water_vertices.len(), 24);
        assert_eq!(mesh.water_indices.len(), 36);

        // When surrounded by water, seagrass should render its crossed quads
        // and NO internal water faces between water and seagrass
        let mut water_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        water_chunk.set(4, 4, 4, BlockId::SEAGRASS);
        water_chunk.set(4, 5, 4, BlockId::WATER); // Above
        water_chunk.set(4, 3, 4, BlockId::WATER); // Below
        water_chunk.set(5, 4, 4, BlockId::WATER); // East
        water_chunk.set(3, 4, 4, BlockId::WATER); // West
        water_chunk.set(4, 4, 5, BlockId::WATER); // South
        water_chunk.set(4, 4, 3, BlockId::WATER); // North

        let submerged_mesh = mesher.mesh_chunk(&water_chunk);

        // Seagrass plant crop vertices: exactly 16
        assert_eq!(submerged_mesh.vertices.len(), 16);
        // The 6 outer water blocks face air outward, but have zero internal faces facing seagrass
        // Each of the 6 surrounding water blocks has 5 faces exposed to air = 30 faces * 4 = 120 vertices
        assert_eq!(submerged_mesh.water_vertices.len(), 120);
    }

    #[test]
    fn test_cow_mesh_generation() {
        let cow = AnimalEntity::new(
            1,
            AnimalKind::Cow,
            glam::Vec3::new(10.0, 64.0, 10.0),
        );
        let mesh = build_animals_mesh(&[cow]);
        assert!(!mesh.vertices.is_empty(), "Cow mesh vertices must not be empty");
        assert!(!mesh.indices.is_empty(), "Cow mesh indices must not be empty");
    }

    #[test]
    fn test_build_falling_blocks_mesh() {
        let registry = BlockRegistry::new();
        let falling = vec![
            blockgame_core::entity::FallingBlock::new(BlockId::SAND, glam::IVec3::new(5, 10, 5)),
            blockgame_core::entity::FallingBlock::new(BlockId::GRAVEL, glam::IVec3::new(6, 12, 6)),
        ];

        let mesh = build_falling_blocks_mesh(&falling, &registry);
        assert_eq!(mesh.face_count(), 12);
        assert_eq!(mesh.vertices.len(), 48);
        assert_eq!(mesh.indices.len(), 72);
    }

    #[test]
    fn test_build_players_mesh() {
        let item_reg = blockgame_core::item::ItemRegistry::new();
        let mut p1 = blockgame_core::net::PlayerModelState::new(
            1,
            "ExplorerAlice",
            glam::Vec3::new(0.0, 64.0, 0.0),
            0,
        );
        p1.held_item = blockgame_core::item::ItemId::DIAMOND_SWORD;
        p1.walk_anim = 1.0;
        p1.swing_anim = 0.5;

        let mesh = build_players_mesh(&[p1], &item_reg);
        // 6 body boxes (2 legs + 1 torso + 1 head + 2 arms) + 1 held item box = 7 boxes * 6 faces = 42 faces
        assert_eq!(mesh.face_count(), 42);
        assert_eq!(mesh.vertices.len(), 42 * 4);
        assert_eq!(mesh.indices.len(), 42 * 6);
    }

    #[test]
    fn test_decorative_block_meshes() {
        let reg = BlockRegistry::new();
        let mesher = ChunkMesher::new(&reg);

        // Stairs: 2 boxes = 12 faces = 48 vertices
        let mut stairs_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        stairs_chunk.set(4, 4, 4, BlockId::OAK_STAIRS);
        let stairs_mesh = mesher.mesh_chunk(&stairs_chunk);
        assert_eq!(stairs_mesh.vertices.len(), 48);

        // Isolated Fence: 1 post box = 6 faces = 24 vertices; connected to neighbor fence adds 2 rail boxes (+48 vertices)
        let mut fence_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        fence_chunk.set(4, 4, 4, BlockId::OAK_FENCE);
        assert_eq!(mesher.mesh_chunk(&fence_chunk).vertices.len(), 24);
        fence_chunk.set(5, 4, 4, BlockId::OAK_FENCE);
        assert_eq!(mesher.mesh_chunk(&fence_chunk).vertices.len(), 2 * (24 + 48));

        // Door (bottom & top, closed & open): 1 box = 24 vertices each
        let mut door_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        door_chunk.set(4, 4, 4, BlockId::OAK_DOOR_CLOSED);
        door_chunk.set(4, 5, 4, BlockId::OAK_DOOR_TOP_CLOSED);
        assert_eq!(mesher.mesh_chunk(&door_chunk).vertices.len(), 48);
        door_chunk.set(4, 4, 4, BlockId::OAK_DOOR_OPEN);
        door_chunk.set(4, 5, 4, BlockId::OAK_DOOR_TOP_OPEN);
        assert_eq!(mesher.mesh_chunk(&door_chunk).vertices.len(), 48);

        // 2-block Bed: foot (1 mattress (5 faces, seam culled) + 2 legs = 68) + head (1 mattress (5 faces, seam culled) + 2 legs = 68) = 136 vertices
        let mut bed_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        bed_chunk.set(4, 4, 4, BlockId::BED);
        bed_chunk.set(4, 4, 3, BlockId::BED_HEAD);
        assert_eq!(mesher.mesh_chunk(&bed_chunk).vertices.len(), 136);

        // Flower (Poppy): crossed quads = 16 vertices
        let mut flower_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        flower_chunk.set(4, 4, 4, BlockId::POPPY);
        assert_eq!(mesher.mesh_chunk(&flower_chunk).vertices.len(), 16);
    }

    #[test]
    fn test_water_chunk_border_unloaded_neighbor_culling() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Water block sitting directly on the +X (x=15) chunk border with air inside the chunk around it
        chunk.set(15, 4, 8, BlockId::WATER);

        // 1. When the +X neighbor chunk is NOT loaded yet (returns None for x >= 16, Some(AIR) elsewhere),
        // the +X boundary wall must NOT be emitted (5 faces = 20 vertices, zero +X face).
        let unloaded_mesh = mesher.mesh_chunk_with_lighting_opt(&chunk, &[], |x, _y, _z| {
            if x >= 16 {
                None
            } else {
                Some(BlockId::AIR)
            }
        });
        assert_eq!(unloaded_mesh.water_vertices.len(), 20);
        assert!(
            unloaded_mesh.water_vertices.iter().all(|v| v.normal[0] < 0.5),
            "Unloaded neighbor chunk must not generate +X water boundary wall"
        );

        // 2. When the +X neighbor chunk IS loaded and contains WATER on x=0, zero +X boundary wall.
        let water_neighbor_mesh = mesher.mesh_chunk_with_lighting_opt(&chunk, &[], |x, y, z| {
            if x == 16 && y == 4 && z == 8 {
                Some(BlockId::WATER)
            } else {
                Some(BlockId::AIR)
            }
        });
        assert_eq!(water_neighbor_mesh.water_vertices.len(), 20);

        // 3. When the +X neighbor chunk IS loaded and contains AIR on x=0, +X water face IS emitted (6 faces = 24 vertices).
        let air_neighbor_mesh =
            mesher.mesh_chunk_with_lighting_opt(&chunk, &[], |_, _, _| Some(BlockId::AIR));
        assert_eq!(air_neighbor_mesh.water_vertices.len(), 24);
    }

    #[test]
    fn test_ice_translucent_mesh_and_culling() {
        let registry = BlockRegistry::new();
        let mesher = ChunkMesher::new(&registry);

        // 1. Ice above Stone: Stone emits all 6 opaque faces (including PosY under transparent Ice),
        // while Ice emits only its 5 exposed faces into translucent water_vertices (not opaque vertices).
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        chunk.set(4, 4, 4, BlockId::STONE);
        chunk.set(4, 5, 4, BlockId::ICE);
        let mesh = mesher.mesh_chunk(&chunk);
        assert_eq!(mesh.vertices.len(), 24, "Stone under Ice must keep all 6 opaque faces");
        assert_eq!(mesh.water_vertices.len(), 20, "Ice must render its 5 exposed faces in translucent pass");

        // 2. Two adjacent Ice blocks cull their shared internal face (10 exposed faces = 40 water_vertices)
        let mut two_ice = Chunk::new(ChunkPos::new(0, 0, 0));
        two_ice.set(4, 5, 4, BlockId::ICE);
        two_ice.set(5, 5, 4, BlockId::ICE);
        let two_ice_mesh = mesher.mesh_chunk(&two_ice);
        assert!(two_ice_mesh.vertices.is_empty(), "Ice must not emit opaque depth-writing vertices");
        assert_eq!(two_ice_mesh.water_vertices.len(), 40);

        // 3. Ice directly above Water: Ice emits 6 faces, Water emits 5 non-Ice faces (zero duplicate coplanar face)
        let mut ice_water = Chunk::new(ChunkPos::new(0, 0, 0));
        ice_water.set(4, 4, 4, BlockId::WATER);
        ice_water.set(4, 5, 4, BlockId::ICE);
        let iw_mesh = mesher.mesh_chunk(&ice_water);
        assert_eq!(iw_mesh.water_vertices.len(), 44);
    }
}
