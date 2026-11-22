use glam::{IVec3, Vec3};

use crate::block::{
    is_farmland_hydrated, try_grow_sugar_cane, BlockId, BlockRegistry, FertilizerTier,
};
use crate::item::{ItemId, ItemSlot};
use crate::monster::compute_light_at;
use crate::world::World;

/// Minimum light level (sky or block light, 0..=15) required for crops to grow.
pub const CROP_MIN_LIGHT_LEVEL: f32 = 9.0;

/// Randomized per-stage crop growth duration range in seconds.
/// 3 stage transitions averaging ~120s each = ~6 minutes total (within ~4–8 minutes).
pub const CROP_STAGE_MIN_SECS: f32 = 80.0;
pub const CROP_STAGE_MAX_SECS: f32 = 160.0;

/// Randomized duration range in seconds for a mature Stem Crop to spawn an adjacent fruit.
pub const STEM_FRUIT_MIN_SECS: f32 = 100.0;
pub const STEM_FRUIT_MAX_SECS: f32 = 200.0;

/// Randomized duration range in seconds for Sugar Cane to grow 1 block (up to height 3).
pub const SUGAR_CANE_MIN_SECS: f32 = 90.0;
pub const SUGAR_CANE_MAX_SECS: f32 = 180.0;

/// Dry farmland reversion timers.
pub const FARMLAND_DRY_EMPTY_SECS: f32 = 5.0;
pub const FARMLAND_DRY_CROP_SECS: f32 = 10.0;

