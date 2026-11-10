use crate::block::BlockId;
use glam::IVec3;

pub const CHUNK_SIZE_X: usize = 16;
pub const CHUNK_SIZE_Y: usize = 16;
pub const CHUNK_SIZE_Z: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z; // 4096
pub const VERTICAL_CHUNKS: i32 = 5; // 5 chunks high = 80 blocks vertical world height

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos(pub IVec3);

impl ChunkPos {
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    #[inline]
    pub fn to_world_origin(&self) -> IVec3 {
        IVec3::new(
            self.0.x * CHUNK_SIZE_X as i32,
            self.0.y * CHUNK_SIZE_Y as i32,
            self.0.z * CHUNK_SIZE_Z as i32,
        )
    }
}

#[derive(Clone)]
pub struct Chunk {
    pub pos: ChunkPos,
    blocks: [BlockId; CHUNK_VOLUME],
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            blocks: [BlockId::AIR; CHUNK_VOLUME],
        }
    }

    #[inline(always)]
    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        x | (z << 4) | (y << 8)
    }

    #[inline(always)]
    pub const fn coords_from_index(index: usize) -> (usize, usize, usize) {
        let x = index & 0x0F;
        let z = (index >> 4) & 0x0F;
        let y = (index >> 8) & 0x0F;
        (x, y, z)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<BlockId> {
        if x < CHUNK_SIZE_X && y < CHUNK_SIZE_Y && z < CHUNK_SIZE_Z {
            Some(self.blocks[Self::index(x, y, z)])
        } else {
            None
        }
    }

    #[inline]
    pub fn get_unchecked(&self, x: usize, y: usize, z: usize) -> BlockId {
        self.blocks[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, id: BlockId) -> bool {
        if x < CHUNK_SIZE_X && y < CHUNK_SIZE_Y && z < CHUNK_SIZE_Z {
            self.blocks[Self::index(x, y, z)] = id;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn raw_blocks(&self) -> &[BlockId; CHUNK_VOLUME] {
        &self.blocks
    }

    #[inline]
    pub fn raw_blocks_mut(&mut self) -> &mut [BlockId; CHUNK_VOLUME] {
        &mut self.blocks
    }

    /// Fills the entire chunk with a specific block.
    pub fn fill(&mut self, id: BlockId) {
        self.blocks.fill(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing_bijection() {
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    let idx = Chunk::index(x, y, z);
                    assert!(idx < CHUNK_VOLUME);
                    let (rx, ry, rz) = Chunk::coords_from_index(idx);
                    assert_eq!((x, y, z), (rx, ry, rz));
                }
            }
        }
    }

    #[test]
    fn test_get_set() {
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        assert_eq!(chunk.get(5, 7, 3), Some(BlockId::AIR));
        chunk.set(5, 7, 3, BlockId::GRASS);
        assert_eq!(chunk.get(5, 7, 3), Some(BlockId::GRASS));
        assert_eq!(chunk.get(16, 0, 0), None);
    }
}
