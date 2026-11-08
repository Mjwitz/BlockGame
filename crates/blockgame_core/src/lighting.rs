use std::collections::VecDeque;

use glam::IVec3;

use crate::{
    block::BlockRegistry,
    chunk::{Chunk, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, CHUNK_VOLUME},
    direction::Direction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightValue(pub u8);

impl LightValue {
    pub const ZERO: Self = Self(0);

    #[inline(always)]
    pub const fn new(sun: u8, block: u8) -> Self {
        Self(((sun & 0x0F) << 4) | (block & 0x0F))
    }

    #[inline(always)]
    pub const fn sun(&self) -> u8 {
        (self.0 >> 4) & 0x0F
    }

    #[inline(always)]
    pub const fn block(&self) -> u8 {
        self.0 & 0x0F
    }

    #[inline(always)]
    pub fn with_sun(&self, sun: u8) -> Self {
        Self::new(sun, self.block())
    }

    #[inline(always)]
    pub fn with_block(&self, block: u8) -> Self {
        Self::new(self.sun(), block)
    }
}

#[derive(Clone)]
pub struct ChunkLightMap {
    data: [u8; CHUNK_VOLUME],
}

impl Default for ChunkLightMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkLightMap {
    pub fn new() -> Self {
        Self {
            data: [0; CHUNK_VOLUME],
        }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> LightValue {
        LightValue(self.data[Chunk::index(x, y, z)])
    }

    #[inline(always)]
    pub fn set(&mut self, x: usize, y: usize, z: usize, val: LightValue) {
        self.data[Chunk::index(x, y, z)] = val.0;
    }

    #[inline(always)]
    pub fn get_sun(&self, x: usize, y: usize, z: usize) -> u8 {
        self.get(x, y, z).sun()
    }

    #[inline(always)]
    pub fn set_sun(&mut self, x: usize, y: usize, z: usize, sun: u8) {
        let cur = self.get(x, y, z);
        self.set(x, y, z, cur.with_sun(sun));
    }

    #[inline(always)]
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> u8 {
        self.get(x, y, z).block()
    }

    #[inline(always)]
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: u8) {
        let cur = self.get(x, y, z);
        self.set(x, y, z, cur.with_block(block));
    }

    #[inline]
    pub fn raw_data(&self) -> &[u8; CHUNK_VOLUME] {
        &self.data
    }
}

/// Computes sunlight and blocklight for a chunk using Breadth-First Search (BFS) flood-fill.
pub fn calculate_chunk_lighting(
    chunk: &Chunk,
    registry: &BlockRegistry,
) -> ChunkLightMap {
    let mut light_map = ChunkLightMap::new();
    let mut sun_queue = VecDeque::new();
    let mut block_queue = VecDeque::new();

    // 1. Vertical Sunlight Pass: cast rays down from the ceiling
    for z in 0..CHUNK_SIZE_Z {
        for x in 0..CHUNK_SIZE_X {
            for y in (0..CHUNK_SIZE_Y).rev() {
                let block = chunk.get_unchecked(x, y, z);
                if registry.is_transparent(block) {
                    light_map.set_sun(x, y, z, 15);
                    sun_queue.push_back(IVec3::new(x as i32, y as i32, z as i32));
                } else {
                    // Struck first solid block in this column; sunlight cannot penetrate further directly
                    break;
                }
            }
        }
    }

    // 2. Identify emissive blocks (torches, glowstones, lava)
    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let block = chunk.get_unchecked(x, y, z);
                if let Some(def) = registry.get(block) {
                    if def.luminance > 0 {
                        light_map.set_block(x, y, z, def.luminance);
                        block_queue.push_back(IVec3::new(x as i32, y as i32, z as i32));
                    }
                }
            }
        }
    }

    // 3. BFS Sunlight Flood-Fill (lateral diffusion under trees and overhangs)
    while let Some(pos) = sun_queue.pop_front() {
        let current_light = light_map.get_sun(pos.x as usize, pos.y as usize, pos.z as usize);
        if current_light <= 1 {
            continue;
        }

        for dir in Direction::ALL {
            let next = pos + dir.normal_ivec();
            if next.x >= 0
                && next.x < CHUNK_SIZE_X as i32
                && next.y >= 0
                && next.y < CHUNK_SIZE_Y as i32
                && next.z >= 0
                && next.z < CHUNK_SIZE_Z as i32
            {
                let nx = next.x as usize;
                let ny = next.y as usize;
                let nz = next.z as usize;
                let neighbor_block = chunk.get_unchecked(nx, ny, nz);

                if registry.is_transparent(neighbor_block) {
                    let neighbor_sun = light_map.get_sun(nx, ny, nz);
                    if neighbor_sun < current_light - 1 {
                        light_map.set_sun(nx, ny, nz, current_light - 1);
                        sun_queue.push_back(next);
                    }
                }
            }
        }
    }

    // 4. BFS Blocklight Flood-Fill (radial light from emissive light sources)
    while let Some(pos) = block_queue.pop_front() {
        let current_light = light_map.get_block(pos.x as usize, pos.y as usize, pos.z as usize);
        if current_light <= 1 {
            continue;
        }

        for dir in Direction::ALL {
            let next = pos + dir.normal_ivec();
            if next.x >= 0
                && next.x < CHUNK_SIZE_X as i32
                && next.y >= 0
                && next.y < CHUNK_SIZE_Y as i32
                && next.z >= 0
                && next.z < CHUNK_SIZE_Z as i32
            {
                let nx = next.x as usize;
                let ny = next.y as usize;
                let nz = next.z as usize;
                let neighbor_block = chunk.get_unchecked(nx, ny, nz);

                if registry.is_transparent(neighbor_block) {
                    let neighbor_light = light_map.get_block(nx, ny, nz);
                    if neighbor_light < current_light - 1 {
                        light_map.set_block(nx, ny, nz, current_light - 1);
                        block_queue.push_back(next);
                    }
                }
            }
        }
    }

    light_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockId;
    use crate::chunk::ChunkPos;

    #[test]
    fn test_light_value_packing() {
        let lv = LightValue::new(15, 7);
        assert_eq!(lv.sun(), 15);
        assert_eq!(lv.block(), 7);

        let lv2 = lv.with_sun(12);
        assert_eq!(lv2.sun(), 12);
        assert_eq!(lv2.block(), 7);
    }

    #[test]
    fn test_sunlight_vertical_penetration() {
        let registry = BlockRegistry::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Place solid stone floor at y = 8
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 8, z, BlockId::STONE);
            }
        }

        let light = calculate_chunk_lighting(&chunk, &registry);

        // Above the floor (y = 9..15), sunlight must be 15
        assert_eq!(light.get_sun(5, 12, 5), 15);
        assert_eq!(light.get_sun(5, 9, 5), 15);

        // Below the floor (y = 0..7), direct vertical sunlight is blocked
        // (with no lateral entrance in an enclosed chunk, it's 0)
        assert_eq!(light.get_sun(5, 7, 5), 0);
        assert_eq!(light.get_sun(5, 0, 5), 0);
    }

    #[test]
    fn test_torch_blocklight_propagation() {
        use crate::block::BlockId;
        let registry = BlockRegistry::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Enclose chunk ceiling to block sunlight
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 15, z, BlockId::STONE);
            }
        }

        // Place a torch (luminance 14) at (8, 5, 8)
        chunk.set(8, 5, 8, BlockId::TORCH);

        let light = calculate_chunk_lighting(&chunk, &registry);

        // Torch voxel itself has luminance 14
        assert_eq!(light.get_block(8, 5, 8), 14);

        // Adjacent voxels have light level 13 (attenuated by 1)
        assert_eq!(light.get_block(9, 5, 8), 13);
        assert_eq!(light.get_block(7, 5, 8), 13);
        assert_eq!(light.get_block(8, 6, 8), 13);
        assert_eq!(light.get_block(8, 4, 8), 13);

        // 2 steps away has light level 12
        assert_eq!(light.get_block(10, 5, 8), 12);
        assert_eq!(light.get_block(8, 7, 8), 12);
    }
}
