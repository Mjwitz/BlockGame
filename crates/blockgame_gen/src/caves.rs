use blockgame_core::{
    block::BlockId,
    chunk::{Chunk, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, CHUNK_VOLUME},
};

use crate::terrain::{Biome, TerrainGenerator};

/// Horizontal size (in blocks) of each deterministic cave-network region (4x4 chunks).
pub const CAVE_REGION_SIZE: i32 = 64;

/// Horizontal size (in blocks) of the macro grid that seeds rare surface-breaching Cave Mouths.
/// One candidate per 576x576 cell; since ~20-25% land in Ocean/River/Swamp and are skipped,
/// dry land averages roughly 1 Cave Mouth per 600-700 blocks.
pub const CAVE_MOUTH_CELL_SIZE: i32 = 576;

/// Regional underground atmosphere governing cave wall materials, pools, and formations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaveZone {
    Dry,
    Damp,
    DeepVolcanic,
}

/// Underground carving shape composed within a cave network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaveArchetype {
    WindingPassage,
    VaultedCavern,
    Fissure,
}

/// Deterministic PRNG (splitmix64) for cave carvers and organic ore veins.
#[derive(Clone, Copy)]
struct Rng64(u64);

impl Rng64 {
    #[inline]
    fn from_coords(seed: u32, x: i32, y: i32, z: i32, salt: u64) -> Self {
        let mut h = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
        h ^= (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = splitmix64(h);
        h ^= (y as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        h = splitmix64(h);
        h ^= (z as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        Self(splitmix64(h))
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64(self.0)
    }

    #[inline]
    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }

    #[inline]
    fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    #[inline]
    fn range_i32(&mut self, lo: i32, hi_inclusive: i32) -> i32 {
        if hi_inclusive <= lo {
            return lo;
        }
        let span = (hi_inclusive - lo + 1) as u64;
        lo + (self.next_u64() % span) as i32
    }
}

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// An ellipsoidal carving primitive generated along a worm, cavern, or fissure.
#[derive(Clone, Copy)]
struct CarveNode {
    cx: f32,
    cy: f32,
    cz: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    allows_surface_breach: bool,
    zone: CaveZone,
    archetype: CaveArchetype,
    pool_water_y: Option<i32>,
}

/// Parameters for carving a single deterministic snaking tunnel path.
#[derive(Clone, Copy)]
struct WormConfig {
    region_origin: (f32, f32),
    start: (f32, f32, f32),
    yaw: f32,
    pitch: f32,
    steps: i32,
    base_radius: f32,
    allows_surface_breach: bool,
    zone: CaveZone,
}

/// Per-voxel carving result for a single 16x16x16 chunk.
#[derive(Clone, Copy)]
pub struct ChunkCarveCell {
    pub carved: bool,
    pub allows_surface_breach: bool,
    pub zone: CaveZone,
    pub archetype: CaveArchetype,
    pub pool_water_y: i32, // 0 means no local water pool
}

impl Default for ChunkCarveCell {
    fn default() -> Self {
        Self {
            carved: false,
            allows_surface_breach: false,
            zone: CaveZone::Dry,
            archetype: CaveArchetype::WindingPassage,
            pool_water_y: 0,
        }
    }
}

/// Computes the regional CaveZone for a given world coordinate `(wx, wy, wz)`.
#[inline]
pub fn cave_zone_at(seed: u32, wx: i32, wy: i32, wz: i32) -> CaveZone {
    if wy <= 14 {
        return CaveZone::DeepVolcanic;
    }
    let rx = wx.div_euclid(CAVE_REGION_SIZE);
    let rz = wz.div_euclid(CAVE_REGION_SIZE);
    let h = splitmix64(
        (seed as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
            ^ (rx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (rz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ 0xCAFE_BABE_1234_5678,
    );
    if (h % 100) < 45 {
        CaveZone::Damp
    } else {
        CaveZone::Dry
    }
}

/// Returns the deterministic lava-pool top Y level (`4..=7`) for a deep region, or `0` if dry.
#[inline]
pub fn region_lava_level(seed: u32, wx: i32, wz: i32) -> i32 {
    let rx = wx.div_euclid(CAVE_REGION_SIZE);
    let rz = wz.div_euclid(CAVE_REGION_SIZE);
    let h = splitmix64(
        (seed as u64).wrapping_mul(0x94D0_49BB_1331_11EB)
            ^ (rx as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (rz as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ 0x1A7A_9001,
    );
    if (h % 100) < 70 {
        4 + ((h >> 8) % 4) as i32 // 4..=7
    } else {
        0
    }
}

/// Returns the jagged Deepstone transition threshold (`14..=18`) at `(wx, wz)`.
/// Voxels strictly below this Y become Deepstone instead of Stone.
#[inline]
pub fn deepstone_ceiling_y(seed: u32, wx: i32, wz: i32) -> i32 {
    let h = splitmix64(
        (seed as u64).wrapping_add(0xD339_570E)
            ^ (wx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (wz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    );
    14 + (h % 5) as i32 // 14..=18
}

/// Deterministic surface Cave Mouth origin inside a 576x576 macro cell, if the spot is dry land.
pub fn cave_mouth_origin_in_cell(
    gen: &TerrainGenerator,
    cell_x: i32,
    cell_z: i32,
) -> Option<(i32, i32, i32)> {
    let mut rng = Rng64::from_coords(gen.seed, cell_x, 0, cell_z, 0xC47E_3007);
    // Keep away from cell borders by 48 blocks so mouths stay well spaced
    let margin = 48;
    let ox = cell_x * CAVE_MOUTH_CELL_SIZE + rng.range_i32(margin, CAVE_MOUTH_CELL_SIZE - margin);
    let oz = cell_z * CAVE_MOUTH_CELL_SIZE + rng.range_i32(margin, CAVE_MOUTH_CELL_SIZE - margin);
    let sy = gen.surface_height(ox, oz);
    if sy <= gen.water_level + 3 {
        return None;
    }
    let biome = gen.get_biome(ox, oz, sy);
    if matches!(
        biome,
        Biome::Ocean | Biome::CoralReef | Biome::River | Biome::Swamp | Biome::Beach
    ) {
        return None;
    }
    // Verify a small dry buffer around the mouth so water does not flood it
    for (dx, dz) in [(-4, 0), (4, 0), (0, -4), (0, 4)] {
        if gen.surface_height(ox + dx, oz + dz) <= gen.water_level + 2 {
            return None;
        }
    }
    Some((ox, sy, oz))
}

/// Generates all carving nodes for a single 64x64 underground cave region.
fn build_region_cave_nodes(
    seed: u32,
    rx: i32,
    rz: i32,
    chunk_min: [f32; 3],
    chunk_max: [f32; 3],
    out: &mut Vec<CarveNode>,
) {
    let mut rng = Rng64::from_coords(seed, rx, 0, rz, 0xC47E_A1B2);

    let ox = (rx * CAVE_REGION_SIZE) as f32 + rng.range_f32(8.0, (CAVE_REGION_SIZE - 8) as f32);
    let oz = (rz * CAVE_REGION_SIZE) as f32 + rng.range_f32(8.0, (CAVE_REGION_SIZE - 8) as f32);
    let oy = rng.range_f32(8.0, 34.0);

    // Quick coarse horizontal rejection: all primary worms, branches, caverns, and fissures
    // stay strictly within 124 blocks of `(ox, oz)` (fitting cleanly inside the 5x5 region scan).
    const MAX_REACH: f32 = 132.0;
    if ox + MAX_REACH < chunk_min[0]
        || ox - MAX_REACH > chunk_max[0]
        || oz + MAX_REACH < chunk_min[2]
        || oz - MAX_REACH > chunk_max[2]
    {
        return;
    }

    let base_zone = cave_zone_at(seed, ox as i32, oy as i32, oz as i32);

    // 1. Primary Winding Passage (80-165 blocks long, gentle walkable slopes)
    let primary_steps = rng.range_i32(42, 78);
    let primary_yaw = rng.range_f32(0.0, std::f32::consts::TAU);
    let primary_pitch = rng.range_f32(-0.18, 0.14);
    let primary_radius = rng.range_f32(2.2, 3.3);
    let mut branch_points: Vec<(f32, f32, f32, f32)> = Vec::new();

    carve_worm_path(
        &mut rng,
        WormConfig {
            region_origin: (ox, oz),
            start: (ox, oy, oz),
            yaw: primary_yaw,
            pitch: primary_pitch,
            steps: primary_steps,
            base_radius: primary_radius,
            allows_surface_breach: false,
            zone: base_zone,
        },
        chunk_min,
        chunk_max,
        Some(&mut branch_points),
        out,
    );

    // 2. 1-3 branching Winding Passages off the main path (bounded relative to region origin `(ox, oz)`)
    let branch_count = rng.range_i32(1, 3) as usize;
    for b in 0..branch_count {
        if let Some(&(bx, by, bz, byaw)) = branch_points.get(b) {
            let side = if (rng.next_u64() & 1) == 0 { 1.0 } else { -1.0 };
            let yaw = byaw + side * rng.range_f32(0.75, 1.55);
            let pitch = rng.range_f32(-0.22, 0.16);
            let steps = rng.range_i32(24, 48);
            let rad = rng.range_f32(1.9, 2.8);
            carve_worm_path(
                &mut rng,
                WormConfig {
                    region_origin: (ox, oz),
                    start: (bx, by, bz),
                    yaw,
                    pitch,
                    steps,
                    base_radius: rad,
                    allows_surface_breach: false,
                    zone: base_zone,
                },
                chunk_min,
                chunk_max,
                None,
                out,
            );
        }
    }

    // 3. Vaulted Cavern at origin or a branch junction (breaks up long tunnels)
    let (vx, vy, vz) = if let Some(&(bx, by, bz, _)) = branch_points.first() {
        (bx, by, bz)
    } else {
        (ox, oy, oz)
    };
    let cav_rx = rng.range_f32(5.2, 8.6);
    let cav_ry = rng.range_f32(4.2, 6.4);
    let cav_rz = rng.range_f32(5.2, 8.6);
    let cav_zone = cave_zone_at(seed, vx as i32, vy as i32, vz as i32);
    let pool_water_y = if cav_zone == CaveZone::Damp && vy >= 14.0 && vy <= 32.0 {
        let floor_y = (vy - cav_ry).floor() as i32;
        Some((floor_y + 2).clamp(12, 34))
    } else {
        None
    };
    push_node_if_overlaps(
        CarveNode {
            cx: vx,
            cy: vy,
            cz: vz,
            rx: cav_rx,
            ry: cav_ry,
            rz: cav_rz,
            allows_surface_breach: false,
            zone: cav_zone,
            archetype: CaveArchetype::VaultedCavern,
            pool_water_y,
        },
        chunk_min,
        chunk_max,
        out,
    );

    // 4. Vertical Fissure in ~40% of regions (tall, narrow rift exposing multiple Y layers)
    if (rng.next_u64() % 100) < 40 {
        let fx = ox + rng.range_f32(-18.0, 18.0);
        let fz = oz + rng.range_f32(-18.0, 18.0);
        let fy = rng.range_f32(14.0, 26.0);
        let mut cur_x = fx;
        let mut cur_y = fy;
        let mut cur_z = fz;
        let mut yaw = rng.range_f32(0.0, std::f32::consts::TAU);
        let mut yaw_vel = 0.0_f32;
        let f_steps = rng.range_i32(18, 34);
        let half_h = rng.range_f32(6.5, 10.0); // 13..20 blocks tall
        let width_r = rng.range_f32(1.5, 2.3); // 3..4.6 blocks wide

        for s in 0..f_steps {
            let t = s as f32 / f_steps as f32;
            let taper = (t * std::f32::consts::PI).sin().clamp(0.35, 1.0);
            let fzone = cave_zone_at(seed, cur_x as i32, cur_y as i32, cur_z as i32);
            push_node_if_overlaps(
                CarveNode {
                    cx: cur_x,
                    cy: cur_y,
                    cz: cur_z,
                    rx: width_r * taper,
                    ry: half_h * taper,
                    rz: width_r * taper,
                    allows_surface_breach: false,
                    zone: fzone,
                    archetype: CaveArchetype::Fissure,
                    pool_water_y: None,
                },
                chunk_min,
                chunk_max,
                out,
            );
            yaw_vel = yaw_vel * 0.65 + rng.range_f32(-0.18, 0.18);
            yaw += yaw_vel;
            cur_x += yaw.cos() * 2.0;
            cur_z += yaw.sin() * 2.0;
            cur_y = (cur_y + rng.range_f32(-0.15, 0.15)).clamp(10.0, 28.0);
            let rdx = cur_x - ox;
            let rdz = cur_z - oz;
            if rdx * rdx + rdz * rdz > 118.0 * 118.0 {
                break;
            }
        }
    }
}

/// Generates a rare surface-breaching Cave Mouth tunnel descending gently from dry land.
fn build_cave_mouth_nodes(
    gen: &TerrainGenerator,
    cell_x: i32,
    cell_z: i32,
    chunk_min: [f32; 3],
    chunk_max: [f32; 3],
    out: &mut Vec<CarveNode>,
) {
    let Some((ox, sy, oz)) = cave_mouth_origin_in_cell(gen, cell_x, cell_z) else {
        return;
    };

    const MAX_MOUTH_REACH: f32 = 96.0;
    if (ox as f32) + MAX_MOUTH_REACH < chunk_min[0]
        || (ox as f32) - MAX_MOUTH_REACH > chunk_max[0]
        || (oz as f32) + MAX_MOUTH_REACH < chunk_min[2]
        || (oz as f32) - MAX_MOUTH_REACH > chunk_max[2]
    {
        return;
    }

    let mut rng = Rng64::from_coords(gen.seed, cell_x, 1, cell_z, 0xC47E_5007);
    let yaw = rng.range_f32(0.0, std::f32::consts::TAU);
    let zone = cave_zone_at(gen.seed, ox, (sy - 10).max(16), oz);

    // Entrance arch slightly above surface sloping smoothly underground
    carve_worm_path(
        &mut rng,
        WormConfig {
            region_origin: (ox as f32, oz as f32),
            start: (ox as f32, (sy + 1) as f32, oz as f32),
            yaw,
            pitch: -0.34, // Descending walkable ramp (~19 deg)
            steps: 42,
            base_radius: 2.9,
            allows_surface_breach: true,
            zone,
        },
        chunk_min,
        chunk_max,
        None,
        out,
    );
}

fn carve_worm_path(
    rng: &mut Rng64,
    cfg: WormConfig,
    chunk_min: [f32; 3],
    chunk_max: [f32; 3],
    mut branch_points: Option<&mut Vec<(f32, f32, f32, f32)>>,
    out: &mut Vec<CarveNode>,
) {
    let (mut x, mut y, mut z) = cfg.start;
    let mut yaw = cfg.yaw;
    let mut pitch = cfg.pitch;
    let mut yaw_vel = 0.0_f32;
    let mut pitch_vel = 0.0_f32;
    let step_len = 2.0_f32;

    for s in 0..cfg.steps {
        let t = s as f32 / cfg.steps.max(1) as f32;
        let pulse = 0.82 + 0.36 * (t * std::f32::consts::PI * 3.0).sin().abs();
        let rad = (cfg.base_radius * pulse).clamp(1.75, 4.2);

        let local_zone = if y <= 14.0 {
            CaveZone::DeepVolcanic
        } else {
            cfg.zone
        };

        // Subtle shallow water pool in low dips of Damp Winding Passages
        let pool_water_y = if !cfg.allows_surface_breach
            && local_zone == CaveZone::Damp
            && y >= 14.0
            && y <= 31.0
            && (s % 19 == 9)
        {
            Some(((y - rad).floor() as i32 + 1).clamp(12, 33))
        } else {
            None
        };

        push_node_if_overlaps(
            CarveNode {
                cx: x,
                cy: y,
                cz: z,
                rx: rad,
                ry: (rad * 0.92).max(1.8),
                rz: rad,
                allows_surface_breach: cfg.allows_surface_breach && s < 14,
                zone: local_zone,
                archetype: CaveArchetype::WindingPassage,
                pool_water_y,
            },
            chunk_min,
            chunk_max,
            out,
        );

        if let Some(ref mut bps) = branch_points {
            if s == cfg.steps / 4 || s == cfg.steps / 2 || s == (cfg.steps * 3) / 4 {
                bps.push((x, y, z, yaw));
            }
        }

        // Smooth snaking turn with gentle walkable pitch
        yaw_vel = yaw_vel * 0.72 + rng.range_f32(-0.24, 0.24);
        yaw += yaw_vel;

        pitch_vel = pitch_vel * 0.65 + rng.range_f32(-0.08, 0.08);
        pitch = (pitch + pitch_vel).clamp(-0.36, 0.28);

        // Gently nudge away from bedrock floor and upper ceiling
        if y < 6.5 {
            pitch = pitch.max(0.10);
        } else if !cfg.allows_surface_breach && y > 35.0 {
            pitch = pitch.min(-0.12);
        } else if cfg.allows_surface_breach && s < 12 {
            // Keep surface Cave Mouth descending steadily into the rock
            pitch = pitch.clamp(-0.42, -0.22);
        }

        let cos_p = pitch.cos();
        x += yaw.cos() * cos_p * step_len;
        z += yaw.sin() * cos_p * step_len;
        y += pitch.sin() * step_len;
        y = y.clamp(3.0, 96.0);

        // Keep excursion strictly bounded relative to the region origin so the 5x5 region
        // scan never clips cross-chunk branches (ADR 0009).
        let dx = x - cfg.region_origin.0;
        let dz = z - cfg.region_origin.1;
        if dx * dx + dz * dz > 118.0 * 118.0 {
            break;
        }
    }
}

#[inline]
fn push_node_if_overlaps(
    node: CarveNode,
    chunk_min: [f32; 3],
    chunk_max: [f32; 3],
    out: &mut Vec<CarveNode>,
) {
    if node.cx + node.rx < chunk_min[0]
        || node.cx - node.rx > chunk_max[0]
        || node.cy + node.ry < chunk_min[1]
        || node.cy - node.ry > chunk_max[1]
        || node.cz + node.rz < chunk_min[2]
        || node.cz - node.rz > chunk_max[2]
    {
        return;
    }
    out.push(node);
}

/// Evaluates all region-seeded cave carvers overlapping the chunk at `origin` and
/// returns a 4096-entry chunk-local carving mask.
pub fn compute_chunk_cave_mask(
    gen: &TerrainGenerator,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
) -> Vec<ChunkCarveCell> {
    let mut mask = vec![ChunkCarveCell::default(); CHUNK_VOLUME];
    // No caves exist above y = 104
    if origin_y > 104 {
        return mask;
    }

    let chunk_min = [origin_x as f32, origin_y as f32, origin_z as f32];
    let chunk_max = [
        (origin_x + CHUNK_SIZE_X as i32) as f32,
        (origin_y + CHUNK_SIZE_Y as i32) as f32,
        (origin_z + CHUNK_SIZE_Z as i32) as f32,
    ];

    let mut nodes: Vec<CarveNode> = Vec::with_capacity(64);

    // 1. Scan 5x5 neighborhood of 64x64 underground cave regions
    let base_rx = origin_x.div_euclid(CAVE_REGION_SIZE);
    let base_rz = origin_z.div_euclid(CAVE_REGION_SIZE);
    for drz in -2..=2 {
        for drx in -2..=2 {
            build_region_cave_nodes(
                gen.seed,
                base_rx + drx,
                base_rz + drz,
                chunk_min,
                chunk_max,
                &mut nodes,
            );
        }
    }

    // 2. Scan 3x3 neighborhood of 576x576 Cave Mouth macro cells
    let base_mx = origin_x.div_euclid(CAVE_MOUTH_CELL_SIZE);
    let base_mz = origin_z.div_euclid(CAVE_MOUTH_CELL_SIZE);
    for dmz in -1..=1 {
        for dmx in -1..=1 {
            build_cave_mouth_nodes(
                gen,
                base_mx + dmx,
                base_mz + dmz,
                chunk_min,
                chunk_max,
                &mut nodes,
            );
        }
    }

    // 3. Rasterize overlapping ellipsoidal nodes into the chunk mask
    for node in nodes {
        let min_lx = ((node.cx - node.rx).floor() as i32 - origin_x).clamp(0, CHUNK_SIZE_X as i32 - 1);
        let max_lx = ((node.cx + node.rx).ceil() as i32 - origin_x).clamp(0, CHUNK_SIZE_X as i32 - 1);
        let min_ly = ((node.cy - node.ry).floor() as i32 - origin_y).clamp(0, CHUNK_SIZE_Y as i32 - 1);
        let max_ly = ((node.cy + node.ry).ceil() as i32 - origin_y).clamp(0, CHUNK_SIZE_Y as i32 - 1);
        let min_lz = ((node.cz - node.rz).floor() as i32 - origin_z).clamp(0, CHUNK_SIZE_Z as i32 - 1);
        let max_lz = ((node.cz + node.rz).ceil() as i32 - origin_z).clamp(0, CHUNK_SIZE_Z as i32 - 1);

        let inv_rx = 1.0 / node.rx.max(0.1);
        let inv_ry = 1.0 / node.ry.max(0.1);
        let inv_rz = 1.0 / node.rz.max(0.1);

        for ly in min_ly..=max_ly {
            let wy = origin_y + ly;
            if wy <= 0 {
                continue; // Never carve bedrock
            }
            let dy = (wy as f32 + 0.5 - node.cy) * inv_ry;
            let dy2 = dy * dy;
            if dy2 > 1.0 {
                continue;
            }
            for lz in min_lz..=max_lz {
                let wz = origin_z + lz;
                let dz = (wz as f32 + 0.5 - node.cz) * inv_rz;
                let dyz2 = dy2 + dz * dz;
                if dyz2 > 1.0 {
                    continue;
                }
                for lx in min_lx..=max_lx {
                    let wx = origin_x + lx;
                    let dx = (wx as f32 + 0.5 - node.cx) * inv_rx;
                    if dx * dx + dyz2 <= 1.0 {
                        let idx = Chunk::index(lx as usize, ly as usize, lz as usize);
                        let cell = &mut mask[idx];
                        cell.carved = true;
                        if node.allows_surface_breach {
                            cell.allows_surface_breach = true;
                        }
                        cell.zone = node.zone;
                        cell.archetype = node.archetype;
                        if let Some(py) = node.pool_water_y {
                            cell.pool_water_y = cell.pool_water_y.max(py);
                        }
                    }
                }
            }
        }
    }

    mask
}

/// Depth-curve specification for an organic Ore Vein type.
struct OreVeinSpec {
    block: BlockId,
    salt: u64,
    min_y: i32,
    max_y: i32,
    min_size: usize,
    max_size: usize,
    /// Peak veins attempted per 8x8x8 macro cell (scaled by depth weight `0.0..=1.0`).
    attempts_per_cell: u32,
    base_chance: f32,
    weight_fn: fn(i32) -> f32,
}

fn diamond_weight(y: i32) -> f32 {
    if !(1..=10).contains(&y) {
        return 0.0;
    }
    if (2..=8).contains(&y) {
        1.0
    } else {
        0.35
    }
}

fn gold_weight(y: i32) -> f32 {
    if !(2..=24).contains(&y) {
        return 0.0;
    }
    // Bell curve peaking y 8..=16
    if (8..=16).contains(&y) {
        1.0
    } else if y < 8 {
        0.25 + 0.75 * ((y - 2) as f32 / 6.0)
    } else {
        0.25 + 0.75 * ((24 - y) as f32 / 8.0)
    }
}

fn iron_weight(y: i32) -> f32 {
    if !(1..=96).contains(&y) {
        return 0.0;
    }
    // Strong peak at y 25..=50, present at all levels
    if (25..=50).contains(&y) {
        1.0
    } else if y < 25 {
        0.28 + 0.72 * ((y - 1) as f32 / 24.0)
    } else {
        0.25 + 0.75 * ((96 - y) as f32 / 46.0)
    }
}

fn copper_weight(y: i32) -> f32 {
    if !(16..=72).contains(&y) {
        return 0.0;
    }
    if (30..=55).contains(&y) {
        1.0
    } else if y < 30 {
        0.25 + 0.75 * ((y - 16) as f32 / 14.0)
    } else {
        0.25 + 0.75 * ((72 - y) as f32 / 17.0)
    }
}

fn coal_weight(y: i32) -> f32 {
    if !(10..=116).contains(&y) {
        return 0.0;
    }
    // Peak across y 40..=80; drops off below y 40 and above y 80
    if (40..=80).contains(&y) {
        1.0
    } else if y < 40 {
        0.12 + 0.88 * ((y - 10) as f32 / 30.0)
    } else {
        0.15 + 0.85 * ((116 - y) as f32 / 36.0)
    }
}

const ORE_SPECS: [OreVeinSpec; 5] = [
    OreVeinSpec {
        block: BlockId::DIAMOND_ORE,
        salt: 0x0D1A_1001,
        min_y: 1,
        max_y: 10,
        min_size: 1,
        max_size: 4,
        attempts_per_cell: 1,
        base_chance: 0.34,
        weight_fn: diamond_weight,
    },
    OreVeinSpec {
        block: BlockId::GOLD_ORE,
        salt: 0x0601_D002,
        min_y: 2,
        max_y: 24,
        min_size: 2,
        max_size: 5,
        attempts_per_cell: 1,
        base_chance: 0.56,
        weight_fn: gold_weight,
    },
    OreVeinSpec {
        block: BlockId::IRON_ORE,
        salt: 0x0120_4003,
        min_y: 1,
        max_y: 96,
        min_size: 3,
        max_size: 6,
        attempts_per_cell: 2,
        base_chance: 0.64,
        weight_fn: iron_weight,
    },
    OreVeinSpec {
        block: BlockId::COPPER_ORE,
        salt: 0x0C09_9004,
        min_y: 16,
        max_y: 72,
        min_size: 3,
        max_size: 7,
        attempts_per_cell: 2,
        base_chance: 0.54,
        weight_fn: copper_weight,
    },
    OreVeinSpec {
        block: BlockId::COAL_ORE,
        salt: 0x0C0A_1005,
        min_y: 10,
        max_y: 116,
        min_size: 4,
        max_size: 10,
        attempts_per_cell: 2,
        base_chance: 0.72,
        weight_fn: coal_weight,
    },
];

/// Stamps deterministic, compact, organic Ore Veins into Stone/Deepstone/Cobblestone voxels
/// of `chunk` using 8x8x8 macro-cells (scanning a 1-cell border so veins cross chunk edges seamlessly).
pub fn stamp_chunk_ore_veins(seed: u32, chunk: &mut Chunk) {
    let origin = chunk.pos.to_world_origin();
    let min_cx = (origin.x - 4).div_euclid(8);
    let max_cx = (origin.x + CHUNK_SIZE_X as i32 + 3).div_euclid(8);
    let min_cy = (origin.y - 4).div_euclid(8).max(0);
    let max_cy = (origin.y + CHUNK_SIZE_Y as i32 + 3).div_euclid(8).min(15);
    let min_cz = (origin.z - 4).div_euclid(8);
    let max_cz = (origin.z + CHUNK_SIZE_Z as i32 + 3).div_euclid(8);

    const DIRS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];

    for spec in &ORE_SPECS {
        let cell_y_lo = (spec.min_y / 8).max(min_cy);
        let cell_y_hi = (spec.max_y / 8).min(max_cy);
        if cell_y_lo > cell_y_hi {
            continue;
        }

        for cy in cell_y_lo..=cell_y_hi {
            for cz in min_cz..=max_cz {
                for cx in min_cx..=max_cx {
                    let mut rng = Rng64::from_coords(seed, cx, cy, cz, spec.salt);
                    for _ in 0..spec.attempts_per_cell {
                        let vx = cx * 8 + rng.range_i32(0, 7);
                        let vy = cy * 8 + rng.range_i32(0, 7);
                        let vz = cz * 8 + rng.range_i32(0, 7);

                        let w = (spec.weight_fn)(vy);
                        if w <= 0.0 || rng.next_f32() >= spec.base_chance * w {
                            continue;
                        }

                        let target_size = rng.range_i32(spec.min_size as i32, spec.max_size as i32) as usize;
                        let mut cur_x = vx;
                        let mut cur_y = vy;
                        let mut cur_z = vz;
                        let mut placed_coords = [(0i32, 0i32, 0i32); 10];
                        let mut count = 0usize;

                        // Deterministic compact random walk (stays within ~2 blocks of seed)
                        for _ in 0..(target_size * 3) {
                            if count >= target_size {
                                break;
                            }
                            if (spec.min_y..=spec.max_y).contains(&cur_y)
                                && !placed_coords[..count].contains(&(cur_x, cur_y, cur_z))
                            {
                                placed_coords[count] = (cur_x, cur_y, cur_z);
                                count += 1;

                                let lx = cur_x - origin.x;
                                let ly = cur_y - origin.y;
                                let lz = cur_z - origin.z;
                                if (0..CHUNK_SIZE_X as i32).contains(&lx)
                                    && (0..CHUNK_SIZE_Y as i32).contains(&ly)
                                    && (0..CHUNK_SIZE_Z as i32).contains(&lz)
                                {
                                    let existing =
                                        chunk.get_unchecked(lx as usize, ly as usize, lz as usize);
                                    if matches!(
                                        existing,
                                        BlockId::STONE | BlockId::DEEPSTONE | BlockId::COBBLESTONE
                                    ) {
                                        chunk.set(lx as usize, ly as usize, lz as usize, spec.block);
                                    }
                                }
                            }

                            // Step from a random already-placed block in the cluster to keep veins compact
                            let anchor = placed_coords[(rng.next_u64() as usize) % count.max(1)];
                            let (dx, dy, dz) = DIRS[(rng.next_u64() as usize) % 6];
                            cur_x = anchor.0 + dx;
                            cur_y = anchor.1 + dy;
                            cur_z = anchor.2 + dz;
                        }
                    }
                }
            }
        }
    }
}

/// Applies regional Cave Zone wall/floor pockets (Mossy Cobblestone, Gravel, Clay, Dirt),
/// Dripstone stalactites/stalagmites, rare Glow Crystals, and occasional Cave/Lava Springs
/// to carved cave interiors inside `chunk`.
pub fn decorate_chunk_caves(
    seed: u32,
    chunk: &mut Chunk,
    cave_mask: &[ChunkCarveCell],
) {
    let origin = chunk.pos.to_world_origin();
    if origin.y > 104 {
        return;
    }

    // Pass 1: Wall & floor material accents (Mossy Cobblestone, Clay, Gravel, Dirt)
    for ly in 0..CHUNK_SIZE_Y {
        let wy = origin.y + ly as i32;
        if wy <= 0 {
            continue;
        }
        for lz in 0..CHUNK_SIZE_Z {
            let wz = origin.z + lz as i32;
            for lx in 0..CHUNK_SIZE_X {
                let wx = origin.x + lx as i32;
                let idx = Chunk::index(lx, ly, lz);
                let cell = cave_mask[idx];
                if !cell.carved {
                    continue;
                }

                // Floor decoration (solid rock directly below carved air/water)
                if ly > 0 {
                    let below = chunk.get_unchecked(lx, ly - 1, lz);
                    if below == BlockId::STONE || below == BlockId::DEEPSTONE {
                        let h = splitmix64(
                            (seed as u64).wrapping_add(0xF100_1234)
                                ^ (wx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                ^ (wy as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                                ^ (wz as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB),
                        );
                        let cur_block = chunk.get_unchecked(lx, ly, lz);
                        if cur_block == BlockId::WATER {
                            // Water pool rim/bed gets Clay & Gravel
                            let bed = if (h % 3) == 0 {
                                BlockId::CLAY
                            } else {
                                BlockId::GRAVEL
                            };
                            chunk.set(lx, ly - 1, lz, bed);
                        } else if cur_block == BlockId::AIR {
                            match cell.zone {
                                CaveZone::Damp => {
                                    let r = h % 100;
                                    if r < 18 {
                                        chunk.set(lx, ly - 1, lz, BlockId::MOSSY_COBBLESTONE);
                                    } else if r < 26 {
                                        chunk.set(lx, ly - 1, lz, BlockId::CLAY);
                                    } else if r < 34 {
                                        chunk.set(lx, ly - 1, lz, BlockId::GRAVEL);
                                    }
                                }
                                CaveZone::Dry => {
                                    let r = h % 100;
                                    if r < 10 {
                                        chunk.set(lx, ly - 1, lz, BlockId::GRAVEL);
                                    } else if r < 15 && wy > 18 {
                                        chunk.set(lx, ly - 1, lz, BlockId::DIRT);
                                    }
                                }
                                CaveZone::DeepVolcanic => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // Pass 2: Dripstone (stalactites & stalagmites), Glow Crystals, and rare Wall Springs
    for ly in 1..(CHUNK_SIZE_Y - 1) {
        let wy = origin.y + ly as i32;
        if wy <= 1 {
            continue;
        }
        for lz in 1..(CHUNK_SIZE_Z - 1) {
            let wz = origin.z + lz as i32;
            for lx in 1..(CHUNK_SIZE_X - 1) {
                let wx = origin.x + lx as i32;
                let idx = Chunk::index(lx, ly, lz);
                let cell = cave_mask[idx];
                if !cell.carved || chunk.get_unchecked(lx, ly, lz) != BlockId::AIR {
                    continue;
                }

                let h = splitmix64(
                    (seed as u64).wrapping_add(0xD219_570E)
                        ^ (wx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (wy as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                        ^ (wz as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93),
                );

                let above = chunk.get_unchecked(lx, ly + 1, lz);
                let below = chunk.get_unchecked(lx, ly - 1, lz);
                let ceiling_rock = matches!(
                    above,
                    BlockId::STONE | BlockId::DEEPSTONE | BlockId::COBBLESTONE
                );
                let floor_rock = matches!(
                    below,
                    BlockId::STONE
                        | BlockId::DEEPSTONE
                        | BlockId::COBBLESTONE
                        | BlockId::MOSSY_COBBLESTONE
                );

                // Stalactites hanging from ceiling (facing = 1)
                if ceiling_rock && below == BlockId::AIR {
                    let r = h % 1000;
                    if r < 6 && wy <= 34 {
                        chunk.set(lx, ly, lz, BlockId::GLOW_CRYSTAL);
                        continue;
                    } else if r < 68 {
                        chunk.set(lx, ly, lz, BlockId::DRIPSTONE);
                        chunk.set_facing(lx, ly, lz, 1);
                        continue;
                    }
                }

                // Stalagmites rising from floor (facing = 0)
                if floor_rock && above == BlockId::AIR {
                    let r = (h >> 12) % 1000;
                    if r < 46 {
                        chunk.set(lx, ly, lz, BlockId::DRIPSTONE);
                        chunk.set_facing(lx, ly, lz, 0);
                        continue;
                    }
                }

                // Rare Glow Crystals on cave walls (below y = 34)
                if wy <= 34 {
                    let wall_rock = [
                        chunk.get_unchecked(lx + 1, ly, lz),
                        chunk.get_unchecked(lx - 1, ly, lz),
                        chunk.get_unchecked(lx, ly, lz + 1),
                        chunk.get_unchecked(lx, ly, lz - 1),
                    ]
                    .iter()
                    .any(|&b| matches!(b, BlockId::STONE | BlockId::DEEPSTONE));

                    if wall_rock {
                        let r = (h >> 24) % 1000;
                        if r < 5 {
                            chunk.set(lx, ly, lz, BlockId::GLOW_CRYSTAL);
                            continue;
                        }
                        // Rare Cave Springs (Water in Damp zones strictly above lava zone y > 8)
                        if cell.zone == CaveZone::Damp
                            && (12..=34).contains(&wy)
                            && below == BlockId::AIR
                            && r == 500
                        {
                            // Cascade spring cleanly downward while staying strictly above y = 8
                            let mut cy = ly;
                            while cy > 0
                                && (origin.y + cy as i32) > 8
                                && chunk.get_unchecked(lx, cy, lz) == BlockId::AIR
                            {
                                chunk.set(lx, cy, lz, BlockId::WATER);
                                cy -= 1;
                            }
                            continue;
                        }
                        // Rare deep Lava Wall Springs (y 4..=11)
                        if cell.zone == CaveZone::DeepVolcanic
                            && (4..=11).contains(&wy)
                            && below == BlockId::AIR
                            && r == 501
                        {
                            let mut cy = ly;
                            while cy > 0 && chunk.get_unchecked(lx, cy, lz) == BlockId::AIR {
                                chunk.set(lx, cy, lz, BlockId::LAVA);
                                cy -= 1;
                            }
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockgame_core::chunk::ChunkPos;
    use std::collections::HashSet;

    #[test]
    fn test_deepstone_stratum_lava_pools_water_pools_and_formations() {
        let gen = TerrainGenerator::new(42);
        let mut found_deepstone = false;
        let mut found_dripstone = false;
        let mut found_glow_crystal = false;
        let mut found_lava = false;
        let mut found_underground_water = false;
        let mut archetypes = HashSet::new();
        let mut zones = HashSet::new();

        for cx in 0..8 {
            for cz in 0..8 {
                for cy in 0..4 {
                    let cpos = ChunkPos::new(cx, cy, cz);
                    let origin = cpos.to_world_origin();
                    let mask = compute_chunk_cave_mask(&gen, origin.x, origin.y, origin.z);
                    for cell in &mask {
                        if cell.carved {
                            archetypes.insert(cell.archetype);
                            zones.insert(cell.zone);
                        }
                    }

                    let chunk = gen.generate_chunk(cpos);
                    for lz in 0..CHUNK_SIZE_Z {
                        let wz = origin.z + lz as i32;
                        for lx in 0..CHUNK_SIZE_X {
                            let wx = origin.x + lx as i32;
                            let sy = gen.surface_height(wx, wz);
                            for ly in 0..CHUNK_SIZE_Y {
                                let wy = origin.y + ly as i32;
                                let b = chunk.get_unchecked(lx, ly, lz);
                                if wy == 0 {
                                    assert_eq!(b, BlockId::BEDROCK, "Bedrock at y=0 must never be carved");
                                }
                                if b == BlockId::DEEPSTONE {
                                    found_deepstone = true;
                                    assert!(wy < 18, "Deepstone must only appear below y=18, got y={}", wy);
                                }
                                if wy >= 1 && wy < 14 {
                                    assert_ne!(b, BlockId::STONE, "Regular Stone must not appear below y=14");
                                }
                                if b == BlockId::DRIPSTONE {
                                    found_dripstone = true;
                                }
                                if b == BlockId::GLOW_CRYSTAL {
                                    found_glow_crystal = true;
                                }
                                if b == BlockId::LAVA {
                                    found_lava = true;
                                    assert!(wy >= 1 && wy <= 11, "Lava must only spawn deep underground, got y={}", wy);
                                }
                                // Underwater beds must remain sealed (never carved within 3 blocks of seabed)
                                if sy <= gen.water_level && wy >= sy - 3 && wy < sy {
                                    assert_ne!(
                                        b,
                                        BlockId::AIR,
                                        "Underwater seabed must not be breached by caves at ({},{},{})",
                                        wx,
                                        wy,
                                        wz
                                    );
                                }
                                if b == BlockId::WATER && wy < sy - 4 {
                                    found_underground_water = true;
                                    assert!(
                                        wy > 8,
                                        "Underground Water Pools/Springs must stay strictly above y=8, got y={}",
                                        wy
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Also sample a 6x6 grid of 64x64 cave regions to verify all three CaveZones occur
        for rz in -3..3 {
            for rx in -3..3 {
                zones.insert(cave_zone_at(gen.seed, rx * CAVE_REGION_SIZE, 24, rz * CAVE_REGION_SIZE));
                zones.insert(cave_zone_at(gen.seed, rx * CAVE_REGION_SIZE, 8, rz * CAVE_REGION_SIZE));
            }
        }

        assert!(found_deepstone, "Expected Deepstone below y=16");
        assert!(found_dripstone, "Expected Dripstone formations in caves");
        assert!(found_glow_crystal, "Expected Glow Crystals in caves");
        assert!(found_lava, "Expected deep Lava Pools in caves");
        assert!(found_underground_water, "Expected underground Water Pools in Damp caves");
        assert!(archetypes.contains(&CaveArchetype::WindingPassage), "Expected WindingPassage");
        assert!(archetypes.contains(&CaveArchetype::VaultedCavern), "Expected VaultedCavern");
        assert!(archetypes.contains(&CaveArchetype::Fissure), "Expected Fissure");
        assert!(zones.contains(&CaveZone::Dry), "Expected Dry CaveZone");
        assert!(zones.contains(&CaveZone::Damp), "Expected Damp CaveZone");
        assert!(zones.contains(&CaveZone::DeepVolcanic), "Expected DeepVolcanic CaveZone");
    }

    #[test]
    fn test_ore_vein_depth_curves_and_reduced_density() {
        let gen = TerrainGenerator::new(1337);
        let mut rock_by_y = [0u64; 128];
        let mut diamond_by_y = [0u64; 128];
        let mut gold_by_y = [0u64; 128];
        let mut iron_by_y = [0u64; 128];
        let mut copper_by_y = [0u64; 128];
        let mut coal_by_y = [0u64; 128];

        // Sample 10x10 chunk columns across all 8 vertical chunks
        for cx in -5..5 {
            for cz in -5..5 {
                for cy in 0..8 {
                    let chunk = gen.generate_chunk(ChunkPos::new(cx, cy, cz));
                    let origin = chunk.pos.to_world_origin();
                    for ly in 0..CHUNK_SIZE_Y {
                        let wy = (origin.y + ly as i32) as usize;
                        for lz in 0..CHUNK_SIZE_Z {
                            for lx in 0..CHUNK_SIZE_X {
                                let b = chunk.get_unchecked(lx, ly, lz);
                                match b {
                                    BlockId::STONE | BlockId::DEEPSTONE | BlockId::COBBLESTONE => {
                                        rock_by_y[wy] += 1;
                                    }
                                    BlockId::DIAMOND_ORE => {
                                        rock_by_y[wy] += 1;
                                        diamond_by_y[wy] += 1;
                                        assert!((1..=10).contains(&wy), "Diamond outside y 1..=10: y={}", wy);
                                    }
                                    BlockId::GOLD_ORE => {
                                        rock_by_y[wy] += 1;
                                        gold_by_y[wy] += 1;
                                        assert!((2..=24).contains(&wy), "Gold outside y 2..=24: y={}", wy);
                                    }
                                    BlockId::IRON_ORE => {
                                        rock_by_y[wy] += 1;
                                        iron_by_y[wy] += 1;
                                        assert!((1..=96).contains(&wy), "Iron outside y 1..=96: y={}", wy);
                                    }
                                    BlockId::COPPER_ORE => {
                                        rock_by_y[wy] += 1;
                                        copper_by_y[wy] += 1;
                                        assert!((16..=72).contains(&wy), "Copper outside y 16..=72: y={}", wy);
                                    }
                                    BlockId::COAL_ORE => {
                                        rock_by_y[wy] += 1;
                                        coal_by_y[wy] += 1;
                                        assert!((10..=116).contains(&wy), "Coal outside y 10..=116: y={}", wy);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        let rate = |ore: &[u64; 128], lo: usize, hi: usize| -> f64 {
            let o: u64 = ore[lo..=hi].iter().sum();
            let r: u64 = rock_by_y[lo..=hi].iter().sum();
            if r == 0 { 0.0 } else { o as f64 / r as f64 }
        };

        // 1. Iron peaks in y 25..=50 vs deep y 1..=15
        let iron_peak = rate(&iron_by_y, 25, 50);
        let iron_deep = rate(&iron_by_y, 1, 15);
        assert!(
            iron_peak > iron_deep * 1.4,
            "Iron should peak in y 25..=50 ({:.4}) over y 1..=15 ({:.4})",
            iron_peak,
            iron_deep
        );

        // 2. Coal peaks in y 40..=80, drops off below y 40, and drops off above y 80
        let coal_peak = rate(&coal_by_y, 40, 80);
        let coal_low = rate(&coal_by_y, 10, 30);
        assert!(
            coal_peak > coal_low * 1.5,
            "Coal should peak in y 40..=80 ({:.4}) over y 10..=30 ({:.4})",
            coal_peak,
            coal_low
        );
        assert!(
            coal_weight(60) > coal_weight(95) && coal_weight(95) > coal_weight(112),
            "Coal density curve must drop off above y=80"
        );

        // 3. Diamonds are genuinely rare (< 0.4% even in y 1..=10)
        let dia_rate = rate(&diamond_by_y, 1, 10);
        assert!(
            dia_rate > 0.0002 && dia_rate < 0.004,
            "Diamond rate in y 1..=10 should be rare (0.02%..0.4%), got {:.4}",
            dia_rate
        );

        // 4. Overall underground ore density (y 1..=60) reduced ~60% from old ~10-12% to ~2-5%
        let total_ores: u64 = (1..=60)
            .map(|y| diamond_by_y[y] + gold_by_y[y] + iron_by_y[y] + copper_by_y[y] + coal_by_y[y])
            .sum();
        let total_rock: u64 = rock_by_y[1..=60].iter().sum();
        let overall_rate = total_ores as f64 / total_rock.max(1) as f64;
        assert!(
            overall_rate > 0.015 && overall_rate < 0.055,
            "Overall ore rate in y 1..=60 should be ~2-5% (~60% reduction), got {:.4}",
            overall_rate
        );
    }

    #[test]
    fn test_rare_dry_land_cave_mouth_spacing() {
        let gen = TerrainGenerator::new(42);
        let mut mouth_count = 0usize;
        // Scan a 6x6 grid of 576x576 cells (~3456 x 3456 blocks)
        for cz in -3..3 {
            for cx in -3..3 {
                if let Some((ox, sy, oz)) = cave_mouth_origin_in_cell(&gen, cx, cz) {
                    mouth_count += 1;
                    assert!(
                        sy > gen.water_level + 3,
                        "Cave Mouth must be on dry land above water_level+3, got y={} at ({},{})",
                        sy,
                        ox,
                        oz
                    );
                }
            }
        }
        // Out of 36 cells, expect roughly 12..34 dry-land Cave Mouths (~1 per 576-750 blocks on land)
        assert!(
            mouth_count >= 10 && mouth_count <= 36,
            "Expected rare dry-land Cave Mouths across 36 macro cells, got {}",
            mouth_count
        );
    }
}

