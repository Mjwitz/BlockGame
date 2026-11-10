use blockgame_core::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    Mountains,
}

pub struct TerrainGenerator {
    pub seed: u32,
    continental_noise: Fbm<Perlin>,
    detail_noise: Fbm<Perlin>,
    cave_noise: Fbm<Perlin>,
    temp_noise: Perlin,
    moisture_noise: Perlin,
    pub water_level: i32,
}

impl TerrainGenerator {
    pub fn new(seed: u32) -> Self {
        let continental_noise = Fbm::<Perlin>::new(seed)
            .set_octaves(3)
            .set_frequency(0.006)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let detail_noise = Fbm::<Perlin>::new(seed.wrapping_add(1013))
            .set_octaves(4)
            .set_frequency(0.02)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let cave_noise = Fbm::<Perlin>::new(seed.wrapping_add(2048))
            .set_octaves(2)
            .set_frequency(0.04)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let temp_noise = Perlin::new(seed.wrapping_add(3072));
        let moisture_noise = Perlin::new(seed.wrapping_add(4096));

        Self {
            seed,
            continental_noise,
            detail_noise,
            cave_noise,
            temp_noise,
            moisture_noise,
            water_level: 38,
        }
    }

    /// Computes the surface elevation at global coordinate (x, z).
    #[inline]
    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        let fx = x as f64;
        let fz = z as f64;

        let cont = self.continental_noise.get([fx, fz]);
        let detail = self.detail_noise.get([fx, fz]);

