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
            let fuel_time = crate::crafting::get_fuel_burn_time(self.fuel.item_id);
            if fuel_time > 0.0 && self.fuel.count > 0 {
                self.fuel.count -= 1;
                if self.fuel.count == 0 {
                    self.fuel = ItemSlot::EMPTY;
                }
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

#[derive(Default)]
pub struct World {
    chunks: HashMap<ChunkPos, Chunk>,
    pub light_sources: HashSet<IVec3>,
    pub chest_containers: HashMap<IVec3, Vec<ItemSlot>>,
    pub furnaces: HashMap<IVec3, FurnaceState>,
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            light_sources: HashSet::new(),
            chest_containers: HashMap::new(),
            furnaces: HashMap::new(),
        }
    }

    pub fn get_or_create_chest(&mut self, pos: IVec3) -> &mut Vec<ItemSlot> {
        self.chest_containers
            .entry(pos)
            .or_insert_with(|| vec![ItemSlot::EMPTY; 27])
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
                if block_id == BlockId::TORCH {
                    self.light_sources.insert(world_pos);
                } else {
                    self.light_sources.remove(&world_pos);
                }
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
        // Index any light sources in this chunk
        let origin = chunk.pos.to_world_origin();
        for y in 0..CHUNK_SIZE_Y {
            for z in 0..CHUNK_SIZE_Z {
                for x in 0..CHUNK_SIZE_X {
                    if chunk.get_unchecked(x, y, z) == BlockId::TORCH {
                        self.light_sources
                            .insert(origin + IVec3::new(x as i32, y as i32, z as i32));
                    }
                }
            }
        }
        self.chunks.insert(chunk.pos, chunk)
    }

    #[inline]
    pub fn remove_chunk(&mut self, pos: ChunkPos) -> Option<Chunk> {
        self.light_sources.retain(|p| {
            let (cp, _, _, _) = world_to_chunk_coords(*p);
            cp != pos
        });
        self.chunks.remove(&pos)
    }

    /// Returns relative coordinates and luminance for all light sources within reach of a chunk.
    pub fn get_light_sources_near(&self, origin: IVec3, max_reach: i32) -> Vec<(IVec3, u8)> {
        let mut sources = Vec::new();
        let min_x = origin.x - max_reach;
        let max_x = origin.x + CHUNK_SIZE_X as i32 + max_reach;
        let min_y = origin.y - max_reach;
        let max_y = origin.y + CHUNK_SIZE_Y as i32 + max_reach;
        let min_z = origin.z - max_reach;
        let max_z = origin.z + CHUNK_SIZE_Z as i32 + max_reach;

        for &pos in &self.light_sources {
            if pos.x >= min_x
                && pos.x <= max_x
                && pos.y >= min_y
                && pos.y <= max_y
                && pos.z >= min_z
                && pos.z <= max_z
            {
                let rel = pos - origin;
                sources.push((rel, 14));
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
}
