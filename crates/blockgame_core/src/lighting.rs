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

pub const BOUNDARY_DIM: usize = 18; // covers [-1..=16]
pub const BOUNDARY_VOLUME: usize = BOUNDARY_DIM * BOUNDARY_DIM * BOUNDARY_DIM; // 5832

#[derive(Clone)]
pub struct ChunkLightMap {
    data: [u8; CHUNK_VOLUME],
    boundary_block: [u8; BOUNDARY_VOLUME],
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
            boundary_block: [0; BOUNDARY_VOLUME],
        }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> LightValue {
        LightValue(self.data[Chunk::index(x, y, z)])
    }

    #[inline(always)]
    pub fn set(&mut self, x: usize, y: usize, z: usize, val: LightValue) {
        self.data[Chunk::index(x, y, z)] = val.0;
        if x < 16 && y < 16 && z < 16 {
            let b_idx = (x + 1) + (y + 1) * BOUNDARY_DIM + (z + 1) * BOUNDARY_DIM * BOUNDARY_DIM;
            self.boundary_block[b_idx] = val.block();
        }
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

    #[inline(always)]
    pub fn get_block_rel(&self, x: i32, y: i32, z: i32) -> u8 {
        if x >= -1 && x <= 16 && y >= -1 && y <= 16 && z >= -1 && z <= 16 {
            let idx = ((x + 1) as usize)
                + ((y + 1) as usize) * BOUNDARY_DIM
                + ((z + 1) as usize) * BOUNDARY_DIM * BOUNDARY_DIM;
            self.boundary_block[idx]
        } else {
            0
        }
    }

    #[inline(always)]
    pub fn set_block_rel(&mut self, x: i32, y: i32, z: i32, block: u8) {
        if x >= -1 && x <= 16 && y >= -1 && y <= 16 && z >= -1 && z <= 16 {
            let idx = ((x + 1) as usize)
                + ((y + 1) as usize) * BOUNDARY_DIM
                + ((z + 1) as usize) * BOUNDARY_DIM * BOUNDARY_DIM;
            self.boundary_block[idx] = block;
        }
        if x >= 0
            && x < CHUNK_SIZE_X as i32
            && y >= 0
            && y < CHUNK_SIZE_Y as i32
            && z >= 0
            && z < CHUNK_SIZE_Z as i32
        {
            self.set_block(x as usize, y as usize, z as usize, block);
        }
    }

    #[inline]
    pub fn raw_data(&self) -> &[u8; CHUNK_VOLUME] {
        &self.data
    }
}

const PAD: i32 = 15;
const PADDED_DIM: usize = 46; // [-15..=30]
const PADDED_VOLUME: usize = PADDED_DIM * PADDED_DIM * PADDED_DIM;

#[inline(always)]
fn padded_index(x: i32, y: i32, z: i32) -> usize {
    ((x + PAD) as usize)
        + ((y + PAD) as usize) * PADDED_DIM
        + ((z + PAD) as usize) * PADDED_DIM * PADDED_DIM
}

