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
        let mut mesh = ChunkMesh::new();

        let query_block = |x: i32, y: i32, z: i32| -> BlockId {
            if x >= 0
                && x < CHUNK_SIZE_X as i32
                && y >= 0
                && y < CHUNK_SIZE_Y as i32
                && z >= 0
                && z < CHUNK_SIZE_Z as i32
            {
                chunk.get_unchecked(x as usize, y as usize, z as usize)
            } else {
                get_neighbor(x, y, z)
            }
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

                    if block_id.is_stairs() {
                        self.build_stairs(&mut mesh, chunk.pos, ix, iy, iz, block_id, &light_map);
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

                    if block_id.is_door() {
                        self.build_door(
                            &mut mesh,
                            chunk.pos,
                            ix,
                            iy,
                            iz,
                            block_id.is_door_open(),
                            block_id.is_door_top(),
                            &light_map,
                        );
                        continue;
                    }

                    if block_id == BlockId::BED {
                        self.build_bed(&mut mesh, chunk.pos, ix, iy, iz, &light_map);
                        continue;
                    }

                    if block_id == BlockId::SEAGRASS {
                        // 1. Build crossed diagonal plant quads for seagrass
                        self.build_crop(&mut mesh, chunk.pos, ix, iy, iz, block_id, &light_map);

                        // 2. Seagrass is waterlogged! Generate water boundary faces for any direction
                        // not adjacent to water or another waterlogged block.
                        for dir in Direction::ALL {
                            let normal = dir.normal_ivec();
                            let nx = ix + normal.x;
                            let ny = iy + normal.y;
                            let nz = iz + normal.z;

                            let neighbor_id = query_block(nx, ny, nz);

                            if neighbor_id.is_water() {
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

                            let neighbor_id = query_block(nx, ny, nz);

                            // Adjacent water blocks (and waterlogged seagrass) should not render internal faces
                            if neighbor_id.is_water() {
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
                            let tile_id = self.registry.get_texture(block_id, dir);
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
            [fx + 1.0, fy, fz],
            [fx, fy, fz + 1.0],
            [fx + 1.0, fy + 1.0, fz + 1.0],
            [fx, fy + 1.0, fz],
        ];
        let diag2_back = [
            [fx, fy, fz + 1.0],
            [fx + 1.0, fy, fz],
            [fx + 1.0, fy + 1.0, fz],
            [fx, fy + 1.0, fz + 1.0],
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

    fn build_stairs(
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
        // Upper back step
        self.add_box(
            mesh,
            fx,
            fy + 0.5,
            fz,
            fx + 1.0,
            fy + 1.0,
            fz + 0.5,
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
        light_map: &blockgame_core::lighting::ChunkLightMap,
    ) {
        let origin = chunk_pos.to_world_origin();
        let fx = (origin.x + x) as f32;
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let tile = if is_top { 333 } else { 334 };
        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        if is_open {
            self.add_box(
                mesh,
                fx,
                fy,
                fz,
                fx + 0.1875,
                fy + 1.0,
                fz + 1.0,
                [tile; 6],
                light,
            );
        } else {
            self.add_box(
                mesh,
                fx,
                fy,
                fz,
                fx + 1.0,
                fy + 1.0,
                fz + 0.1875,
                [tile; 6],
                light,
            );
        }
    }

    fn build_bed(
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
        let fy = (origin.y + y) as f32;
        let fz = (origin.z + z) as f32;

        let lv = light_map.get(x as usize, y as usize, z as usize);
        let light = [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0];

        // Mattress & frame
        self.add_box(
            mesh,
            fx + 0.05,
            fy + 0.1875,
            fz + 0.05,
            fx + 0.95,
            fy + 0.5625,
            fz + 0.95,
            [318, 318, 318, 318, 317, 4],
            light,
        );
        // 4 corner oak legs
        for (lx0, lx1) in [(0.05, 0.22), (0.78, 0.95)] {
            for (lz0, lz1) in [(0.05, 0.22), (0.78, 0.95)] {
                self.add_box(
                    mesh,
                    fx + lx0,
                    fy,
                    fz + lz0,
                    fx + lx1,
                    fy + 0.1875,
                    fz + lz1,
                    [4; 6],
                    light,
                );
            }
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

        // Bed: 1 mattress + 4 legs = 5 boxes = 120 vertices
        let mut bed_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        bed_chunk.set(4, 4, 4, BlockId::BED);
        assert_eq!(mesher.mesh_chunk(&bed_chunk).vertices.len(), 120);

        // Flower (Poppy): crossed quads = 16 vertices
        let mut flower_chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        flower_chunk.set(4, 4, 4, BlockId::POPPY);
        assert_eq!(mesher.mesh_chunk(&flower_chunk).vertices.len(), 16);
    }
}
