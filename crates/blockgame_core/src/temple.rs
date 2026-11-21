use std::collections::HashMap;

use glam::{IVec3, Vec3};

use crate::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    item::{ItemId, ItemSlot},
    monster::{MonsterEntity, MonsterKind},
    world::{world_to_chunk_coords, World},
};

/// Coordinates with `x >= ARENA_BAND_START_X` are reserved for instanced boss arenas.
/// Overworld terrain generation and normal night-monster spawning are suppressed here.
pub const ARENA_BAND_START_X: i32 = 150_000;
pub const ARENA_ORIGIN_BASE_X: i32 = 200_000;
pub const ARENA_FLOOR_Y: i32 = 32;
pub const ARENA_HALF_SIZE: i32 = 18;
pub const ARENA_HEIGHT: i32 = 18;

/// Coarse overworld grid spacing (in blocks) for deterministic boss temple placement.
pub const TEMPLE_GRID_SIZE: i32 = 320;

/// Maximum distance (in blocks) at which an in-combat boss health bar is displayed (Issue #9).
pub const BOSS_HUD_MAX_DISTANCE: f32 = 500.0;

/// Cooldown (seconds) before a defeated boss can be re-summoned via the Boss Altar.
pub const BOSS_RECHALLENGE_COOLDOWN_SECS: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TempleKind {
    Desert,
    Glacier,
    Jungle,
    DarkForest,
}

