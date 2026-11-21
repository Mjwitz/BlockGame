use blockgame_core::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    temple::{temple_id_from_grid, TempleKind, ARENA_BAND_START_X, TEMPLE_GRID_SIZE},
};
use glam::IVec3;

use crate::terrain::{Biome, TerrainGenerator};

/// Deterministic world-space placement of a biome boss temple exterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplePlacement {
    pub temple_id: u64,
    pub kind: TempleKind,
    /// Center-base block of the temple on the surface (`y` = floor level).
    pub origin: IVec3,
    /// Bottom block of the glowing `TEMPLE_PORTAL` inside the temple sanctum.
    pub portal_pos: IVec3,
}

#[inline]
fn hash_grid(seed: u32, gx: i32, gz: i32) -> u64 {
    let mut x = (seed as u64)
        .wrapping_add((gx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((gz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn biome_to_temple_kind(biome: Biome) -> Option<TempleKind> {
    match biome {
        Biome::Desert | Biome::RedDesert => Some(TempleKind::Desert),
        Biome::Glacier | Biome::SnowyTundra => Some(TempleKind::Glacier),
        Biome::Jungle => Some(TempleKind::Jungle),
        Biome::DarkForest => Some(TempleKind::DarkForest),
        _ => None,
    }
}

impl TerrainGenerator {
    /// Returns the deterministic boss temple placed in grid cell `(gx, gz)`, if the cell's
    /// biome is one of the 4 temple biomes and terrain elevation is suitable.
    pub fn find_temple_in_grid(&self, gx: i32, gz: i32) -> Option<TemplePlacement> {
        let base_x = gx * TEMPLE_GRID_SIZE;
        if base_x >= ARENA_BAND_START_X - TEMPLE_GRID_SIZE {
            return None;
        }
        let base_z = gz * TEMPLE_GRID_SIZE;

        let h = hash_grid(self.seed, gx, gz);
        // Keep a 48-block margin from grid edges so adjacent temples never overlap
        let margin = 48;
        let span = (TEMPLE_GRID_SIZE - margin * 2) as u64;
        let ox = base_x + margin + ((h & 0xFFFF) % span) as i32;
        let oz = base_z + margin + (((h >> 16) & 0xFFFF) % span) as i32;

        // Avoid spawning right on top of world origin (0, 0) spawn area
        if ox.abs() < 64 && oz.abs() < 64 {
            return None;
        }

        let surface_y = self.surface_height(ox, oz);
        if surface_y <= self.water_level + 2 || surface_y >= 98 {
            return None;
        }

        let biome = self.get_biome(ox, oz, surface_y);
        let kind = biome_to_temple_kind(biome)?;

        let temple_id = temple_id_from_grid(self.seed, gx, gz);
        let origin = IVec3::new(ox, surface_y, oz);
        let portal_pos = origin + IVec3::new(0, 1, 3);

        Some(TemplePlacement {
            temple_id,
            kind,
            origin,
            portal_pos,
        })
    }

    /// Returns any temples whose exterior footprint intersects `cpos`.
    pub fn find_temples_for_chunk(&self, cpos: ChunkPos) -> Vec<TemplePlacement> {
        let origin = cpos.to_world_origin();
        if origin.x >= ARENA_BAND_START_X {
            return Vec::new();
        }
        let min_gx = (origin.x - 16).div_euclid(TEMPLE_GRID_SIZE);
        let max_gx = (origin.x + CHUNK_SIZE_X as i32 + 16).div_euclid(TEMPLE_GRID_SIZE);
        let min_gz = (origin.z - 16).div_euclid(TEMPLE_GRID_SIZE);
        let max_gz = (origin.z + CHUNK_SIZE_Z as i32 + 16).div_euclid(TEMPLE_GRID_SIZE);

        let mut results = Vec::new();
        for gx in min_gx..=max_gx {
            for gz in min_gz..=max_gz {
                if let Some(t) = self.find_temple_in_grid(gx, gz) {
                    let dx_min = t.origin.x - 11;
                    let dx_max = t.origin.x + 11;
                    let dz_min = t.origin.z - 11;
                    let dz_max = t.origin.z + 11;
                    let dy_min = t.origin.y - 4;
                    let dy_max = t.origin.y + 16;

                    let chunk_max_x = origin.x + CHUNK_SIZE_X as i32 - 1;
                    let chunk_max_y = origin.y + CHUNK_SIZE_Y as i32 - 1;
                    let chunk_max_z = origin.z + CHUNK_SIZE_Z as i32 - 1;

                    if dx_max >= origin.x
                        && dx_min <= chunk_max_x
                        && dz_max >= origin.z
                        && dz_min <= chunk_max_z
                        && dy_max >= origin.y
                        && dy_min <= chunk_max_y
                    {
                        results.push(t);
                    }
                }
            }
        }
        results
    }

    /// Searches outward in grid rings from `(from_x, from_z)` for the nearest temple.
    pub fn find_nearest_temple(
        &self,
        from_x: i32,
        from_z: i32,
        kind_filter: Option<TempleKind>,
        max_ring: i32,
    ) -> Option<TemplePlacement> {
        let cgx = from_x.div_euclid(TEMPLE_GRID_SIZE);
        let cgz = from_z.div_euclid(TEMPLE_GRID_SIZE);
        let mut best: Option<(i64, TemplePlacement)> = None;

        for ring in 0..=max_ring {
            for gx in (cgx - ring)..=(cgx + ring) {
                for gz in (cgz - ring)..=(cgz + ring) {
                    if ring > 0 && (gx - cgx).abs() < ring && (gz - cgz).abs() < ring {
                        continue;
                    }
                    if let Some(t) = self.find_temple_in_grid(gx, gz) {
                        if let Some(filter) = kind_filter {
                            if t.kind != filter {
                                continue;
                            }
                        }
                        let dx = (t.origin.x - from_x) as i64;
                        let dz = (t.origin.z - from_z) as i64;
                        let dist2 = dx * dx + dz * dz;
                        if best.map_or(true, |(b_d2, _)| dist2 < b_d2) {
                            best = Some((dist2, t));
                        }
                    }
                }
            }
            if best.is_some() && ring >= 2 {
                break;
            }
        }
        best.map(|(_, t)| t)
    }
}

#[inline]
fn set_world_in_chunk(chunk: &mut Chunk, wx: i32, wy: i32, wz: i32, block: BlockId) {
    let origin = chunk.pos.to_world_origin();
    let lx = wx - origin.x;
    let ly = wy - origin.y;
    let lz = wz - origin.z;
    if lx >= 0
        && lx < CHUNK_SIZE_X as i32
        && ly >= 0
        && ly < CHUNK_SIZE_Y as i32
        && lz >= 0
        && lz < CHUNK_SIZE_Z as i32
    {
        chunk.set(lx as usize, ly as usize, lz as usize, block);
    }
}

/// Stamps the chunk's slice of a biome-themed overworld boss temple exterior + interior portal.
pub fn stamp_temple_exterior(chunk: &mut Chunk, temple: &TemplePlacement) {
    let o = temple.origin;
    let primary = temple.kind.primary_block();
    let accent = temple.kind.accent_block();
    let floor = temple.kind.floor_block();

    // 1. Solid foundation skirt (4 blocks deep) so temples never float on sloped terrain
    for dx in -9i32..=9 {
        for dz in -9i32..=9 {
            for dy in -4i32..=0 {
                let b = if dy == 0 {
                    if dx.abs() <= 6 && dz.abs() <= 6 && (dx + dz).rem_euclid(3) == 0 {
                        primary
                    } else {
                        floor
                    }
                } else {
                    primary
                };
                set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
            }
        }
    }

    // 2. Clear interior air & build biome-specific silhouette
    match temple.kind {
        TempleKind::Desert => {
            // Stepped Pyramid of the Blazing Sun (9-block half-width tapering to apex at dy=10)
            for dy in 1i32..=10 {
                let r: i32 = (9 - dy).max(1);
                for dx in -r..=r {
                    for dz in -r..=r {
                        let on_shell = dx.abs() == r || dz.abs() == r || dy == 10;
                        let b = if on_shell {
                            if dy % 3 == 0 && (dx.abs() == r && dz.abs() == r) {
                                accent
                            } else {
                                primary
                            }
                        } else {
                            BlockId::AIR
                        };
                        set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
                    }
                }
            }
            // 4 corner obelisks
            for (cx, cz) in [(-9i32, -9i32), (9, -9), (-9, 9), (9, 9)] {
                for dy in 1i32..=6 {
                    let b = if dy == 6 { BlockId::TEMPLE_BRAZIER } else { primary };
                    set_world_in_chunk(chunk, o.x + cx, o.y + dy, o.z + cz, b);
                }
            }
        }
        TempleKind::Glacier => {
            // Crystalline Citadel of Eternal Frost with 4 towering ice spires
            for dy in 1i32..=8 {
                let r: i32 = if dy <= 6 { 8 } else { 6 };
                for dx in -r..=r {
                    for dz in -r..=r {
                        let on_shell = dx.abs() == r || dz.abs() == r || dy == 8;
                        let b = if on_shell {
                            if (dx + dz + dy).rem_euclid(4) == 0 {
                                accent
                            } else {
                                primary
                            }
                        } else {
                            BlockId::AIR
                        };
                        set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
                    }
                }
            }
            for (cx, cz) in [(-8i32, -8i32), (8, -8), (-8, 8), (8, 8)] {
                for dy in 1i32..=13 {
                    let b = if dy == 13 { BlockId::TEMPLE_BRAZIER } else if dy % 2 == 0 { accent } else { primary };
                    set_world_in_chunk(chunk, o.x + cx, o.y + dy, o.z + cz, b);
                }
            }
        }
        TempleKind::Jungle => {
            // Terraced Ziggurat of the Wilds with mossy foliage overhang
            for dy in 1i32..=9 {
                let r: i32 = if dy <= 3 { 9 } else if dy <= 6 { 7 } else { 5 };
                for dx in -r..=r {
                    for dz in -r..=r {
                        let on_shell = dx.abs() == r || dz.abs() == r || dy == 9;
                        let b = if on_shell {
                            if dx.abs() == r && dz.abs() == r {
                                accent
                            } else if dy == 9 && (dx.abs() == 5 || dz.abs() == 5) {
                                BlockId::JUNGLE_LEAVES
                            } else {
                                primary
                            }
                        } else {
                            BlockId::AIR
                        };
                        set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
                    }
                }
            }
        }
        TempleKind::DarkForest => {
            // Gothic Cathedral of Whispers with pitched obsidian roof
            for dy in 1i32..=10 {
                let rx: i32 = if dy <= 5 { 7 } else { (7 - (dy - 5)).max(1) };
                let rz: i32 = 9;
                for dx in -rx..=rx {
                    for dz in -rz..=rz {
                        let on_shell = dx.abs() == rx || dz.abs() == rz || dy == 10;
                        let b = if on_shell {
                            if dz.abs() % 3 == 0 && dx.abs() == rx {
                                accent
                            } else {
                                primary
                            }
                        } else {
                            BlockId::AIR
                        };
                        set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
                    }
                }
            }
        }
    }

    // 3. Carve front entrance archway on the -Z side so players can walk straight in
    for dz in -10..=-3 {
        for dx in -1..=1 {
            for dy in 1..=3 {
                set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, BlockId::AIR);
            }
        }
    }
    // Entrance braziers
    set_world_in_chunk(chunk, o.x - 2, o.y + 2, o.z - 9, BlockId::TEMPLE_BRAZIER);
    set_world_in_chunk(chunk, o.x + 2, o.y + 2, o.z - 9, BlockId::TEMPLE_BRAZIER);

    // Interior lighting braziers
    for (bx, bz) in [(-4, 0), (4, 0), (-4, 4), (4, 4)] {
        set_world_in_chunk(chunk, o.x + bx, o.y + 1, o.z + bz, primary);
        set_world_in_chunk(chunk, o.x + bx, o.y + 2, o.z + bz, BlockId::TEMPLE_BRAZIER);
    }

    // 4. Inner Sanctum Boss Portal Frame at temple.portal_pos
    let pp = temple.portal_pos;
    for dx in -2..=2 {
        set_world_in_chunk(chunk, pp.x + dx, pp.y + 3, pp.z, accent);
    }
    for dy in 0..=2 {
        set_world_in_chunk(chunk, pp.x - 2, pp.y + dy, pp.z, primary);
        set_world_in_chunk(chunk, pp.x + 2, pp.y + dy, pp.z, primary);
    }
    for dx in -1..=1 {
        for dy in 0..=2 {
            set_world_in_chunk(chunk, pp.x + dx, pp.y + dy, pp.z, BlockId::TEMPLE_PORTAL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_band_returns_empty_chunk() {
        let gen = TerrainGenerator::new(42);
        let cpos = ChunkPos::new((ARENA_BAND_START_X / 16) + 10, 2, 0);
        let chunk = gen.generate_chunk(cpos);
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    assert_eq!(chunk.get(x, y, z), Some(BlockId::AIR));
                }
            }
        }
    }

    #[test]
    fn test_all_four_temple_biomes_discoverable() {
        let gen = TerrainGenerator::new(1337);
        for kind in [
            TempleKind::Desert,
            TempleKind::Glacier,
            TempleKind::Jungle,
            TempleKind::DarkForest,
        ] {
            let found = gen.find_nearest_temple(0, 0, Some(kind), 24);
            assert!(
                found.is_some(),
                "Expected to find a {:?} temple within 24 grid cells",
                kind
            );
            let t = found.unwrap();
            assert_eq!(t.kind, kind);
        }
    }
}
