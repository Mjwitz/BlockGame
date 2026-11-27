use blockgame_core::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z, VERTICAL_CHUNKS},
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {
    Plains,
    FlowerMeadow,
    Forest,
    BirchForest,
    Desert,
    RedDesert,
    Mountains,
    SnowyTundra,
    Glacier,
    Swamp,
    Savanna,
    Ocean,
    Jungle,
    Badlands,
    Beach,
    StonyShore,
    River,
    Taiga,
    DarkForest,
    CoralReef,
}

impl Biome {
    /// Atmospheric fog RGB color associated with this biome.
    #[inline]
    pub fn fog_color(&self) -> [f32; 3] {
        match self {
            Biome::Desert => [0.82, 0.70, 0.52],       // Warm golden-amber desert haze
            Biome::RedDesert => [0.88, 0.64, 0.46],    // Sunlit red-sand desert haze
            Biome::Badlands => [0.85, 0.58, 0.44],     // Terracotta canyon dust
            Biome::SnowyTundra => [0.78, 0.84, 0.92], // Cool arctic pale-blue mist
            Biome::Glacier => [0.72, 0.88, 0.96],      // Crisp glacial ice-blue mist
            Biome::Swamp => [0.44, 0.52, 0.38],       // Murky moss-green swamp fog
            Biome::Jungle => [0.50, 0.72, 0.50],      // Vibrant tropical emerald mist
            Biome::Savanna => [0.86, 0.80, 0.60],     // Sun-drenched golden afternoon
            Biome::Mountains => [0.65, 0.75, 0.90],   // Crisp alpine thin mist
            Biome::Ocean => [0.38, 0.58, 0.86],       // Deep marine azure
            Biome::CoralReef => [0.22, 0.65, 0.78],   // Vibrant turquoise/aquamarine lagoon
            Biome::Beach => [0.75, 0.82, 0.92],       // Sunny coastal breeze
            Biome::StonyShore => [0.66, 0.74, 0.82],  // Cool sea-spray rocky coast
            Biome::River => [0.55, 0.75, 0.95],       // Clear flowing stream atmosphere
            Biome::Taiga => [0.62, 0.74, 0.72],       // Cool evergreen pine mist
            Biome::DarkForest => [0.32, 0.42, 0.35],  // Dense canopy shadowy gloom
            Biome::BirchForest => [0.72, 0.82, 0.88], // Bright sunlit silver-birch woodland
            Biome::FlowerMeadow => [0.68, 0.84, 0.98], // Crisp vibrant meadow sky
            Biome::Forest | Biome::Plains => [0.60, 0.78, 0.98], // Clear temperate sky
        }
    }

    /// Atmospheric sky tint RGB factor to modulate with sun light.
    #[inline]
    pub fn sky_tint(&self) -> [f32; 3] {
        match self {
            Biome::Desert => [1.05, 0.95, 0.80],
            Biome::RedDesert => [1.08, 0.92, 0.78],
            Biome::Badlands => [1.08, 0.88, 0.75],
            Biome::SnowyTundra => [0.92, 0.96, 1.05],
            Biome::Glacier => [0.90, 0.98, 1.08],
            Biome::Swamp => [0.85, 0.92, 0.80],
            Biome::Jungle => [0.90, 1.05, 0.88],
            Biome::Savanna => [1.04, 0.98, 0.85],
            Biome::Mountains => [0.95, 0.98, 1.05],
            Biome::Ocean => [0.88, 0.95, 1.08],
            Biome::CoralReef => [0.90, 1.08, 1.12],
            Biome::Beach => [1.00, 1.00, 1.02],
            Biome::StonyShore => [0.95, 0.98, 1.02],
            Biome::River => [0.98, 1.02, 1.05],
            Biome::Taiga => [0.94, 0.98, 1.02],
            Biome::DarkForest => [0.88, 0.94, 0.86],
            Biome::BirchForest => [1.02, 1.01, 0.96],
            Biome::FlowerMeadow => [1.03, 1.02, 0.98],
            Biome::Forest | Biome::Plains => [1.00, 1.00, 1.00],
        }
    }
}

#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[inline]
fn seed_offset_pair(seed: u32, channel: u64) -> [f64; 2] {
    let h1 = splitmix64((seed as u64) ^ channel.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let h2 = splitmix64(h1 ^ 0xA076_1D64_78BD_642F);
    // Non-integer fractional offsets prevent Perlin zero-crossing alignment at (0, 0)
    let ox = ((h1 & 0xFFFF) as f64 - 32768.0) * 1.37 + 0.413;
    let oz = ((h2 & 0xFFFF) as f64 - 32768.0) * 1.41 + 0.689;
    [ox, oz]
}

#[inline]
fn seed_unit_f64(seed: u32, channel: u64) -> f64 {
    let h = splitmix64((seed as u64).wrapping_add(channel.wrapping_mul(0xD6E8_FEB8_6659_FD93)));
    ((h >> 11) as f64) / ((1u64 << 53) as f64)
}

#[inline]
fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = if (edge1 - edge0).abs() < 1e-9 {
        0.0
    } else {
        ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0)
    };
    t * t * (3.0 - 2.0 * t)
}

pub struct TerrainGenerator {
    pub seed: u32,
    continental_noise: Fbm<Perlin>,
    ridge_noise: Fbm<Perlin>,
    detail_noise: Fbm<Perlin>,
    cave_noise: Fbm<Perlin>,
    tunnel_noise_a: Fbm<Perlin>,
    tunnel_noise_b: Fbm<Perlin>,
    warp_noise_x: Perlin,
    warp_noise_z: Perlin,
    temp_noise: Perlin,
    temp_detail_noise: Perlin,
    moisture_noise: Perlin,
    moisture_detail_noise: Perlin,
    river_noise: Perlin,
    cont_offset: [f64; 2],
    ridge_offset: [f64; 2],
    detail_offset: [f64; 2],
    warp_offset: [f64; 2],
    temp_offset: [f64; 2],
    moist_offset: [f64; 2],
    river_offset: [f64; 2],
    seed_temp_bias: f64,
    seed_moist_bias: f64,
    seed_altitude_bias: f64,
    seed_relief_scale: f64,
    pub water_level: i32,
}

impl TerrainGenerator {
    pub fn new(seed: u32) -> Self {
        let continental_noise = Fbm::<Perlin>::new(seed)
            .set_octaves(2)
            .set_frequency(0.0022)
            .set_lacunarity(2.0)
            .set_persistence(0.45);

        let ridge_noise = Fbm::<Perlin>::new(seed.wrapping_add(1571))
            .set_octaves(4)
            .set_frequency(0.0072)
            .set_lacunarity(2.1)
            .set_persistence(0.48);

        let detail_noise = Fbm::<Perlin>::new(seed.wrapping_add(1013))
            .set_octaves(4)
            .set_frequency(0.022)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let cave_noise = Fbm::<Perlin>::new(seed.wrapping_add(2048))
            .set_octaves(2)
            .set_frequency(0.04)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let tunnel_noise_a = Fbm::<Perlin>::new(seed.wrapping_add(2591))
            .set_octaves(2)
            .set_frequency(0.027)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let tunnel_noise_b = Fbm::<Perlin>::new(seed.wrapping_add(2879))
            .set_octaves(2)
            .set_frequency(0.027)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        let warp_noise_x = Perlin::new(seed.wrapping_add(3511));
        let warp_noise_z = Perlin::new(seed.wrapping_add(3889));
        let temp_noise = Perlin::new(seed.wrapping_add(3072));
        let temp_detail_noise = Perlin::new(seed.wrapping_add(3319));
        let moisture_noise = Perlin::new(seed.wrapping_add(4096));
        let moisture_detail_noise = Perlin::new(seed.wrapping_add(4457));
        let river_noise = Perlin::new(seed.wrapping_add(5120));

        // Mild per-seed world character biases so seeds feel distinct while keeping all biomes reachable
        let seed_temp_bias = (seed_unit_f64(seed, 21) - 0.5) * 0.14;
        let seed_moist_bias = (seed_unit_f64(seed, 22) - 0.5) * 0.12;
        let seed_altitude_bias = (seed_unit_f64(seed, 23) - 0.5) * 4.5;
        let seed_relief_scale = 0.90 + seed_unit_f64(seed, 24) * 0.34;

        Self {
            seed,
            continental_noise,
            ridge_noise,
            detail_noise,
            cave_noise,
            tunnel_noise_a,
            tunnel_noise_b,
            warp_noise_x,
            warp_noise_z,
            temp_noise,
            temp_detail_noise,
            moisture_noise,
            moisture_detail_noise,
            river_noise,
            cont_offset: seed_offset_pair(seed, 1),
            ridge_offset: seed_offset_pair(seed, 3),
            detail_offset: seed_offset_pair(seed, 4),
            warp_offset: seed_offset_pair(seed, 5),
            temp_offset: seed_offset_pair(seed, 6),
            moist_offset: seed_offset_pair(seed, 7),
            river_offset: seed_offset_pair(seed, 8),
            seed_temp_bias,
            seed_moist_bias,
            seed_altitude_bias,
            seed_relief_scale,
            water_level: 38,
        }
    }

    #[inline]
    fn sample_warp(&self, x: i32, z: i32) -> (f64, f64) {
        let fx = x as f64;
        let fz = z as f64;
        let wx = self.warp_noise_x.get([
            fx * 0.0032 + self.warp_offset[0],
            fz * 0.0032 + self.warp_offset[1],
        ]) * 14.0;
        let wz = self.warp_noise_z.get([
            fx * 0.0032 + self.warp_offset[1],
            fz * 0.0032 - self.warp_offset[0],
        ]) * 14.0;
        (fx + wx, fz + wz)
    }

    #[inline]
    fn sample_climate(&self, wx: f64, wz: f64) -> (f64, f64) {
        let temp = self
            .temp_noise
            .get([wx * 0.0021 + self.temp_offset[0], wz * 0.0021 + self.temp_offset[1]])
            + 0.14
                * self.temp_detail_noise.get([
                    wx * 0.0048 + self.temp_offset[0],
                    wz * 0.0048 + self.temp_offset[1],
                ])
            + self.seed_temp_bias;

        let moisture = self
            .moisture_noise
            .get([wx * 0.0022 + self.moist_offset[0], wz * 0.0022 + self.moist_offset[1]])
            + 0.14
                * self.moisture_detail_noise.get([
                    wx * 0.0050 + self.moist_offset[0],
                    wz * 0.0050 + self.moist_offset[1],
                ])
            + self.seed_moist_bias;

        (temp, moisture)
    }

