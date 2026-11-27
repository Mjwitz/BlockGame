use std::collections::{HashSet, VecDeque};

use glam::{IVec3, Vec3};

use crate::{
    block::{BlockId, BlockRegistry},
    item::ItemSlot,
    world::World,
};

/// Fluid metadata level for a stationary full-height Source Block (scoopable with a Bucket).
pub const FLUID_SOURCE: u8 = 0;
/// Fluid metadata level for a vertically falling liquid stream.
pub const FLUID_FALLING: u8 = 8;
/// Maximum horizontal spread distance (in blocks) for Water.
pub const WATER_MAX_SPREAD: u8 = 7;
/// Maximum horizontal spread distance (in blocks) for Lava.
pub const LAVA_MAX_SPREAD: u8 = 3;
/// Tick interval in seconds for Water flow updates (~4 ticks/sec).
pub const WATER_TICK_INTERVAL: f32 = 0.25;
/// Tick interval in seconds for Lava flow updates (~1.33 ticks/sec).
pub const LAVA_TICK_INTERVAL: f32 = 0.75;

const HORIZ_DIRS: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
const ALL_DIRS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Z,
    IVec3::NEG_Z,
    IVec3::Y,
    IVec3::NEG_Y,
];

/// Returns the visual surface height (`0.0625..=1.0`) for a Water or Lava voxel given its fluid level.
/// Source Block (`0`) is `0.875`, stepping down `1/8` per decay level (`1 -> 0.75`, ..., `6 -> 0.125`, `7 -> 0.0625`),
/// and `1.0` when falling (`8`) or submerged beneath the same fluid.
#[inline]
pub fn fluid_surface_height(level: u8, above_same_fluid: bool) -> f32 {
    if above_same_fluid || level >= FLUID_FALLING {
        1.0
    } else if level == FLUID_SOURCE {
        0.875
    } else {
        let clamped = level.min(7);
        ((7 - clamped) as f32 / 8.0).max(0.0625)
    }
}

/// Computes a normalized horizontal flow push vector for a Water voxel at `pos`
/// based on the local flow-level gradient (returns `Vec3::ZERO` in still Source Pools).
pub fn fluid_flow_vector(pos: IVec3, world: &World) -> Vec3 {
    let here_block = world.get_block(pos);
    if !here_block.is_water() {
        return Vec3::ZERO;
    }
    let here_lvl = world.get_fluid_level(pos);
    let eff_height = |lvl: u8| -> i32 {
        if lvl == FLUID_SOURCE || lvl == FLUID_FALLING {
            8
        } else {
            (8 - lvl.min(7) as i32).max(1)
        }
    };
    let h0 = eff_height(here_lvl);
    let mut flow = Vec3::ZERO;
    for dir in HORIZ_DIRS {
        let np = pos + dir;
        let nb = world.get_block(np);
        if nb.is_water() {
            let hn = eff_height(world.get_fluid_level(np));
            let diff = h0 - hn;
            if diff != 0 {
                flow += dir.as_vec3() * (diff as f32);
            }
        } else if nb.is_air() && here_lvl != FLUID_SOURCE {
            flow += dir.as_vec3();
        }
    }
    if flow.length_squared() > 1e-5 {
        flow.normalize()
    } else {
        Vec3::ZERO
    }
}

/// Scheduled cellular fluid simulator driving Minecraft-style Water and Lava physics (ADR 0010).
#[derive(Debug, Default, Clone)]
pub struct FluidSimulation {
    water_queue: VecDeque<IVec3>,
    water_queued: HashSet<IVec3>,
    water_accum: f32,
    lava_queue: VecDeque<IVec3>,
    lava_queued: HashSet<IVec3>,
    lava_accum: f32,
}