        // Base elevation around y = 46, varying up to mountains (y ~ 65-72) or water depressions (y ~ 28-37)
        let height = 46.0 + (cont * 18.0) + (detail * 8.0);
        (height.round() as i32).max(1)
    }

    /// Determines the biome at global coordinate (x, z).
    #[inline]
    pub fn get_biome(&self, x: i32, z: i32, height: i32) -> Biome {
        if height > 58 {
            return Biome::Mountains;
        }

        let fx = x as f64 * 0.003;
        let fz = z as f64 * 0.003;

        let temp = self.temp_noise.get([fx, fz]);
        let moisture = self.moisture_noise.get([fx, fz]);

        if temp > 0.3 && moisture < -0.1 {
            Biome::Desert
        } else if moisture > 0.15 {
            Biome::Forest
        } else {
            Biome::Plains
        }
    }

    /// Checks if a 3D coordinate is carved out as a cave.
    #[inline]
    pub fn is_cave(&self, x: i32, y: i32, z: i32) -> bool {
        // No caves at bedrock or in the sky
        if y <= 1 {
            return false;
        }

        let fx = x as f64;
        let fy = y as f64;
        let fz = z as f64;

        let density = self.cave_noise.get([fx, fy, fz]);
        // Cavern threshold
        density > 0.45
    }

    /// Generates the base terrain for a specific chunk (without cross-chunk multi-block structures).
    pub fn generate_chunk(&self, pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new(pos);
        let origin = pos.to_world_origin();

        for lz in 0..CHUNK_SIZE_Z {
            let wz = origin.z + lz as i32;
            for lx in 0..CHUNK_SIZE_X {
                let wx = origin.x + lx as i32;

                let surface_y = self.surface_height(wx, wz);
                let biome = self.get_biome(wx, wz, surface_y);
                let is_underwater = surface_y <= self.water_level;

                for ly in 0..CHUNK_SIZE_Y {
                    let wy = origin.y + ly as i32;

                    let block = if wy == 0 {
                        BlockId::BEDROCK
                    } else if wy < surface_y {
                        // Check if carved by cave system (leave surface top soil intact)
                        if wy < surface_y - 3 && self.is_cave(wx, wy, wz) {
                            BlockId::AIR
                        } else if wy == surface_y - 1 {
                            if is_underwater || surface_y <= self.water_level + 1 {
                                BlockId::SAND
                            } else {
                                match biome {
                                    Biome::Desert => BlockId::SAND,
                                    Biome::Mountains if surface_y > 62 => BlockId::STONE,
                                    _ => BlockId::GRASS,
                                }
                            }
                        } else if wy >= surface_y - 4 {
                            if is_underwater || surface_y <= self.water_level + 1 {
                                BlockId::SAND
                            } else {
                                match biome {
                                    Biome::Desert => BlockId::SAND,
                                    _ => BlockId::DIRT,
                                }
                            }
                        } else {
                            // Subsurface stone with ore veins
                            self.get_subsurface_block(wx, wy, wz)
                        }
                    } else if wy <= self.water_level {
                        BlockId::WATER
                    } else {
                        BlockId::AIR
                    };

                    chunk.set(lx, ly, lz, block);
                }
            }
        }

        chunk
    }

    /// Evaluates underground voxel coordinate to place ore veins (Diamond, Gold, Iron, Copper, Coal)
    /// clustered deterministically into natural underground deposits.
    #[inline]
    pub fn get_subsurface_block(&self, wx: i32, wy: i32, wz: i32) -> BlockId {
        let cx = wx >> 2;
        let cy = wy >> 2;
        let cz = wz >> 2;

        // Diamond Ore: Deep layer (y: 1..16), rare
        if wy >= 1 && wy <= 16 {
            let h = hash3d(cx, cy, cz, self.seed.wrapping_add(9901));
            if h % 31 == 0 {
                let vx = (cx << 2) + ((h & 3) as i32);
                let vy = (cy << 2) + (((h >> 2) & 3) as i32);
                let vz = (cz << 2) + (((h >> 4) & 3) as i32);
                let dist_sq = (wx - vx) * (wx - vx) + (wy - vy) * (wy - vy) + (wz - vz) * (wz - vz);
                if dist_sq <= 2 {
                    return BlockId::DIAMOND_ORE;
                }
            }
        }

        // Gold Ore: Mid-deep layer (y: 4..28)
        if wy >= 4 && wy <= 28 {
            let h = hash3d(cx, cy, cz, self.seed.wrapping_add(9907));
            if h % 23 == 0 {
                let vx = (cx << 2) + ((h & 3) as i32);
                let vy = (cy << 2) + (((h >> 2) & 3) as i32);
                let vz = (cz << 2) + (((h >> 4) & 3) as i32);
                let dist_sq = (wx - vx) * (wx - vx) + (wy - vy) * (wy - vy) + (wz - vz) * (wz - vz);
                if dist_sq <= 3 {
                    return BlockId::GOLD_ORE;
                }
            }
        }

        // Iron Ore: Broad subsurface range (y: 6..52)
        if wy >= 6 && wy <= 52 {
            let h = hash3d(cx, cy, cz, self.seed.wrapping_add(9923));
            if h % 15 == 0 {
                let vx = (cx << 2) + ((h & 3) as i32);
                let vy = (cy << 2) + (((h >> 2) & 3) as i32);
                let vz = (cz << 2) + (((h >> 4) & 3) as i32);
                let dist_sq = (wx - vx) * (wx - vx) + (wy - vy) * (wy - vy) + (wz - vz) * (wz - vz);
                if dist_sq <= 4 {
                    return BlockId::IRON_ORE;
                }
            }
        }

        // Copper Ore: (y: 14..56)
        if wy >= 14 && wy <= 56 {
            let h = hash3d(cx, cy, cz, self.seed.wrapping_add(9931));
            if h % 13 == 0 {
                let vx = (cx << 2) + ((h & 3) as i32);
                let vy = (cy << 2) + (((h >> 2) & 3) as i32);
                let vz = (cz << 2) + (((h >> 4) & 3) as i32);
                let dist_sq = (wx - vx) * (wx - vx) + (wy - vy) * (wy - vy) + (wz - vz) * (wz - vz);
                if dist_sq <= 4 {
                    return BlockId::COPPER_ORE;
                }
            }
        }

        // Coal Ore: Abundant across upper & middle underground (y: 18..72)
        if wy >= 18 && wy <= 72 {
            let h = hash3d(cx, cy, cz, self.seed.wrapping_add(9941));
            if h % 9 == 0 {
                let vx = (cx << 2) + ((h & 3) as i32);
                let vy = (cy << 2) + (((h >> 2) & 3) as i32);
                let vz = (cz << 2) + (((h >> 4) & 3) as i32);
                let dist_sq = (wx - vx) * (wx - vx) + (wy - vy) * (wy - vy) + (wz - vz) * (wz - vz);
                if dist_sq <= 5 {
                    return BlockId::COAL_ORE;
                }
            }
        }

        // Cobblestone patches
        if (wx.wrapping_mul(13) ^ wy.wrapping_mul(17) ^ wz.wrapping_mul(23)) % 23 == 0 {
            BlockId::COBBLESTONE
        } else {
            BlockId::STONE
        }
    }

    /// Populates foliage, trees (Oak, Birch, Spruce) and tall grass on chunks where the ground exists.
    pub fn populate_chunk_features(&self, chunk: &mut Chunk) {
        let origin = chunk.pos.to_world_origin();

        for lz in 0..CHUNK_SIZE_Z {
            let wz = origin.z + lz as i32;
            for lx in 0..CHUNK_SIZE_X {
                let wx = origin.x + lx as i32;

                let surface_y = self.surface_height(wx, wz);
                if surface_y <= self.water_level {
                    continue; // No trees or tall grass underwater
                }

                let local_surface_y = surface_y - origin.y;
                let biome = self.get_biome(wx, wz, surface_y);

                // Tall grass scattering on surface grass blocks
                if local_surface_y >= 1 && local_surface_y < CHUNK_SIZE_Y as i32 {
                    let ly = local_surface_y as usize;
                    if chunk.get(lx, ly - 1, lz) == Some(BlockId::GRASS)
                        && chunk.get(lx, ly, lz) == Some(BlockId::AIR)
                    {
                        let grass_chance = match biome {
                            Biome::Plains => 20,
                            Biome::Forest => 12,
                            _ => 0,
                        };
                        if grass_chance > 0
                            && (tree_hash(wx, wz, self.seed.wrapping_add(777)) % 100) < grass_chance
                        {
                            chunk.set(lx, ly, lz, BlockId::TALL_GRASS);
                        }
                    }
                }

                // Trees: require a margin of 2 blocks from chunk edges for leaves
                if lx < 2 || lx >= (CHUNK_SIZE_X - 2) || lz < 2 || lz >= (CHUNK_SIZE_Z - 2) {
                    continue;
                }

                if local_surface_y < 1 || local_surface_y >= (CHUNK_SIZE_Y as i32 - 7) {
                    continue;
                }

                let ly = local_surface_y as usize;
                if chunk.get(lx, ly - 1, lz) != Some(BlockId::GRASS) {
                    continue;
                }

                let th = tree_hash(wx, wz, self.seed);

                match biome {
                    Biome::Forest => {
                        if th % 22 == 0 {
                            let is_birch = (th >> 8) % 2 == 0;
                            if is_birch {
                                // Birch Tree: 5 blocks high trunk, BIRCH_LOG and BIRCH_LEAVES
                                for ty in ly..(ly + 5).min(CHUNK_SIZE_Y) {
                                    chunk.set(lx, ty, lz, BlockId::BIRCH_LOG);
                                }
                                let crown_start = ly + 3;
                                let crown_end = (ly + 6).min(CHUNK_SIZE_Y - 1);
                                for cy in crown_start..=crown_end {
                                    let radius: i32 = if cy >= ly + 5 { 1 } else { 2 };
                                    for dz in -radius..=radius {
                                        for dx in -radius..=radius {
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0 && cx < CHUNK_SIZE_X as i32 && cz >= 0 && cz < CHUNK_SIZE_Z as i32 {
                                                if dx.abs() == 2 && dz.abs() == 2 && (dx + dz) % 2 == 0 {
                                                    continue;
                                                }
                                                if chunk.get(cx as usize, cy, cz as usize) == Some(BlockId::AIR) {
                                                    chunk.set(cx as usize, cy, cz as usize, BlockId::BIRCH_LEAVES);
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Oak Tree: 4 blocks high trunk, WOOD_LOG and LEAVES
                                for ty in ly..(ly + 4).min(CHUNK_SIZE_Y) {
                                    chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                                }
                                let crown_start = ly + 2;
                                let crown_end = (ly + 5).min(CHUNK_SIZE_Y - 1);
                                for cy in crown_start..=crown_end {
                                    let radius: i32 = if cy >= ly + 4 { 1 } else { 2 };
                                    for dz in -radius..=radius {
                                        for dx in -radius..=radius {
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0 && cx < CHUNK_SIZE_X as i32 && cz >= 0 && cz < CHUNK_SIZE_Z as i32 {
                                                if dx.abs() == 2 && dz.abs() == 2 && (dx + dz) % 2 == 0 {
                                                    continue;
                                                }
                                                if chunk.get(cx as usize, cy, cz as usize) == Some(BlockId::AIR) {
                                                    chunk.set(cx as usize, cy, cz as usize, BlockId::LEAVES);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Plains => {
                        if th % 80 == 0 {
                            for ty in ly..(ly + 4).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                            }
                            let crown_start = ly + 2;
                            let crown_end = (ly + 5).min(CHUNK_SIZE_Y - 1);
                            for cy in crown_start..=crown_end {
                                let radius: i32 = if cy >= ly + 4 { 1 } else { 2 };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0 && cx < CHUNK_SIZE_X as i32 && cz >= 0 && cz < CHUNK_SIZE_Z as i32 {
                                            if dx.abs() == 2 && dz.abs() == 2 && (dx + dz) % 2 == 0 {
                                                continue;
                                            }
                                            if chunk.get(cx as usize, cy, cz as usize) == Some(BlockId::AIR) {
                                                chunk.set(cx as usize, cy, cz as usize, BlockId::LEAVES);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Mountains => {
                        if th % 40 == 0 {
                            // Spruce Pine Tree in Mountains
                            let trunk_height = 5 + ((th >> 4) % 2) as usize;
                            for ty in ly..(ly + trunk_height).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::SPRUCE_LOG);
                            }
                            let top_y = ly + trunk_height;
                            if top_y < CHUNK_SIZE_Y {
                                chunk.set(lx, top_y, lz, BlockId::SPRUCE_LEAVES);
                            }
                            for dy in 1..=trunk_height.min(4) {
                                if dy > top_y {
                                    break;
                                }
                                let cy = top_y - dy;
                                let radius: i32 = if dy % 2 == 1 { 1 } else { 2 };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        if dx.abs() == 2 && dz.abs() == 2 {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0 && cx < CHUNK_SIZE_X as i32 && cz >= 0 && cz < CHUNK_SIZE_Z as i32 {
                                            if chunk.get(cx as usize, cy, cz as usize) == Some(BlockId::AIR) {
                                                chunk.set(cx as usize, cy, cz as usize, BlockId::SPRUCE_LEAVES);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn tree_hash(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E3779B9)
        ^ (z as u32).wrapping_mul(0x85EBCA6B)
        ^ seed.wrapping_mul(0xC2B2AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB352D);
    h ^= h >> 15;
    h
}

#[inline]
fn hash3d(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ (z as u32).wrapping_mul(1274126177)
        ^ seed;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_determinism() {
        let gen1 = TerrainGenerator::new(42);
        let gen2 = TerrainGenerator::new(42);

        for z in -20..20 {
            for x in -20..20 {
                assert_eq!(gen1.surface_height(x, z), gen2.surface_height(x, z));
            }
        }
    }

    #[test]
    fn test_chunk_generation_and_features() {
        let gen = TerrainGenerator::new(12345);
        let mut chunk = gen.generate_chunk(ChunkPos::new(0, 1, 0));
        gen.populate_chunk_features(&mut chunk);

        // Bedrock is at y = 0 (chunk y=1 starts at world y=16)
        // Ensure chunk contains non-air blocks
        let has_blocks = chunk.raw_blocks().iter().any(|b| !b.is_air());
        assert!(has_blocks);
    }

    #[test]
    fn test_ore_vein_generation() {
        let gen = TerrainGenerator::new(42);
        let mut found_coal = false;
        let mut found_iron = false;
        let mut found_copper = false;
        let mut found_gold = false;
        let mut found_diamond = false;

        // Sample underground across a 4x4 area of chunks at bottom layers (0..5 vertical chunks)
        for cx in 0..4 {
            for cz in 0..4 {
                for cy in 0..5 {
                    let chunk = gen.generate_chunk(ChunkPos::new(cx, cy, cz));
                    for b in chunk.raw_blocks() {
                        if *b == BlockId::COAL_ORE {
                            found_coal = true;
                        }
                        if *b == BlockId::IRON_ORE {
                            found_iron = true;
                        }
                        if *b == BlockId::COPPER_ORE {
                            found_copper = true;
                        }
                        if *b == BlockId::GOLD_ORE {
                            found_gold = true;
                        }
                        if *b == BlockId::DIAMOND_ORE {
                            found_diamond = true;
                        }
                    }
                }
            }
        }

        assert!(found_coal, "Expected coal ore to generate underground");
        assert!(found_iron, "Expected iron ore to generate underground");
        assert!(found_copper, "Expected copper ore to generate underground");
        assert!(found_gold, "Expected gold ore to generate underground");
        assert!(
            found_diamond,
            "Expected diamond ore to generate underground"
        );
    }
}