    #[inline]
    fn sample_river_val(&self, wx: f64, wz: f64) -> f64 {
        self.river_noise
            .get([
                wx * 0.0038 + self.river_offset[0],
                wz * 0.0038 + self.river_offset[1],
            ])
            .abs()
    }

    #[inline]
    fn classify_cell_biome(&self, sx: f64, sz: f64, cell_hash: u64) -> Biome {
        let (cell_temp, cell_moist) = self.sample_climate(sx, sz);
        let h_pick = (cell_hash >> 32) % 100;

        if cell_temp < -0.13 {
            if cell_temp < -0.24 || h_pick < 36 {
                Biome::Glacier
            } else if h_pick >= 84 {
                Biome::Mountains
            } else {
                Biome::SnowyTundra
            }
        } else if cell_temp < 0.03 {
            if cell_moist > 0.07 {
                if h_pick < 18 {
                    Biome::Mountains
                } else if h_pick < 36 {
                    Biome::Swamp
                } else {
                    Biome::Taiga
                }
            } else if cell_moist < -0.06 {
                if h_pick < 20 {
                    Biome::Mountains
                } else if h_pick < 52 {
                    Biome::FlowerMeadow
                } else {
                    Biome::Plains
                }
            } else if h_pick < 42 {
                Biome::BirchForest
            } else if h_pick < 82 {
                Biome::Forest
            } else {
                Biome::Mountains
            }
        } else if cell_temp < 0.16 {
            if cell_moist > 0.12 {
                if h_pick < 46 {
                    Biome::Swamp
                } else if h_pick < 86 {
                    Biome::DarkForest
                } else {
                    Biome::Jungle
                }
            } else if cell_moist > -0.02 {
                match h_pick % 5 {
                    0 => Biome::BirchForest,
                    1 => Biome::Forest,
                    2 => Biome::DarkForest,
                    3 => Biome::FlowerMeadow,
                    _ => Biome::Plains,
                }
            } else if h_pick < 18 {
                Biome::Mountains
            } else if h_pick < 42 {
                Biome::Savanna
            } else if h_pick < 66 {
                Biome::FlowerMeadow
            } else {
                Biome::Plains
            }
        } else if cell_moist > 0.10 {
            Biome::Jungle
        } else if cell_moist > -0.03 {
            if h_pick < 55 {
                Biome::Savanna
            } else if h_pick < 85 {
                Biome::Desert
            } else {
                Biome::Badlands
            }
        } else if h_pick < 36 {
            Biome::Desert
        } else if h_pick < 70 {
            Biome::RedDesert
        } else {
            Biome::Badlands
        }
    }

    /// Evaluates the 3x3 Voronoi macro biome cells around warped coordinate `(wx, wz)`.
    /// Guarantees every non-river/non-shore biome spans at least one macro cell (~260 blocks),
    /// while returning smooth C1 boundary weights across neighboring cells for terrain height blending.
    #[inline]
    fn sample_macro_cells(&self, wx: f64, wz: f64) -> (Biome, u64, f64, [(Biome, f64); 9]) {
        const CELL_SIZE: f64 = 260.0;
        const TRANSITION_WIDTH: f64 = 56.0;

        let gx = (wx / CELL_SIZE).floor() as i32;
        let gz = (wz / CELL_SIZE).floor() as i32;

        let mut entries: [(Biome, f64); 9] = [(Biome::Plains, 0.0); 9];
        let mut dists = [f64::MAX; 9];
        let mut best_biome = Biome::Plains;
        let mut best_hash = 0u64;
        let mut best_temp = 0.0;
        let mut min_dist = f64::MAX;

        let mut idx = 0usize;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let cx = gx + dx;
                let cz = gz + dz;
                let h = splitmix64(
                    (cx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (cz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
                        ^ (self.seed as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93),
                );
                let jx = ((h & 0xFFFF) as f64) / 65535.0;
                let jz = (((h >> 16) & 0xFFFF) as f64) / 65535.0;
                // Keep sites inside [0.28, 0.72] of each cell so neighbors are always >= 114 blocks apart
                let sx = (cx as f64 + 0.28 + jx * 0.44) * CELL_SIZE;
                let sz = (cz as f64 + 0.28 + jz * 0.44) * CELL_SIZE;

                let site_cont = self
                    .continental_noise
                    .get([sx + self.cont_offset[0], sz + self.cont_offset[1]]);
                // Deep-ocean cells gently yield coastal fringes to neighboring land cells so
                // tiny 10-block ocean-cell corners don't dot coastlines.
                let ocean_penalty = ((-0.16 - site_cont).max(0.0) * 180.0).min(75.0);

                let d = ((wx - sx).powi(2) + (wz - sz).powi(2)).sqrt() + ocean_penalty;
                let biome = self.classify_cell_biome(sx, sz, h);

                entries[idx] = (biome, 0.0);
                dists[idx] = d;
                if d < min_dist {
                    min_dist = d;
                    best_biome = biome;
                    best_hash = h;
                    best_temp = self.sample_climate(sx, sz).0;
                }
                idx += 1;
            }
        }

        let mut total_w = 0.0;
        for i in 0..9 {
            let delta = (dists[i] - min_dist).max(0.0);
            let t = ((TRANSITION_WIDTH - delta) / TRANSITION_WIDTH).clamp(0.0, 1.0);
            let w = t * t;
            entries[i].1 = w;
            total_w += w;
        }
        if total_w > 0.0 {
            for e in &mut entries {
                e.1 /= total_w;
            }
        }

        (best_biome, best_hash, best_temp, entries)
    }

    /// Computes the surface elevation at global coordinate (x, z).
    #[inline]
    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        let fx = x as f64;
        let fz = z as f64;
        let (wx, wz) = self.sample_warp(x, z);

        let cont = self
            .continental_noise
            .get([wx + self.cont_offset[0], wz + self.cont_offset[1]]);
        let ridge_raw = self
            .ridge_noise
            .get([wx + self.ridge_offset[0], wz + self.ridge_offset[1]]);
        let ridge = (1.0 - ridge_raw.abs()).clamp(0.0, 1.0).powi(2);
        let detail = self
            .detail_noise
            .get([fx + self.detail_offset[0], fz + self.detail_offset[1]]);

        let inland_mask = smoothstep(-0.16, 0.14, cont);
        let shore_shelf = smoothstep(-0.20, -0.10, cont);

        let (_best_biome, _best_hash, _best_temp, cell_weights) = self.sample_macro_cells(wx, wz);

        let mut verticality_sum = 0.0;
        let mut base_lift_sum = 0.0;
        let mut mountain_w = 0.0;
        let mut badlands_w = 0.0;
        let mut red_desert_w = 0.0;
        let mut desert_w = 0.0;
        let mut savanna_w = 0.0;
        let mut jungle_w = 0.0;
        let mut swamp_w = 0.0;

        for &(b, w) in &cell_weights {
            if w <= 0.0 {
                continue;
            }
            let (v, lift) = match b {
                Biome::Plains | Biome::FlowerMeadow | Biome::RedDesert => (0.07, 0.8),
                Biome::Swamp => (0.06, -1.2),
                Biome::Glacier | Biome::SnowyTundra => (0.10, 0.9),
                Biome::Desert => (0.12, 1.0),
                Biome::Savanna => (0.18, 2.0),
                Biome::Forest | Biome::BirchForest | Biome::DarkForest | Biome::Taiga => (0.34, 2.2),
                Biome::Jungle => (0.76, 4.5),
                Biome::Badlands => (0.85, 6.0),
                Biome::Mountains => (1.00, 12.0),
                _ => (0.20, 1.0),
            };
            verticality_sum += v * w;
            base_lift_sum += lift * w;
            match b {
                Biome::Mountains => mountain_w += w,
                Biome::Badlands => badlands_w += w,
                Biome::RedDesert => red_desert_w += w,
                Biome::Desert => desert_w += w,
                Biome::Savanna => savanna_w += w,
                Biome::Jungle => jungle_w += w,
                Biome::Swamp => swamp_w += w,
                _ => {}
            }
        }

        let verticality = verticality_sum.clamp(0.06, 1.0);

        // 1. Continental curve: oceans plunge smoothly, coastal shelf separates shore, inland rises per biome
        let base_elev = if cont < -0.20 {
            let ocean_t = ((-0.20 - cont) / 0.45).clamp(0.0, 1.35);
            (self.water_level as f64 + 0.4) - ocean_t * 17.0
        } else {
            let land_t = ((cont + 0.20) / 0.85).clamp(0.0, 1.35);
            let slope_rise = 2.6 + 11.5 * verticality;
            (self.water_level as f64 + 0.6)
                + shore_shelf * 2.5
                + base_lift_sum * inland_mask
                + land_t.powf(1.0 + 0.15 * verticality) * slope_rise
                + self.seed_altitude_bias * (0.25 + 0.75 * verticality) * inland_mask
        };

        // 2. Alpine mountain peaks inside Mountains cells and gentle-to-rolling hills scaled by verticality
        let mountain_peaks = (10.0 + smoothstep(0.15, 0.88, ridge) * 44.0)
            * self.seed_relief_scale
            * inland_mask
            * mountain_w;
        let rolling_hills = smoothstep(0.32, 0.92, ridge) * 11.0 * inland_mask * verticality.powf(1.2);

        let mut height = base_elev + mountain_peaks + rolling_hills;

        // 3. Biome-specific silhouettes
        if badlands_w > 0.01 {
            let mesa_signal = (ridge * 0.65 + (cont + 0.2) * 0.45).clamp(0.0, 1.2);
            let raw_mesa = smoothstep(0.20, 0.68, mesa_signal) * 28.0;
            let terrace_step = 4.0;
            let snapped = (raw_mesa / terrace_step).floor() * terrace_step;
            let frac = (raw_mesa - snapped) / terrace_step;
            let smooth_terrace = snapped + terrace_step * smoothstep(0.72, 0.96, frac);
            height += smooth_terrace * badlands_w * inland_mask;
        }