impl FluidSimulation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_pending(&self) -> bool {
        !self.water_queue.is_empty() || !self.lava_queue.is_empty()
    }

    #[inline]
    fn enqueue_water(&mut self, pos: IVec3) {
        if pos.y >= 0 && self.water_queued.insert(pos) {
            self.water_queue.push_back(pos);
        }
    }

    #[inline]
    fn enqueue_lava(&mut self, pos: IVec3) {
        if pos.y >= 0 && self.lava_queued.insert(pos) {
            self.lava_queue.push_back(pos);
        }
    }

    #[inline]
    fn enqueue_fluid(&mut self, pos: IVec3, is_lava: bool) {
        if is_lava {
            self.enqueue_lava(pos);
        } else {
            self.enqueue_water(pos);
        }
    }

    /// Solidifies Lava at `pos` into Obsidian (if Source Block) or Cobblestone (if Flowing Fluid).
    fn solidify_lava_at(&mut self, pos: IVec3, world: &mut World, modified: &mut Vec<IVec3>) {
        let solid = if world.get_fluid_level(pos) == FLUID_SOURCE {
            BlockId::OBSIDIAN
        } else {
            BlockId::COBBLESTONE
        };
        world.set_block(pos, solid);
        world.set_fluid_level(pos, 0);
        modified.push(pos);
        self.schedule_block_and_neighbors(pos, world);
    }

    /// Schedules `pos` and its 6 neighbors for fluid evaluation whenever a block changes.
    pub fn schedule_block_and_neighbors(&mut self, pos: IVec3, world: &World) {
        let mut check_and_queue = |p: IVec3| {
            let b = world.get_block(p);
            if b == BlockId::WATER {
                self.enqueue_water(p);
            } else if b == BlockId::LAVA {
                self.enqueue_lava(p);
            }
        };
        check_and_queue(pos);
        for d in ALL_DIRS {
            check_and_queue(pos + d);
        }
    }

    /// Advances scheduled Water (~4 ticks/s) and Lava (~1.33 ticks/s) waves.
    /// Returns `(modified_positions, washed_item_drops)`.
    pub fn tick(
        &mut self,
        dt: f32,
        world: &mut World,
        registry: &BlockRegistry,
    ) -> (Vec<IVec3>, Vec<(IVec3, ItemSlot)>) {
        let mut modified = Vec::new();
        let mut drops = Vec::new();

        self.water_accum += dt;
        if self.water_accum >= WATER_TICK_INTERVAL {
            self.water_accum = (self.water_accum - WATER_TICK_INTERVAL).min(WATER_TICK_INTERVAL);
            let wave = self.water_queue.len().min(256);
            for _ in 0..wave {
                if let Some(pos) = self.water_queue.pop_front() {
                    self.water_queued.remove(&pos);
                    self.step_fluid(pos, false, world, registry, &mut modified, &mut drops);
                }
            }
        }

        self.lava_accum += dt;
        if self.lava_accum >= LAVA_TICK_INTERVAL {
            self.lava_accum = (self.lava_accum - LAVA_TICK_INTERVAL).min(LAVA_TICK_INTERVAL);
            let wave = self.lava_queue.len().min(128);
            for _ in 0..wave {
                if let Some(pos) = self.lava_queue.pop_front() {
                    self.lava_queued.remove(&pos);
                    self.step_fluid(pos, true, world, registry, &mut modified, &mut drops);
                }
            }
        }

        (modified, drops)
    }

    fn step_fluid(
        &mut self,
        pos: IVec3,
        is_lava: bool,
        world: &mut World,
        registry: &BlockRegistry,
        modified: &mut Vec<IVec3>,
        drops: &mut Vec<(IVec3, ItemSlot)>,
    ) {
        let fluid_id = if is_lava { BlockId::LAVA } else { BlockId::WATER };
        let max_spread = if is_lava {
            LAVA_MAX_SPREAD
        } else {
            WATER_MAX_SPREAD
        };

        if world.get_block(pos) != fluid_id {
            return;
        }
        let mut level = world.get_fluid_level(pos);

        // 1. Water + Lava solidification reactions
        if !is_lava {
            // Water touching Lava horizontally or below: Lava Source -> Obsidian, Flowing Lava -> Cobblestone.
            // (Lava directly above Water is handled by Lava flowing downward -> Stone.)
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y] {
                let np = pos + d;
                if world.get_block(np) == BlockId::LAVA {
                    self.solidify_lava_at(np, world, modified);
                }
            }
        } else if HORIZ_DIRS
            .iter()
            .any(|&d| world.get_block(pos + d).is_water())
        {
            // Lava horizontally touching Water solidifies itself (Source -> Obsidian, Flowing -> Cobblestone)
            self.solidify_lava_at(pos, world, modified);
            return;
        }

        // 2. Infinite 2x2 Water Source formation (only for non-source Water over solid or source Water)
        if !is_lava && level != FLUID_SOURCE {
            let source_neighbors = HORIZ_DIRS
                .iter()
                .filter(|&&d| {
                    let np = pos + d;
                    world.get_block(np) == BlockId::WATER
                        && world.get_fluid_level(np) == FLUID_SOURCE
                })
                .count();
            let below_pos = pos - IVec3::Y;
            let below_b = world.get_block(below_pos);
            let supported = registry.is_solid(below_b)
                || (below_b == BlockId::WATER && world.get_fluid_level(below_pos) == FLUID_SOURCE);
            if source_neighbors >= 2 && supported {
                level = FLUID_SOURCE;
                world.set_fluid_level(pos, FLUID_SOURCE);
                modified.push(pos);
                self.schedule_block_and_neighbors(pos, world);
            }
        }

        // 3. Draining / Decay check for non-source Flowing Fluid
        if level != FLUID_SOURCE {
            let above_same = world.get_block(pos + IVec3::Y) == fluid_id;
            let best_level = if above_same {
                Some(FLUID_FALLING)
            } else {
                let mut min_h: Option<u8> = None;
                for d in HORIZ_DIRS {
                    let np = pos + d;
                    if world.get_block(np) == fluid_id {
                        let nl = world.get_fluid_level(np);
                        let eff = if nl == FLUID_FALLING { 0 } else { nl };
                        min_h = Some(min_h.map_or(eff, |m| m.min(eff)));
                    }
                }
                min_h.and_then(|m| {
                    if m < max_spread {
                        Some(m + 1)
                    } else {
                        None
                    }
                })
            };

            match best_level {
                None => {
                    // Cut off from any source: recede/drain
                    if level == FLUID_FALLING || level >= max_spread {
                        world.set_block(pos, BlockId::AIR);
                        world.set_fluid_level(pos, 0);
                        modified.push(pos);
                        self.schedule_block_and_neighbors(pos, world);
                        return;
                    } else {
                        let next_decay = level + 1;
                        world.set_fluid_level(pos, next_decay);
                        modified.push(pos);
                        self.enqueue_fluid(pos, is_lava);
                        self.schedule_block_and_neighbors(pos, world);
                        return;
                    }
                }
                Some(target) if target != level => {
                    if level != FLUID_FALLING && target > level {
                        // Receding toward higher decay level
                        world.set_fluid_level(pos, target);
                        modified.push(pos);
                        self.schedule_block_and_neighbors(pos, world);
                        return;
                    } else {
                        level = target;
                        world.set_fluid_level(pos, level);
                        modified.push(pos);
                    }
                }
                _ => {}
            }
        }

        // 4. Downward flow
        let below_pos = pos - IVec3::Y;
        if below_pos.y >= 0 {
            let below_b = world.get_block(below_pos);
            if is_lava && below_b.is_water() {
                // Lava flowing down onto Water creates Stone
                world.set_block(below_pos, BlockId::STONE);
                world.set_fluid_level(below_pos, 0);
                modified.push(below_pos);
                self.schedule_block_and_neighbors(below_pos, world);
                return;
            } else if !is_lava && below_b == BlockId::LAVA {
                self.solidify_lava_at(below_pos, world, modified);
                return;
            } else if can_flow_into(below_b) {
                if !is_lava && below_b.is_fragile() {
                    if let Some(item_id) = registry.get(below_b).and_then(|def| def.drop_item) {
                        drops.push((below_pos, ItemSlot::new(item_id, 1)));
                    }
                }
                world.set_block(below_pos, fluid_id);
                world.set_fluid_level(below_pos, FLUID_FALLING);
                modified.push(below_pos);
                self.enqueue_fluid(below_pos, is_lava);
                return;
            } else if below_b == fluid_id {
                let below_lvl = world.get_fluid_level(below_pos);
                if below_lvl != FLUID_SOURCE && below_lvl != FLUID_FALLING {
                    world.set_fluid_level(below_pos, FLUID_FALLING);
                    modified.push(below_pos);
                    self.enqueue_fluid(below_pos, is_lava);
                }
                // Falling fluid hitting same fluid below only spreads horizontally if below is a Source Block
                if below_lvl != FLUID_SOURCE {
                    return;
                }
            }
        }

        // 5. Horizontal spread (seeking downhill drop-offs within 4 blocks)
        let next_level = if level == FLUID_SOURCE || level == FLUID_FALLING {
            1
        } else if level < max_spread {
            level + 1
        } else {
            return;
        };

        let spread_dirs = find_downhill_dirs(pos, fluid_id, world, registry);
        for d in spread_dirs {
            let np = pos + d;
            let nb = world.get_block(np);
            if !is_lava && nb == BlockId::LAVA {
                self.solidify_lava_at(np, world, modified);
            } else if is_lava && nb.is_water() {
                world.set_block(np, BlockId::COBBLESTONE);
                world.set_fluid_level(np, 0);
                modified.push(np);
                self.schedule_block_and_neighbors(np, world);
            } else if can_flow_into(nb) {
                if !is_lava && nb.is_fragile() {
                    if let Some(item_id) = registry.get(nb).and_then(|def| def.drop_item) {
                        drops.push((np, ItemSlot::new(item_id, 1)));
                    }
                }
                world.set_block(np, fluid_id);
                world.set_fluid_level(np, next_level);
                modified.push(np);
                self.enqueue_fluid(np, is_lava);
            } else if nb == fluid_id {
                let nl = world.get_fluid_level(np);
                if nl != FLUID_SOURCE && nl != FLUID_FALLING && nl > next_level {
                    world.set_fluid_level(np, next_level);
                    modified.push(np);
                    self.enqueue_fluid(np, is_lava);
                }
            }
        }
    }
}

