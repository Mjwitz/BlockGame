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
            water_level: 16,
        }
    }

    /// Computes the surface elevation at global coordinate (x, z).
    #[inline]
    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        let fx = x as f64;
        let fz = z as f64;

        let cont = self.continental_noise.get([fx, fz]);
        let detail = self.detail_noise.get([fx, fz]);

        // Base elevation around y = 20, varying up to mountains (y ~ 45) or valleys (y ~ 10)
        let height = 22.0 + (cont * 18.0) + (detail * 8.0);
        (height.round() as i32).max(1)
    }

    /// Determines the biome at global coordinate (x, z).
    #[inline]
    pub fn get_biome(&self, x: i32, z: i32, height: i32) -> Biome {
        if height > 34 {
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

                for ly in 0..CHUNK_SIZE_Y {
                    let wy = origin.y + ly as i32;

                    let block = if wy == 0 {
                        BlockId::BEDROCK
                    } else if wy < surface_y {
                        // Check if carved by cave system
                        if self.is_cave(wx, wy, wz) {
                            BlockId::AIR
                        } else if wy == surface_y - 1 {
                            match biome {
                                Biome::Desert => BlockId::SAND,
                                Biome::Mountains if surface_y > 36 => BlockId::STONE,
                                _ => BlockId::GRASS,
                            }
                        } else if wy >= surface_y - 4 {
                            match biome {
                                Biome::Desert => BlockId::SAND,
                                _ => BlockId::DIRT,
                            }
                        } else {
                            // Subsurface stone with ore veins
                            self.get_subsurface_block(wx, wy, wz)
                        }
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

        // Diamond Ore: Deep layer (y: 1..12), rare
        if wy >= 1 && wy <= 12 {
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

        // Gold Ore: Mid-deep layer (y: 2..18)
        if wy >= 2 && wy <= 18 {
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

        // Iron Ore: Broad subsurface range (y: 4..34)
        if wy >= 4 && wy <= 34 {
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

        // Copper Ore: (y: 8..38)
        if wy >= 8 && wy <= 38 {
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

        // Coal Ore: Abundant across upper & middle underground (y: 10..48)
        if wy >= 10 && wy <= 48 {
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

    /// Populates foliage and trees on chunks where the ground exists.
    pub fn populate_chunk_features(&self, chunk: &mut Chunk) {
        let origin = chunk.pos.to_world_origin();

        for lz in 2..(CHUNK_SIZE_Z - 2) {
            let wz = origin.z + lz as i32;
            for lx in 2..(CHUNK_SIZE_X - 2) {
                let wx = origin.x + lx as i32;

                let surface_y = self.surface_height(wx, wz);
                let biome = self.get_biome(wx, wz, surface_y);

                // Tree spawn conditions
                let spawn_tree = match biome {
                    Biome::Forest => tree_hash(wx, wz, self.seed) % 25 == 0,
                    Biome::Plains => tree_hash(wx, wz, self.seed) % 90 == 0,
                    _ => false,
                };

                if !spawn_tree {
                    continue;
                }

                // Check if the tree base is within this vertical chunk
                let local_surface_y = surface_y - origin.y;
                if local_surface_y < 1 || local_surface_y >= (CHUNK_SIZE_Y as i32 - 6) {
                    continue;
                }

                let ly = local_surface_y as usize;

                // Ensure ground below is Grass
                if chunk.get(lx, ly - 1, lz) != Some(BlockId::GRASS) {
                    continue;
                }

                // Trunk (4 blocks high)
                for ty in ly..(ly + 4) {
                    chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                }

                // Leaves crown
                let crown_start = ly + 2;
                let crown_end = (ly + 5).min(CHUNK_SIZE_Y - 1);
                for cy in crown_start..=crown_end {
                    let radius: i32 = if cy >= ly + 4 { 1 } else { 2 };
                    for dz in -radius..=radius {
                        for dx in -radius..=radius {
                            let cx = lx as i32 + dx;
                            let cz = lz as i32 + dz;
                            if cx >= 0
                                && cx < CHUNK_SIZE_X as i32
                                && cz >= 0
                                && cz < CHUNK_SIZE_Z as i32
                            {
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

        // Sample underground across a 4x4 area of chunks at bottom layers
        for cx in 0..4 {
            for cz in 0..4 {
                for cy in 0..3 {
                    let chunk = gen.generate_chunk(ChunkPos::new(cx, cy, cz));
                    for b in chunk.raw_blocks() {
                        if *b == BlockId::COAL_ORE { found_coal = true; }
                        if *b == BlockId::IRON_ORE { found_iron = true; }
                        if *b == BlockId::COPPER_ORE { found_copper = true; }
                        if *b == BlockId::GOLD_ORE { found_gold = true; }
                        if *b == BlockId::DIAMOND_ORE { found_diamond = true; }
                    }
                }
            }
        }

        assert!(found_coal, "Expected coal ore to generate underground");
        assert!(found_iron, "Expected iron ore to generate underground");
        assert!(found_copper, "Expected copper ore to generate underground");
        assert!(found_gold, "Expected gold ore to generate underground");
        assert!(found_diamond, "Expected diamond ore to generate underground");
    }
}
