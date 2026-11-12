use glam::{IVec3, Vec3};

use crate::{
    block::{BlockId, BlockRegistry},
    item::ItemSlot,
    world::{world_to_chunk_coords, World},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallingBlockStatus {
    Falling,
    Landed(IVec3),
    Despawned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FallingBlock {
    pub block_id: BlockId,
    pub position: Vec3,
    pub velocity_y: f32,
    pub time_falling: f32,
}

impl FallingBlock {
    pub fn new(block_id: BlockId, initial_pos: IVec3) -> Self {
        Self {
            block_id,
            position: initial_pos.as_vec3(),
            velocity_y: 0.0,
            time_falling: 0.0,
        }
    }

    pub fn with_velocity(block_id: BlockId, position: Vec3, velocity_y: f32) -> Self {
        Self {
            block_id,
            position,
            velocity_y,
            time_falling: 0.0,
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        world: &World,
        registry: &BlockRegistry,
    ) -> FallingBlockStatus {
        self.time_falling += dt;

        // Apply gravity acceleration
        let gravity = 20.0;
        self.velocity_y -= gravity * dt;
        self.velocity_y = self.velocity_y.max(-25.0);

        let target_y = self.position.y + self.velocity_y * dt;

        // Despawn if falling into void below y = -64 or after 30 seconds
        if self.position.y < -64.0 || target_y < -64.0 || self.time_falling > 30.0 {
            return FallingBlockStatus::Despawned;
        }

        let curr_bottom_int = self.position.y.floor() as i32;
        let target_bottom_int = target_y.floor() as i32;
        let x_int = self.position.x.floor() as i32;
        let z_int = self.position.z.floor() as i32;

        if target_bottom_int < curr_bottom_int {
            for check_y in (target_bottom_int..curr_bottom_int).rev() {
                let check_pos = IVec3::new(x_int, check_y, z_int);
                let block = world.get_block(check_pos);

                if registry.is_solid(block) {
                    let land_pos = check_pos + IVec3::Y;
                    self.position.y = land_pos.y as f32;
                    return FallingBlockStatus::Landed(land_pos);
                }
            }
        }

        self.position.y = target_y;
        FallingBlockStatus::Falling
    }
}

static NEXT_LOCAL_DROP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Returns a unique monotonic drop ID for local/singleplayer drops.
pub fn next_local_drop_id() -> u64 {
    NEXT_LOCAL_DROP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemDropStatus {
    Alive,
    Collected(ItemSlot),
    Despawned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDrop {
    pub drop_id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub item: ItemSlot,
    pub age: f32,
    pub despawn_time: f32,
    pub pickup_delay: f32,
    pub pickup_pending: bool,
}

impl ItemDrop {
    pub fn new(position: Vec3, item: ItemSlot) -> Self {
        Self {
            drop_id: next_local_drop_id(),
            position,
            velocity: Vec3::new(0.0, 2.0, 0.0), // small upward hop when spawned
            item,
            age: 0.0,
            despawn_time: 300.0, // 5 minutes
            pickup_delay: 0.5,   // 0.5s delay before magnetic pickup
            pickup_pending: false,
        }
    }

    pub fn with_velocity(position: Vec3, velocity: Vec3, item: ItemSlot) -> Self {
        Self {
            drop_id: next_local_drop_id(),
            position,
            velocity,
            item,
            age: 0.0,
            despawn_time: 300.0,
            pickup_delay: 0.5,
            pickup_pending: false,
        }
    }

    pub fn with_id_and_velocity(
        drop_id: u64,
        position: Vec3,
        velocity: Vec3,
        item: ItemSlot,
    ) -> Self {
        Self {
            drop_id,
            position,
            velocity,
            item,
            age: 0.0,
            despawn_time: 300.0,
            pickup_delay: 0.5,
            pickup_pending: false,
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        world: &World,
        registry: &BlockRegistry,
        player_pos: Vec3,
    ) -> ItemDropStatus {
        self.age += dt;
        if self.age >= self.despawn_time {
            return ItemDropStatus::Despawned;
        }

        if self.pickup_delay > 0.0 {
            self.pickup_delay -= dt;
        }

        // 1. Gravity, Buoyancy and Ground Collision
        let current_voxel = IVec3::new(
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );
        let (cp, lx, ly, lz) = world_to_chunk_coords(current_voxel);
        let current_block = if let Some(chunk) = world.get_chunk(cp) {
            chunk.get(lx, ly, lz).unwrap_or(BlockId::AIR)
        } else {
            BlockId::AIR
        };

        if current_block == BlockId::WATER || current_block == BlockId::SEAGRASS {
            // Water buoyancy: upward buoyant force + water drag
            self.velocity.y += 12.0 * dt;
            self.velocity.y = self.velocity.y.min(2.5); // Max upward float velocity
            self.velocity.x *= 0.85;
            self.velocity.z *= 0.85;
        } else {
            self.velocity.y -= 18.0 * dt;
            self.velocity.x *= 0.92;
            self.velocity.z *= 0.92;
        }

        let next_pos = self.position + self.velocity * dt;

        // Check if next position collides with a solid voxel
        let check_y = next_pos.y.floor() as i32;
        let check_x = next_pos.x.floor() as i32;
        let check_z = next_pos.z.floor() as i32;

        let (cp_next, nlx, nly, nlz) = world_to_chunk_coords(IVec3::new(check_x, check_y, check_z));
        let is_solid = if let Some(chunk) = world.get_chunk(cp_next) {
            chunk
                .get(nlx, nly, nlz)
                .map_or(false, |b| registry.is_solid(b))
        } else {
            false
        };

        if is_solid {
            self.position.y = (check_y + 1) as f32;
            self.velocity = Vec3::ZERO;
        } else {
            self.position = next_pos;
        }

        // 2. Player Magnetism & Collection
        if self.pickup_delay <= 0.0 {
            // Player's center of mass (mid-body at feet + 0.8)
            let player_center = player_pos + Vec3::new(0.0, 0.8, 0.0);
            let to_center = player_center - self.position;
            let dist = to_center.length();

            // Horizontal distance and vertical height difference from player feet
            let dx = self.position.x - player_pos.x;
            let dz = self.position.z - player_pos.z;
            let dy = self.position.y - player_pos.y;
            let horiz_dist = (dx * dx + dz * dz).sqrt();

            // Instant pickup if player steps near or over the item
            // (within 1.5m horizontally and within player vertical volume: -0.5m below feet to 2.0m above)
            if (horiz_dist < 1.5 && dy >= -0.5 && dy <= 2.0) || dist < 1.3 {
                return ItemDropStatus::Collected(self.item);
            } else if dist < 3.2 {
                let magnet_speed = 7.0;
                self.position += to_center.normalize() * magnet_speed * dt;
            }
        }

        ItemDropStatus::Alive
    }

    /// Checks if this item drop is in range and eligible to be collected by a player at `player_pos`.
    pub fn can_be_collected_by(&self, player_pos: Vec3) -> bool {
        if self.pickup_delay > 0.0 || self.pickup_pending {
            return false;
        }
        let player_center = player_pos + Vec3::new(0.0, 0.8, 0.0);
        let to_center = player_center - self.position;
        let dist = to_center.length();

        let dx = self.position.x - player_pos.x;
        let dz = self.position.z - player_pos.z;
        let dy = self.position.y - player_pos.y;
        let horiz_dist = (dx * dx + dz * dz).sqrt();

        (horiz_dist < 1.5 && dy >= -0.5 && dy <= 2.0) || dist < 1.3
    }

    /// Updates item drop physics, collision, and player magnetism without locally collecting it.
    /// Returns false if the drop despawned due to age.
    pub fn update_physics_only(
        &mut self,
        dt: f32,
        world: &World,
        registry: &BlockRegistry,
        player_pos: Vec3,
    ) -> bool {
        self.age += dt;
        if self.age >= self.despawn_time {
            return false;
        }

        if self.pickup_delay > 0.0 {
            self.pickup_delay -= dt;
        }

        // 1. Gravity, Buoyancy and Ground Collision
        let current_voxel = IVec3::new(
            self.position.x.floor() as i32,
            self.position.y.floor() as i32,
            self.position.z.floor() as i32,
        );
        let (cp, lx, ly, lz) = world_to_chunk_coords(current_voxel);
        let current_block = if let Some(chunk) = world.get_chunk(cp) {
            chunk.get(lx, ly, lz).unwrap_or(BlockId::AIR)
        } else {
            BlockId::AIR
        };

        if current_block == BlockId::WATER || current_block == BlockId::SEAGRASS {
            self.velocity.y += 12.0 * dt;
            self.velocity.y = self.velocity.y.min(2.5);
            self.velocity.x *= 0.85;
            self.velocity.z *= 0.85;
        } else {
            self.velocity.y -= 18.0 * dt;
            self.velocity.x *= 0.92;
            self.velocity.z *= 0.92;
        }

        let next_pos = self.position + self.velocity * dt;

        let check_y = next_pos.y.floor() as i32;
        let check_x = next_pos.x.floor() as i32;
        let check_z = next_pos.z.floor() as i32;

        let (cp_next, nlx, nly, nlz) = world_to_chunk_coords(IVec3::new(check_x, check_y, check_z));
        let is_solid = if let Some(chunk) = world.get_chunk(cp_next) {
            chunk
                .get(nlx, nly, nlz)
                .map_or(false, |b| registry.is_solid(b))
        } else {
            false
        };

        if is_solid {
            self.position.y = (check_y + 1) as f32;
            self.velocity = Vec3::ZERO;
        } else {
            self.position = next_pos;
        }

        // 2. Player Magnetism
        if self.pickup_delay <= 0.0 {
            let player_center = player_pos + Vec3::new(0.0, 0.8, 0.0);
            let to_center = player_center - self.position;
            let dist = to_center.length();
            if dist > 0.001 && dist < 3.2 {
                let magnet_speed = 7.0;
                self.position += to_center.normalize() * magnet_speed * dt;
            }
        }

        true
    }

    /// Renders visual bobbing height offset
    #[inline]
    pub fn visual_bob_y(&self) -> f32 {
        (self.age * 3.5).sin() * 0.08
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ItemId;

    #[test]
    fn test_item_drop_collection_within_radius() {
        let registry = BlockRegistry::new();
        let world = World::new();
        let mut drop = ItemDrop::new(Vec3::new(0.0, 5.0, 0.0), ItemSlot::new(ItemId::WOOD_LOG, 2));
        drop.pickup_delay = 0.0;

        // Player standing directly at item drop (player feet at y=5.0)
        let status = drop.update(0.1, &world, &registry, Vec3::new(0.0, 5.0, 0.0));
        assert_eq!(
            status,
            ItemDropStatus::Collected(ItemSlot::new(ItemId::WOOD_LOG, 2))
        );
    }

    #[test]
    fn test_item_drop_collection_while_walking_over() {
        let registry = BlockRegistry::new();
        let world = World::new();
        let mut drop = ItemDrop::new(Vec3::new(2.0, 5.0, 2.0), ItemSlot::new(ItemId::RAW_BEEF, 1));
        drop.pickup_delay = 0.0;

        // Player walking on the ground 1.0 block away horizontally
        let status = drop.update(0.1, &world, &registry, Vec3::new(2.8, 5.0, 2.0));
        assert_eq!(
            status,
            ItemDropStatus::Collected(ItemSlot::new(ItemId::RAW_BEEF, 1))
        );
    }

    #[test]
    fn test_item_drop_magnetism_attracts_towards_player() {
        let registry = BlockRegistry::new();
        let world = World::new();
        let mut drop = ItemDrop::new(Vec3::new(0.0, 5.0, 0.0), ItemSlot::new(ItemId::COAL, 1));
        drop.pickup_delay = 0.0;

        // Player standing 2.5 blocks away horizontally
        let player_pos = Vec3::new(2.5, 5.0, 0.0);
        let initial_dist = (drop.position - player_pos).length();

        let status = drop.update(0.1, &world, &registry, player_pos);
        assert_eq!(status, ItemDropStatus::Alive);

        let new_dist = (drop.position - player_pos).length();
        assert!(
            new_dist < initial_dist,
            "Magnetism should attract item toward player"
        );
    }

    #[test]
    fn test_falling_sand_and_gravel() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let chunk = crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0));
        world.insert_chunk(chunk);

        // 1. Sand falls through air onto solid stone
        world.set_block(IVec3::new(5, 5, 5), BlockId::STONE);
        let mut sand = FallingBlock::new(BlockId::SAND, IVec3::new(5, 10, 5));

        let mut landed_pos = None;
        for _ in 0..100 {
            match sand.update(0.05, &world, &registry) {
                FallingBlockStatus::Falling => {}
                FallingBlockStatus::Landed(pos) => {
                    landed_pos = Some(pos);
                    break;
                }
                FallingBlockStatus::Despawned => break,
            }
        }
        assert_eq!(landed_pos, Some(IVec3::new(5, 6, 5)));

        // 2. Gravel falls through water and lands on riverbed
        world.set_block(IVec3::new(8, 2, 8), BlockId::DIRT);
        world.set_block(IVec3::new(8, 3, 8), BlockId::WATER);
        world.set_block(IVec3::new(8, 4, 8), BlockId::WATER);
        let mut gravel = FallingBlock::new(BlockId::GRAVEL, IVec3::new(8, 7, 8));

        let mut gravel_landed = None;
        for _ in 0..100 {
            match gravel.update(0.05, &world, &registry) {
                FallingBlockStatus::Falling => {}
                FallingBlockStatus::Landed(pos) => {
                    gravel_landed = Some(pos);
                    break;
                }
                FallingBlockStatus::Despawned => break,
            }
        }
        // Lands at (8, 3, 8) directly on top of DIRT bed at (8, 2, 8), displacing the water at y=3
        assert_eq!(gravel_landed, Some(IVec3::new(8, 3, 8)));

        // 3. Falling block into void despawns
        let mut void_sand = FallingBlock::with_velocity(BlockId::SAND, Vec3::new(0.0, -60.0, 0.0), -20.0);
        let status = void_sand.update(0.5, &world, &registry);
        assert_eq!(status, FallingBlockStatus::Despawned);
    }

    #[test]
    fn test_item_drop_buoyancy_in_water() {
        let mut world = World::new();
        let registry = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Water at y=10 and y=11, air at y=12
        world.set_block(IVec3::new(5, 10, 5), BlockId::WATER);
        world.set_block(IVec3::new(5, 11, 5), BlockId::WATER);
        world.set_block(IVec3::new(5, 12, 5), BlockId::AIR);

        let mut drop = ItemDrop::new(
            Vec3::new(5.0, 10.2, 5.0),
            ItemSlot::new(crate::item::ItemId::COAL, 1),
        );
        drop.velocity = Vec3::ZERO;

        // In water, buoyancy pulls upward
        let status = drop.update(0.1, &world, &registry, Vec3::new(100.0, 100.0, 100.0));
        assert_eq!(status, ItemDropStatus::Alive);
        assert!(drop.velocity.y > 0.0, "Item in water must accelerate upward from buoyancy");
        assert!(drop.position.y > 10.2, "Item position must rise toward surface");
    }
}
