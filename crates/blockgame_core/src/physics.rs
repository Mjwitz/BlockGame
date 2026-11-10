use glam::{IVec3, Vec3};

use crate::{
    block::{BlockId, BlockRegistry},
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn from_block(pos: IVec3) -> Self {
        let min = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        Self {
            min,
            max: min + Vec3::ONE,
        }
    }

    #[inline]
    pub fn from_player(position: Vec3, half_width: f32, height: f32) -> Self {
        Self {
            min: Vec3::new(position.x - half_width, position.y, position.z - half_width),
            max: Vec3::new(
                position.x + half_width,
                position.y + height,
                position.z + half_width,
            ),
        }
    }

    #[inline]
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    #[inline]
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Fast slab ray-AABB intersection test returning distance `t` along `ray_dir` if hit.
    pub fn intersect_ray(&self, ray_origin: Vec3, ray_dir: Vec3, max_dist: f32) -> Option<f32> {
        let mut tmin = 0.0_f32;
        let mut tmax = max_dist;

        for i in 0..3 {
            let origin = ray_origin[i];
            let dir = ray_dir[i];
            let min = self.min[i];
            let max = self.max[i];

            if dir.abs() < 1e-6 {
                if origin < min || origin > max {
                    return None;
                }
            } else {
                let ood = 1.0 / dir;
                let mut t1 = (min - origin) * ood;
                let mut t2 = (max - origin) * ood;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some(tmin)
    }
}

pub struct PlayerPhysics {
    pub position: Vec3, // Feet position
    pub velocity: Vec3,
    pub half_width: f32, // 0.3 (total width 0.6)
    pub height: f32,     // 1.8
    pub eye_offset: f32, // 1.62
    pub on_ground: bool,
    pub flying: bool,
    pub fall_distance: f32,
}

impl PlayerPhysics {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            half_width: 0.3,
            height: 1.8,
            eye_offset: 1.62,
            on_ground: false,
            flying: false,
            fall_distance: 0.0,
        }
    }

    #[inline]
    pub fn eye_position(&self) -> Vec3 {
        Vec3::new(
            self.position.x,
            self.position.y + self.eye_offset,
            self.position.z,
        )
    }

    #[inline]
    pub fn aabb(&self) -> AABB {
        AABB::from_player(self.position, self.half_width, self.height)
    }

    /// Toggles flying mode (spectator vs standard physics)
    pub fn toggle_flight(&mut self) {
        self.flying = !self.flying;
        if self.flying {
            self.velocity = Vec3::ZERO;
            self.fall_distance = 0.0;
        }
    }

    /// Returns whether the player's bounding box is submerged or touching water.
    pub fn is_in_water(&self, world: &World) -> bool {
        let player_box = self.aabb();
        let min_x = player_box.min.x.floor() as i32;
        let max_x = player_box.max.x.floor() as i32;
        let min_y = player_box.min.y.floor() as i32;
        let max_y = player_box.max.y.floor() as i32;
        let min_z = player_box.min.z.floor() as i32;
        let max_z = player_box.max.z.floor() as i32;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if world.get_block(IVec3::new(x, y, z)) == BlockId::WATER {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Updates player physics over a delta time `dt` against the voxel `world`.
    /// Returns any fall damage incurred upon landing.
    pub fn update(
        &mut self,
        dt: f32,
        wish_dir: Vec3,
        sprinting: bool,
        jumping: bool,
        world: &World,
        registry: &BlockRegistry,
    ) -> f32 {
        if self.flying {
            // Free flight mode
            let speed = if sprinting { 30.0 } else { 15.0 };
            self.position += wish_dir * speed * dt;
            self.velocity = Vec3::ZERO;
            self.on_ground = false;
            self.fall_distance = 0.0;
            return 0.0;
        }

        let in_water = self.is_in_water(world);
        let prev_y = self.position.y;
        let was_on_ground = self.on_ground;

        if in_water {
            // Cancel fall damage in water
            self.fall_distance = 0.0;

            // Water gravity & buoyancy
            let water_gravity = -6.0;
            self.velocity.y += water_gravity * dt;

            // Water vertical drag
            self.velocity.y *= (1.0 - 2.5 * dt).clamp(0.0, 1.0);

            // Swimming upward
            if jumping {
                self.velocity.y = 4.0;
            }

            // Water horizontal movement
            let target_speed = if sprinting { 3.8 } else { 2.4 };
            let target_vel = wish_dir * target_speed;
            let accel = 8.0;
            self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
            self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);
        } else {
            // Apply gravity
            let gravity = -28.0;
            self.velocity.y += gravity * dt;

            // Ground jumping
            if self.on_ground && jumping {
                self.velocity.y = 8.5; // Jump velocity
                self.on_ground = false;
                self.fall_distance = 0.0;
            }

            // Horizontal movement & acceleration
            let target_speed = if sprinting { 6.0 } else { 4.3 };
            let target_vel = wish_dir * target_speed;

            let accel = if self.on_ground { 15.0 } else { 3.0 };
            self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
            self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);
        }

        // Swept axis-separated collision resolution
        self.resolve_movement(dt, world, registry);

        // Calculate fall distance and landing damage
        let mut fall_damage = 0.0;
        if in_water {
            self.fall_distance = 0.0;
        } else if self.on_ground {
            if !was_on_ground && self.fall_distance > 3.0 {
                fall_damage = self.fall_distance - 3.0;
            }
            self.fall_distance = 0.0;
        } else {
            if self.position.y < prev_y {
                self.fall_distance += prev_y - self.position.y;
            } else if self.velocity.y > 0.0 {
                self.fall_distance = 0.0;
            }
        }

        fall_damage
    }

    fn resolve_movement(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        // Step Y axis
        self.position.y += self.velocity.y * dt;
        let mut player_box = self.aabb();

        let min_x = (player_box.min.x.floor() as i32) - 1;
        let max_x = (player_box.max.x.ceil() as i32) + 1;
        let min_y = (player_box.min.y.floor() as i32) - 1;
        let max_y = (player_box.max.y.ceil() as i32) + 1;
        let min_z = (player_box.min.z.floor() as i32) - 1;
        let max_z = (player_box.max.z.ceil() as i32) + 1;

        self.on_ground = false;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if player_box.intersects(&block_box) {
                            if self.velocity.y < 0.0 {
                                // Landed on ground
                                self.position.y = block_box.max.y;
                                self.velocity.y = 0.0;
                                self.on_ground = true;
                            } else if self.velocity.y > 0.0 {
                                // Hit ceiling
                                self.position.y = block_box.min.y - self.height;
                                self.velocity.y = 0.0;
                            }
                            player_box = self.aabb();
                        }
                    }
                }
            }
        }

        // Step X axis
        self.position.x += self.velocity.x * dt;
        player_box = self.aabb();

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if player_box.intersects(&block_box) {
                            if self.velocity.x > 0.0 {
                                self.position.x = block_box.min.x - self.half_width;
                                self.velocity.x = 0.0;
                            } else if self.velocity.x < 0.0 {
                                self.position.x = block_box.max.x + self.half_width;
                                self.velocity.x = 0.0;
                            }
                            player_box = self.aabb();
                        }
                    }
                }
            }
        }

        // Step Z axis
        self.position.z += self.velocity.z * dt;
        player_box = self.aabb();

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if player_box.intersects(&block_box) {
                            if self.velocity.z > 0.0 {
                                self.position.z = block_box.min.z - self.half_width;
                                self.velocity.z = 0.0;
                            } else if self.velocity.z < 0.0 {
                                self.position.z = block_box.max.z + self.half_width;
                                self.velocity.z = 0.0;
                            }
                            player_box = self.aabb();
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
    use crate::chunk::{Chunk, ChunkPos};

    #[test]
    fn test_aabb_intersection() {
        let box1 = AABB::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let box2 = AABB::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5));
        let box3 = AABB::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));

        assert!(box1.intersects(&box2));
        assert!(!box1.intersects(&box3));
    }

    #[test]
    fn test_player_landing_on_solid_ground() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Create solid stone floor at y = 0
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, crate::block::BlockId::STONE);
            }
        }
        world.insert_chunk(chunk);

        // Drop player from y = 5.0
        let mut player = PlayerPhysics::new(Vec3::new(8.0, 5.0, 8.0));
        assert!(!player.on_ground);

        // Step simulation for 1.0 second
        for _ in 0..60 {
            player.update(1.0 / 60.0, Vec3::ZERO, false, false, &world, &registry);
        }

        // Player must have landed on top of block at y = 1.0
        assert!(player.on_ground);
        assert!((player.position.y - 1.0).abs() < 0.01);
        assert_eq!(player.velocity.y, 0.0);
    }

    #[test]
    fn test_player_fall_damage() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, crate::block::BlockId::STONE);
            }
        }
        world.insert_chunk(chunk);

        // Drop player from y = 10.0 (falls 9.0 blocks down to y = 1.0)
        let mut player = PlayerPhysics::new(Vec3::new(8.0, 10.0, 8.0));
        let mut total_fall_damage = 0.0;

        for _ in 0..60 {
            let dmg = player.update(1.0 / 60.0, Vec3::ZERO, false, false, &world, &registry);
            total_fall_damage += dmg;
        }

        assert!(player.on_ground);
        // Fell 9.0 blocks, threshold is 3.0 blocks -> ~6.0 damage (3 hearts)
        assert!(
            total_fall_damage > 5.5 && total_fall_damage < 6.5,
            "Expected ~6.0 fall damage, got {}",
            total_fall_damage
        );
    }
}
