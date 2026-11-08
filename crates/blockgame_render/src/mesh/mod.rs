pub mod vertex;

use blockgame_core::{
    block::{BlockId, BlockRegistry},
    chunk::{Chunk, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    direction::Direction,
};
pub use vertex::VoxelVertex;

use crate::atlas::TextureAtlas;

pub struct ChunkMesh {
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
}

impl ChunkMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn face_count(&self) -> usize {
        self.indices.len() / 6
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

                    for dir in Direction::ALL {
                        let normal = dir.normal_ivec();
                        let nx = ix + normal.x;
                        let ny = iy + normal.y;
                        let nz = iz + normal.z;

                        let neighbor_id = query_block(nx, ny, nz);

                        // Only generate face if the neighbor block is transparent
                        if self.registry.is_transparent(neighbor_id) {
                            let tile_id = self.registry.get_texture(block_id, dir);
                            let uv = TextureAtlas::get_uv_bounds(tile_id);
                            
                            // Sample light from adjacent air block (supports boundary voxels from [-1..16])
                            let light = if nx >= 0 && nx < CHUNK_SIZE_X as i32
                                && ny >= 0 && ny < CHUNK_SIZE_Y as i32
                                && nz >= 0 && nz < CHUNK_SIZE_Z as i32 {
                                let lv = light_map.get(nx as usize, ny as usize, nz as usize);
                                [lv.sun() as f32 / 15.0, lv.block() as f32 / 15.0]
                            } else {
                                let blk = light_map.get_block_rel(nx, ny, nz);
                                [1.0, blk as f32 / 15.0]
                            };

                            self.build_face(&mut mesh, chunk.pos, ix, iy, iz, dir, uv, light, &query_block);
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
        let v_side_top = v_min + v_height * (1.0 / 16.0);
        let v_side_bottom = v_min + v_height * (16.0 / 16.0);

        let u_top_left = u_min + u_width * (7.0 / 16.0);
        let u_top_right = u_min + u_width * (9.0 / 16.0);
        let v_top_min = v_min + v_height * (2.0 / 16.0);
        let v_top_max = v_min + v_height * (4.0 / 16.0);

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
            [[u_side_left, v_side_bottom], [u_side_right, v_side_bottom], [u_side_right, v_side_top], [u_side_left, v_side_top]],
            [0.0, 0.0, 1.0],
        );

        // 2. North Face (-Z)
        add_quad(
            [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
            [[u_side_right, v_side_bottom], [u_side_left, v_side_bottom], [u_side_left, v_side_top], [u_side_right, v_side_top]],
            [0.0, 0.0, -1.0],
        );

        // 3. East Face (+X)
        add_quad(
            [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            [[u_side_left, v_side_bottom], [u_side_right, v_side_bottom], [u_side_right, v_side_top], [u_side_left, v_side_top]],
            [1.0, 0.0, 0.0],
        );

        // 4. West Face (-X)
        add_quad(
            [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
            [[u_side_left, v_side_bottom], [u_side_right, v_side_bottom], [u_side_right, v_side_top], [u_side_left, v_side_top]],
            [-1.0, 0.0, 0.0],
        );

        // 5. Top Face (+Y)
        add_quad(
            [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
            [[u_top_left, v_top_max], [u_top_right, v_top_max], [u_top_right, v_top_min], [u_top_left, v_top_min]],
            [0.0, 1.0, 0.0],
        );

        // 6. Bottom Face (-Y)
        add_quad(
            [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
            [[u_top_left, v_top_min], [u_top_right, v_top_min], [u_top_right, v_top_max], [u_top_left, v_top_max]],
            [0.0, -1.0, 0.0],
        );
    }

    fn calculate_face_ao<F>(
        &self,
        x: i32,
        y: i32,
        z: i32,
        dir: Direction,
        query: &F,
    ) -> [f32; 4]
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
            let rh = 0.12;  // height radius (total height ~0.24)
            let light = [1.0, 1.0]; // Self-illuminated warm torch glow
            let ao = 1.0;

            let [u_min, v_min, u_max, v_max] = TextureAtlas::get_uv_bounds(13);
            let u_width = u_max - u_min;
            let v_height = v_max - v_min;

            let u_side_left = u_min + u_width * (7.0 / 16.0);
            let u_side_right = u_min + u_width * (9.0 / 16.0);
            let v_side_top = v_min + v_height * (1.0 / 16.0);
            let v_side_bottom = v_min + v_height * (16.0 / 16.0);

            let side_uvs = [
                [u_side_left, v_side_bottom],
                [u_side_right, v_side_bottom],
                [u_side_right, v_side_top],
                [u_side_left, v_side_top],
            ];

            let u_top_left = u_min + u_width * (7.0 / 16.0);
            let u_top_right = u_min + u_width * (9.0 / 16.0);
            let v_top_min = v_min + v_height * (2.0 / 16.0);
            let v_top_max = v_min + v_height * (4.0 / 16.0);

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
    }

    #[test]
    fn test_build_item_drops_mesh() {
        let item_registry = blockgame_core::item::ItemRegistry::new();
        let drops = vec![
            blockgame_core::entity::ItemDrop::new(
                glam::Vec3::new(10.0, 5.0, 10.0),
                blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::WOOD_LOG, 3),
            ),
        ];

        let mesh = build_item_drops_mesh(&drops, &item_registry);
        // Mini cube has 6 faces (24 vertices, 36 indices)
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);

        // Torch drop
        let torch_drops = vec![
            blockgame_core::entity::ItemDrop::new(
                glam::Vec3::new(10.0, 5.0, 10.0),
                blockgame_core::item::ItemSlot::new(blockgame_core::item::ItemId::TORCH, 1),
            ),
        ];
        let torch_mesh = build_item_drops_mesh(&torch_drops, &item_registry);
        assert_eq!(torch_mesh.face_count(), 6);
        assert_eq!(torch_mesh.vertices.len(), 24);
        assert_eq!(torch_mesh.indices.len(), 36);
        // Torch drop is self-illuminated: light == [1.0, 1.0]
        assert_eq!(torch_mesh.vertices[0].light, [1.0, 1.0]);
    }
}
