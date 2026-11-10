use glam::{IVec3, Vec3};

use crate::{
    block::BlockRegistry,
    item::ItemSlot,
    world::{world_to_chunk_coords, World},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemDropStatus {
    Alive,
    Collected(ItemSlot),
    Despawned,
}

#[derive(Debug, Clone)]
pub struct ItemDrop {
    pub position: Vec3,
    pub velocity: Vec3,
    pub item: ItemSlot,
    pub age: f32,
    pub despawn_time: f32,
    pub pickup_delay: f32,
}

impl ItemDrop {
    pub fn new(position: Vec3, item: ItemSlot) -> Self {
        Self {
            position,
            velocity: Vec3::new(0.0, 2.0, 0.0), // small upward hop when spawned
            item,
            age: 0.0,
            despawn_time: 300.0, // 5 minutes
            pickup_delay: 0.5,   // 0.5s delay before magnetic pickup
        }
    }

    pub fn with_velocity(position: Vec3, velocity: Vec3, item: ItemSlot) -> Self {
        Self {
            position,
            velocity,
            item,
            age: 0.0,
            despawn_time: 300.0,
            pickup_delay: 0.5,
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

        // 1. Gravity and Ground Collision
        self.velocity.y -= 18.0 * dt;
        self.velocity.x *= 0.92;
        self.velocity.z *= 0.92;

        let next_pos = self.position + self.velocity * dt;

        // Check if next position collides with a solid voxel
        let check_y = next_pos.y.floor() as i32;
        let check_x = next_pos.x.floor() as i32;
        let check_z = next_pos.z.floor() as i32;

        let (cp, lx, ly, lz) = world_to_chunk_coords(IVec3::new(check_x, check_y, check_z));
        let is_solid = if let Some(chunk) = world.get_chunk(cp) {
            chunk
                .get(lx, ly, lz)
                .map_or(false, |b| !registry.is_transparent(b))
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
}
