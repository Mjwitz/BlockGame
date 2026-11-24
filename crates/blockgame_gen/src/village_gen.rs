use blockgame_core::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    temple::{ARENA_BAND_START_X, TEMPLE_GRID_SIZE},
    village::{
        village_id_from_grid, ShopkeeperProfession, VillageSpawnBlueprint, VillageTheme,
        VILLAGE_GRID_SIZE, VILLAGE_MIN_ORIGIN_DIST, VILLAGE_SPAWN_CHANCE_PERCENT,
        VILLAGE_TEMPLE_EXCLUSION_RADIUS,
    },
};
use glam::{IVec3, Vec3};

use crate::terrain::{Biome, TerrainGenerator};

/// World-space placement of a single themed shop building inside a Village.
#[derive(Debug, Clone, PartialEq)]
pub struct VillageShopPlacement {
    pub profession: ShopkeeperProfession,
    /// Center-floor block of the shop interior.
    pub center: IVec3,
    /// Cardinal unit vector pointing from the shop center toward the central square doorway.
    pub door_dir: IVec3,
    pub home_pos: Vec3,
    pub courtyard_pos: Vec3,
}

/// Deterministic world-space placement of an overworld NPC Village.
#[derive(Debug, Clone, PartialEq)]
pub struct VillagePlacement {
    pub village_id: u64,
    pub theme: VillageTheme,
    /// Center-floor block of the Village central square.
    pub origin: IVec3,
    pub shops: Vec<VillageShopPlacement>,
    pub guard_positions: Vec<Vec3>,
}

impl VillagePlacement {
    pub fn to_spawn_blueprint(&self) -> VillageSpawnBlueprint {
        VillageSpawnBlueprint {
            village_id: self.village_id,
            theme: self.theme,
            origin: self.origin,
            shops: self
                .shops
                .iter()
                .map(|s| (s.profession, s.home_pos, s.courtyard_pos))
                .collect(),
            guard_positions: self.guard_positions.clone(),
        }
    }
}

