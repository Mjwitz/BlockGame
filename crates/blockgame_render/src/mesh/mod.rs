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
    pub fn mesh_chunk(&self, chunk: &Chunk) -> ChunkMesh {
        self.mesh_chunk_with_neighbors(chunk, |_, _, _| BlockId::AIR)
    }

    /// Meshes a chunk with a neighbor lookup callback for out-of-bounds coordinates.
    pub fn mesh_chunk_with_neighbors<F>(&self, chunk: &Chunk, get_neighbor: F) -> ChunkMesh
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
                            self.build_face(&mut mesh, ix, iy, iz, dir, uv, &query_block);
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
        x: i32,
        y: i32,
        z: i32,
        dir: Direction,
        uv_bounds: [f32; 4],
        query: &F,
    ) where
        F: Fn(i32, i32, i32) -> BlockId,
    {
        let [u_min, v_min, u_max, v_max] = uv_bounds;
        let fx = x as f32;
        let fy = y as f32;
        let fz = z as f32;
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
}