        if red_desert_w > 0.01 {
            let ripple = ((fx * 0.028 + fz * 0.016 + detail * 0.9).sin().abs()) * 1.6;
            height += ripple * red_desert_w * inland_mask;
        }

        if desert_w > 0.01 {
            let dune = ((fx * 0.036 + fz * 0.020 + detail * 1.4).sin().abs()) * 3.4;
            height += dune * desert_w * inland_mask;
        }

        if savanna_w > 0.01 {
            let plateau = smoothstep(0.45, 0.68, ridge) * 11.0;
            height += plateau * savanna_w * inland_mask;
        }

        if jungle_w > 0.01 {
            height += (3.5 + detail * 6.0) * jungle_w * inland_mask;
        }

        if swamp_w > 0.01 {
            let basin_target = (self.water_level as f64) + 1.2 + detail * 1.1;
            height += (basin_target - height) * (swamp_w * shore_shelf * 0.88);
        }

        // Crisp low-amplitude surface texture in flat biomes; craggy peaks in mountains & badlands;
        // smoothly zeroed at shoreline cont = -0.20 so ocean puddles / micro-islands don't dot coasts.
        let detail_amp = if cont < -0.20 {
            smoothstep(-0.20, -0.27, cont) * 2.4
        } else {
            (0.45 + 1.55 * verticality + 3.6 * mountain_w) * shore_shelf
        };
        height += detail * detail_amp;

        // 4. Winding river channels cutting gently through land (and deeper gorges through hills)
        let river_val = self.sample_river_val(wx, wz);
        const RIVER_WIDTH: f64 = 0.036;
        if height > (self.water_level - 2) as f64 && river_val < RIVER_WIDTH {
            let t = (river_val / RIVER_WIDTH).powi(2);
            let river_bed = (self.water_level - 2) as f64;
            height = river_bed + (height - river_bed) * t;
        }