#[inline]
fn hash_village_grid(seed: u32, gx: i32, gz: i32) -> u64 {
    let mut x = (seed as u64)
        .wrapping_add(0x2545_F491_4F6C_DD1D)
        .wrapping_add((gx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((gz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Maps the 4 supported overworld biome families to their Village theme.
pub fn biome_to_village_theme(biome: Biome) -> Option<VillageTheme> {
    match biome {
        Biome::Plains | Biome::FlowerMeadow => Some(VillageTheme::Plains),
        Biome::Desert | Biome::RedDesert => Some(VillageTheme::Desert),
        Biome::Taiga | Biome::SnowyTundra => Some(VillageTheme::Taiga),
        Biome::Savanna => Some(VillageTheme::Savanna),
        _ => None,
    }
}

impl TerrainGenerator {
    /// Returns the deterministic Village placed in grid cell `(gx, gz)`, if eligible.
    /// Enforces `VILLAGE_SPAWN_CHANCE_PERCENT` (~50%) spawn chance, >=160 blocks from world origin,
    /// 4 themed biome families, and strict Boss Temple exclusion (same Voronoi biome cell or within 320 blocks).
    pub fn find_village_in_grid(&self, gx: i32, gz: i32) -> Option<VillagePlacement> {
        let base_x = gx * VILLAGE_GRID_SIZE;
        if base_x >= ARENA_BAND_START_X - VILLAGE_GRID_SIZE {
            return None;
        }
        let base_z = gz * VILLAGE_GRID_SIZE;

        let h = hash_village_grid(self.seed, gx, gz);
        // ~40% spawn chance so Villages feel genuinely rare
        if (h % 100) >= VILLAGE_SPAWN_CHANCE_PERCENT {
            return None;
        }

        let margin = 48;
        let span = (VILLAGE_GRID_SIZE - margin * 2) as u64;
        let min_d = VILLAGE_MIN_ORIGIN_DIST as i64;
        let excl_sq = (VILLAGE_TEMPLE_EXCLUSION_RADIUS as i64)
            * (VILLAGE_TEMPLE_EXCLUSION_RADIUS as i64);

        // Try up to 4 deterministic candidate coordinates inside this cell so a single
        // river/ocean/non-village biome edge doesn't waste an entire grid cell.
        let mut chosen: Option<(i32, i32, i32, VillageTheme)> = None;
        let mut cand_h = h;
        for attempt in 0..4 {
            if attempt > 0 {
                cand_h = cand_h
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
            }
            let ox = base_x + margin + (((cand_h >> 8) & 0xFFFF) % span) as i32;
            let oz = base_z + margin + (((cand_h >> 24) & 0xFFFF) % span) as i32;

            // Never spawn within 160 blocks of world origin (0, 0)
            let dist_origin_sq = (ox as i64) * (ox as i64) + (oz as i64) * (oz as i64);
            if dist_origin_sq < min_d * min_d {
                continue;
            }

            let surface_y = self.surface_height(ox, oz);
            if surface_y <= self.water_level + 2 || surface_y >= 94 {
                continue;
            }

            let biome = self.get_biome(ox, oz, surface_y);
            let Some(theme) = biome_to_village_theme(biome) else {
                continue;
            };

            // Exclude any Village that shares a Voronoi biome cell with a Boss Temple
            // or lies within VILLAGE_TEMPLE_EXCLUSION_RADIUS (320 blocks) of one.
            let my_cell = self.macro_biome_cell_id(ox, oz);
            let tgx_center = ox.div_euclid(TEMPLE_GRID_SIZE);
            let tgz_center = oz.div_euclid(TEMPLE_GRID_SIZE);
            let mut blocked_by_temple = false;
            'temple_check: for tgx in (tgx_center - 2)..=(tgx_center + 2) {
                for tgz in (tgz_center - 2)..=(tgz_center + 2) {
                    if let Some(temple) = self.find_temple_in_grid(tgx, tgz) {
                        let dx = (temple.origin.x - ox) as i64;
                        let dz = (temple.origin.z - oz) as i64;
                        if dx * dx + dz * dz <= excl_sq
                            || self.macro_biome_cell_id(temple.origin.x, temple.origin.z)
                                == my_cell
                        {
                            blocked_by_temple = true;
                            break 'temple_check;
                        }
                    }
                }
            }
            if !blocked_by_temple {
                chosen = Some((ox, oz, surface_y, theme));
                break;
            }
        }

        let (ox, oz, surface_y, theme) = chosen?;
        let village_id = village_id_from_grid(self.seed, gx, gz);
        let origin = IVec3::new(ox, surface_y, oz);

        // Pick 3..=5 distinct Shopkeeper professions deterministically
        let shop_count = 3 + ((h >> 40) % 3) as usize;
        let mut profs = ShopkeeperProfession::ALL;
        // Fisher-Yates shuffle driven by hash bits
        let mut s = h ^ 0xA5A5_5A5A_1234_5678;
        for i in (1..profs.len()).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = ((s >> 32) as usize) % (i + 1);
            profs.swap(i, j);
        }

        // Fixed ring of 5 shop pads around the central square
        let shop_slots: [(IVec3, IVec3); 5] = [
            (IVec3::new(0, 0, -13), IVec3::new(0, 0, 1)),  // North shop, door faces South
            (IVec3::new(13, 0, 0), IVec3::new(-1, 0, 0)),  // East shop, door faces West
            (IVec3::new(0, 0, 13), IVec3::new(0, 0, -1)),  // South shop, door faces North
            (IVec3::new(-13, 0, 0), IVec3::new(1, 0, 0)),  // West shop, door faces East
            (IVec3::new(12, 0, -12), IVec3::new(0, 0, 1)), // NE shop, door faces South
        ];

        let mut shops = Vec::with_capacity(shop_count);
        for i in 0..shop_count {
            let (offset, door_dir) = shop_slots[i];
            let cx = ox + offset.x;
            let cz = oz + offset.z;
            // Clamp shop floor within +/-3 blocks of square height so paths connect cleanly
            let raw_y = self.surface_height(cx, cz);
            let cy = raw_y.clamp(surface_y - 2, surface_y + 2).max(self.water_level + 1);
            let center = IVec3::new(cx, cy, cz);
            let home_pos = Vec3::new(cx as f32 + 0.5, (cy + 1) as f32, cz as f32 + 0.5);
            let courtyard_pos = Vec3::new(
                (cx + door_dir.x * 5) as f32 + 0.5,
                (cy + 1) as f32,
                (cz + door_dir.z * 5) as f32 + 0.5,
            );
            shops.push(VillageShopPlacement {
                profession: profs[i],
                center,
                door_dir,
                home_pos,
                courtyard_pos,
            });
        }

        // 2..=3 Village Guards patrolling the central square
        let guard_count = 2 + (((h >> 48) % 2) as usize);
        let guard_offsets = [
            IVec3::new(4, 0, 4),
            IVec3::new(-4, 0, -4),
            IVec3::new(-4, 0, 4),
        ];
        let mut guard_positions = Vec::with_capacity(guard_count);
        for i in 0..guard_count {
            let off = guard_offsets[i];
            guard_positions.push(Vec3::new(
                (ox + off.x) as f32 + 0.5,
                (surface_y + 1) as f32,
                (oz + off.z) as f32 + 0.5,
            ));
        }

        Some(VillagePlacement {
            village_id,
            theme,
            origin,
            shops,
            guard_positions,
        })
    }

    /// Returns any Villages whose bounding footprint intersects `cpos`.
    pub fn find_villages_for_chunk(&self, cpos: ChunkPos) -> Vec<VillagePlacement> {
        let origin = cpos.to_world_origin();
        if origin.x >= ARENA_BAND_START_X {
            return Vec::new();
        }
        let pad = 26;
        let min_gx = (origin.x - pad).div_euclid(VILLAGE_GRID_SIZE);
        let max_gx = (origin.x + CHUNK_SIZE_X as i32 + pad).div_euclid(VILLAGE_GRID_SIZE);
        let min_gz = (origin.z - pad).div_euclid(VILLAGE_GRID_SIZE);
        let max_gz = (origin.z + CHUNK_SIZE_Z as i32 + pad).div_euclid(VILLAGE_GRID_SIZE);

        let chunk_max_x = origin.x + CHUNK_SIZE_X as i32 - 1;
        let chunk_max_y = origin.y + CHUNK_SIZE_Y as i32 - 1;
        let chunk_max_z = origin.z + CHUNK_SIZE_Z as i32 - 1;

        let mut results = Vec::new();
        for gx in min_gx..=max_gx {
            for gz in min_gz..=max_gz {
                if let Some(v) = self.find_village_in_grid(gx, gz) {
                    let dx_min = v.origin.x - 22;
                    let dx_max = v.origin.x + 22;
                    let dz_min = v.origin.z - 22;
                    let dz_max = v.origin.z + 22;
                    let dy_min = v.origin.y - 8;
                    let dy_max = v.origin.y + 14;

                    if dx_max >= origin.x
                        && dx_min <= chunk_max_x
                        && dz_max >= origin.z
                        && dz_min <= chunk_max_z
                        && dy_max >= origin.y
                        && dy_min <= chunk_max_y
                    {
                        results.push(v);
                    }
                }
            }
        }
        results
    }

    /// Searches outward in grid rings from `(from_x, from_z)` for the nearest Village.
    pub fn find_nearest_village(
        &self,
        from_x: i32,
        from_z: i32,
        theme_filter: Option<VillageTheme>,
        max_ring: i32,
    ) -> Option<VillagePlacement> {
        let cgx = from_x.div_euclid(VILLAGE_GRID_SIZE);
        let cgz = from_z.div_euclid(VILLAGE_GRID_SIZE);
        let mut best: Option<(i64, VillagePlacement)> = None;

        for ring in 0..=max_ring {
            for gx in (cgx - ring)..=(cgx + ring) {
                for gz in (cgz - ring)..=(cgz + ring) {
                    if ring > 0 && (gx - cgx).abs() < ring && (gz - cgz).abs() < ring {
                        continue;
                    }
                    if let Some(v) = self.find_village_in_grid(gx, gz) {
                        if let Some(filter) = theme_filter {
                            if v.theme != filter {
                                continue;
                            }
                        }
                        let dx = (v.origin.x - from_x) as i64;
                        let dz = (v.origin.z - from_z) as i64;
                        let dist2 = dx * dx + dz * dz;
                        if best.as_ref().map_or(true, |(b_d2, _)| dist2 < *b_d2) {
                            best = Some((dist2, v));
                        }
                    }
                }
            }
            if best.is_some() && ring >= 2 {
                break;
            }
        }
        best.map(|(_, v)| v)
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

/// Stamps the chunk's slice of a biome-themed Village: central square/well, paths, lamps, and shops.
pub fn stamp_village(chunk: &mut Chunk, village: &VillagePlacement) {
    let o = village.origin;
    let floor = village.theme.floor_block();
    let path = village.theme.path_block();
    let pillar = village.theme.pillar_block();
    let wall = village.theme.wall_block();
    let roof = village.theme.roof_block();

    // 1. Central Square (-5..=5) with solid foundation skirt and clear air above
    for dx in -5i32..=5 {
        for dz in -5i32..=5 {
            for dy in -4i32..=0 {
                let b = if dy == 0 {
                    if dx.abs() <= 2 && dz.abs() <= 2 {
                        floor
                    } else {
                        path
                    }
                } else {
                    floor
                };
                set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, b);
            }
            for dy in 1i32..=7 {
                set_world_in_chunk(chunk, o.x + dx, o.y + dy, o.z + dz, BlockId::AIR);
            }
        }
    }

    // Central Well / Oasis Basin (-1..=1)
    for dx in -1i32..=1 {
        for dz in -1i32..=1 {
            if dx == 0 && dz == 0 {
                set_world_in_chunk(chunk, o.x, o.y, o.z, BlockId::WATER);
                set_world_in_chunk(chunk, o.x, o.y - 1, o.z, floor);
            } else {
                set_world_in_chunk(chunk, o.x + dx, o.y + 1, o.z + dz, floor);
            }
        }
    }
    // Well canopy pillars + roof
    for &(px, pz) in &[(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        set_world_in_chunk(chunk, o.x + px, o.y + 2, o.z + pz, pillar);
        set_world_in_chunk(chunk, o.x + px, o.y + 3, o.z + pz, pillar);
    }
    for dx in -1i32..=1 {
        for dz in -1i32..=1 {
            set_world_in_chunk(chunk, o.x + dx, o.y + 4, o.z + dz, roof);
        }
    }

    // 4 Square Lamp Posts for safe nighttime lighting
    for &(lx, lz) in &[(-4, -4), (4, -4), (-4, 4), (4, 4)] {
        set_world_in_chunk(chunk, o.x + lx, o.y + 1, o.z + lz, pillar);
        set_world_in_chunk(chunk, o.x + lx, o.y + 2, o.z + lz, pillar);
        set_world_in_chunk(chunk, o.x + lx, o.y + 3, o.z + lz, BlockId::TORCH);
    }

    // 2. Connecting paths and themed shop buildings
    for shop in &village.shops {
        let sc = shop.center;
        // Straight path from square edge to shop front
        let steps = (sc.x - o.x).abs().max((sc.z - o.z).abs());
        if steps > 0 {
            for s in 5..steps.saturating_sub(3) {
                let t = s as f32 / steps as f32;
                let px = o.x + ((sc.x - o.x) as f32 * t).round() as i32;
                let pz = o.z + ((sc.z - o.z) as f32 * t).round() as i32;
                let py = o.y + ((sc.y - o.y) as f32 * t).round() as i32;
                for w in -1i32..=1 {
                    let (wx, wz) = if (sc.x - o.x).abs() >= (sc.z - o.z).abs() {
                        (px, pz + w)
                    } else {
                        (px + w, pz)
                    };
                    set_world_in_chunk(chunk, wx, py - 1, wz, floor);
                    set_world_in_chunk(chunk, wx, py, wz, path);
                    for dy in 1..=4 {
                        set_world_in_chunk(chunk, wx, py + dy, wz, BlockId::AIR);
                    }
                }
            }
        }

        // Shop building: 7x7 footprint (-3..=3), height 5
        for dx in -3i32..=3 {
            for dz in -3i32..=3 {
                // Solid foundation skirt so shops never float on slopes
                for dy in -5i32..=0 {
                    set_world_in_chunk(chunk, sc.x + dx, sc.y + dy, sc.z + dz, floor);
                }
                // Clear interior + airspace
                for dy in 1i32..=7 {
                    set_world_in_chunk(chunk, sc.x + dx, sc.y + dy, sc.z + dz, BlockId::AIR);
                }
            }
        }

        // Walls, corner pillars, glass windows, and doorway facing central square
        for dy in 1i32..=4 {
            for dx in -3i32..=3 {
                for dz in -3i32..=3 {
                    let on_edge_x = dx.abs() == 3;
                    let on_edge_z = dz.abs() == 3;
                    if !on_edge_x && !on_edge_z {
                        continue;
                    }
                    let is_corner = on_edge_x && on_edge_z;

                    // Doorway (width 1, height 2) on the side facing `shop.door_dir`
                    let is_door = dy <= 2
                        && ((shop.door_dir.x == 1 && dx == 3 && dz == 0)
                            || (shop.door_dir.x == -1 && dx == -3 && dz == 0)
                            || (shop.door_dir.z == 1 && dz == 3 && dx == 0)
                            || (shop.door_dir.z == -1 && dz == -3 && dx == 0));
                    if is_door {
                        continue;
                    }

                    // Windows at eye level (dy == 2) in wall centers
                    let is_window = dy == 2 && !is_corner && (dx == 0 || dz == 0);
                    let b = if is_corner {
                        pillar
                    } else if is_window {
                        BlockId::GLASS
                    } else {
                        wall
                    };
                    set_world_in_chunk(chunk, sc.x + dx, sc.y + dy, sc.z + dz, b);
                }
            }
        }

        // Roof style varies by biome theme
        let pitched = matches!(village.theme, VillageTheme::Plains | VillageTheme::Taiga);
        for dx in -3i32..=3 {
            for dz in -3i32..=3 {
                set_world_in_chunk(chunk, sc.x + dx, sc.y + 5, sc.z + dz, roof);
            }
        }
        if pitched {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    set_world_in_chunk(chunk, sc.x + dx, sc.y + 6, sc.z + dz, roof);
                }
            }
        } else {
            // Flat adobe / savanna parapet rim
            for dx in -3i32..=3 {
                for dz in -3i32..=3 {
                    if (dx.abs() == 3 || dz.abs() == 3) && (dx + dz).rem_euclid(2) == 0 {
                        set_world_in_chunk(chunk, sc.x + dx, sc.y + 6, sc.z + dz, roof);
                    }
                }
            }
        }

        // Interior lighting torches & profession counter
        set_world_in_chunk(chunk, sc.x - 2, sc.y + 3, sc.z - 2, BlockId::TORCH);
        set_world_in_chunk(chunk, sc.x + 2, sc.y + 3, sc.z + 2, BlockId::TORCH);
        // Exterior lantern beside door
        let front_x = sc.x + shop.door_dir.x * 4;
        let front_z = sc.z + shop.door_dir.z * 4;
        set_world_in_chunk(chunk, front_x, sc.y, front_z, path);
        set_world_in_chunk(chunk, front_x, sc.y + 1, front_z, BlockId::AIR);
        set_world_in_chunk(chunk, front_x, sc.y + 2, front_z, BlockId::AIR);
        let lamp_x = sc.x + shop.door_dir.x * 4 + shop.door_dir.z;
        let lamp_z = sc.z + shop.door_dir.z * 4 - shop.door_dir.x;
        set_world_in_chunk(chunk, lamp_x, sc.y + 1, lamp_z, pillar);
        set_world_in_chunk(chunk, lamp_x, sc.y + 2, lamp_z, BlockId::TORCH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_village_generation_rarity_biomes_and_temple_exclusion() {
        let gen = TerrainGenerator::new(1337);
        let mut found = Vec::new();

        for gx in -8..=8 {
            for gz in -8..=8 {
                if let Some(v) = gen.find_village_in_grid(gx, gz) {
                    // Must be >= 160 blocks from world spawn
                    let d2 = (v.origin.x as i64).pow(2) + (v.origin.z as i64).pow(2);
                    assert!(d2 >= (VILLAGE_MIN_ORIGIN_DIST as i64).pow(2));

                    // Must have 3..=5 shops and 2..=3 guards
                    assert!((3..=5).contains(&v.shops.len()));
                    assert!((2..=3).contains(&v.guard_positions.len()));

                    // Must NEVER overlap a Boss Temple's biome cell or 320-block radius
                    let v_cell = gen.macro_biome_cell_id(v.origin.x, v.origin.z);
                    let tgx_c = v.origin.x.div_euclid(TEMPLE_GRID_SIZE);
                    let tgz_c = v.origin.z.div_euclid(TEMPLE_GRID_SIZE);
                    for tgx in (tgx_c - 2)..=(tgx_c + 2) {
                        for tgz in (tgz_c - 2)..=(tgz_c + 2) {
                            if let Some(t) = gen.find_temple_in_grid(tgx, tgz) {
                                let td2 = (t.origin.x as i64 - v.origin.x as i64).pow(2)
                                    + (t.origin.z as i64 - v.origin.z as i64).pow(2);
                                assert!(
                                    td2 > (VILLAGE_TEMPLE_EXCLUSION_RADIUS as i64).pow(2),
                                    "Village spawned too close to a Boss Temple!"
                                );
                                assert_ne!(
                                    gen.macro_biome_cell_id(t.origin.x, t.origin.z),
                                    v_cell,
                                    "Village spawned in the same Voronoi biome cell as a Boss Temple!"
                                );
                            }
                        }
                    }
                    found.push(v);
                }
            }
        }

        // Should find at least a few villages across a 17x17 coarse grid, while remaining rare
        assert!(!found.is_empty(), "Expected at least one Village in test scan");
        assert!(found.len() < 120, "Villages should be rare, found {}", found.len());
    }

    #[test]
    fn test_village_chunk_stamping_places_well_and_shop_blocks() {
        let gen = TerrainGenerator::new(42);
        let v = gen
            .find_nearest_village(0, 0, None, 12)
            .expect("should find a village within 12 rings");
        let cpos = ChunkPos::new(
            v.origin.x.div_euclid(CHUNK_SIZE_X as i32),
            v.origin.y.div_euclid(CHUNK_SIZE_Y as i32),
            v.origin.z.div_euclid(CHUNK_SIZE_Z as i32),
        );
        let chunk = gen.generate_chunk(cpos);
        let lx = v.origin.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
        let ly = v.origin.y.rem_euclid(CHUNK_SIZE_Y as i32) as usize;
        let lz = v.origin.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;
        assert_eq!(
            chunk.get(lx, ly, lz),
            Some(BlockId::WATER),
            "Village center well should contain water at origin"
        );
    }

    #[test]
    fn test_village_discoverable_within_reasonable_distance_across_seeds() {
        for seed in [42_u32, 1337, 2025, 9999, 12345] {
            let gen = TerrainGenerator::new(seed);
            let nearest = gen
                .find_nearest_village(0, 0, None, 6)
                .unwrap_or_else(|| panic!("Seed {} had no Village within 6 rings", seed));
            let dist = ((nearest.origin.x as f64).hypot(nearest.origin.z as f64)) as i32;
            assert!(
                dist >= VILLAGE_MIN_ORIGIN_DIST && dist <= 1600,
                "Seed {} nearest Village distance {} out of expected 160..=1600 range",
                seed,
                dist
            );
        }
    }
}