/// Maximum horizontal distance (in blocks) from any active player for farming simulation ticks.
pub const ACTIVE_SIM_RADIUS_BLOCKS: f32 = 112.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropEntry {
    pub pos: IVec3,
    pub timer: f32,
    pub target_secs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FarmlandEntry {
    pub pos: IVec3,
    pub dry_timer: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SugarCaneEntry {
    pub pos: IVec3,
    pub timer: f32,
    pub target_secs: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FarmingEvent {
    BlockChanged { pos: IVec3, block: BlockId },
    SpawnDrop { pos: IVec3, item: ItemSlot },
}

/// Shared farming simulation (ADR-0004): ticked locally by `WorldManager` in singleplayer
/// and authoritatively by `MultiplayerServer` in multiplayer.
#[derive(Debug, Clone)]
pub struct FarmingSimulation {
    pub planted_crops: Vec<CropEntry>,
    pub active_farmland: Vec<FarmlandEntry>,
    pub growing_sugar_cane: Vec<SugarCaneEntry>,
    rng_state: u64,
}

impl Default for FarmingSimulation {
    fn default() -> Self {
        Self::new(0x5EED_FA4A_1234_5678)
    }
}

impl FarmingSimulation {
    pub fn new(seed: u64) -> Self {
        Self {
            planted_crops: Vec::new(),
            active_farmland: Vec::new(),
            growing_sugar_cane: Vec::new(),
            rng_state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed },
        }
    }

    /// Reconstructs a `FarmingSimulation` from persisted `WorldSaveData` timers.
    pub fn from_save_data(data: &crate::save::WorldSaveData) -> Self {
        let mut sim = Self::new(data.metadata.seed as u64 ^ 0xFA4A_1234);
        for &(pos, timer) in &data.planted_crops {
            let target_secs = sim.roll_stage_duration();
            sim.planted_crops.push(CropEntry {
                pos,
                timer,
                target_secs,
            });
        }
        for &(pos, dry_timer) in &data.active_farmland {
            sim.active_farmland.push(FarmlandEntry { pos, dry_timer });
        }
        for &(pos, timer) in &data.growing_sugar_cane {
            let target_secs = sim.roll_sugar_cane_duration();
            sim.growing_sugar_cane.push(SugarCaneEntry {
                pos,
                timer,
                target_secs,
            });
        }
        sim
    }

    /// Exports current crop, farmland, and sugar cane timers into `WorldSaveData`.
    pub fn export_to_save_data(&self, data: &mut crate::save::WorldSaveData) {
        data.planted_crops = self
            .planted_crops
            .iter()
            .map(|c| (c.pos, c.timer))
            .collect();
        data.active_farmland = self
            .active_farmland
            .iter()
            .map(|f| (f.pos, f.dry_timer))
            .collect();
        data.growing_sugar_cane = self
            .growing_sugar_cane
            .iter()
            .map(|s| (s.pos, s.timer))
            .collect();
    }

    fn next_u32(&mut self) -> u32 {
        // xorshift64*
        let mut x = self.rng_state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng_state = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D)) >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }

    pub fn roll_stage_duration(&mut self) -> f32 {
        CROP_STAGE_MIN_SECS + self.next_f32() * (CROP_STAGE_MAX_SECS - CROP_STAGE_MIN_SECS)
    }

    pub fn roll_stem_fruit_duration(&mut self) -> f32 {
        STEM_FRUIT_MIN_SECS + self.next_f32() * (STEM_FRUIT_MAX_SECS - STEM_FRUIT_MIN_SECS)
    }

    pub fn roll_sugar_cane_duration(&mut self) -> f32 {
        SUGAR_CANE_MIN_SECS + self.next_f32() * (SUGAR_CANE_MAX_SECS - SUGAR_CANE_MIN_SECS)
    }

    /// Registers a newly planted farmland crop with a randomized stage duration.
    pub fn plant_crop(&mut self, pos: IVec3) {
        let target_secs = self.roll_stage_duration();
        if let Some(existing) = self.planted_crops.iter_mut().find(|c| c.pos == pos) {
            existing.timer = 0.0;
            existing.target_secs = target_secs;
        } else {
            self.planted_crops.push(CropEntry {
                pos,
                timer: 0.0,
                target_secs,
            });
        }
    }

    /// Tracks a farmland tile for hydration/reversion updates.
    pub fn track_farmland(&mut self, pos: IVec3) {
        if !self.active_farmland.iter().any(|f| f.pos == pos) {
            self.active_farmland.push(FarmlandEntry {
                pos,
                dry_timer: 0.0,
            });
        }
    }

    /// Tracks a sugar cane base/stalk for vertical growth up to height 3.
    pub fn track_sugar_cane(&mut self, pos: IVec3) {
        if !self.growing_sugar_cane.iter().any(|s| s.pos == pos) {
            let target_secs = self.roll_sugar_cane_duration();
            self.growing_sugar_cane.push(SugarCaneEntry {
                pos,
                timer: 0.0,
                target_secs,
            });
        }
    }

    /// Applies a passive `FertilizerTier` to a farmland block at `farmland_pos`.
    /// Only upgrades strictly higher tiers (None -> Compost -> Bone); applying the same or
    /// lower tier returns `None` so items are never wasted.
    pub fn apply_fertilizer(
        &mut self,
        farmland_pos: IVec3,
        tier: FertilizerTier,
        world: &mut World,
    ) -> Option<BlockId> {
        let current = world.get_block(farmland_pos);
        if !current.is_farmland() || tier <= current.farmland_fertilizer() {
            return None;
        }
        let updated = current.with_farmland_fertilizer(tier);
        world.set_block(farmland_pos, updated);
        self.track_farmland(farmland_pos);
        Some(updated)
    }

    /// Called when a mature non-stem farmland crop at `crop_pos` is harvested.
    /// Returns the farmland's `FertilizerTier` (for bonus yield) and clears the fertilizer
    /// for the completed harvest cycle (leaving hydration intact).
    pub fn consume_fertilizer_on_harvest(
        &mut self,
        crop_pos: IVec3,
        world: &mut World,
    ) -> (FertilizerTier, Option<BlockId>) {
        let below_pos = crop_pos + IVec3::NEG_Y;
        let below = world.get_block(below_pos);
        if !below.is_farmland() {
            return (FertilizerTier::None, None);
        }
        let tier = below.farmland_fertilizer();
        if tier == FertilizerTier::None {
            return (FertilizerTier::None, None);
        }
        let reset = below.with_farmland_fertilizer(FertilizerTier::None);
        world.set_block(below_pos, reset);
        (tier, Some(reset))
    }

    fn is_near_any_player(pos: IVec3, active_players: &[Vec3]) -> bool {
        let r2 = ACTIVE_SIM_RADIUS_BLOCKS * ACTIVE_SIM_RADIUS_BLOCKS;
        let px = pos.x as f32 + 0.5;
        let pz = pos.z as f32 + 0.5;
        active_players.iter().any(|p| {
            let dx = p.x - px;
            let dz = p.z - pz;
            dx * dx + dz * dz <= r2
        })
    }

    /// Returns the default popped drop when a crop loses its farmland or is uprooted.
    pub fn uproot_drop_for_crop(block: BlockId) -> ItemSlot {
        match block {
            BlockId::WHEAT_3 => ItemSlot::new(ItemId::WHEAT, 1),
            BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 => {
                ItemSlot::new(ItemId::WHEAT_SEEDS, 1)
            }
            BlockId::CARROTS_3 => ItemSlot::new(ItemId::CARROT, 2),
            BlockId::CARROTS_0 | BlockId::CARROTS_1 | BlockId::CARROTS_2 => {
                ItemSlot::new(ItemId::CARROT, 1)
            }
            BlockId::POTATOES_3 => ItemSlot::new(ItemId::POTATO, 2),
            BlockId::POTATOES_0 | BlockId::POTATOES_1 | BlockId::POTATOES_2 => {
                ItemSlot::new(ItemId::POTATO, 1)
            }
            BlockId::TOMATOES_3 => ItemSlot::new(ItemId::TOMATO, 2),
            BlockId::TOMATOES_0 | BlockId::TOMATOES_1 | BlockId::TOMATOES_2 => {
                ItemSlot::new(ItemId::TOMATO_SEEDS, 1)
            }
            BlockId::MELON_STEM_0
            | BlockId::MELON_STEM_1
            | BlockId::MELON_STEM_2
            | BlockId::MELON_STEM_3 => ItemSlot::new(ItemId::MELON_SEEDS, 1),
            BlockId::PUMPKIN_STEM_0
            | BlockId::PUMPKIN_STEM_1
            | BlockId::PUMPKIN_STEM_2
            | BlockId::PUMPKIN_STEM_3 => ItemSlot::new(ItemId::PUMPKIN_SEEDS, 1),
            _ => ItemSlot::EMPTY,
        }
    }

    /// Advances farmland hydration/reversion, crop growth, stem fruit spawning, and sugar cane
    /// growth for elapsed seconds `dt`. Returns all block/drop mutations for meshing & net sync.
    pub fn tick(
        &mut self,
        dt: f32,
        sun_intensity: f32,
        active_players: &[Vec3],
        world: &mut World,
        registry: &BlockRegistry,
    ) -> Vec<FarmingEvent> {
        let mut events = Vec::new();
        if dt <= 0.0 || active_players.is_empty() {
            return events;
        }

        // 1. Farmland hydration & dry reversion
        let mut farmland_idx = 0;
        while farmland_idx < self.active_farmland.len() {
            let pos = self.active_farmland[farmland_idx].pos;
            if !Self::is_near_any_player(pos, active_players) {
                farmland_idx += 1;
                continue;
            }
            let current = world.get_block(pos);
            if !current.is_farmland() {
                self.active_farmland.swap_remove(farmland_idx);
                continue;
            }

            let hydrated = is_farmland_hydrated(pos, |p| world.get_block(p));
            let desired = current.with_farmland_hydration(hydrated);
            if desired != current {
                world.set_block(pos, desired);
                events.push(FarmingEvent::BlockChanged {
                    pos,
                    block: desired,
                });
            }

            if hydrated {
                self.active_farmland[farmland_idx].dry_timer = 0.0;
                farmland_idx += 1;
            } else {
                let above_pos = pos + IVec3::Y;
                let above = world.get_block(above_pos);
                let has_crop = above.is_farmland_crop();
                let revert_limit = if has_crop {
                    FARMLAND_DRY_CROP_SECS
                } else {
                    FARMLAND_DRY_EMPTY_SECS
                };

                self.active_farmland[farmland_idx].dry_timer += dt;
                if self.active_farmland[farmland_idx].dry_timer >= revert_limit {
                    // Revert to dirt (clears any fertilizer on this tile)
                    world.set_block(pos, BlockId::DIRT);
                    events.push(FarmingEvent::BlockChanged {
                        pos,
                        block: BlockId::DIRT,
                    });
                    if has_crop {
                        let drop = Self::uproot_drop_for_crop(above);
                        world.set_block(above_pos, BlockId::AIR);
                        events.push(FarmingEvent::BlockChanged {
                            pos: above_pos,
                            block: BlockId::AIR,
                        });
                        if !drop.is_empty() {
                            events.push(FarmingEvent::SpawnDrop {
                                pos: above_pos,
                                item: drop,
                            });
                        }
                    }
                    self.active_farmland.swap_remove(farmland_idx);
                } else {
                    farmland_idx += 1;
                }
            }
        }

        // 2. Planted crops & Stem Crops
        let mut crop_idx = 0;
        while crop_idx < self.planted_crops.len() {
            let pos = self.planted_crops[crop_idx].pos;
            if !Self::is_near_any_player(pos, active_players) {
                crop_idx += 1;
                continue;
            }
            let current = world.get_block(pos);
            if !current.is_farmland_crop() {
                self.planted_crops.swap_remove(crop_idx);
                continue;
            }

            let below_pos = pos + IVec3::NEG_Y;
            let below = world.get_block(below_pos);
            if !below.is_farmland() {
                let drop = Self::uproot_drop_for_crop(current);
                world.set_block(pos, BlockId::AIR);
                events.push(FarmingEvent::BlockChanged {
                    pos,
                    block: BlockId::AIR,
                });
                if !drop.is_empty() {
                    events.push(FarmingEvent::SpawnDrop { pos, item: drop });
                }
                self.planted_crops.swap_remove(crop_idx);
                continue;
            }

            // Non-stem mature crops stop growing once mature
            if current.is_mature_crop() && !current.is_mature_stem() {
                self.planted_crops.swap_remove(crop_idx);
                continue;
            }

            // Require hydrated farmland and light >= 9
            if !is_farmland_hydrated(below_pos, |p| world.get_block(p)) {
                crop_idx += 1;
                continue;
            }
            let light = compute_light_at(pos, sun_intensity, world, registry);
            if light < CROP_MIN_LIGHT_LEVEL {
                crop_idx += 1;
                continue;
            }

            let speed_mult = below.farmland_fertilizer().speed_multiplier();
            self.planted_crops[crop_idx].timer += dt * speed_mult;

            if self.planted_crops[crop_idx].timer >= self.planted_crops[crop_idx].target_secs {
                if let Some(next_stage) = current.next_crop_stage() {
                    world.set_block(pos, next_stage);
                    events.push(FarmingEvent::BlockChanged {
                        pos,
                        block: next_stage,
                    });
                    if next_stage.is_mature_stem() {
                        self.planted_crops[crop_idx].timer = 0.0;
                        self.planted_crops[crop_idx].target_secs = self.roll_stem_fruit_duration();
                        crop_idx += 1;
                    } else if next_stage.is_mature_crop() {
                        self.planted_crops.swap_remove(crop_idx);
                    } else {
                        self.planted_crops[crop_idx].timer = 0.0;
                        self.planted_crops[crop_idx].target_secs = self.roll_stage_duration();
                        crop_idx += 1;
                    }
                } else if let Some(fruit_block) = current.stem_fruit_block() {
                    // Mature Stem Crop: attempt to spawn 1 adjacent fruit if none already adjacent
                    let cardinals = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
                    let already_has_fruit = cardinals
                        .iter()
                        .any(|&off| world.get_block(pos + off) == fruit_block);
                    if !already_has_fruit {
                        let valid_spots: Vec<IVec3> = cardinals
                            .iter()
                            .map(|&off| pos + off)
                            .filter(|&cand| {
                                if !world.get_block(cand).is_air() {
                                    return false;
                                }
                                let ground = world.get_block(cand + IVec3::NEG_Y);
                                ground.is_farmland()
                                    || matches!(ground, BlockId::GRASS | BlockId::DIRT)
                            })
                            .collect();
                        if !valid_spots.is_empty() {
                            let pick = (self.next_u32() as usize) % valid_spots.len();
                            let fruit_pos = valid_spots[pick];
                            world.set_block(fruit_pos, fruit_block);
                            events.push(FarmingEvent::BlockChanged {
                                pos: fruit_pos,
                                block: fruit_block,
                            });
                            // Consume fertilizer on the stem's farmland tile once first fruit spawns
                            if below.farmland_fertilizer() != FertilizerTier::None {
                                let reset = below.with_farmland_fertilizer(FertilizerTier::None);
                                world.set_block(below_pos, reset);
                                events.push(FarmingEvent::BlockChanged {
                                    pos: below_pos,
                                    block: reset,
                                });
                            }
                        }
                    }
                    self.planted_crops[crop_idx].timer = 0.0;
                    self.planted_crops[crop_idx].target_secs = self.roll_stem_fruit_duration();
                    crop_idx += 1;
                } else {
                    self.planted_crops.swap_remove(crop_idx);
                }
            } else {
                crop_idx += 1;
            }
        }

        // 3. Sugar Cane growth (up to 3 blocks tall on water-adjacent sand/dirt/grass)
        let mut cane_idx = 0;
        while cane_idx < self.growing_sugar_cane.len() {
            let pos = self.growing_sugar_cane[cane_idx].pos;
            if !Self::is_near_any_player(pos, active_players) {
                cane_idx += 1;
                continue;
            }
            if world.get_block(pos) != BlockId::SUGAR_CANE {
                self.growing_sugar_cane.swap_remove(cane_idx);
                continue;
            }
            self.growing_sugar_cane[cane_idx].timer += dt;
            if self.growing_sugar_cane[cane_idx].timer
                >= self.growing_sugar_cane[cane_idx].target_secs
            {
                if let Some(grow_pos) =
                    try_grow_sugar_cane(pos, registry, |p| world.get_block(p))
                {
                    world.set_block(grow_pos, BlockId::SUGAR_CANE);
                    events.push(FarmingEvent::BlockChanged {
                        pos: grow_pos,
                        block: BlockId::SUGAR_CANE,
                    });
                    self.growing_sugar_cane[cane_idx].timer = 0.0;
                    self.growing_sugar_cane[cane_idx].target_secs = self.roll_sugar_cane_duration();
                    cane_idx += 1;
                } else {
                    // Reached max height 3 or blocked
                    self.growing_sugar_cane.swap_remove(cane_idx);
                }
            } else {
                cane_idx += 1;
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, ChunkPos};

    fn setup_lit_farm_world() -> (World, BlockRegistry) {
        let mut world = World::new();
        for cx in -1..=1 {
            for cz in -1..=1 {
                world.insert_chunk(Chunk::new(ChunkPos::new(cx, 0, cz)));
            }
        }
        let reg = BlockRegistry::new();
        // Hydrated farmland at (5, 1, 5) with water at (6, 1, 5)
        world.set_block(IVec3::new(6, 1, 5), BlockId::WATER);
        world.set_block(IVec3::new(5, 1, 5), BlockId::FARMLAND_MOIST);
        (world, reg)
    }

    #[test]
    fn test_randomized_crop_growth_requires_light_and_hydration() {
        let (mut world, reg) = setup_lit_farm_world();
        let mut sim = FarmingSimulation::new(42);
        let crop_pos = IVec3::new(5, 2, 5);
        world.set_block(crop_pos, BlockId::CARROTS_0);
        sim.plant_crop(crop_pos);

        assert!(sim.planted_crops[0].target_secs >= CROP_STAGE_MIN_SECS);
        assert!(sim.planted_crops[0].target_secs <= CROP_STAGE_MAX_SECS);

        let players = [Vec3::new(5.0, 2.0, 5.0)];

        // In pitch darkness (sun_intensity = 0.0, no torch): should NOT advance
        sim.tick(200.0, 0.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(crop_pos), BlockId::CARROTS_0);

        // In daylight (sun_intensity = 1.0): advances to stage 1
        sim.tick(200.0, 1.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(crop_pos), BlockId::CARROTS_1);
    }

    #[test]
    fn test_passive_fertilizer_upgrade_speed_and_harvest_consumption() {
        let (mut world, reg) = setup_lit_farm_world();
        let mut sim = FarmingSimulation::new(99);
        let farm_pos = IVec3::new(5, 1, 5);
        let crop_pos = IVec3::new(5, 2, 5);
        world.set_block(crop_pos, BlockId::POTATOES_0);
        sim.plant_crop(crop_pos);
        sim.planted_crops[0].target_secs = 120.0;

        // Apply Compost (1.5x)
        assert_eq!(
            sim.apply_fertilizer(farm_pos, FertilizerTier::Compost, &mut world),
            Some(BlockId::FARMLAND_COMPOST_MOIST)
        );
        // Re-applying Compost is blocked
        assert_eq!(
            sim.apply_fertilizer(farm_pos, FertilizerTier::Compost, &mut world),
            None
        );
        // Upgrading to Bone Fertilizer (2.0x) succeeds
        assert_eq!(
            sim.apply_fertilizer(farm_pos, FertilizerTier::Bone, &mut world),
            Some(BlockId::FARMLAND_BONE_MOIST)
        );

        let players = [Vec3::new(5.0, 2.0, 5.0)];
        // 60s * 2.0x = 120s -> advances 1 stage
        sim.tick(60.0, 1.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(crop_pos), BlockId::POTATOES_1);

        // Harvesting consumes fertilizer and returns FertilizerTier::Bone (+1 bonus yield)
        let (tier, reset_block) = sim.consume_fertilizer_on_harvest(crop_pos, &mut world);
        assert_eq!(tier, FertilizerTier::Bone);
        assert_eq!(tier.bonus_yield(), 1);
        assert_eq!(reset_block, Some(BlockId::FARMLAND_MOIST));
        assert_eq!(world.get_block(farm_pos), BlockId::FARMLAND_MOIST);
    }

    #[test]
    fn test_stem_crop_spawns_fruit_and_consumes_fertilizer() {
        let (mut world, reg) = setup_lit_farm_world();
        let mut sim = FarmingSimulation::new(7);
        let farm_pos = IVec3::new(5, 1, 5);
        let stem_pos = IVec3::new(5, 2, 5);
        // Put dirt on cardinal neighbors so fruit can spawn
        world.set_block(IVec3::new(4, 1, 5), BlockId::DIRT);
        world.set_block(stem_pos, BlockId::MELON_STEM_3);
        sim.apply_fertilizer(farm_pos, FertilizerTier::Compost, &mut world);
        sim.planted_crops.push(CropEntry {
            pos: stem_pos,
            timer: 0.0,
            target_secs: 60.0,
        });

        let players = [Vec3::new(5.0, 2.0, 5.0)];
        sim.tick(50.0, 1.0, &players, &mut world, &reg); // 50 * 1.5 = 75 >= 60
        assert_eq!(world.get_block(IVec3::new(4, 2, 5)), BlockId::MELON);
        // Fertilizer consumed on first fruit spawn
        assert_eq!(world.get_block(farm_pos), BlockId::FARMLAND_MOIST);
    }

    #[test]
    fn test_sugar_cane_grows_up_to_3_blocks_near_water() {
        let (mut world, reg) = setup_lit_farm_world();
        let mut sim = FarmingSimulation::new(123);
        world.set_block(IVec3::new(7, 1, 5), BlockId::SAND);
        world.set_block(IVec3::new(7, 2, 5), BlockId::SUGAR_CANE);
        sim.growing_sugar_cane.push(SugarCaneEntry {
            pos: IVec3::new(7, 2, 5),
            timer: 0.0,
            target_secs: 10.0,
        });
        let players = [Vec3::new(7.0, 2.0, 5.0)];
        sim.tick(10.0, 1.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(7, 3, 5)), BlockId::SUGAR_CANE);
        sim.growing_sugar_cane[0].target_secs = 10.0;
        sim.tick(10.0, 1.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(7, 4, 5)), BlockId::SUGAR_CANE);
        // Does not exceed height 3
        sim.growing_sugar_cane[0].target_secs = 10.0;
        sim.tick(10.0, 1.0, &players, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(7, 5, 5)), BlockId::AIR);
    }

    #[test]
    fn test_cooking_pot_sweet_cake_and_herbal_tea_return_empty_buckets() {
        let mut pot = crate::world::CookingPotState::new();
        pot.ingredients = [
            ItemSlot::new(ItemId::MILK_BUCKET, 1),
            ItemSlot::new(ItemId::EGG, 1),
            ItemSlot::new(ItemId::SUGAR, 1),
            ItemSlot::new(ItemId::WHEAT, 1),
        ];
        pot.fuel = ItemSlot::new(ItemId::COAL, 1);

        let lit_changed = pot.update(10.0);
        assert!(lit_changed);
        assert_eq!(pot.output, ItemSlot::new(ItemId::SWEET_CAKE, 1));
        assert_eq!(pot.ingredients[0], ItemSlot::new(ItemId::BUCKET, 1));
        assert!(pot.ingredients[1].is_empty());

        // Now brew Herbal Tea with Water Bucket + Sugar + Leaves
        pot.output = ItemSlot::EMPTY;
        pot.ingredients = [
            ItemSlot::new(ItemId::WATER_BUCKET, 1),
            ItemSlot::new(ItemId::SUGAR, 1),
            ItemSlot::new(ItemId::LEAVES, 1),
            ItemSlot::EMPTY,
        ];
        pot.update(10.0);
        assert_eq!(pot.output, ItemSlot::new(ItemId::HERBAL_TEA, 2));
        assert_eq!(pot.ingredients[0], ItemSlot::new(ItemId::BUCKET, 1));
    }

    #[test]
    fn test_unsafe_food_status_effects_and_herbal_tea_cure() {
        let reg = crate::item::ItemRegistry::new();
        let mut stats = crate::player::PlayerStats::new();
        stats.hunger = 10.0;
        stats.saturation = 0.0;

        // Eating Pufferfish applies Poison
        let puffer = reg.get_food(ItemId::PUFFERFISH).unwrap();
        assert!(stats.consume_food(&puffer, 1.0, 0.0));
        assert!(stats.has_status_effect(crate::item::FoodEffectKind::Poison));

        // Poison ticks non-lethal damage down to 1.0 HP
        stats.health = 2.0;
        stats.update(3.5, 64.0, false);
        assert_eq!(stats.health, 1.0);
        assert!(!stats.is_dead);

        // Eating Rotten Flesh applies Food Poisoning
        let flesh = reg.get_food(ItemId::ROTTEN_FLESH).unwrap();
        assert!(stats.consume_food(&flesh, 1.0, 0.1));
        assert!(stats.has_status_effect(crate::item::FoodEffectKind::FoodPoisoning));

        // Herbal Tea is drinkable even at full hunger and clears all negative effects
        stats.hunger = stats.max_hunger;
        let tea = reg.get_food(ItemId::HERBAL_TEA).unwrap();
        assert!(stats.can_consume(&tea));
        assert!(stats.consume_food(&tea, 1.0, 0.0));
        assert!(stats.status_effects.is_empty());
    }

    #[test]
    fn test_species_breeding_feed_herding_milking_mutton_and_eggs() {
        let (world, reg) = setup_lit_farm_world();
        let mut pig = crate::animal::AnimalEntity::new(1, crate::animal::AnimalKind::Pig, Vec3::new(5.0, 2.0, 5.0));
        assert!(!pig.try_feed(ItemId::WHEAT));
        assert!(pig.try_feed(ItemId::CARROT));

        let mut chicken = crate::animal::AnimalEntity::new(2, crate::animal::AnimalKind::Chicken, Vec3::new(5.0, 2.0, 5.0));
        assert!(!chicken.try_feed(ItemId::CARROT));
        assert!(chicken.try_feed(ItemId::TOMATO_SEEDS));
        chicken.egg_timer = 0.5;

        let cow = crate::animal::AnimalEntity::new(3, crate::animal::AnimalKind::Cow, Vec3::new(5.0, 2.0, 5.0));
        assert!(cow.can_milk(ItemId::BUCKET));
        assert!(!pig.can_milk(ItemId::BUCKET));

        let sheep = crate::animal::AnimalEntity::new(4, crate::animal::AnimalKind::Sheep, Vec3::new(5.0, 2.0, 5.0));
        let sheep_drops = sheep.generate_drops();
        assert!(sheep_drops.iter().any(|d| d.item_id == ItemId::RAW_MUTTON));
        assert!(sheep_drops.iter().any(|d| d.item_id == ItemId::WHITE_WOOL));

        // Tick chicken egg laying via shared tick_animals
        let mut animals = vec![chicken];
        let mut next_id = 10;
        let drops = crate::animal::tick_animals(
            &mut animals,
            &mut next_id,
            1.0,
            &[(Vec3::new(8.0, 2.0, 5.0), ItemId::WHEAT_SEEDS)],
            &world,
            &reg,
        );
        assert!(drops.iter().any(|(_, s)| s.item_id == ItemId::EGG));
    }

    #[test]
    fn test_issue12_network_packets_roundtrip() {
        use crate::net::{ClientPacket, ServerPacket, SyncedAnimalState};
        let c1 = ClientPacket::InteractAnimal {
            animal_id: 7,
            held_item: ItemId::CARROT,
        };
        let enc = c1.encode().unwrap();
        assert_eq!(ClientPacket::decode(&enc[8..]).unwrap(), c1);

        let c2 = ClientPacket::CookingPotUpdate {
            pos: IVec3::new(1, 64, -3),
            ingredients: [
                ItemSlot::new(ItemId::TOMATO, 2),
                ItemSlot::new(ItemId::TOMATO, 1),
                ItemSlot::EMPTY,
                ItemSlot::EMPTY,
            ],
            fuel: ItemSlot::new(ItemId::COAL, 4),
            output: ItemSlot::new(ItemId::TOMATO_SOUP, 1),
            burn_time: 12.5,
            max_burn_time: 80.0,
            cook_progress: 3.25,
        };
        let enc2 = c2.encode().unwrap();
        assert_eq!(ClientPacket::decode(&enc2[8..]).unwrap(), c2);

        let s1 = ServerPacket::AnimalsSync {
            animals: vec![SyncedAnimalState {
                id: 3,
                kind: crate::animal::AnimalKind::Cow,
                position: Vec3::new(1.0, 64.0, 2.0),
                velocity: Vec3::ZERO,
                yaw: 1.5,
                health: 10.0,
                max_health: 10.0,
                hurt_timer: 0.0,
                love_timer: 15.0,
                breed_cooldown: 0.0,
                baby_timer: 0.0,
                is_moving: true,
                walk_anim_timer: 0.5,
            }],
        };
        let enc_s1 = s1.encode().unwrap();
        assert_eq!(ServerPacket::decode(&enc_s1[8..]).unwrap(), s1);
    }
}

