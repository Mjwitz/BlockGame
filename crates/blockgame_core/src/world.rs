use std::collections::{HashMap, HashSet};

use glam::IVec3;

use crate::{
    block::BlockId,
    chunk::{Chunk, ChunkPos, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    item::ItemSlot,
};

/// Converts a global integer voxel coordinate into its containing `ChunkPos`
/// and the local `(x, y, z)` indices within that chunk.
#[inline]
pub fn world_to_chunk_coords(world_pos: IVec3) -> (ChunkPos, usize, usize, usize) {
    let cx = world_pos.x.div_euclid(CHUNK_SIZE_X as i32);
    let cy = world_pos.y.div_euclid(CHUNK_SIZE_Y as i32);
    let cz = world_pos.z.div_euclid(CHUNK_SIZE_Z as i32);

    let lx = world_pos.x.rem_euclid(CHUNK_SIZE_X as i32) as usize;
    let ly = world_pos.y.rem_euclid(CHUNK_SIZE_Y as i32) as usize;
    let lz = world_pos.z.rem_euclid(CHUNK_SIZE_Z as i32) as usize;

    (ChunkPos::new(cx, cy, cz), lx, ly, lz)
}

/// Converts a chunk position and local voxel coordinate into a global world coordinate.
#[inline]
pub fn chunk_to_world_coords(chunk_pos: ChunkPos, lx: usize, ly: usize, lz: usize) -> IVec3 {
    IVec3::new(
        chunk_pos.0.x * CHUNK_SIZE_X as i32 + lx as i32,
        chunk_pos.0.y * CHUNK_SIZE_Y as i32 + ly as i32,
        chunk_pos.0.z * CHUNK_SIZE_Z as i32 + lz as i32,
    )
}

pub const FURNACE_COOK_TIME: f32 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct FurnaceState {
    pub input: ItemSlot,
    pub fuel: ItemSlot,
    pub output: ItemSlot,
    pub burn_time_remaining: f32,
    pub max_burn_time: f32,
    pub cook_progress: f32,
}

impl Default for FurnaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl FurnaceState {
    pub const COOK_TIME: f32 = FURNACE_COOK_TIME;
    pub const FURNACE_COOK_TIME: f32 = FURNACE_COOK_TIME;

    pub fn new() -> Self {
        Self {
            input: ItemSlot::EMPTY,
            fuel: ItemSlot::EMPTY,
            output: ItemSlot::EMPTY,
            burn_time_remaining: 0.0,
            max_burn_time: 0.0,
            cook_progress: 0.0,
        }
    }

    #[inline]
    pub fn is_burning(&self) -> bool {
        self.burn_time_remaining > 0.0
    }

    /// Ticks the furnace state for elapsed seconds `dt`.
    /// Returns true if the block's visual lit state changed.
    pub fn update(&mut self, dt: f32) -> bool {
        let was_burning = self.is_burning();
        let smelting_out = crate::crafting::get_smelting_output(self.input.item_id);

        let can_smelt = if let Some(out_id) = smelting_out {
            if self.input.is_empty() {
                false
            } else if self.output.is_empty() {
                true
            } else {
                self.output.item_id == out_id && self.output.count < 64
            }
        } else {
            false
        };

        // If fire expired but we have a valid recipe and fuel, ignite the next fuel item
        if !self.is_burning() && can_smelt {
            let fuel_time = crate::crafting::consume_fuel_slot(&mut self.fuel);
            if fuel_time > 0.0 {
                self.burn_time_remaining = fuel_time;
                self.max_burn_time = fuel_time;
            }
        }

        // Process burning
        if self.burn_time_remaining > 0.0 {
            self.burn_time_remaining = (self.burn_time_remaining - dt).max(0.0);

            if can_smelt {
                self.cook_progress += dt;
                if self.cook_progress >= FURNACE_COOK_TIME {
                    self.cook_progress = 0.0;
                    if let Some(out_id) = smelting_out {
                        if self.output.is_empty() {
                            self.output = ItemSlot::new(out_id, 1);
                        } else {
                            self.output.count += 1;
                        }
                        self.input.count -= 1;
                        if self.input.count == 0 {
                            self.input = ItemSlot::EMPTY;
                        }
                    }
                }
            } else {
                self.cook_progress = (self.cook_progress - dt * 2.0).max(0.0);
            }
        } else {
            self.cook_progress = (self.cook_progress - dt * 2.0).max(0.0);
        }

        was_burning != self.is_burning()
    }
}

pub const COOKING_POT_COOK_TIME: f32 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CookingPotState {
    pub ingredients: [ItemSlot; 4],
    pub fuel: ItemSlot,
    pub output: ItemSlot,
    pub burn_time_remaining: f32,
    pub max_burn_time: f32,
    pub cook_progress: f32,
}

impl Default for CookingPotState {
    fn default() -> Self {
        Self::new()
    }
}

impl CookingPotState {
    pub const COOK_TIME: f32 = COOKING_POT_COOK_TIME;

    pub fn new() -> Self {
        Self {
            ingredients: [ItemSlot::EMPTY; 4],
            fuel: ItemSlot::EMPTY,
            output: ItemSlot::EMPTY,
            burn_time_remaining: 0.0,
            max_burn_time: 0.0,
            cook_progress: 0.0,
        }
    }

    #[inline]
    pub fn is_burning(&self) -> bool {
        self.burn_time_remaining > 0.0
    }

    /// Ticks the Cooking Pot for elapsed seconds `dt`.
    /// Returns true if the block's visual lit state changed (`COOKING_POT` <-> `COOKING_POT_LIT`).
    pub fn update(&mut self, dt: f32) -> bool {
        let was_burning = self.is_burning();
        let recipe_out = crate::crafting::get_cooking_pot_output(&self.ingredients);

        let can_cook = if let Some(out_slot) = recipe_out {
            if self.output.is_empty() {
                true
            } else {
                // Meals stack up to 16 (no Bowls required)
                self.output.item_id == out_slot.item_id
                    && self.output.count.saturating_add(out_slot.count) <= 16
            }
        } else {
            false
        };

        if !self.is_burning() && can_cook {
            let fuel_time = crate::crafting::consume_fuel_slot(&mut self.fuel);
            if fuel_time > 0.0 {
                self.burn_time_remaining = fuel_time;
                self.max_burn_time = fuel_time;
            }
        }

        if self.burn_time_remaining > 0.0 {
            self.burn_time_remaining = (self.burn_time_remaining - dt).max(0.0);

            if can_cook {
                self.cook_progress += dt;
                if self.cook_progress >= COOKING_POT_COOK_TIME {
                    self.cook_progress = 0.0;
                    if let Some(out_slot) = recipe_out {
                        if self.output.is_empty() {
                            self.output = out_slot;
                        } else {
                            self.output.count = self.output.count.saturating_add(out_slot.count);
                        }
                        for slot in &mut self.ingredients {
                            if slot.is_empty() {
                                continue;
                            }
                            if slot.item_id == crate::item::ItemId::MILK_BUCKET
                                || slot.item_id == crate::item::ItemId::WATER_BUCKET
                            {
                                // Return empty bucket in the input slot so players never lose iron buckets
                                *slot = ItemSlot::new(crate::item::ItemId::BUCKET, 1);
                            } else {
                                slot.count -= 1;
                                if slot.count == 0 {
                                    *slot = ItemSlot::EMPTY;
                                }
                            }
                        }
                    }
                }
            } else {
                self.cook_progress = (self.cook_progress - dt * 2.0).max(0.0);
            }
        } else {
            self.cook_progress = (self.cook_progress - dt * 2.0).max(0.0);
        }

        was_burning != self.is_burning()
    }
}

#[derive(Default)]
pub struct World {
    chunks: HashMap<ChunkPos, Chunk>,
    pub dirty_chunks: HashSet<ChunkPos>,
    pub chunk_light_sources: HashMap<ChunkPos, HashSet<IVec3>>,
    pub chest_containers: HashMap<IVec3, Vec<ItemSlot>>,
    pub furnaces: HashMap<IVec3, FurnaceState>,
    pub cooking_pots: HashMap<IVec3, CookingPotState>,
    pub power_nodes: HashSet<IVec3>,
    pub coal_generators: HashMap<IVec3, crate::power::CoalGeneratorState>,
    pub batteries: HashMap<IVec3, crate::power::BatteryState>,
    pub electric_furnaces: HashMap<IVec3, crate::power::ElectricFurnaceState>,
    pub powered_storage_terminals: HashSet<IVec3>,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            dirty_chunks: HashSet::new(),
            chunk_light_sources: HashMap::new(),
            chest_containers: HashMap::new(),
            furnaces: HashMap::new(),
            cooking_pots: HashMap::new(),
            power_nodes: HashSet::new(),
            coal_generators: HashMap::new(),
            batteries: HashMap::new(),
            electric_furnaces: HashMap::new(),
            powered_storage_terminals: HashSet::new(),
        }
    }

    pub fn get_or_create_cooking_pot(&mut self, pos: IVec3) -> &mut CookingPotState {
        self.cooking_pots.entry(pos).or_default()
    }

    pub fn get_cooking_pot(&self, pos: IVec3) -> Option<&CookingPotState> {
        self.cooking_pots.get(&pos)
    }

    pub fn get_cooking_pot_mut(&mut self, pos: IVec3) -> Option<&mut CookingPotState> {
        self.cooking_pots.get_mut(&pos)
    }

    pub fn remove_cooking_pot(&mut self, pos: IVec3) -> Option<CookingPotState> {
        self.cooking_pots.remove(&pos)
    }

    pub fn get_or_create_chest(&mut self, pos: IVec3) -> &mut Vec<ItemSlot> {
        let target_len = self.get_block(pos).chest_slot_count();
        let entry = self
            .chest_containers
            .entry(pos)
            .or_insert_with(|| vec![ItemSlot::EMPTY; target_len]);
        if entry.len() < target_len {
            entry.resize(target_len, ItemSlot::EMPTY);
        }
        entry
    }

    pub fn get_chest(&self, pos: IVec3) -> Option<&Vec<ItemSlot>> {
        self.chest_containers.get(&pos)
    }

    pub fn get_chest_mut(&mut self, pos: IVec3) -> Option<&mut Vec<ItemSlot>> {
        self.chest_containers.get_mut(&pos)
    }

    pub fn remove_chest(&mut self, pos: IVec3) -> Option<Vec<ItemSlot>> {
        self.chest_containers.remove(&pos)
    }

    pub fn get_or_create_furnace(&mut self, pos: IVec3) -> &mut FurnaceState {
        self.furnaces.entry(pos).or_insert_with(FurnaceState::new)
    }

    pub fn get_furnace(&self, pos: IVec3) -> Option<&FurnaceState> {
        self.furnaces.get(&pos)
    }

    pub fn get_furnace_mut(&mut self, pos: IVec3) -> Option<&mut FurnaceState> {
        self.furnaces.get_mut(&pos)
    }

    pub fn remove_furnace(&mut self, pos: IVec3) -> Option<FurnaceState> {
        self.furnaces.remove(&pos)
    }

    pub fn get_or_create_coal_generator(&mut self, pos: IVec3) -> &mut crate::power::CoalGeneratorState {
        self.coal_generators
            .entry(pos)
            .or_insert_with(crate::power::CoalGeneratorState::new)
    }

    pub fn remove_coal_generator(&mut self, pos: IVec3) -> Option<crate::power::CoalGeneratorState> {
        self.coal_generators.remove(&pos)
    }

    pub fn get_or_create_battery(&mut self, pos: IVec3) -> &mut crate::power::BatteryState {
        let block = self.get_block(pos);
        self.batteries
            .entry(pos)
            .or_insert_with(|| crate::power::BatteryState::for_block(block))
    }

    pub fn remove_battery(&mut self, pos: IVec3) -> Option<crate::power::BatteryState> {
        self.batteries.remove(&pos)
    }

    pub fn get_or_create_electric_furnace(
        &mut self,
        pos: IVec3,
    ) -> &mut crate::power::ElectricFurnaceState {
        self.electric_furnaces
            .entry(pos)
            .or_insert_with(crate::power::ElectricFurnaceState::new)
    }

    pub fn remove_electric_furnace(
        &mut self,
        pos: IVec3,
    ) -> Option<crate::power::ElectricFurnaceState> {
        self.electric_furnaces.remove(&pos)
    }

    #[inline]
    pub fn get_block(&self, world_pos: IVec3) -> BlockId {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(world_pos);
        match self.chunks.get(&cpos) {
            Some(chunk) => chunk.get_unchecked(lx, ly, lz),
            None => BlockId::AIR,
        }
    }

    #[inline]
    pub fn set_block(&mut self, world_pos: IVec3, block_id: BlockId) -> bool {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(world_pos);
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            let ok = chunk.set(lx, ly, lz, block_id);
            if ok {
                if block_id.is_air() {
                    chunk.set_facing(lx, ly, lz, 0);
                }
                self.dirty_chunks.insert(cpos);
                if block_id.is_emissive() {
                    self.chunk_light_sources
                        .entry(cpos)
                        .or_default()
                        .insert(world_pos);
                } else if let Some(bucket) = self.chunk_light_sources.get_mut(&cpos) {
                    bucket.remove(&world_pos);
                    if bucket.is_empty() {
                        self.chunk_light_sources.remove(&cpos);
                    }
                }
                if block_id.is_power_block() {
                    self.power_nodes.insert(world_pos);
                } else {
                    self.power_nodes.remove(&world_pos);
                }
            }
            ok
        } else {
            false
        }
    }

    #[inline]
    pub fn get_block_facing(&self, world_pos: IVec3) -> u8 {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(world_pos);
        match self.chunks.get(&cpos) {
            Some(chunk) => chunk.get_facing_unchecked(lx, ly, lz),
            None => 0,
        }
    }

    #[inline]
    pub fn set_block_facing(&mut self, world_pos: IVec3, facing: u8) -> bool {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(world_pos);
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            let ok = chunk.set_facing(lx, ly, lz, facing);
            if ok {
                self.dirty_chunks.insert(cpos);
            }
            ok
        } else {
            false
        }
    }

    #[inline]
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    #[inline]
    pub fn get_chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
        self.chunks.get_mut(&pos)
    }

    #[inline]
    pub fn insert_chunk(&mut self, chunk: Chunk) -> Option<Chunk> {
        // Index any light sources and power blocks in this chunk
        let origin = chunk.pos.to_world_origin();
        let mut chunk_lights = HashSet::new();
        if !chunk.is_all_air() {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    for x in 0..CHUNK_SIZE_X {
                        let block = chunk.get_unchecked(x, y, z);
                        if block.is_emissive() {
                            chunk_lights
                                .insert(origin + IVec3::new(x as i32, y as i32, z as i32));
                        }
                        if block.is_power_block() {
                            self.power_nodes
                                .insert(origin + IVec3::new(x as i32, y as i32, z as i32));
                        }
                    }
                }
            }
        }
        if chunk_lights.is_empty() {
            self.chunk_light_sources.remove(&chunk.pos);
        } else {
            self.chunk_light_sources.insert(chunk.pos, chunk_lights);
        }
        self.chunks.insert(chunk.pos, chunk)
    }

    /// Inserts a modified chunk loaded from save data and marks it as dirty.
    #[inline]
    pub fn insert_dirty_chunk(&mut self, chunk: Chunk) -> Option<Chunk> {
        self.dirty_chunks.insert(chunk.pos);
        self.insert_chunk(chunk)
    }

    #[inline]
    pub fn remove_chunk(&mut self, pos: ChunkPos) -> Option<Chunk> {
        self.chunk_light_sources.remove(&pos);
        if !self.power_nodes.is_empty() {
            self.power_nodes.retain(|p| {
                let (cp, _, _, _) = world_to_chunk_coords(*p);
                cp != pos
            });
        }
        self.chunks.remove(&pos)
    }

    #[inline]
    pub fn has_light_source(&self, pos: IVec3) -> bool {
        let (cpos, _, _, _) = world_to_chunk_coords(pos);
        self.chunk_light_sources
            .get(&cpos)
            .map_or(false, |set| set.contains(&pos))
    }

    /// Returns relative coordinates and luminance for all light sources within reach of a chunk.
    /// Queries only the bounded neighborhood of chunk buckets (O(27) for `max_reach <= 15`).
    pub fn get_light_sources_near(&self, origin: IVec3, max_reach: i32) -> Vec<(IVec3, u8)> {
        let mut sources = Vec::new();
        let min_x = origin.x - max_reach;
        let max_x = origin.x + CHUNK_SIZE_X as i32 + max_reach;
        let min_y = origin.y - max_reach;
        let max_y = origin.y + CHUNK_SIZE_Y as i32 + max_reach;
        let min_z = origin.z - max_reach;
        let max_z = origin.z + CHUNK_SIZE_Z as i32 + max_reach;

        let (center_cp, _, _, _) = world_to_chunk_coords(origin);
        let r = (max_reach.max(0) + 15) / 16;

        for dy in -r..=r {
            let cy = center_cp.0.y + dy;
            if cy < 0 || cy >= crate::chunk::VERTICAL_CHUNKS {
                continue;
            }
            for dz in -r..=r {
                for dx in -r..=r {
                    let cp = ChunkPos::new(center_cp.0.x + dx, cy, center_cp.0.z + dz);
                    let Some(bucket) = self.chunk_light_sources.get(&cp) else {
                        continue;
                    };
                    for &pos in bucket {
                        if pos.x >= min_x
                            && pos.x <= max_x
                            && pos.y >= min_y
                            && pos.y <= max_y
                            && pos.z >= min_z
                            && pos.z <= max_z
                        {
                            let rel = pos - origin;
                            let lum = self.get_block(pos).luminance();
                            if lum > 0 {
                                sources.push((rel, lum));
                            }
                        }
                    }
                }
            }
        }
        sources
    }

    #[inline]
    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    #[inline]
    pub fn chunks(&self) -> &HashMap<ChunkPos, Chunk> {
        &self.chunks
    }

    /// Returns the 4-bit fluid metadata level (`0` = Source Block, `1..=7` = horizontal decay, `8` = falling).
    #[inline]
    pub fn get_fluid_level(&self, pos: IVec3) -> u8 {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(pos);
        self.chunks
            .get(&cpos)
            .map_or(0, |c| c.get_facing_unchecked(lx, ly, lz) & 0x0F)
    }

    /// Sets the 4-bit fluid metadata level (`0..=8`) at `pos` and marks the chunk dirty.
    #[inline]
    pub fn set_fluid_level(&mut self, pos: IVec3, level: u8) -> bool {
        let (cpos, lx, ly, lz) = world_to_chunk_coords(pos);
        if let Some(chunk) = self.chunks.get_mut(&cpos) {
            chunk.set_facing(lx, ly, lz, level & 0x0F);
            self.dirty_chunks.insert(cpos);
            true
        } else {
            false
        }
    }

    /// Checks if all 6 immediate neighbor chunks (East, West, Top, Bottom, South, North) are loaded.
    pub fn are_neighbors_loaded(&self, pos: ChunkPos) -> bool {
        let p = pos.0;
        self.has_chunk(ChunkPos::new(p.x + 1, p.y, p.z))
            && self.has_chunk(ChunkPos::new(p.x - 1, p.y, p.z))
            && self.has_chunk(ChunkPos::new(p.x, p.y + 1, p.z))
            && self.has_chunk(ChunkPos::new(p.x, p.y - 1, p.z))
            && self.has_chunk(ChunkPos::new(p.x, p.y, p.z + 1))
            && self.has_chunk(ChunkPos::new(p.x, p.y, p.z - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_to_chunk_coordinates() {
        // Positive coordinates
        let (cpos, lx, ly, lz) = world_to_chunk_coords(IVec3::new(35, 18, 5));
        assert_eq!(cpos, ChunkPos::new(2, 1, 0));
        assert_eq!((lx, ly, lz), (3, 2, 5));

        // Negative coordinates
        let (cpos_neg, nlx, nly, nlz) = world_to_chunk_coords(IVec3::new(-1, -17, -16));
        assert_eq!(cpos_neg, ChunkPos::new(-1, -2, -1));
        assert_eq!((nlx, nly, nlz), (15, 15, 0));
    }

    #[test]
    fn test_world_block_read_write() {
        let mut world = World::new();
        let chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        world.insert_chunk(chunk);

        assert_eq!(world.get_block(IVec3::new(5, 5, 5)), BlockId::AIR);
        world.set_block(IVec3::new(5, 5, 5), BlockId::STONE);
        assert_eq!(world.get_block(IVec3::new(5, 5, 5)), BlockId::STONE);

        // Accessing unloaded chunk returns AIR safely
        assert_eq!(world.get_block(IVec3::new(100, 100, 100)), BlockId::AIR);
    }

    #[test]
    fn test_world_chest_containers() {
        let mut world = World::new();
        let pos = IVec3::new(10, 64, 10);

        assert!(world.get_chest(pos).is_none());

        let chest = world.get_or_create_chest(pos);
        assert_eq!(chest.len(), 27);
        chest[0] = crate::item::ItemSlot::new(crate::item::ItemId::DIAMOND, 5);

        assert_eq!(world.get_chest(pos).unwrap()[0].count, 5);

        let removed = world.remove_chest(pos).unwrap();
        assert_eq!(removed[0].item_id, crate::item::ItemId::DIAMOND);
        assert_eq!(removed[0].count, 5);
        assert!(world.get_chest(pos).is_none());
    }

    #[test]
    fn test_furnace_smelting_and_fuel() {
        let mut world = World::new();
        let pos = IVec3::new(12, 64, 12);

        let furnace = world.get_or_create_furnace(pos);
        // Put 1 Raw Iron as input and 1 Coal as fuel
        furnace.input = ItemSlot::new(crate::item::ItemId::RAW_IRON, 1);
        furnace.fuel = ItemSlot::new(crate::item::ItemId::COAL, 1);

        assert!(!furnace.is_burning());

        // Step 1: initial tick starts burning fuel (Coal burns for 80s)
        let state_changed = furnace.update(0.1);
        assert!(state_changed); // Lit state changed from unlit to burning
        assert!(furnace.is_burning());
        assert_eq!(furnace.fuel.count, 0); // Consumed 1 coal
        assert!(furnace.burn_time_remaining > 79.0);

        // Step 2: tick for remaining cook time (8.0s total)
        furnace.update(FURNACE_COOK_TIME);

        // Input should now be empty and output should have 1 Iron Ingot!
        assert!(furnace.input.is_empty());
        assert_eq!(furnace.output.item_id, crate::item::ItemId::IRON_INGOT);
        assert_eq!(furnace.output.count, 1);
        assert!(furnace.is_burning()); // Coal still has ~71s of burn time left
    }

    #[test]
    fn test_sea_lantern_light_sources_cross_chunk() {
        let mut world = World::new();
        // Insert chunk at (0, 0, 0)
        let chunk0 = Chunk::new(ChunkPos::new(0, 0, 0));
        world.insert_chunk(chunk0);

        // Place a sea lantern at chunk border (15, 10, 15)
        let sea_lantern_pos = IVec3::new(15, 10, 15);
        world.set_block(sea_lantern_pos, BlockId::SEA_LANTERN);

        // Check light sources near chunk (1, 0, 0) whose origin is (16, 0, 0)
        let chunk1_origin = IVec3::new(16, 0, 0);
        let sources = world.get_light_sources_near(chunk1_origin, 15);
        assert!(!sources.is_empty(), "Sea lantern should be visible from neighbor chunk");
        let (rel_pos, lum) = sources[0];
        assert_eq!(rel_pos, sea_lantern_pos - chunk1_origin);
        assert_eq!(lum, 15, "Sea lantern luminance must be 15");
    }

    #[test]
    fn test_lava_bucket_furnace_fuel_returns_empty_bucket() {
        let mut furnace = FurnaceState::new();
        furnace.input = ItemSlot::new(crate::item::ItemId::RAW_IRON, 2);
        furnace.fuel = ItemSlot::new(crate::item::ItemId::LAVA_BUCKET, 1);

        let changed = furnace.update(1.0);
        assert!(changed, "Furnace should ignite with Lava Bucket");
        assert!(furnace.is_burning());
        assert!(
            (furnace.max_burn_time - 1000.0).abs() < 0.01,
            "Lava Bucket must burn for 1000s"
        );
        assert_eq!(
            furnace.fuel,
            ItemSlot::new(crate::item::ItemId::BUCKET, 1),
            "Consuming a Lava Bucket in a Furnace must leave an empty Bucket"
        );
    }

    #[test]
    fn test_lava_and_glow_crystal_lighting_and_water_conversion() {
        let reg = crate::block::BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));

        // Luminance checks
        let lava_pos = IVec3::new(4, 4, 4);
        let crystal_pos = IVec3::new(8, 6, 8);
        world.set_block(lava_pos, BlockId::LAVA);
        world.set_block(crystal_pos, BlockId::GLOW_CRYSTAL);
        let sources = world.get_light_sources_near(IVec3::ZERO, 15);
        assert!(sources.contains(&(lava_pos, 14)), "Lava must emit light level 14");
        assert!(sources.contains(&(crystal_pos, 7)), "Glow Crystal must emit light level 7");

        // Water placed next to source Lava converts source Lava -> Obsidian
        world.set_block(IVec3::new(4, 3, 4), BlockId::DEEPSTONE);
        world.set_block(IVec3::new(5, 3, 4), BlockId::DEEPSTONE);
        crate::block::place_water_source(IVec3::new(5, 4, 4), &mut world);
        assert_eq!(
            world.get_block(lava_pos),
            BlockId::OBSIDIAN,
            "Water touching source Lava must convert it to Obsidian"
        );

        // Submerged lower layer of a multi-block-deep Lava Pool also converts to Obsidian
        for x in 1..=3 {
            for z in 1..=3 {
                world.set_block(IVec3::new(x, 1, z), BlockId::LAVA);
                world.set_block(IVec3::new(x, 2, z), BlockId::LAVA);
            }
        }
        world.set_block(IVec3::new(2, 0, 2), BlockId::BEDROCK);
        // Place Water touching the submerged center (2, 1, 2) from an adjacent carved pocket
        world.set_block(IVec3::new(2, 1, 3), BlockId::AIR);
        crate::block::place_water_source(IVec3::new(2, 1, 3), &mut world);
        assert_eq!(
            world.get_block(IVec3::new(2, 1, 2)),
            BlockId::OBSIDIAN,
            "Submerged still Lava Pool block must convert to Obsidian, not Cobblestone"
        );

        // Flowing/cascading Lava hitting Water converts to Cobblestone
        world.set_block(IVec3::new(10, 4, 10), BlockId::WATER);
        world.set_block(IVec3::new(11, 3, 10), BlockId::DEEPSTONE);
        crate::block::place_lava_source(IVec3::new(11, 6, 10), &mut world);
        assert_eq!(world.get_block(IVec3::new(11, 6, 10)), BlockId::LAVA);
        assert_eq!(world.get_block(IVec3::new(11, 5, 10)), BlockId::LAVA);
        assert_eq!(
            world.get_block(IVec3::new(11, 4, 10)),
            BlockId::COBBLESTONE,
            "Cascading/flowing Lava meeting Water must convert to Cobblestone"
        );

        // Harvesting rules for new cave blocks
        assert!(reg.can_harvest(BlockId::DEEPSTONE, Some(crate::tool::ToolType::Pickaxe), crate::tool::ToolTier::Wood));
        assert_eq!(
            reg.get_drop(BlockId::DEEPSTONE, Some(crate::tool::ToolType::Pickaxe), crate::tool::ToolTier::Wood),
            Some((crate::item::ItemId::DEEPSTONE_COBBLESTONE, 1))
        );
        assert!(!reg.can_harvest(BlockId::DRIPSTONE, None, crate::tool::ToolTier::None));
        assert!(reg.can_harvest(BlockId::DRIPSTONE, Some(crate::tool::ToolType::Pickaxe), crate::tool::ToolTier::Wood));
        assert!(reg.can_harvest(BlockId::GLOW_CRYSTAL, None, crate::tool::ToolTier::None));
        assert_eq!(
            reg.get_drop(BlockId::GLOW_CRYSTAL, None, crate::tool::ToolTier::None),
            Some((crate::item::ItemId::GLOW_CRYSTAL, 1))
        );
        assert!(!reg.can_harvest(BlockId::OBSIDIAN, Some(crate::tool::ToolType::Pickaxe), crate::tool::ToolTier::Iron));
        assert!(reg.can_harvest(BlockId::OBSIDIAN, Some(crate::tool::ToolType::Pickaxe), crate::tool::ToolTier::Diamond));

        // Ceiling Dripstone (including stacked pair) and wall Glow Crystal break when supporting rock is mined
        let ceil = IVec3::new(2, 10, 2);
        let stalactite = IVec3::new(2, 9, 2);
        let stalactite_lower = IVec3::new(2, 8, 2);
        let wall_crystal = IVec3::new(3, 10, 2);
        world.set_block(ceil, BlockId::DEEPSTONE);
        world.set_block(stalactite, BlockId::DRIPSTONE);
        world.set_block(stalactite_lower, BlockId::DRIPSTONE);
        world.set_block(wall_crystal, BlockId::GLOW_CRYSTAL);
        world.set_block(ceil, BlockId::AIR);
        let (drops, modified, _) = crate::block::break_unsupported_attached(ceil, &mut world, &reg);
        assert_eq!(world.get_block(stalactite), BlockId::AIR);
        assert_eq!(world.get_block(stalactite_lower), BlockId::AIR);
        assert_eq!(world.get_block(wall_crystal), BlockId::AIR);
        assert!(modified.contains(&stalactite));
        assert!(modified.contains(&stalactite_lower));
        assert!(modified.contains(&wall_crystal));
        assert!(drops.iter().any(|(_, s)| s.item_id == crate::item::ItemId::GLOW_CRYSTAL));
    }
}