#[inline]
fn can_flow_into(b: BlockId) -> bool {
    b.is_air() || b.is_fragile()
}

fn is_loaded_drop(pos: IVec3, world: &World, registry: &BlockRegistry) -> bool {
    if pos.y <= 0 {
        return false;
    }
    let below = pos - IVec3::Y;
    let (cp, _, _, _) = crate::world::world_to_chunk_coords(below);
    world.has_chunk(cp) && !registry.is_solid(world.get_block(below))
}

/// BFS up to 4 blocks horizontally to find the shortest path(s) to a downhill drop-off.
/// Returns the optimal horizontal directions (or all 4 directions if no drop exists within 4 blocks).
fn find_downhill_dirs(
    origin: IVec3,
    fluid_id: BlockId,
    world: &World,
    registry: &BlockRegistry,
) -> Vec<IVec3> {
    let mut best_dist = 5u8;
    let mut dir_scores = [5u8; 4];

    for (i, &start_dir) in HORIZ_DIRS.iter().enumerate() {
        let start = origin + start_dir;
        let (scp, _, _, _) = crate::world::world_to_chunk_coords(start);
        if !world.has_chunk(scp) {
            continue;
        }
        let sb = world.get_block(start);
        if !(can_flow_into(sb) || sb == fluid_id) {
            continue;
        }
        // Check immediate drop at distance 1
        if is_loaded_drop(start, world, registry) {
            dir_scores[i] = 1;
            best_dist = best_dist.min(1);
            continue;
        }
        // Bounded BFS up to distance 4
        let mut q = VecDeque::new();
        let mut visited = [IVec3::ZERO; 32];
        let mut vcount = 0usize;
        q.push_back((start, 1u8));
        visited[vcount] = start;
        vcount += 1;

        while let Some((cur, dist)) = q.pop_front() {
            if dist >= best_dist || dist >= 4 {
                continue;
            }
            for &d in &HORIZ_DIRS {
                let np = cur + d;
                if np == origin || visited[..vcount].contains(&np) {
                    continue;
                }
                let (ncp, _, _, _) = crate::world::world_to_chunk_coords(np);
                if !world.has_chunk(ncp) {
                    continue;
                }
                let nb = world.get_block(np);
                if !(can_flow_into(nb) || nb == fluid_id) {
                    continue;
                }
                let nd = dist + 1;
                if is_loaded_drop(np, world, registry) {
                    dir_scores[i] = dir_scores[i].min(nd);
                    best_dist = best_dist.min(nd);
                    break;
                }
                if vcount < visited.len() {
                    visited[vcount] = np;
                    vcount += 1;
                    q.push_back((np, nd));
                }
            }
        }
    }

    if best_dist <= 4 {
        HORIZ_DIRS
            .iter()
            .enumerate()
            .filter(|&(i, _)| dir_scores[i] == best_dist)
            .map(|(_, &d)| d)
            .collect()
    } else {
        HORIZ_DIRS.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, ChunkPos};

    fn flat_stone_world() -> World {
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));
        world.insert_chunk(Chunk::new(ChunkPos::new(1, 0, 0)));
        for x in 0..32 {
            for z in 0..16 {
                world.set_block(IVec3::new(x, 4, z), BlockId::STONE);
            }
        }
        world
    }

    #[test]
    fn test_water_spreads_7_blocks_and_drains_when_source_removed() {
        let reg = BlockRegistry::new();
        let mut world = flat_stone_world();
        let mut sim = FluidSimulation::new();

        let src = IVec3::new(8, 5, 8);
        world.set_block(src, BlockId::WATER);
        world.set_fluid_level(src, FLUID_SOURCE);
        sim.schedule_block_and_neighbors(src, &world);

        for _ in 0..12 {
            sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        }

        // Spreads 7 blocks horizontally from source (x=8..=15) and stops at x=16
        assert_eq!(world.get_block(IVec3::new(15, 5, 8)), BlockId::WATER);
        assert_eq!(world.get_fluid_level(IVec3::new(15, 5, 8)), 7);
        assert_eq!(world.get_block(IVec3::new(16, 5, 8)), BlockId::AIR);

        // Flow vector pushes outward (+X) in flowing water
        let fv = fluid_flow_vector(IVec3::new(10, 5, 8), &world);
        assert!(fv.x > 0.5, "Expected +X flow vector, got {:?}", fv);

        // Removing the Source Block drains downstream flowing water
        world.set_block(src, BlockId::AIR);
        world.set_fluid_level(src, 0);
        sim.schedule_block_and_neighbors(src, &world);
        for _ in 0..24 {
            sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        }
        assert_eq!(world.get_block(IVec3::new(10, 5, 8)), BlockId::AIR);
        assert_eq!(world.get_block(IVec3::new(15, 5, 8)), BlockId::AIR);
    }

    #[test]
    fn test_lava_spreads_3_blocks_slower_and_no_infinite_source() {
        let reg = BlockRegistry::new();
        let mut world = flat_stone_world();
        let mut sim = FluidSimulation::new();

        let src = IVec3::new(8, 5, 8);
        world.set_block(src, BlockId::LAVA);
        world.set_fluid_level(src, FLUID_SOURCE);
        sim.schedule_block_and_neighbors(src, &world);

        for _ in 0..8 {
            sim.tick(LAVA_TICK_INTERVAL, &mut world, &reg);
        }

        assert_eq!(world.get_block(IVec3::new(11, 5, 8)), BlockId::LAVA);
        assert_eq!(world.get_fluid_level(IVec3::new(11, 5, 8)), 3);
        assert_eq!(world.get_block(IVec3::new(12, 5, 8)), BlockId::AIR);

        // Two adjacent Lava sources do NOT form an infinite source
        world.set_block(IVec3::new(2, 5, 2), BlockId::LAVA);
        world.set_fluid_level(IVec3::new(2, 5, 2), FLUID_SOURCE);
        world.set_block(IVec3::new(4, 5, 2), BlockId::LAVA);
        world.set_fluid_level(IVec3::new(4, 5, 2), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(3, 5, 2), &world);
        for _ in 0..4 {
            sim.tick(LAVA_TICK_INTERVAL, &mut world, &reg);
        }
        assert_eq!(world.get_block(IVec3::new(3, 5, 2)), BlockId::LAVA);
        assert_ne!(
            world.get_fluid_level(IVec3::new(3, 5, 2)),
            FLUID_SOURCE,
            "Lava must NOT form infinite sources"
        );
    }

    #[test]
    fn test_infinite_2x2_water_source_and_downhill_pathing() {
        let reg = BlockRegistry::new();
        let mut world = flat_stone_world();
        let mut sim = FluidSimulation::new();

        // 2x2 infinite water source: two diagonal sources promote the empty corner to a Source Block
        world.set_block(IVec3::new(2, 5, 2), BlockId::WATER);
        world.set_fluid_level(IVec3::new(2, 5, 2), FLUID_SOURCE);
        world.set_block(IVec3::new(4, 5, 2), BlockId::WATER);
        world.set_fluid_level(IVec3::new(4, 5, 2), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(3, 5, 2), &world);
        for _ in 0..4 {
            sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        }
        assert_eq!(world.get_block(IVec3::new(3, 5, 2)), BlockId::WATER);
        assert_eq!(
            world.get_fluid_level(IVec3::new(3, 5, 2)),
            FLUID_SOURCE,
            "Two horizontally adjacent Water Source Blocks must create a new Source Block"
        );

        // Downhill pathing: punch a hole 2 blocks east of a Water source -> flows east into the drop
        world.set_block(IVec3::new(12, 4, 8), BlockId::AIR);
        world.set_block(IVec3::new(10, 5, 8), BlockId::WATER);
        world.set_fluid_level(IVec3::new(10, 5, 8), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(10, 5, 8), &world);
        for _ in 0..6 {
            sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        }
        assert_eq!(world.get_block(IVec3::new(12, 4, 8)), BlockId::WATER);
        assert_eq!(world.get_fluid_level(IVec3::new(12, 4, 8)), FLUID_FALLING);
        assert_eq!(
            world.get_block(IVec3::new(9, 5, 8)),
            BlockId::AIR,
            "Water should prioritize the nearby downhill drop instead of spreading opposite"
        );
    }

    #[test]
    fn test_three_way_water_lava_reactions() {
        let reg = BlockRegistry::new();
        let mut world = flat_stone_world();
        let mut sim = FluidSimulation::new();

        // 1. Water touching Lava Source -> Obsidian
        world.set_block(IVec3::new(2, 5, 8), BlockId::LAVA);
        world.set_fluid_level(IVec3::new(2, 5, 8), FLUID_SOURCE);
        world.set_block(IVec3::new(3, 5, 8), BlockId::WATER);
        world.set_fluid_level(IVec3::new(3, 5, 8), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(3, 5, 8), &world);
        sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(2, 5, 8)), BlockId::OBSIDIAN);

        // 2. Water touching Flowing Lava -> Cobblestone
        world.set_block(IVec3::new(6, 5, 8), BlockId::LAVA);
        world.set_fluid_level(IVec3::new(6, 5, 8), 2);
        world.set_block(IVec3::new(7, 5, 8), BlockId::WATER);
        world.set_fluid_level(IVec3::new(7, 5, 8), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(7, 5, 8), &world);
        sim.tick(WATER_TICK_INTERVAL, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(6, 5, 8)), BlockId::COBBLESTONE);

        // 3. Lava flowing down onto Water -> Stone
        world.set_block(IVec3::new(10, 5, 8), BlockId::WATER);
        world.set_fluid_level(IVec3::new(10, 5, 8), FLUID_SOURCE);
        world.set_block(IVec3::new(10, 6, 8), BlockId::LAVA);
        world.set_fluid_level(IVec3::new(10, 6, 8), FLUID_SOURCE);
        sim.schedule_block_and_neighbors(IVec3::new(10, 6, 8), &world);
        sim.tick(LAVA_TICK_INTERVAL, &mut world, &reg);
        assert_eq!(world.get_block(IVec3::new(10, 5, 8)), BlockId::STONE);
    }
}