        let max_surface = (CHUNK_SIZE_Y as i32 * VERTICAL_CHUNKS) - 12;
        (height.round() as i32).clamp(2, max_surface)
    }

    /// Determines the biome at global coordinate (x, z).
    #[inline]
    pub fn get_biome(&self, x: i32, z: i32, height: i32) -> Biome {
        let (wx, wz) = self.sample_warp(x, z);
        let (cell_biome, cell_hash, cell_site_temp, _) = self.sample_macro_cells(wx, wz);

        let cont = self
            .continental_noise
            .get([wx + self.cont_offset[0], wz + self.cont_offset[1]]);

        // River channels: narrow corridors explicitly exempt from macro biome size limits (land/coast only)
        if cont >= -0.20 {
            let river_val = self.sample_river_val(wx, wz);
            if river_val < 0.036 && height <= self.water_level + 1 {
                return Biome::River;
            }
        }

        // Inland Swamp marsh pools retain Swamp biome so lily pads and murky water generate naturally
        if cell_biome == Biome::Swamp && cont >= -0.14 && height <= self.water_level + 6 {
            return Biome::Swamp;
        }

        if height <= self.water_level {
            if cell_site_temp > 0.10 || (cell_site_temp > -0.02 && (cell_hash >> 40) % 3 == 0) {
                return Biome::CoralReef;
            }
            return Biome::Ocean;
        }

        // Coastal shore transitions along ocean borders (width & rockiness governed by macro cell)
        if cont < -0.14 && height <= self.water_level + 5 {
            return match cell_biome {
                Biome::Glacier => Biome::Glacier,
                Biome::SnowyTundra | Biome::Mountains | Biome::Taiga | Biome::DarkForest => {
                    Biome::StonyShore
                }
                _ => {
                    if (cell_hash >> 44) % 5 == 0 {
                        Biome::StonyShore
                    } else {
                        Biome::Beach
                    }
                }
            };
        }

        cell_biome
    }

    /// Searches deterministically outward from (0, 0) for a safe, dry-land spawn point
    /// never inside a River, Ocean, CoralReef, or water basin, and avoiding sheer cliff edges.
    /// Returns `(spawn_x, surface_y, spawn_z)`.
    pub fn find_spawn_coords(&self) -> (i32, i32, i32) {
        let is_good_land = |x: i32, z: i32, max_slope: i32| -> Option<i32> {
            let h = self.surface_height(x, z);
            if h <= self.water_level + 1 || h >= self.water_level + 36 {
                return None;
            }
            let biome = self.get_biome(x, z, h);
            if matches!(biome, Biome::River | Biome::Ocean | Biome::CoralReef) {
                return None;
            }
            // Check local 5x5 neighborhood for dry terrain and tolerable slope
            for dz in -2..=2 {
                for dx in -2..=2 {
                    let nh = self.surface_height(x + dx, z + dz);
                    if nh <= self.water_level {
                        return None;
                    }
                    if (nh - h).abs() > max_slope {
                        return None;
                    }
                }
            }
            Some(h)
        };

        // Pass 1: Look for comfortable dry land with gentle slope (max_slope <= 3)
        let max_radius: i32 = 640;
        let step_i: i32 = 4;
        let step_u: usize = step_i as usize;
        for r in (0..=max_radius).step_by(step_u) {
            if r == 0 {
                if let Some(h) = is_good_land(0, 0, 3) {
                    return (0, h, 0);
                }
                continue;
            }
            for x in (-r..=r).step_by(step_u) {
                for &z in &[-r, r] {
                    if let Some(h) = is_good_land(x, z, 3) {
                        return (x, h, z);
                    }
                }
            }
            for z in ((-r + step_i)..=(r - step_i)).step_by(step_u) {
                for &x in &[-r, r] {
                    if let Some(h) = is_good_land(x, z, 3) {
                        return (x, h, z);
                    }
                }
            }
        }

        // Pass 2: Relaxed slope fallback up to 1024 blocks so spawn is still guaranteed dry land
        for r in (0_i32..=1024).step_by(8) {
            for z in (-r..=r).step_by(8) {
                for x in (-r..=r).step_by(8) {
                    if z.abs() != r && x.abs() != r {
                        continue;
                    }
                    let h = self.surface_height(x, z);
                    let biome = self.get_biome(x, z, h);
                    if h > self.water_level + 1
                        && !matches!(biome, Biome::River | Biome::Ocean | Biome::CoralReef)
                    {
                        return (x, h, z);
                    }
                }
            }
        }

        (0, self.surface_height(0, 0), 0)
    }

    /// Checks if a 3D coordinate is carved out as a cavern or winding tunnel cave.
    #[inline]
    pub fn is_cave(&self, x: i32, y: i32, z: i32) -> bool {
        if y <= 1 {
            return false;
        }

        let fx = x as f64 + self.detail_offset[0] * 0.1;
        let fy = y as f64;
        let fz = z as f64 + self.detail_offset[1] * 0.1;

        let density = self.cave_noise.get([fx, fy, fz]);
        if density > 0.44 {
            return true;
        }

        // Winding spaghetti tunnels between depth 4 and surface cliffs
        let ta = self.tunnel_noise_a.get([fx, fy * 1.15, fz]);
        let tb = self.tunnel_noise_b.get([fx, fy * 1.15, fz]);
        ta.abs() < 0.062 && tb.abs() < 0.062
    }

    /// Generates the base terrain for a specific chunk (without cross-chunk multi-block structures).
    pub fn generate_chunk(&self, pos: ChunkPos) -> Chunk {
        let mut chunk = Chunk::new(pos);
        let origin = pos.to_world_origin();

        // Coordinates >= ARENA_BAND_START_X are reserved for instanced boss arenas (pure void).
        if origin.x >= blockgame_core::temple::ARENA_BAND_START_X {
            return chunk;
        }

        // Precompute deterministic region-seeded cave carvers for this chunk (ADR 0009)
        let cave_mask = crate::caves::compute_chunk_cave_mask(self, origin.x, origin.y, origin.z);

        // Precompute surface heights across the chunk plus a 1-block border for steepness/cliff painting
        let mut heights = [[0i32; 18]; 18];
        for dz in 0..18usize {
            let wz = origin.z + dz as i32 - 1;
            for dx in 0..18usize {
                let wx = origin.x + dx as i32 - 1;
                heights[dz][dx] = self.surface_height(wx, wz);
            }
        }

        for lz in 0..CHUNK_SIZE_Z {
            let wz = origin.z + lz as i32;
            for lx in 0..CHUNK_SIZE_X {
                let wx = origin.x + lx as i32;

                let h_idx_x = lx + 1;
                let h_idx_z = lz + 1;
                let surface_y = heights[h_idx_z][h_idx_x];
                let max_neighbor_drop = (surface_y - heights[h_idx_z][h_idx_x - 1])
                    .max(surface_y - heights[h_idx_z][h_idx_x + 1])
                    .max(surface_y - heights[h_idx_z - 1][h_idx_x])
                    .max(surface_y - heights[h_idx_z + 1][h_idx_x]);
                let is_steep_cliff = max_neighbor_drop >= 3;

                let biome = self.get_biome(wx, wz, surface_y);
                let is_underwater = surface_y <= self.water_level;
                let is_wet_surface = is_underwater
                    || surface_y <= self.water_level + 2
                    || matches!(
                        biome,
                        Biome::Ocean | Biome::CoralReef | Biome::River | Biome::Swamp
                    );
                let deepstone_ceil = crate::caves::deepstone_ceiling_y(self.seed, wx, wz);
                let lava_level = crate::caves::region_lava_level(self.seed, wx, wz);

                for ly in 0..CHUNK_SIZE_Y {
                    let wy = origin.y + ly as i32;

                    let block = if wy == 0 {
                        BlockId::BEDROCK
                    } else if wy < surface_y {
                        let cell = cave_mask[Chunk::index(lx, ly, lz)];
                        // Only rare designated dry-land Cave Mouths may breach the surface;
                        // underwater beds and wet biomes always keep a 4-block rock seal.
                        let cave_carved = if !cell.carved {
                            false
                        } else if is_wet_surface {
                            wy < surface_y - 4
                        } else if cell.allows_surface_breach {
                            true
                        } else {
                            wy < surface_y - 4
                        };

                        if cave_carved {
                            if lava_level > 0 && wy >= 1 && wy <= lava_level {
                                BlockId::LAVA
                            } else if cell.pool_water_y >= 12
                                && wy >= 12
                                && wy <= cell.pool_water_y.min(34)
                            {
                                BlockId::WATER
                            } else {
                                BlockId::AIR
                            }
                        } else if wy == surface_y - 1 {
                            if is_underwater || surface_y <= self.water_level + 1 {
                                if biome == Biome::Ocean || biome == Biome::CoralReef {
                                    let gh = (wx.wrapping_mul(17) ^ wz.wrapping_mul(31))
                                        .unsigned_abs()
                                        % 3;
                                    if gh == 0 {
                                        BlockId::GRAVEL
                                    } else {
                                        BlockId::SAND
                                    }
                                } else if biome == Biome::StonyShore {
                                    let sh = (wx.wrapping_mul(13) ^ wz.wrapping_mul(23))
                                        .unsigned_abs()
                                        % 4;
                                    if sh == 0 {
                                        BlockId::COBBLESTONE
                                    } else if sh == 1 {
                                        BlockId::STONE
                                    } else {
                                        BlockId::GRAVEL
                                    }
                                } else if biome == Biome::Glacier {
                                    if is_underwater {
                                        BlockId::GRAVEL
                                    } else {
                                        BlockId::ICE
                                    }
                                } else if biome == Biome::RedDesert {
                                    BlockId::RED_SAND
                                } else if biome == Biome::River {
                                    let rh = (wx.wrapping_mul(19) ^ wz.wrapping_mul(29))
                                        .unsigned_abs()
                                        % 5;
                                    if rh == 0 {
                                        BlockId::CLAY
                                    } else if rh <= 2 {
                                        BlockId::GRAVEL
                                    } else {
                                        BlockId::SAND
                                    }
                                } else {
                                    BlockId::SAND
                                }
                            } else if is_steep_cliff {
                                // Steep vertical cliffs expose natural rock, terracotta, ice, or sandstone strata
                                match biome {
                                    Biome::Badlands => {
                                        if (surface_y / 2) % 2 == 0 {
                                            BlockId::TERRACOTTA
                                        } else {
                                            BlockId::RED_SAND
                                        }
                                    }
                                    Biome::RedDesert => BlockId::TERRACOTTA,
                                    Biome::Desert | Biome::Beach => BlockId::SANDSTONE,
                                    Biome::Glacier => BlockId::ICE,
                                    _ => BlockId::STONE,
                                }
                            } else {
                                match biome {
                                    Biome::Desert | Biome::Beach => BlockId::SAND,
                                    Biome::RedDesert => BlockId::RED_SAND,
                                    Biome::Badlands => {
                                        if (surface_y / 2) % 2 == 0 {
                                            BlockId::RED_SAND
                                        } else {
                                            BlockId::TERRACOTTA
                                        }
                                    }
                                    Biome::StonyShore => {
                                        let sh = (wx.wrapping_mul(13) ^ wz.wrapping_mul(23))
                                            .unsigned_abs()
                                            % 4;
                                        if sh == 0 {
                                            BlockId::COBBLESTONE
                                        } else if sh == 1 {
                                            BlockId::STONE
                                        } else {
                                            BlockId::GRAVEL
                                        }
                                    }
                                    Biome::Glacier => {
                                        let gh = (wx.wrapping_mul(17) ^ wz.wrapping_mul(41))
                                            .unsigned_abs()
                                            % 10;
                                        if gh < 7 {
                                            BlockId::ICE
                                        } else {
                                            BlockId::SNOW_BLOCK
                                        }
                                    }
                                    Biome::SnowyTundra => BlockId::SNOW_BLOCK,
                                    Biome::Taiga => {
                                        let ph = (wx.wrapping_mul(23) ^ wz.wrapping_mul(37))
                                            .unsigned_abs()
                                            % 5;
                                        if ph <= 2 {
                                            BlockId::PODZOL
                                        } else {
                                            BlockId::GRASS
                                        }
                                    }
                                    Biome::River => {
                                        let rh = (wx.wrapping_mul(19) ^ wz.wrapping_mul(29))
                                            .unsigned_abs()
                                            % 4;
                                        if rh == 0 {
                                            BlockId::CLAY
                                        } else if rh == 1 {
                                            BlockId::GRAVEL
                                        } else {
                                            BlockId::SAND
                                        }
                                    }
                                    Biome::Mountains if surface_y >= 78 => BlockId::SNOW_BLOCK,
                                    Biome::Mountains if surface_y >= 65 => {
                                        if (wx.wrapping_mul(11) ^ wz.wrapping_mul(29))
                                            .unsigned_abs()
                                            % 5
                                            == 0
                                        {
                                            BlockId::GRAVEL
                                        } else {
                                            BlockId::STONE
                                        }
                                    }
                                    Biome::Mountains => BlockId::GRASS,
                                    Biome::Swamp
                                        if surface_y <= self.water_level + 2
                                            && (wx.wrapping_add(wz).unsigned_abs() % 3 == 0) =>
                                    {
                                        BlockId::CLAY
                                    }
                                    _ => BlockId::GRASS,
                                }
                            }
                        } else if wy >= surface_y - 4 {
                            if is_underwater || surface_y <= self.water_level + 1 {
                                if biome == Biome::Ocean || biome == Biome::CoralReef {
                                    let gh = (wx.wrapping_mul(17) ^ wz.wrapping_mul(31))
                                        .unsigned_abs()
                                        % 3;
                                    if gh == 0 {
                                        BlockId::GRAVEL
                                    } else {
                                        BlockId::SAND
                                    }
                                } else if biome == Biome::StonyShore {
                                    BlockId::GRAVEL
                                } else if biome == Biome::Glacier {
                                    if wy < surface_y - 2 {
                                        BlockId::STONE
                                    } else {
                                        BlockId::ICE
                                    }
                                } else if biome == Biome::RedDesert {
                                    if wy < surface_y - 2 {
                                        BlockId::SANDSTONE
                                    } else {
                                        BlockId::RED_SAND
                                    }
                                } else if biome == Biome::River {
                                    let rh = (wx.wrapping_mul(19) ^ wz.wrapping_mul(29))
                                        .unsigned_abs()
                                        % 3;
                                    if rh == 0 {
                                        BlockId::GRAVEL
                                    } else {
                                        BlockId::DIRT
                                    }
                                } else {
                                    BlockId::SAND
                                }
                            } else if is_steep_cliff
                                && !matches!(
                                    biome,
                                    Biome::Badlands
                                        | Biome::RedDesert
                                        | Biome::Desert
                                        | Biome::Beach
                                        | Biome::Glacier
                                )
                            {
                                BlockId::STONE
                            } else {
                                match biome {
                                    Biome::Desert | Biome::Beach => {
                                        if wy < surface_y - 2 {
                                            BlockId::SANDSTONE
                                        } else {
                                            BlockId::SAND
                                        }
                                    }
                                    Biome::RedDesert => {
                                        if wy < surface_y - 2 {
                                            BlockId::TERRACOTTA
                                        } else {
                                            BlockId::RED_SAND
                                        }
                                    }
                                    Biome::Glacier => {
                                        if wy < surface_y - 2 {
                                            BlockId::STONE
                                        } else {
                                            BlockId::ICE
                                        }
                                    }
                                    Biome::StonyShore => {
                                        if wy < surface_y - 2 {
                                            BlockId::STONE
                                        } else {
                                            BlockId::GRAVEL
                                        }
                                    }
                                    Biome::Badlands => {
                                        let layer = (wy / 2) % 3;
                                        if layer == 0 || layer == 2 {
                                            BlockId::TERRACOTTA
                                        } else {
                                            BlockId::RED_SAND
                                        }
                                    }
                                    Biome::Mountains if surface_y >= 65 => BlockId::STONE,
                                    Biome::Swamp
                                        if surface_y <= self.water_level + 2
                                            && (wx.wrapping_add(wz).unsigned_abs() % 3 == 0) =>
                                    {
                                        BlockId::CLAY
                                    }
                                    _ => BlockId::DIRT,
                                }
                            }
                        } else if wy < deepstone_ceil {
                            BlockId::DEEPSTONE
                        } else if (wx.wrapping_mul(13) ^ wy.wrapping_mul(17) ^ wz.wrapping_mul(23))
                            % 23
                            == 0
                        {
                            BlockId::COBBLESTONE
                        } else {
                            BlockId::STONE
                        }
                    } else if wy <= self.water_level {
                        if matches!(biome, Biome::SnowyTundra | Biome::Glacier)
                            && wy == self.water_level
                        {
                            BlockId::ICE
                        } else {
                            BlockId::WATER
                        }
                    } else {
                        BlockId::AIR
                    };

                    chunk.set(lx, ly, lz, block);
                }
            }
        }

        // Stamp depth-curved organic Ore Veins into Stone/Deepstone/Cobblestone
        crate::caves::stamp_chunk_ore_veins(self.seed, &mut chunk);

        // Decorate carved caves with zone materials, Dripstone, Glow Crystals, and rare springs
        crate::caves::decorate_chunk_caves(self.seed, &mut chunk, &cave_mask);

        for temple in self.find_temples_for_chunk(pos) {
            crate::temple_gen::stamp_temple_exterior(&mut chunk, &temple);
        }
        for village in self.find_villages_for_chunk(pos) {
            crate::village_gen::stamp_village(&mut chunk, &village);
        }

        chunk
    }

    /// Returns the winning Voronoi macro biome cell ID at world coordinates `(wx, wz)`.
    #[inline]
    pub fn macro_biome_cell_id(&self, wx: i32, wz: i32) -> u64 {
        self.sample_macro_cells(wx as f64, wz as f64).1
    }

    /// Evaluates the base underground rock stratum (Deepstone below the jagged transition, else Stone/Cobblestone).
    #[inline]
    pub fn get_subsurface_block(&self, wx: i32, wy: i32, wz: i32) -> BlockId {
        if wy < crate::caves::deepstone_ceiling_y(self.seed, wx, wz) {
            BlockId::DEEPSTONE
        } else if (wx.wrapping_mul(13) ^ wy.wrapping_mul(17) ^ wz.wrapping_mul(23)) % 23 == 0 {
            BlockId::COBBLESTONE
        } else {
            BlockId::STONE
        }
    }

    /// Populates foliage, trees, boulders, coral reefs, flowers, and cacti on chunks.
    pub fn populate_chunk_features(&self, chunk: &mut Chunk) {
        let origin = chunk.pos.to_world_origin();
        if origin.x >= blockgame_core::temple::ARENA_BAND_START_X {
            return;
        }

        for lz in 0..CHUNK_SIZE_Z {
            let wz = origin.z + lz as i32;
            for lx in 0..CHUNK_SIZE_X {
                let wx = origin.x + lx as i32;

                let surface_y = self.surface_height(wx, wz);
                let biome = self.get_biome(wx, wz, surface_y);
                if surface_y <= self.water_level {
                    // Check for lily pads in Swamp on top of still water
                    if biome == Biome::Swamp {
                        let local_water_top = (self.water_level + 1) - origin.y;
                        if local_water_top >= 1 && local_water_top < CHUNK_SIZE_Y as i32 {
                            let ly = local_water_top as usize;
                            if chunk.get(lx, ly - 1, lz) == Some(BlockId::WATER)
                                && chunk.get(lx, ly, lz) == Some(BlockId::AIR)
                            {
                                let lh = tree_hash(wx, wz, self.seed.wrapping_add(999));
                                if lh % 10 == 0 {
                                    chunk.set(lx, ly, lz, BlockId::LILY_PAD);
                                }
                            }
                        }
                    }

                    // Underwater features: Coral reefs, Sea Lanterns, and Seagrass
                    let local_surface_y = surface_y - origin.y;
                    if local_surface_y >= 1 && local_surface_y < (CHUNK_SIZE_Y as i32 - 1) {
                        let ly = local_surface_y as usize;
                        let seabed = chunk.get(lx, ly - 1, lz);
                        if (seabed == Some(BlockId::SAND) || seabed == Some(BlockId::GRAVEL))
                            && chunk.get(lx, ly, lz) == Some(BlockId::WATER)
                        {
                            let water_depth = self.water_level - surface_y;
                            let ch = tree_hash(wx, wz, self.seed.wrapping_add(1234));
                            if biome == Biome::CoralReef && water_depth >= 2 {
                                if ch % 11 == 0 {
                                    let col_height = (2 + ((ch >> 4) % 4) as usize)
                                        .min(water_depth.saturating_sub(1) as usize);
                                    let coral_type = if (ch >> 8) % 2 == 0 {
                                        BlockId::CORAL_BLOCK
                                    } else {
                                        BlockId::TUBE_CORAL_BLOCK
                                    };
                                    for cy in ly..(ly + col_height).min(CHUNK_SIZE_Y) {
                                        chunk.set(lx, cy, lz, coral_type);
                                    }
                                    if (ch >> 12) % 3 == 0 && col_height >= 2 {
                                        let lantern_y =
                                            (ly + col_height - 1).min(CHUNK_SIZE_Y - 1);
                                        chunk.set(lx, lantern_y, lz, BlockId::SEA_LANTERN);
                                    }
                                } else if ch % 23 == 0 {
                                    chunk.set(lx, ly, lz, BlockId::SEA_LANTERN);
                                } else if ch % 3 == 0 {
                                    chunk.set(lx, ly, lz, BlockId::SEAGRASS);
                                }
                            } else if (biome == Biome::Ocean || biome == Biome::River)
                                && ch % 7 == 0
                            {
                                chunk.set(lx, ly, lz, BlockId::SEAGRASS);
                            }
                        }
                    }
                    continue;
                }

                let local_surface_y = surface_y - origin.y;

                // Tall grass, flowers, pumpkins, and melons on surface grass and podzol
                if local_surface_y >= 1 && local_surface_y < CHUNK_SIZE_Y as i32 {
                    let ly = local_surface_y as usize;
                    let under = chunk.get(lx, ly - 1, lz);
                    if (under == Some(BlockId::GRASS) || under == Some(BlockId::PODZOL))
                        && chunk.get(lx, ly, lz) == Some(BlockId::AIR)
                    {
                        let grass_chance = match biome {
                            Biome::Plains => 20,
                            Biome::FlowerMeadow => 12,
                            Biome::Forest => 14,
                            Biome::BirchForest => 18,
                            Biome::Savanna => 26,
                            Biome::Swamp => 16,
                            Biome::Jungle => 36,
                            Biome::Taiga => 18,
                            Biome::DarkForest => 15,
                            Biome::Mountains => 8,
                            _ => 0,
                        };
                        if grass_chance > 0
                            && (tree_hash(wx, wz, self.seed.wrapping_add(777)) % 100) < grass_chance
                        {
                            chunk.set(lx, ly, lz, BlockId::TALL_GRASS);
                        } else {
                            let fh = tree_hash(wx, wz, self.seed.wrapping_add(919));
                            let flower_pct = match biome {
                                Biome::FlowerMeadow => 38,
                                Biome::Plains => 6,
                                Biome::BirchForest => 8,
                                Biome::Forest => 4,
                                Biome::Swamp => 3,
                                Biome::DarkForest => 2,
                                Biome::Taiga | Biome::Savanna => 2,
                                Biome::Mountains => 2,
                                _ => 0,
                            };
                            if flower_pct > 0 && (fh % 100) < flower_pct {
                                let flower = match biome {
                                    Biome::Swamp => BlockId::BLUE_ORCHID,
                                    Biome::BirchForest => match (fh >> 8) % 3 {
                                        0 => BlockId::OXEYE_DAISY,
                                        1 => BlockId::DANDELION,
                                        _ => BlockId::ALLIUM,
                                    },
                                    Biome::FlowerMeadow => {
                                        // Natural color-grouped flower patches across sunny meadows
                                        let patch = tree_hash(
                                            wx >> 3,
                                            wz >> 3,
                                            self.seed.wrapping_add(555),
                                        );
                                        let pick = if (fh >> 12) % 3 == 0 {
                                            (fh >> 8) % 6
                                        } else {
                                            patch % 6
                                        };
                                        match pick {
                                            0 => BlockId::ALLIUM,
                                            1 => BlockId::ORANGE_TULIP,
                                            2 => BlockId::PINK_TULIP,
                                            3 => BlockId::OXEYE_DAISY,
                                            4 => BlockId::POPPY,
                                            _ => BlockId::DANDELION,
                                        }
                                    }
                                    Biome::Taiga | Biome::Savanna | Biome::Mountains => {
                                        if (fh >> 8) % 2 == 0 {
                                            BlockId::DANDELION
                                        } else {
                                            BlockId::POPPY
                                        }
                                    }
                                    _ => match (fh >> 8) % 6 {
                                        0 => BlockId::POPPY,
                                        1 => BlockId::DANDELION,
                                        2 => BlockId::ALLIUM,
                                        3 => BlockId::ORANGE_TULIP,
                                        4 => BlockId::PINK_TULIP,
                                        _ => BlockId::OXEYE_DAISY,
                                    },
                                };
                                chunk.set(lx, ly, lz, flower);
                            } else if matches!(
                                biome,
                                Biome::Plains
                                    | Biome::FlowerMeadow
                                    | Biome::Forest
                                    | Biome::BirchForest
                                    | Biome::Taiga
                            ) && fh % 240 == 0
                            {
                                chunk.set(lx, ly, lz, BlockId::PUMPKIN);
                            } else if matches!(biome, Biome::Jungle | Biome::Swamp)
                                && fh % 200 == 0
                            {
                                chunk.set(lx, ly, lz, BlockId::MELON);
                            } else if matches!(
                                biome,
                                Biome::Plains | Biome::FlowerMeadow | Biome::Savanna
                            ) && fh % 210 == 17
                            {
                                chunk.set(lx, ly - 1, lz, BlockId::FARMLAND);
                                chunk.set(lx, ly, lz, BlockId::CARROTS_3);
                            } else if matches!(
                                biome,
                                Biome::Forest | Biome::BirchForest | Biome::Taiga | Biome::Plains
                            ) && fh % 220 == 31
                            {
                                chunk.set(lx, ly - 1, lz, BlockId::FARMLAND);
                                chunk.set(lx, ly, lz, BlockId::POTATOES_3);
                            } else if matches!(
                                biome,
                                Biome::FlowerMeadow | Biome::Jungle | Biome::Plains
                            ) && fh % 230 == 47
                            {
                                chunk.set(lx, ly - 1, lz, BlockId::FARMLAND);
                                chunk.set(lx, ly, lz, BlockId::TOMATOES_3);
                            }
                        }
                    }
                }

                // Water-adjacent wild Sugar Cane (1–3 blocks tall on sand/dirt/grass)
                if local_surface_y >= 1
                    && local_surface_y < (CHUNK_SIZE_Y as i32 - 4)
                    && lx >= 1
                    && lx < (CHUNK_SIZE_X - 1)
                    && lz >= 1
                    && lz < (CHUNK_SIZE_Z - 1)
                {
                    let ly = local_surface_y as usize;
                    let under = chunk.get(lx, ly - 1, lz);
                    if matches!(
                        under,
                        Some(BlockId::SAND)
                            | Some(BlockId::RED_SAND)
                            | Some(BlockId::DIRT)
                            | Some(BlockId::GRASS)
                    ) && chunk.get(lx, ly, lz) == Some(BlockId::AIR)
                    {
                        let near_water = [
                            chunk.get(lx + 1, ly - 1, lz),
                            chunk.get(lx - 1, ly - 1, lz),
                            chunk.get(lx, ly - 1, lz + 1),
                            chunk.get(lx, ly - 1, lz - 1),
                        ]
                        .iter()
                        .any(|b| b.map(|id| id.is_water()).unwrap_or(false));
                        if near_water {
                            let sh = tree_hash(wx, wz, self.seed.wrapping_add(777));
                            if sh % 14 == 0 {
                                let cane_h = 1 + ((sh >> 4) % 3) as usize;
                                for cy in ly..(ly + cane_h).min(CHUNK_SIZE_Y) {
                                    if chunk.get(lx, cy, lz) == Some(BlockId::AIR) {
                                        chunk.set(lx, cy, lz, BlockId::SUGAR_CANE);
                                    }
                                }
                            }
                        }
                    }
                }

                // Desert & RedDesert Cacti on sand / red sand
                if matches!(biome, Biome::Desert | Biome::RedDesert)
                    && local_surface_y >= 1
                    && local_surface_y < (CHUNK_SIZE_Y as i32 - 4)
                {
                    let ly = local_surface_y as usize;
                    let under = chunk.get(lx, ly - 1, lz);
                    if (under == Some(BlockId::SAND) || under == Some(BlockId::RED_SAND))
                        && chunk.get(lx, ly, lz) == Some(BlockId::AIR)
                    {
                        let ch = tree_hash(wx, wz, self.seed.wrapping_add(888));
                        let mod_freq = if biome == Biome::RedDesert { 58 } else { 42 };
                        if ch % mod_freq == 0 {
                            let cactus_height = 1 + ((ch >> 4) % 4) as usize;
                            for cy in ly..(ly + cactus_height).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, cy, lz, BlockId::CACTUS);
                            }
                        }
                    }
                }

                // Trees, spires & boulders: keep 2-block margin from chunk horizontal edges
                if lx < 2 || lx >= (CHUNK_SIZE_X - 2) || lz < 2 || lz >= (CHUNK_SIZE_Z - 2) {
                    continue;
                }

                if local_surface_y < 1 || local_surface_y >= (CHUNK_SIZE_Y as i32 - 7) {
                    continue;
                }

                let ly = local_surface_y as usize;
                let ground = chunk.get(lx, ly - 1, lz);
                let valid_ground = match biome {
                    Biome::SnowyTundra => {
                        ground == Some(BlockId::SNOW_BLOCK) || ground == Some(BlockId::GRASS)
                    }
                    Biome::Glacier => {
                        ground == Some(BlockId::ICE) || ground == Some(BlockId::SNOW_BLOCK)
                    }
                    Biome::StonyShore => {
                        ground == Some(BlockId::GRAVEL)
                            || ground == Some(BlockId::COBBLESTONE)
                            || ground == Some(BlockId::STONE)
                    }
                    Biome::Swamp => ground == Some(BlockId::GRASS) || ground == Some(BlockId::CLAY),
                    Biome::Taiga => ground == Some(BlockId::PODZOL) || ground == Some(BlockId::GRASS),
                    Biome::DarkForest => {
                        ground == Some(BlockId::GRASS)
                            || ground == Some(BlockId::DIRT)
                            || ground == Some(BlockId::PODZOL)
                    }
                    Biome::Mountains => {
                        surface_y <= 70
                            && (ground == Some(BlockId::GRASS) || ground == Some(BlockId::STONE))
                    }
                    _ => ground == Some(BlockId::GRASS),
                };
                if !valid_ground {
                    continue;
                }

                let th = tree_hash(wx, wz, self.seed);

                match biome {
                    Biome::Forest => {
                        if (th >> 12) % 90 == 0 {
                            // Forest mossy boulder landmark
                            chunk.set(lx, ly, lz, BlockId::MOSSY_COBBLESTONE);
                            if ly + 1 < CHUNK_SIZE_Y {
                                chunk.set(lx, ly + 1, lz, BlockId::MOSSY_COBBLESTONE);
                            }
                        } else if th % 20 == 0 {
                            let is_birch = (th >> 8) % 2 == 0;
                            if is_birch {
                                let trunk_h = 5 + ((th >> 4) % 3) as usize;
                                for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                    chunk.set(lx, ty, lz, BlockId::BIRCH_LOG);
                                }
                                let crown_start = ly + trunk_h.saturating_sub(2);
                                let crown_end = (ly + trunk_h + 1).min(CHUNK_SIZE_Y - 1);
                                for cy in crown_start..=crown_end {
                                    let radius: i32 = if cy >= ly + trunk_h { 1 } else { 2 };
                                    for dz in -radius..=radius {
                                        for dx in -radius..=radius {
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0
                                                && cx < CHUNK_SIZE_X as i32
                                                && cz >= 0
                                                && cz < CHUNK_SIZE_Z as i32
                                            {
                                                if dx.abs() == 2
                                                    && dz.abs() == 2
                                                    && (dx + dz) % 2 == 0
                                                {
                                                    continue;
                                                }
                                                if chunk.get(cx as usize, cy, cz as usize)
                                                    == Some(BlockId::AIR)
                                                {
                                                    chunk.set(
                                                        cx as usize,
                                                        cy,
                                                        cz as usize,
                                                        BlockId::BIRCH_LEAVES,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                let trunk_h = 4 + ((th >> 4) % 3) as usize;
                                for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                    chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                                }
                                let crown_start = ly + trunk_h.saturating_sub(2);
                                let crown_end = (ly + trunk_h + 1).min(CHUNK_SIZE_Y - 1);
                                for cy in crown_start..=crown_end {
                                    let radius: i32 = if cy >= ly + trunk_h { 1 } else { 2 };
                                    for dz in -radius..=radius {
                                        for dx in -radius..=radius {
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0
                                                && cx < CHUNK_SIZE_X as i32
                                                && cz >= 0
                                                && cz < CHUNK_SIZE_Z as i32
                                            {
                                                if dx.abs() == 2
                                                    && dz.abs() == 2
                                                    && (dx + dz) % 2 == 0
                                                {
                                                    continue;
                                                }
                                                if chunk.get(cx as usize, cy, cz as usize)
                                                    == Some(BlockId::AIR)
                                                {
                                                    chunk.set(
                                                        cx as usize,
                                                        cy,
                                                        cz as usize,
                                                        BlockId::LEAVES,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Plains => {
                        if th % 75 == 0 {
                            let trunk_h = 4 + ((th >> 4) % 2) as usize;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                            }
                            let crown_start = ly + 2;
                            let crown_end = (ly + trunk_h + 1).min(CHUNK_SIZE_Y - 1);
                            for cy in crown_start..=crown_end {
                                let radius: i32 = if cy >= ly + trunk_h { 1 } else { 2 };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if dx.abs() == 2
                                                && dz.abs() == 2
                                                && (dx + dz) % 2 == 0
                                            {
                                                continue;
                                            }
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Savanna => {
                        if th % 40 == 0 {
                            let trunk_h = 5 + ((th >> 4) % 2) as usize;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::ACACIA_LOG);
                            }
                            let top_y = ly + trunk_h;
                            for cy in (top_y - 1)..=(top_y + 1).min(CHUNK_SIZE_Y - 1) {
                                let radius: i32 = if cy == top_y + 1 {
                                    1
                                } else if cy == top_y {
                                    2
                                } else {
                                    1
                                };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        if radius == 2 && dx.abs() == 2 && dz.abs() == 2 {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::ACACIA_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Swamp => {
                        if th % 28 == 0 {
                            let trunk_h = 4;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::WOOD_LOG);
                            }
                            let crown_start = ly + 2;
                            let crown_end = (ly + 5).min(CHUNK_SIZE_Y - 1);
                            for cy in crown_start..=crown_end {
                                let radius: i32 = 2;
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        if dx.abs() == 2 && dz.abs() == 2 && (dx + dz) % 2 == 0 {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::SnowyTundra => {
                        if th % 34 == 0 {
                            let trunk_height = 5 + ((th >> 4) % 3) as usize;
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
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::SPRUCE_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Mountains => {
                        if ground == Some(BlockId::GRASS) && th % 36 == 0 {
                            let trunk_height = 5 + ((th >> 4) % 3) as usize;
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
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::SPRUCE_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Jungle => {
                        if th % 15 == 0 {
                            let trunk_h = 6 + ((th >> 4) % 5) as usize;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::JUNGLE_LOG);
                            }
                            let top_y = ly + trunk_h;
                            for cy in (top_y.saturating_sub(2))..=(top_y + 1).min(CHUNK_SIZE_Y - 1)
                            {
                                let radius: i32 = if cy == top_y + 1 {
                                    1
                                } else if cy >= top_y.saturating_sub(1) {
                                    2
                                } else {
                                    1
                                };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        if radius == 2
                                            && dx.abs() == 2
                                            && dz.abs() == 2
                                            && (dx + dz) % 2 == 0
                                        {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::JUNGLE_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        } else if th % 21 == 0 {
                            chunk.set(lx, ly, lz, BlockId::JUNGLE_LOG);
                            for dz in -1..=1 {
                                for dx in -1..=1 {
                                    let cx = lx as i32 + dx;
                                    let cz = lz as i32 + dz;
                                    if cx >= 0
                                        && cx < CHUNK_SIZE_X as i32
                                        && cz >= 0
                                        && cz < CHUNK_SIZE_Z as i32
                                    {
                                        if chunk.get(cx as usize, ly, cz as usize)
                                            == Some(BlockId::AIR)
                                        {
                                            chunk.set(
                                                cx as usize,
                                                ly,
                                                cz as usize,
                                                BlockId::JUNGLE_LEAVES,
                                            );
                                        }
                                        if ly + 1 < CHUNK_SIZE_Y
                                            && chunk.get(cx as usize, ly + 1, cz as usize)
                                                == Some(BlockId::AIR)
                                        {
                                            chunk.set(
                                                cx as usize,
                                                ly + 1,
                                                cz as usize,
                                                BlockId::JUNGLE_LEAVES,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Taiga => {
                        if (th >> 12) % 60 == 0 {
                            for dy in 0..2 {
                                for dz in -1_i32..=1 {
                                    for dx in -1_i32..=1 {
                                        if dy == 1 && (dx.abs() + dz.abs() > 1) {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cy = ly + dy;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                            && cy < CHUNK_SIZE_Y
                                        {
                                            chunk.set(
                                                cx as usize,
                                                cy,
                                                cz as usize,
                                                BlockId::MOSSY_COBBLESTONE,
                                            );
                                        }
                                    }
                                }
                            }
                        } else if th % 17 == 0 {
                            let trunk_height = 6 + ((th >> 4) % 4) as usize;
                            for ty in ly..(ly + trunk_height).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::SPRUCE_LOG);
                            }
                            let top_y = ly + trunk_height;
                            if top_y < CHUNK_SIZE_Y {
                                chunk.set(lx, top_y, lz, BlockId::SPRUCE_LEAVES);
                            }
                            for dy in 1..=trunk_height.min(5) {
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
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::SPRUCE_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::DarkForest => {
                        if th % 13 == 0
                            && lx >= 2
                            && lx < (CHUNK_SIZE_X - 4)
                            && lz >= 2
                            && lz < (CHUNK_SIZE_Z - 4)
                        {
                            let is_shroom = (th >> 9) % 7 == 0;
                            if is_shroom {
                                let is_red = (th >> 5) % 2 == 0;
                                let stalk_height = 4 + ((th >> 3) % 2) as usize;
                                for ty in ly..(ly + stalk_height).min(CHUNK_SIZE_Y) {
                                    chunk.set(lx, ty, lz, BlockId::MUSHROOM_STEM);
                                }
                                let top_y = ly + stalk_height;
                                let cap_block = if is_red {
                                    BlockId::RED_MUSHROOM_BLOCK
                                } else {
                                    BlockId::BROWN_MUSHROOM_BLOCK
                                };
                                if top_y < CHUNK_SIZE_Y {
                                    for dz in -1_i32..=1 {
                                        for dx in -1_i32..=1 {
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0
                                                && cx < CHUNK_SIZE_X as i32
                                                && cz >= 0
                                                && cz < CHUNK_SIZE_Z as i32
                                            {
                                                chunk.set(cx as usize, top_y, cz as usize, cap_block);
                                            }
                                        }
                                    }
                                }
                            } else {
                                let trunk_height = 6 + ((th >> 3) % 3) as usize;
                                for ty in ly..(ly + trunk_height).min(CHUNK_SIZE_Y) {
                                    for ox in 0..2 {
                                        for oz in 0..2 {
                                            chunk.set(
                                                lx + ox,
                                                ty,
                                                lz + oz,
                                                BlockId::DARK_OAK_LOG,
                                            );
                                        }
                                    }
                                }
                                let top_y = ly + trunk_height;
                                for dy in 0..=3 {
                                    if dy > top_y {
                                        break;
                                    }
                                    let cy = top_y + 1 - dy;
                                    if cy >= CHUNK_SIZE_Y {
                                        continue;
                                    }
                                    let radius: i32 = if dy == 0 { 2 } else { 3 };
                                    for dz in -radius..=(radius + 1) {
                                        for dx in -radius..=(radius + 1) {
                                            if (dx == -radius || dx == radius + 1)
                                                && (dz == -radius || dz == radius + 1)
                                            {
                                                continue;
                                            }
                                            let cx = lx as i32 + dx;
                                            let cz = lz as i32 + dz;
                                            if cx >= 0
                                                && cx < CHUNK_SIZE_X as i32
                                                && cz >= 0
                                                && cz < CHUNK_SIZE_Z as i32
                                            {
                                                if chunk.get(cx as usize, cy, cz as usize)
                                                    == Some(BlockId::AIR)
                                                {
                                                    chunk.set(
                                                        cx as usize,
                                                        cy,
                                                        cz as usize,
                                                        BlockId::DARK_OAK_LEAVES,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::BirchForest => {
                        if (th >> 12) % 95 == 0 {
                            chunk.set(lx, ly, lz, BlockId::MOSSY_COBBLESTONE);
                        } else if th % 16 == 0 {
                            let trunk_h = 5 + ((th >> 4) % 4) as usize;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, BlockId::BIRCH_LOG);
                            }
                            let crown_start = ly + trunk_h.saturating_sub(2);
                            let crown_end = (ly + trunk_h + 1).min(CHUNK_SIZE_Y - 1);
                            for cy in crown_start..=crown_end {
                                let radius: i32 = if cy >= ly + trunk_h { 1 } else { 2 };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if dx.abs() == 2
                                                && dz.abs() == 2
                                                && (dx + dz) % 2 == 0
                                            {
                                                continue;
                                            }
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::BIRCH_LEAVES,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::FlowerMeadow => {
                        if th % 115 == 0 {
                            let is_birch = (th >> 8) % 2 == 0;
                            let (log, leaves) = if is_birch {
                                (BlockId::BIRCH_LOG, BlockId::BIRCH_LEAVES)
                            } else {
                                (BlockId::WOOD_LOG, BlockId::LEAVES)
                            };
                            let trunk_h = 4 + ((th >> 4) % 2) as usize;
                            for ty in ly..(ly + trunk_h).min(CHUNK_SIZE_Y) {
                                chunk.set(lx, ty, lz, log);
                            }
                            let crown_start = ly + 2;
                            let crown_end = (ly + trunk_h + 1).min(CHUNK_SIZE_Y - 1);
                            for cy in crown_start..=crown_end {
                                let radius: i32 = if cy >= ly + trunk_h { 1 } else { 2 };
                                for dz in -radius..=radius {
                                    for dx in -radius..=radius {
                                        if dx.abs() == 2
                                            && dz.abs() == 2
                                            && (dx + dz) % 2 == 0
                                        {
                                            continue;
                                        }
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(cx as usize, cy, cz as usize, leaves);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Biome::Glacier => {
                        if th % 42 == 0 {
                            // Crystalline glacial ice spires rising above flat ice sheets
                            let spire_h = 5 + ((th >> 4) % 6) as usize;
                            for dy in 0..spire_h {
                                let cy = ly + dy;
                                if cy >= CHUNK_SIZE_Y {
                                    break;
                                }
                                if dy < spire_h / 2 {
                                    for (dx, dz) in [(0_i32, 0_i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                                        let cx = lx as i32 + dx;
                                        let cz = lz as i32 + dz;
                                        if cx >= 0
                                            && cx < CHUNK_SIZE_X as i32
                                            && cz >= 0
                                            && cz < CHUNK_SIZE_Z as i32
                                        {
                                            if chunk.get(cx as usize, cy, cz as usize)
                                                == Some(BlockId::AIR)
                                            {
                                                chunk.set(
                                                    cx as usize,
                                                    cy,
                                                    cz as usize,
                                                    BlockId::ICE,
                                                );
                                            }
                                        }
                                    }
                                } else if chunk.get(lx, cy, lz) == Some(BlockId::AIR) {
                                    chunk.set(lx, cy, lz, BlockId::ICE);
                                }
                            }
                        }
                    }
                    Biome::StonyShore => {
                        if th % 48 == 0 {
                            let boulder_block = if (th >> 6) % 2 == 0 {
                                BlockId::COBBLESTONE
                            } else {
                                BlockId::STONE
                            };
                            chunk.set(lx, ly, lz, boulder_block);
                            if (th >> 10) % 3 == 0 && ly + 1 < CHUNK_SIZE_Y {
                                chunk.set(lx, ly + 1, lz, boulder_block);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Re-stamp intersecting boss temples & villages so foliage/trees never clip into interiors
        let cpos = chunk.pos;
        for temple in self.find_temples_for_chunk(cpos) {
            crate::temple_gen::stamp_temple_exterior(chunk, &temple);
        }
        for village in self.find_villages_for_chunk(cpos) {
            crate::village_gen::stamp_village(chunk, &village);
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

#[allow(dead_code)]
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
        assert_eq!(gen1.find_spawn_coords(), gen2.find_spawn_coords());
    }

    #[test]
    fn test_chunk_generation_and_features() {
        let gen = TerrainGenerator::new(12345);
        let mut chunk = gen.generate_chunk(ChunkPos::new(0, 1, 0));
        gen.populate_chunk_features(&mut chunk);

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
        assert!(found_diamond, "Expected diamond ore to generate underground");
    }

    #[test]
    fn test_biome_diversity_and_features() {
        let gen = TerrainGenerator::new(1337);
        let mut biomes_found = std::collections::HashSet::new();

        for z in (-2200..2200).step_by(20) {
            for x in (-2200..2200).step_by(20) {
                let h = gen.surface_height(x, z);
                let biome = gen.get_biome(x, z, h);
                biomes_found.insert(biome);
            }
        }

        assert!(biomes_found.contains(&Biome::Plains), "Expected Plains biome");
        assert!(
            biomes_found.contains(&Biome::FlowerMeadow),
            "Expected FlowerMeadow biome"
        );
        assert!(biomes_found.contains(&Biome::Forest), "Expected Forest biome");
        assert!(
            biomes_found.contains(&Biome::BirchForest),
            "Expected BirchForest biome"
        );
        assert!(biomes_found.contains(&Biome::Mountains), "Expected Mountains biome");
        assert!(biomes_found.contains(&Biome::Desert), "Expected Desert biome");
        assert!(
            biomes_found.contains(&Biome::RedDesert),
            "Expected RedDesert biome"
        );
        assert!(
            biomes_found.contains(&Biome::SnowyTundra),
            "Expected SnowyTundra biome"
        );
        assert!(biomes_found.contains(&Biome::Glacier), "Expected Glacier biome");
        assert!(biomes_found.contains(&Biome::Savanna), "Expected Savanna biome");
        assert!(biomes_found.contains(&Biome::Swamp), "Expected Swamp biome");
        assert!(biomes_found.contains(&Biome::Ocean), "Expected Ocean biome");
        assert!(biomes_found.contains(&Biome::Jungle), "Expected Jungle biome");
        assert!(biomes_found.contains(&Biome::Badlands), "Expected Badlands biome");
        assert!(biomes_found.contains(&Biome::Beach), "Expected Beach biome");
        assert!(
            biomes_found.contains(&Biome::StonyShore),
            "Expected StonyShore biome"
        );
        assert!(biomes_found.contains(&Biome::River), "Expected River biome");
        assert!(biomes_found.contains(&Biome::Taiga), "Expected Taiga biome");
        assert!(biomes_found.contains(&Biome::DarkForest), "Expected DarkForest biome");
        assert!(biomes_found.contains(&Biome::CoralReef), "Expected CoralReef biome");
    }

    #[test]
    fn test_minimum_biome_extent() {
        use std::collections::VecDeque;

        // Verify across multiple seeds that non-river/non-shore biomes form large, contiguous
        // macro regions rather than tiny 10-second pockets.
        const STEP: i32 = 10;
        const HALF: i32 = 120; // 240x240 grid => 2400x2400 blocks
        let grid_n = (HALF * 2) as usize;

        for seed in [42u32, 1337, 99999] {
            let gen = TerrainGenerator::new(seed);
            let mut grid = vec![Biome::Plains; grid_n * grid_n];
            for gz in 0..grid_n {
                let z = (gz as i32 - HALF) * STEP;
                for gx in 0..grid_n {
                    let x = (gx as i32 - HALF) * STEP;
                    let h = gen.surface_height(x, z);
                    grid[gz * grid_n + gx] = gen.get_biome(x, z, h);
                }
            }

            let mut visited = vec![false; grid_n * grid_n];
            let mut tiny_inland_pockets = Vec::new();

            for gz in 0..grid_n {
                for gx in 0..grid_n {
                    let idx = gz * grid_n + gx;
                    if visited[idx] {
                        continue;
                    }
                    let biome = grid[idx];
                    let mut queue = VecDeque::new();
                    let mut cells = Vec::new();
                    visited[idx] = true;
                    queue.push_back((gx, gz));

                    let mut touches_edge = false;
                    let mut touches_river_or_water = false;
                    while let Some((cx, cz)) = queue.pop_front() {
                        cells.push((cx, cz));
                        if cx == 0 || cz == 0 || cx + 1 == grid_n || cz + 1 == grid_n {
                            touches_edge = true;
                        }
                        for (nx, nz) in [
                            (cx.wrapping_sub(1), cz),
                            (cx + 1, cz),
                            (cx, cz.wrapping_sub(1)),
                            (cx, cz + 1),
                        ] {
                            if nx < grid_n && nz < grid_n {
                                let nidx = nz * grid_n + nx;
                                let nb = grid[nidx];
                                if nb == biome {
                                    if !visited[nidx] {
                                        visited[nidx] = true;
                                        queue.push_back((nx, nz));
                                    }
                                } else if matches!(
                                    nb,
                                    Biome::River
                                        | Biome::Beach
                                        | Biome::StonyShore
                                        | Biome::Ocean
                                        | Biome::CoralReef
                                ) {
                                    touches_river_or_water = true;
                                }
                            }
                        }
                    }

                    if touches_edge
                        || matches!(
                            biome,
                            Biome::River
                                | Biome::Beach
                                | Biome::StonyShore
                                | Biome::Ocean
                                | Biome::CoralReef
                        )
                    {
                        continue;
                    }

                    let area_sq_blocks = cells.len() * (STEP as usize) * (STEP as usize);
                    if !touches_river_or_water && area_sq_blocks < 10_000 {
                        let (sx, sz) = cells[0];
                        tiny_inland_pockets.push((
                            biome,
                            area_sq_blocks,
                            (sx as i32 - HALF) * STEP,
                            (sz as i32 - HALF) * STEP,
                        ));
                    }
                }
            }

            assert!(
                tiny_inland_pockets.is_empty(),
                "Seed {} produced tiny land-locked biome pockets (< 10000 sq blocks): {:?}",
                seed,
                tiny_inland_pockets
            );

            // Verify macro-land region median area (bridging River/Shore overlays)
            let mut pure_grid = vec![Biome::Plains; grid_n * grid_n];
            for gz in 0..grid_n {
                let z = (gz as i32 - HALF) * STEP;
                for gx in 0..grid_n {
                    let x = (gx as i32 - HALF) * STEP;
                    let (wx, wz) = gen.sample_warp(x, z);
                    let cont = gen
                        .continental_noise
                        .get([wx + gen.cont_offset[0], wz + gen.cont_offset[1]]);
                    pure_grid[gz * grid_n + gx] = if cont < -0.20 {
                        Biome::Ocean
                    } else {
                        gen.sample_macro_cells(wx, wz).0
                    };
                }
            }
            let mut pvisited = vec![false; grid_n * grid_n];
            let mut pure_areas = Vec::new();
            for gz in 0..grid_n {
                for gx in 0..grid_n {
                    let idx = gz * grid_n + gx;
                    if pvisited[idx] || pure_grid[idx] == Biome::Ocean {
                        continue;
                    }
                    let b = pure_grid[idx];
                    let mut q = VecDeque::new();
                    pvisited[idx] = true;
                    q.push_back((gx, gz));
                    let mut count = 0usize;
                    let mut edge = false;
                    while let Some((cx, cz)) = q.pop_front() {
                        count += 1;
                        if cx == 0 || cz == 0 || cx + 1 == grid_n || cz + 1 == grid_n {
                            edge = true;
                        }
                        for (nx, nz) in [
                            (cx.wrapping_sub(1), cz),
                            (cx + 1, cz),
                            (cx, cz.wrapping_sub(1)),
                            (cx, cz + 1),
                        ] {
                            if nx < grid_n && nz < grid_n {
                                let nidx = nz * grid_n + nx;
                                if !pvisited[nidx] && pure_grid[nidx] == b {
                                    pvisited[nidx] = true;
                                    q.push_back((nx, nz));
                                }
                            }
                        }
                    }
                    if !edge {
                        pure_areas.push(count * (STEP as usize) * (STEP as usize));
                    }
                }
            }
            pure_areas.sort_unstable();
            let median_macro_area = pure_areas.get(pure_areas.len() / 2).copied().unwrap_or(0);
            assert!(
                median_macro_area >= 35_000,
                "Seed {} median macro-land biome area too small: {} sq blocks",
                seed,
                median_macro_area
            );
        }

        // Also verify 1D interior run lengths remain broad across slices
        let gen = TerrainGenerator::new(1337);
        let mut run_lengths = Vec::new();
        for z in (-1500..=1500).step_by(300) {
            let mut cur: Option<Biome> = None;
            let mut run = 0i32;
            let mut prev_was_river_or_water = true;
            for x in (-1500..=1500).step_by(5) {
                let h = gen.surface_height(x, z);
                let b = gen.get_biome(x, z, h);
                let is_water_or_shore = matches!(
                    b,
                    Biome::River
                        | Biome::Beach
                        | Biome::StonyShore
                        | Biome::Ocean
                        | Biome::CoralReef
                );
                if Some(b) == cur {
                    run += 5;
                } else {
                    if let Some(prev_b) = cur {
                        if !matches!(
                            prev_b,
                            Biome::River
                                | Biome::Beach
                                | Biome::StonyShore
                                | Biome::Ocean
                                | Biome::CoralReef
                        ) && !prev_was_river_or_water
                            && !is_water_or_shore
                        {
                            run_lengths.push(run);
                        }
                    }
                    prev_was_river_or_water = cur.map_or(true, |pb| {
                        matches!(
                            pb,
                            Biome::River
                                | Biome::Beach
                                | Biome::StonyShore
                                | Biome::Ocean
                                | Biome::CoralReef
                        )
                    });
                    cur = Some(b);
                    run = 5;
                }
            }
        }
        run_lengths.sort_unstable();
        let median_run = run_lengths.get(run_lengths.len() / 2).copied().unwrap_or(0);
        assert!(
            median_run >= 140,
            "Expected median 1D land biome run >= 140 blocks, got {} ({:?})",
            median_run,
            run_lengths
        );
    }

    #[test]
    fn test_biome_relief_contrast() {
        let gen = TerrainGenerator::new(1337);
        let mut flat_slopes = Vec::new();
        let mut flat_heights = Vec::new();
        let mut vert_slopes = Vec::new();
        let mut vert_heights = Vec::new();

        for z in (-800..800).step_by(12) {
            for x in (-800..800).step_by(12) {
                let h = gen.surface_height(x, z);
                let biome = gen.get_biome(x, z, h);
                let slope = (h - gen.surface_height(x + 4, z))
                    .abs()
                    .max((h - gen.surface_height(x, z + 4)).abs());
                match biome {
                    Biome::Plains | Biome::FlowerMeadow | Biome::Swamp | Biome::RedDesert => {
                        flat_heights.push(h as f64);
                        flat_slopes.push(slope as f64);
                    }
                    Biome::Mountains | Biome::Badlands => {
                        vert_heights.push(h as f64);
                        vert_slopes.push(slope as f64);
                    }
                    _ => {}
                }
            }
        }

        assert!(
            !flat_heights.is_empty() && !vert_heights.is_empty(),
            "Expected both flat and vertical biome samples"
        );
        let avg_flat_h = flat_heights.iter().sum::<f64>() / flat_heights.len() as f64;
        let avg_vert_h = vert_heights.iter().sum::<f64>() / vert_heights.len() as f64;
        let avg_flat_slope = flat_slopes.iter().sum::<f64>() / flat_slopes.len() as f64;
        let avg_vert_slope = vert_slopes.iter().sum::<f64>() / vert_slopes.len() as f64;

        assert!(
            avg_flat_h < 50.0,
            "Flat biomes should remain gently elevated near sea level, got avg y={:.2}",
            avg_flat_h
        );
        assert!(
            avg_vert_h > avg_flat_h + 15.0,
            "Vertical biomes ({:.2}) should tower clearly above flat biomes ({:.2})",
            avg_vert_h,
            avg_flat_h
        );
        assert!(
            avg_flat_slope < 2.1 && avg_vert_slope > avg_flat_slope * 2.5,
            "Flat biomes should have noticeably calmer slopes ({:.2}) than vertical biomes ({:.2})",
            avg_flat_slope,
            avg_vert_slope
        );
    }

    #[test]
    fn test_seed_diversity_verticality_and_dry_spawn() {
        let mut spawn_signatures = std::collections::HashSet::new();
        let mut max_observed_height = 0;

        for seed in [1u32, 42, 1337, 99999, 202511, 777777, 888888, 1234567] {
            let gen = TerrainGenerator::new(seed);
            let (sx, sy, sz) = gen.find_spawn_coords();
            let sbiome = gen.get_biome(sx, sz, sy);
            assert!(
                sy > gen.water_level + 1,
                "Seed {} spawned in or too close to water: y={} <= {}",
                seed,
                sy,
                gen.water_level + 1
            );
            assert!(
                !matches!(sbiome, Biome::River | Biome::Ocean | Biome::CoralReef),
                "Seed {} spawned in water biome {:?}",
                seed,
                sbiome
            );

            spawn_signatures.insert((
                gen.surface_height(0, 0),
                gen.get_biome(0, 0, gen.surface_height(0, 0)),
                sx,
                sz,
            ));

            // Check that verticality creates genuine peaks above y = 65
            for z in (-500..=500).step_by(20) {
                for x in (-500..=500).step_by(20) {
                    max_observed_height = max_observed_height.max(gen.surface_height(x, z));
                }
            }
        }

        assert!(
            spawn_signatures.len() >= 6,
            "Different seeds must vary terrain/biomes near spawn, only got {} unique signatures",
            spawn_signatures.len()
        );
        assert!(
            max_observed_height >= 72,
            "Expected tall mountain peaks (>= 72), max observed was {}",
            max_observed_height
        );
    }
}