/// Computes sunlight and cross-chunk blocklight using Breadth-First Search (BFS) flood-fill.
pub fn calculate_chunk_lighting_with_sources<F>(
    chunk: &Chunk,
    registry: &BlockRegistry,
    light_sources_rel: &[(IVec3, u8)],
    query_block: F,
) -> ChunkLightMap
where
    F: Fn(i32, i32, i32) -> crate::block::BlockId,
{
    let mut light_map = ChunkLightMap::new();
    let mut sun_queue = VecDeque::new();

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

    // 2. BFS Sunlight Flood-Fill (lateral diffusion under trees and overhangs)
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

    // 3. Blocklight flood fill on padded grid
    let mut grid = vec![0u8; PADDED_VOLUME];
    let mut block_queue = VecDeque::new();

    // Add internal emissives from this chunk
    for y in 0..CHUNK_SIZE_Y {
        for z in 0..CHUNK_SIZE_Z {
            for x in 0..CHUNK_SIZE_X {
                let block = chunk.get_unchecked(x, y, z);
                if let Some(def) = registry.get(block) {
                    if def.luminance > 0 {
                        let idx = padded_index(x as i32, y as i32, z as i32);
                        grid[idx] = def.luminance;
                        block_queue.push_back(IVec3::new(x as i32, y as i32, z as i32));
                    }
                }
            }
        }
    }

    // Add neighbor light sources in reach
    for &(rel_pos, lum) in light_sources_rel {
        if rel_pos.x >= -PAD
            && rel_pos.x <= 15 + PAD
            && rel_pos.y >= -PAD
            && rel_pos.y <= 15 + PAD
            && rel_pos.z >= -PAD
            && rel_pos.z <= 15 + PAD
        {
            let idx = padded_index(rel_pos.x, rel_pos.y, rel_pos.z);
            if lum > grid[idx] {
                grid[idx] = lum;
                block_queue.push_back(rel_pos);
            }
        }
    }

    // BFS Blocklight Flood-Fill across chunk boundaries
    while let Some(pos) = block_queue.pop_front() {
        let cur_light = grid[padded_index(pos.x, pos.y, pos.z)];
        if cur_light <= 1 {
            continue;
        }

        for dir in Direction::ALL {
            let next = pos + dir.normal_ivec();
            if next.x >= -PAD
                && next.x <= 15 + PAD
                && next.y >= -PAD
                && next.y <= 15 + PAD
                && next.z >= -PAD
                && next.z <= 15 + PAD
            {
                let next_idx = padded_index(next.x, next.y, next.z);
                if grid[next_idx] < cur_light - 1 {
                    let is_transp = if next.x >= 0
                        && next.x < CHUNK_SIZE_X as i32
                        && next.y >= 0
                        && next.y < CHUNK_SIZE_Y as i32
                        && next.z >= 0
                        && next.z < CHUNK_SIZE_Z as i32
                    {
                        registry.is_transparent(chunk.get_unchecked(
                            next.x as usize,
                            next.y as usize,
                            next.z as usize,
                        ))
                    } else {
                        registry.is_transparent(query_block(next.x, next.y, next.z))
                    };

                    if is_transp {
                        grid[next_idx] = cur_light - 1;
                        block_queue.push_back(next);
                    }
                }
            }
        }
    }

    // Transfer blocklight to boundary_block (covering [-1..=16]) and data (covering [0..16])
    for z in -1..=16 {
        for y in -1..=16 {
            for x in -1..=16 {
                let light_val = grid[padded_index(x, y, z)];
                light_map.set_block_rel(x, y, z, light_val);
            }
        }
    }

    light_map
}

/// Standalone chunk lighting calculation without external light sources.
pub fn calculate_chunk_lighting(chunk: &Chunk, registry: &BlockRegistry) -> ChunkLightMap {
    calculate_chunk_lighting_with_sources(chunk, registry, &[], |_, _, _| {
        crate::block::BlockId::AIR
    })
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

    #[test]
    fn test_cross_chunk_torch_blocklight_propagation() {
        let registry = BlockRegistry::new();
        let chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Torch located across the chunk boundary at rel (-1, 5, 8) with luminance 14
        let neighbor_sources = vec![(IVec3::new(-1, 5, 8), 14)];
        let light = calculate_chunk_lighting_with_sources(
            &chunk,
            &registry,
            &neighbor_sources,
            |_, _, _| BlockId::AIR,
        );

        // Torch penetrates across border: at x = 0 (1 step away), light level is 13!
        assert_eq!(light.get_block(0, 5, 8), 13);
        // At x = 1 (2 steps away), light level is 12!
        assert_eq!(light.get_block(1, 5, 8), 12);
        // At x = 2 (3 steps away), light level is 11!
        assert_eq!(light.get_block(2, 5, 8), 11);

        // Boundary accessor get_block_rel also reflects the source
        assert_eq!(light.get_block_rel(-1, 5, 8), 14);
        assert_eq!(light.get_block_rel(0, 5, 8), 13);
    }
}