impl TempleKind {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Desert => 0,
            Self::Glacier => 1,
            Self::Jungle => 2,
            Self::DarkForest => 3,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Desert),
            1 => Some(Self::Glacier),
            2 => Some(Self::Jungle),
            3 => Some(Self::DarkForest),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Desert => "Pyramid of the Blazing Sun",
            Self::Glacier => "Citadel of Eternal Frost",
            Self::Jungle => "Ziggurat of the Wilds",
            Self::DarkForest => "Cathedral of Whispers",
        }
    }

    pub fn boss_kind(self) -> MonsterKind {
        match self {
            Self::Desert => MonsterKind::SunPharaoh,
            Self::Glacier => MonsterKind::FrostTitan,
            Self::Jungle => MonsterKind::OvergrownGolem,
            Self::DarkForest => MonsterKind::ShadowLich,
        }
    }

    pub fn minion_kinds(self) -> (MonsterKind, MonsterKind) {
        match self {
            Self::Desert => (MonsterKind::SandMummy, MonsterKind::SunArcher),
            Self::Glacier => (MonsterKind::FrostWraith, MonsterKind::FrostWraith),
            Self::Jungle => (MonsterKind::JungleStalker, MonsterKind::JungleStalker),
            Self::DarkForest => (MonsterKind::ShadowKnight, MonsterKind::ShadowKnight),
        }
    }

    pub fn primary_block(self) -> BlockId {
        match self {
            Self::Desert => BlockId::TEMPLE_SANDSTONE,
            Self::Glacier => BlockId::TEMPLE_ICE_BRICK,
            Self::Jungle => BlockId::TEMPLE_MOSS_BRICK,
            Self::DarkForest => BlockId::TEMPLE_OBSIDIAN,
        }
    }

    pub fn accent_block(self) -> BlockId {
        match self {
            Self::Desert => BlockId::GOLD_ORE,
            Self::Glacier => BlockId::SNOW_BLOCK,
            Self::Jungle => BlockId::JUNGLE_LOG,
            Self::DarkForest => BlockId::DARK_OAK_LOG,
        }
    }

    pub fn floor_block(self) -> BlockId {
        match self {
            Self::Desert => BlockId::SANDSTONE,
            Self::Glacier => BlockId::ICE,
            Self::Jungle => BlockId::TEMPLE_STONE,
            Self::DarkForest => BlockId::TEMPLE_STONE,
        }
    }

    pub fn treasure_loot(self) -> Vec<ItemSlot> {
        match self {
            Self::Desert => vec![
                ItemSlot::new(ItemId::GOLD_INGOT, 16),
                ItemSlot::new(ItemId::DIAMOND, 4),
                ItemSlot::new(ItemId::ARROW, 32),
                ItemSlot::new(ItemId::BREAD, 8),
                ItemSlot::new(ItemId::TEMPLE_SANDSTONE, 24),
            ],
            Self::Glacier => vec![
                ItemSlot::new(ItemId::DIAMOND, 6),
                ItemSlot::new(ItemId::IRON_INGOT, 16),
                ItemSlot::new(ItemId::ARROW, 32),
                ItemSlot::new(ItemId::COOKED_PORKCHOP, 8),
                ItemSlot::new(ItemId::TEMPLE_ICE_BRICK, 24),
            ],
            Self::Jungle => vec![
                ItemSlot::new(ItemId::DIAMOND, 5),
                ItemSlot::new(ItemId::GOLD_INGOT, 12),
                ItemSlot::new(ItemId::IRON_INGOT, 16),
                ItemSlot::new(ItemId::COOKED_SALMON, 8),
                ItemSlot::new(ItemId::TEMPLE_MOSS_BRICK, 24),
            ],
            Self::DarkForest => vec![
                ItemSlot::new(ItemId::DIAMOND, 8),
                ItemSlot::new(ItemId::GOLD_INGOT, 14),
                ItemSlot::new(ItemId::ARROW, 48),
                ItemSlot::new(ItemId::COOKED_PORKCHOP, 10),
                ItemSlot::new(ItemId::TEMPLE_OBSIDIAN, 16),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempleBounds {
    pub min: IVec3,
    pub max: IVec3,
}

impl TempleBounds {
    #[inline]
    pub fn contains(&self, pos: IVec3) -> bool {
        pos.x >= self.min.x
            && pos.x <= self.max.x
            && pos.y >= self.min.y
            && pos.y <= self.max.y
            && pos.z >= self.min.z
            && pos.z <= self.max.z
    }
}

#[derive(Debug, Clone)]
pub struct TempleInstanceState {
    pub temple_id: u64,
    pub kind: TempleKind,
    pub overworld_origin: IVec3,
    pub overworld_portal_pos: IVec3,
    pub arena_origin: IVec3,
    pub arena_built: bool,
    pub boss_defeated: bool,
    pub active_boss_id: Option<u32>,
    pub rechallenge_cooldown: f32,
    pub overworld_bounds: TempleBounds,
    pub arena_bounds: TempleBounds,
}

impl TempleInstanceState {
    pub fn new(
        temple_id: u64,
        kind: TempleKind,
        overworld_origin: IVec3,
        overworld_portal_pos: IVec3,
    ) -> Self {
        let arena_origin = arena_origin_for_temple(temple_id, kind);
        let overworld_bounds = TempleBounds {
            min: overworld_origin + IVec3::new(-12, -2, -12),
            max: overworld_origin + IVec3::new(12, 16, 12),
        };
        let arena_bounds = TempleBounds {
            min: arena_origin + IVec3::new(-ARENA_HALF_SIZE - 1, -1, -ARENA_HALF_SIZE - 1),
            max: arena_origin + IVec3::new(ARENA_HALF_SIZE + 1, ARENA_HEIGHT + 1, ARENA_HALF_SIZE + 1),
        };
        Self {
            temple_id,
            kind,
            overworld_origin,
            overworld_portal_pos,
            arena_origin,
            arena_built: false,
            boss_defeated: false,
            active_boss_id: None,
            rechallenge_cooldown: 0.0,
            overworld_bounds,
            arena_bounds,
        }
    }

    /// Spawn point for players entering this temple's instanced arena.
    pub fn arena_entry_spawn(&self) -> Vec3 {
        Vec3::new(
            self.arena_origin.x as f32 + 0.5,
            (self.arena_origin.y + 1) as f32,
            (self.arena_origin.z - ARENA_HALF_SIZE + 4) as f32 + 0.5,
        )
    }

    /// Return point in the overworld right in front of the temple portal.
    pub fn overworld_return_spawn(&self) -> Vec3 {
        Vec3::new(
            self.overworld_portal_pos.x as f32 + 0.5,
            (self.overworld_portal_pos.y + 1) as f32,
            (self.overworld_portal_pos.z - 2) as f32 + 0.5,
        )
    }

    /// Position of the Boss Summoning Altar in the center of the arena.
    pub fn altar_pos(&self) -> IVec3 {
        self.arena_origin + IVec3::new(0, 1, 0)
    }

    /// Position of the Return Portal in the arena.
    pub fn return_portal_pos(&self) -> IVec3 {
        self.arena_origin + IVec3::new(0, 1, -ARENA_HALF_SIZE + 2)
    }

    /// Position of the Boss Treasure Chest in the arena.
    pub fn treasure_chest_pos(&self) -> IVec3 {
        self.arena_origin + IVec3::new(0, 1, ARENA_HALF_SIZE - 3)
    }

    /// Whether `pos` lies on the outer structural shell of the arena (always unbreakable).
    pub fn is_arena_shell(&self, pos: IVec3) -> bool {
        if !self.arena_bounds.contains(pos) {
            return false;
        }
        let dx = (pos.x - self.arena_origin.x).abs();
        let dy = pos.y - self.arena_origin.y;
        let dz = (pos.z - self.arena_origin.z).abs();
        dx >= ARENA_HALF_SIZE || dz >= ARENA_HALF_SIZE || dy <= 0 || dy >= ARENA_HEIGHT
    }
}

/// Top-of-screen boss health bar state (Issue #9).
#[derive(Debug, Clone, PartialEq)]
pub struct BossHudState {
    pub name: String,
    pub kind: MonsterKind,
    pub health: f32,
    pub max_health: f32,
    pub phase: u8,
    pub distance: f32,
}

/// Manages all discovered/active boss temples, instanced arenas, and block protection.
#[derive(Debug, Clone, Default)]
pub struct TempleManager {
    pub temples: HashMap<u64, TempleInstanceState>,
    /// Saved overworld return coordinates per player name (`"local"` in singleplayer).
    pub player_return_positions: HashMap<String, Vec3>,
    /// Additional protected bounding boxes synced from server to multiplayer clients.
    pub synced_protected_zones: Vec<TempleBounds>,
}

impl TempleManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_temple(
        &mut self,
        temple_id: u64,
        kind: TempleKind,
        overworld_origin: IVec3,
        overworld_portal_pos: IVec3,
    ) -> &mut TempleInstanceState {
        self.temples.entry(temple_id).or_insert_with(|| {
            TempleInstanceState::new(temple_id, kind, overworld_origin, overworld_portal_pos)
        })
    }

    pub fn find_temple_by_portal(&self, portal_pos: IVec3) -> Option<u64> {
        self.temples
            .values()
            .find(|t| {
                let d = (t.overworld_portal_pos - portal_pos).abs();
                d.x <= 2 && d.y <= 3 && d.z <= 2
            })
            .map(|t| t.temple_id)
    }

    pub fn find_temple_by_arena_pos(&self, pos: IVec3) -> Option<u64> {
        self.temples
            .values()
            .find(|t| t.arena_bounds.contains(pos))
            .map(|t| t.temple_id)
    }

    /// Returns `true` if breaking or placing at `pos` is forbidden due to temple/arena protection.
    pub fn is_block_protected(&self, pos: IVec3, existing_block: BlockId) -> bool {
        if existing_block.is_temple_interactive() {
            return true;
        }
        for zone in &self.synced_protected_zones {
            if zone.contains(pos) {
                return true;
            }
        }
        for temple in self.temples.values() {
            if temple.arena_bounds.contains(pos) {
                if !temple.boss_defeated || temple.is_arena_shell(pos) {
                    return true;
                }
            }
            if !temple.boss_defeated && temple.overworld_bounds.contains(pos) {
                return true;
            }
        }
        false
    }

    /// Returns `true` if opening or looting a chest at `pos` is forbidden because the temple's boss is still alive.
    pub fn is_chest_locked(&self, pos: IVec3) -> bool {
        for zone in &self.synced_protected_zones {
            if zone.contains(pos) {
                return true;
            }
        }
        for temple in self.temples.values() {
            if !temple.boss_defeated
                && (temple.arena_bounds.contains(pos) || temple.overworld_bounds.contains(pos))
            {
                return true;
            }
        }
        false
    }

    /// Returns all currently active protected bounding zones (for syncing to MP clients).
    pub fn active_protected_zones(&self) -> Vec<TempleBounds> {
        let mut zones = Vec::new();
        for temple in self.temples.values() {
            if !temple.boss_defeated {
                zones.push(temple.overworld_bounds);
                zones.push(temple.arena_bounds);
            }
        }
        zones
    }

    /// Advance re-challenge cooldown timers.
    pub fn tick_cooldowns(&mut self, dt: f32) {
        for temple in self.temples.values_mut() {
            if temple.rechallenge_cooldown > 0.0 {
                temple.rechallenge_cooldown = (temple.rechallenge_cooldown - dt).max(0.0);
            }
        }
    }

    /// Marks the temple's boss as defeated, unlocking block protection and starting altar cooldown.
    pub fn mark_boss_defeated(&mut self, temple_id: u64) -> Option<TempleKind> {
        let temple = self.temples.get_mut(&temple_id)?;
        temple.boss_defeated = true;
        temple.active_boss_id = None;
        temple.rechallenge_cooldown = BOSS_RECHALLENGE_COOLDOWN_SECS;
        Some(temple.kind)
    }
}

/// Deterministic unique temple ID from coarse grid coordinates `(gx, gz)` and world `seed`.
pub fn temple_id_from_grid(seed: u32, gx: i32, gz: i32) -> u64 {
    let mut h = (seed as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ ((gx as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        ^ ((gz as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    if h == 0 { 1 } else { h }
}

/// Deterministic arena origin inside the reserved `X >= 200_000` coordinate band.
pub fn arena_origin_for_temple(temple_id: u64, kind: TempleKind) -> IVec3 {
    let slot_x = ((temple_id & 0x3FF) as i32) * 256;
    let slot_z = ((((temple_id >> 10) & 0x3FF) as i32) - 512) * 256 + (kind.to_u8() as i32) * 64;
    IVec3::new(ARENA_ORIGIN_BASE_X + slot_x, ARENA_FLOOR_Y, slot_z)
}

#[inline]
pub fn is_in_arena_band(pos: Vec3) -> bool {
    pos.x >= ARENA_BAND_START_X as f32
}

/// Helper to ensure a chunk exists in `world` before writing an arena block.
fn set_arena_block(world: &mut World, pos: IVec3, block: BlockId) {
    let (cpos, _, _, _) = world_to_chunk_coords(pos);
    if !world.chunks().contains_key(&cpos) {
        world.insert_chunk(Chunk::new(cpos));
    }
    world.set_block(pos, block);
}

/// Builds (or resets) a complete biome-themed instanced boss arena inside `world`.
pub fn build_arena_into_world(world: &mut World, state: &TempleInstanceState) {
    let origin = state.arena_origin;
    let primary = state.kind.primary_block();
    let accent = state.kind.accent_block();
    let floor = state.kind.floor_block();

    // Ensure all intersecting chunks exist and are clean air first
    let min_c = world_to_chunk_coords(state.arena_bounds.min).0;
    let max_c = world_to_chunk_coords(state.arena_bounds.max).0;
    for cx in min_c.0.x..=max_c.0.x {
        for cy in min_c.0.y..=max_c.0.y {
            for cz in min_c.0.z..=max_c.0.z {
                let cpos = ChunkPos::new(cx, cy, cz);
                if !world.chunks().contains_key(&cpos) {
                    world.insert_chunk(Chunk::new(cpos));
                }
            }
        }
    }

    for dx in -ARENA_HALF_SIZE..=ARENA_HALF_SIZE {
        for dz in -ARENA_HALF_SIZE..=ARENA_HALF_SIZE {
            for dy in 0..=ARENA_HEIGHT {
                let p = origin + IVec3::new(dx, dy, dz);
                let is_wall = dx.abs() == ARENA_HALF_SIZE || dz.abs() == ARENA_HALF_SIZE;
                let is_floor = dy == 0;
                let is_ceiling = dy == ARENA_HEIGHT;

                let block = if is_floor {
                    if (dx + dz).rem_euclid(4) == 0 {
                        primary
                    } else {
                        floor
                    }
                } else if is_ceiling {
                    if dx.abs() % 6 == 0 || dz.abs() % 6 == 0 {
                        accent
                    } else {
                        primary
                    }
                } else if is_wall {
                    if dy == 1 || dy == ARENA_HEIGHT - 1 {
                        BlockId::TEMPLE_OBSIDIAN
                    } else if (dx + dz + dy).rem_euclid(5) == 0 {
                        accent
                    } else {
                        primary
                    }
                } else {
                    BlockId::AIR
                };
                set_arena_block(world, p, block);
            }
        }
    }

    // 4 symmetrical cover pillars with glowing braziers for dramatic arena lighting
    let pillar_offsets = [(-9, -7), (9, -7), (-9, 7), (9, 7)];
    for (px, pz) in pillar_offsets {
        for dy in 1..ARENA_HEIGHT {
            for ox in 0..=1 {
                for oz in 0..=1 {
                    let b = if dy == 1 || dy == ARENA_HEIGHT - 1 {
                        accent
                    } else {
                        primary
                    };
                    set_arena_block(world, origin + IVec3::new(px + ox, dy, pz + oz), b);
                }
            }
        }
        // Brazier sconces mounted around pillar mid-height
        set_arena_block(world, origin + IVec3::new(px - 1, 5, pz), BlockId::TEMPLE_BRAZIER);
        set_arena_block(world, origin + IVec3::new(px + 2, 5, pz + 1), BlockId::TEMPLE_BRAZIER);
    }

    // Wall braziers along the perimeter
    for offset in [-12, 0, 12] {
        set_arena_block(
            world,
            origin + IVec3::new(offset, 6, -ARENA_HALF_SIZE + 1),
            BlockId::TEMPLE_BRAZIER,
        );
        set_arena_block(
            world,
            origin + IVec3::new(offset, 6, ARENA_HALF_SIZE - 1),
            BlockId::TEMPLE_BRAZIER,
        );
        set_arena_block(
            world,
            origin + IVec3::new(-ARENA_HALF_SIZE + 1, 6, offset),
            BlockId::TEMPLE_BRAZIER,
        );
        set_arena_block(
            world,
            origin + IVec3::new(ARENA_HALF_SIZE - 1, 6, offset),
            BlockId::TEMPLE_BRAZIER,
        );
    }

    // Central Boss Summoning Altar dais
    for dx in -1..=1 {
        for dz in -1..=1 {
            set_arena_block(world, origin + IVec3::new(dx, 0, dz), accent);
        }
    }
    set_arena_block(world, state.altar_pos(), BlockId::BOSS_ALTAR);

    // Return Portal archway at the entry side
    let rp = state.return_portal_pos();
    for dx in -1..=1 {
        set_arena_block(world, rp + IVec3::new(dx, 3, 0), accent);
    }
    for dy in 0..=2 {
        set_arena_block(world, rp + IVec3::new(-1, dy, 0), primary);
        set_arena_block(world, rp + IVec3::new(1, dy, 0), primary);
    }
    set_arena_block(world, rp, BlockId::ARENA_RETURN_PORTAL);
    set_arena_block(world, rp + IVec3::new(0, 1, 0), BlockId::ARENA_RETURN_PORTAL);

    // Boss Treasure Chest on a pedestal at the far end of the arena (stays empty & sealed until boss is defeated)
    let chest_pos = state.treasure_chest_pos();
    set_arena_block(world, chest_pos + IVec3::new(0, -1, 0), accent);
    set_arena_block(world, chest_pos, BlockId::DIAMOND_CHEST);
    let slots = world.get_or_create_chest(chest_pos);
    for slot in slots.iter_mut() {
        *slot = ItemSlot::EMPTY;
    }

    // Mark all arena chunks dirty so meshes rebuild immediately
    for cx in min_c.0.x..=max_c.0.x {
        for cy in min_c.0.y..=max_c.0.y {
            for cz in min_c.0.z..=max_c.0.z {
                world.dirty_chunks.insert(ChunkPos::new(cx, cy, cz));
            }
        }
    }
}

/// Populates the arena's Boss Treasure Chest with biome-themed loot once the boss is defeated.
pub fn populate_arena_treasure_chest(world: &mut World, state: &TempleInstanceState) {
    let chest_pos = state.treasure_chest_pos();
    if world.get_block(chest_pos) != BlockId::DIAMOND_CHEST {
        set_arena_block(world, chest_pos, BlockId::DIAMOND_CHEST);
    }
    let slots = world.get_or_create_chest(chest_pos);
    for slot in slots.iter_mut() {
        *slot = ItemSlot::EMPTY;
    }
    for (idx, item) in state.kind.treasure_loot().into_iter().enumerate() {
        if idx < slots.len() {
            slots[idx] = item;
        }
    }
}

/// Spawns the boss + 2 flanking temple guardians inside the instanced arena.
pub fn spawn_arena_encounter(
    state: &TempleInstanceState,
    next_monster_id: &mut u32,
) -> Vec<MonsterEntity> {
    let mut spawned = Vec::with_capacity(3);
    let origin = state.arena_origin;

    let boss_id = *next_monster_id;
    *next_monster_id = next_monster_id.wrapping_add(1);
    let boss_pos = Vec3::new(
        origin.x as f32 + 0.5,
        (origin.y + 1) as f32,
        (origin.z + 8) as f32 + 0.5,
    );
    let mut boss = MonsterEntity::new_temple_mob(boss_id, state.kind.boss_kind(), boss_pos, state.temple_id);
    boss.in_combat = true;
    spawned.push(boss);

    let (m1_kind, m2_kind) = state.kind.minion_kinds();
    let m1_id = *next_monster_id;
    *next_monster_id = next_monster_id.wrapping_add(1);
    spawned.push(MonsterEntity::new_temple_mob(
        m1_id,
        m1_kind,
        Vec3::new((origin.x - 5) as f32 + 0.5, (origin.y + 1) as f32, (origin.z + 5) as f32 + 0.5),
        state.temple_id,
    ));

    let m2_id = *next_monster_id;
    *next_monster_id = next_monster_id.wrapping_add(1);
    spawned.push(MonsterEntity::new_temple_mob(
        m2_id,
        m2_kind,
        Vec3::new((origin.x + 5) as f32 + 0.5, (origin.y + 1) as f32, (origin.z + 5) as f32 + 0.5),
        state.temple_id,
    ));

    spawned
}

/// Computes the top-of-screen boss health bar for the nearest in-combat boss within 500 blocks.
pub fn compute_boss_hud(monsters: &[MonsterEntity], player_pos: Vec3) -> Option<BossHudState> {
    let mut best: Option<BossHudState> = None;
    for m in monsters {
        if m.dead || !m.kind.is_boss() || !m.in_combat {
            continue;
        }
        let dist = m.position.distance(player_pos);
        if dist > BOSS_HUD_MAX_DISTANCE {
            continue;
        }
        let replace = match &best {
            None => true,
            Some(curr) => dist < curr.distance,
        };
        if replace {
            best = Some(BossHudState {
                name: m.kind.name().to_string(),
                kind: m.kind,
                health: m.health.max(0.0),
                max_health: m.max_health.max(1.0),
                phase: m.phase.clamp(1, 3),
                distance: dist,
            });
        }
    }
    best
}

/// Helper to make sure we don't accidentally drop unused imports for chunk constants.
pub fn arena_chunk_span() -> (i32, i32, i32) {
    (CHUNK_SIZE_X as i32, CHUNK_SIZE_Y as i32, CHUNK_SIZE_Z as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boss_hud_500_block_limit() {
        let mut boss = MonsterEntity::new(1, MonsterKind::SunPharaoh, Vec3::new(0.0, 33.0, 0.0));
        boss.in_combat = true;

        // Within 500 blocks -> visible
        let hud = compute_boss_hud(&[boss.clone()], Vec3::new(499.0, 33.0, 0.0));
        assert!(hud.is_some());
        assert_eq!(hud.unwrap().name, "Sun Pharaoh");

        // Beyond 500 blocks -> hidden
        let hud_far = compute_boss_hud(&[boss.clone()], Vec3::new(501.0, 33.0, 0.0));
        assert!(hud_far.is_none());

        // Not in combat -> hidden
        boss.in_combat = false;
        let hud_idle = compute_boss_hud(&[boss], Vec3::new(10.0, 33.0, 0.0));
        assert!(hud_idle.is_none());
    }

    #[test]
    fn test_arena_build_and_protection() {
        let mut world = World::new();
        let mut mgr = TempleManager::new();
        let tid = temple_id_from_grid(12345, 2, -3);
        let state = mgr
            .register_temple(tid, TempleKind::Glacier, IVec3::new(640, 50, -960), IVec3::new(640, 51, -960))
            .clone();

        build_arena_into_world(&mut world, &state);

        assert_eq!(world.get_block(state.altar_pos()), BlockId::BOSS_ALTAR);
        assert_eq!(world.get_block(state.return_portal_pos()), BlockId::ARENA_RETURN_PORTAL);
        assert_eq!(world.get_block(state.treasure_chest_pos()), BlockId::DIAMOND_CHEST);

        // Before boss defeat, interior arena blocks and overworld temple blocks are protected
        let interior = state.arena_origin + IVec3::new(3, 2, 3);
        assert!(mgr.is_block_protected(interior, BlockId::AIR));
        assert!(mgr.is_block_protected(state.overworld_origin + IVec3::new(2, 2, 2), BlockId::TEMPLE_ICE_BRICK));

        // After boss defeat, interior arena & overworld temple unlock, but portals/altar/shell stay protected
        mgr.mark_boss_defeated(tid);
        assert!(!mgr.is_block_protected(interior, BlockId::AIR));
        assert!(!mgr.is_block_protected(state.overworld_origin + IVec3::new(2, 2, 2), BlockId::TEMPLE_ICE_BRICK));
        assert!(mgr.is_block_protected(state.altar_pos(), BlockId::BOSS_ALTAR));
        assert!(mgr.is_block_protected(state.arena_origin, BlockId::TEMPLE_ICE_BRICK)); // floor shell
    }

    #[test]
    fn test_boss_phases_and_unique_loot() {
        let world = World::new();
        let reg = crate::block::BlockRegistry::new();

        for (kind, expected_weapon, expected_armor, expected_core) in [
            (MonsterKind::SunPharaoh, ItemId::SUNBLADE, ItemId::PHARAOH_CROWN, ItemId::SUN_CORE),
            (MonsterKind::FrostTitan, ItemId::FROSTBLADE, ItemId::GLACIAL_CHESTPLATE, ItemId::FROST_HEART),
            (MonsterKind::OvergrownGolem, ItemId::VERDANT_GREATBLADE, ItemId::GOLEM_GREAVES, ItemId::ANCIENT_VINE_CORE),
            (MonsterKind::ShadowLich, ItemId::SHADOWFANG_BLADE, ItemId::LICH_BOOTS, ItemId::SOUL_PHYLACTERY),
        ] {
            let mut boss = MonsterEntity::new_temple_mob(10, kind, Vec3::new(0.0, 33.0, 0.0), 99);
            assert_eq!(boss.phase, 1);

            // Drop HP below 66% -> Phase 2
            boss.take_damage(boss.max_health * 0.4, Vec3::Z);
            let out2 = boss.update(0.05, Vec3::new(0.0, 33.0, 6.0), 0.0, &world, &reg);
            assert_eq!(boss.phase, 2);
            assert_eq!(out2.phase_changed, Some(2));

            // Drop HP below 33% -> Phase 3
            boss.take_damage(boss.max_health * 0.35, Vec3::Z);
            let out3 = boss.update(0.05, Vec3::new(0.0, 33.0, 6.0), 0.0, &world, &reg);
            assert_eq!(boss.phase, 3);
            assert_eq!(out3.phase_changed, Some(3));

            let drops = boss.generate_drops();
            assert!(drops.iter().any(|s| s.item_id == expected_weapon));
            assert!(drops.iter().any(|s| s.item_id == expected_armor));
            assert!(drops.iter().any(|s| s.item_id == expected_core));
        }
    }

    #[test]
    fn test_glacier_frost_titan_spawn_and_chest_lock() {
        let mut world = World::new();
        let reg = crate::block::BlockRegistry::new();
        let mut mgr = TempleManager::new();
        let tid = temple_id_from_grid(99, 1, 1);
        let state = mgr
            .register_temple(tid, TempleKind::Glacier, IVec3::new(320, 50, 320), IVec3::new(320, 51, 323))
            .clone();

        build_arena_into_world(&mut world, &state);
        let chest_pos = state.treasure_chest_pos();

        // Before boss defeat: chest is locked and empty
        assert!(mgr.is_chest_locked(chest_pos));
        let slots = world.chest_containers.get(&chest_pos).unwrap();
        assert!(slots.iter().all(|s| s.is_empty()));

        // Spawn Frost Titan + 2 Frost Wraiths and simulate 120 ticks
        let mut next_id = 1;
        let mut mobs = spawn_arena_encounter(&state, &mut next_id);
        assert_eq!(mobs.len(), 3);
        assert_eq!(mobs[0].kind, MonsterKind::FrostTitan);
        let player_pos = state.arena_entry_spawn();

        for _ in 0..120 {
            for m in mobs.iter_mut() {
                m.update(0.05, player_pos, 0.0, &world, &reg);
            }
        }
        assert!(!mobs[0].dead, "Frost Titan should remain alive in Glacier arena");
        assert!(mobs[0].position.y >= 32.5 && mobs[0].position.y <= 50.0);
        let hud = compute_boss_hud(&mobs, player_pos);
        assert!(hud.is_some());
        assert_eq!(hud.unwrap().name, "Frost Titan");

        // After boss defeat: chest unlocks and receives biome treasure loot
        mgr.mark_boss_defeated(tid);
        populate_arena_treasure_chest(&mut world, &state);
        assert!(!mgr.is_chest_locked(chest_pos));
        let unlocked_slots = world.chest_containers.get(&chest_pos).unwrap();
        assert!(unlocked_slots.iter().any(|s| !s.is_empty()));
    }
}
