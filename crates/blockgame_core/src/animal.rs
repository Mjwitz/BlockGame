use glam::{IVec3, Vec3};
use std::f32::consts::PI;

use crate::{
    block::{BlockId, BlockRegistry},
    item::{ItemId, ItemSlot},
    physics::AABB,
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimalKind {
    Cow,
    Pig,
    Chicken,
}

impl AnimalKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cow => "Cow",
            Self::Pig => "Pig",
            Self::Chicken => "Chicken",
        }
    }

    pub fn dimensions(&self) -> (f32, f32) {
        // (half_width, height)
        match self {
            Self::Cow => (0.45, 1.4),
            Self::Pig => (0.45, 0.9),
            Self::Chicken => (0.25, 0.7),
        }
    }

    pub fn max_health(&self) -> f32 {
        match self {
            Self::Cow => 10.0,
            Self::Pig => 10.0,
            Self::Chicken => 4.0,
        }
    }

    pub fn walk_speed(&self) -> f32 {
        match self {
            Self::Cow => 1.2,
            Self::Pig => 1.3,
            Self::Chicken => 1.4,
        }
    }

    pub fn flee_speed(&self) -> f32 {
        match self {
            Self::Cow => 3.2,
            Self::Pig => 3.5,
            Self::Chicken => 3.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnimalEntity {
    pub id: u32,
    pub kind: AnimalKind,
    pub position: Vec3, // Feet position
    pub velocity: Vec3,
    pub yaw: f32, // Horizontal angle in radians
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub flee_timer: f32,
    pub flee_dir: Vec3,
    pub wander_timer: f32,
    pub is_moving: bool,
    pub walk_anim_timer: f32,
    pub on_ground: bool,
    pub dead: bool,
    pub rng_state: u64,
    pub stuck_timer: f32,
    pub breath: f32,
    pub max_breath: f32,
    pub drown_timer: f32,
}

impl AnimalEntity {
    pub fn new(id: u32, kind: AnimalKind, position: Vec3) -> Self {
        let max_hp = kind.max_health();
        let mut seed = ((id as u64) + 1).wrapping_mul(0x517cc1b727220a95);
        seed ^= (position.x.to_bits() as u64).wrapping_mul(0x6c62272e07bb0142);
        seed ^= (position.z.to_bits() as u64).wrapping_mul(0x62b821756295c58d);
        if seed == 0 {
            seed = 0x853c49e6748fea9b;
        }
        let wander_init = 1.0 + (id % 3) as f32 * 0.8;
        Self {
            id,
            kind,
            position,
            velocity: Vec3::ZERO,
            yaw: ((id * 37) % 360) as f32 * (PI / 180.0),
            health: max_hp,
            max_health: max_hp,
            hurt_timer: 0.0,
            flee_timer: 0.0,
            flee_dir: Vec3::ZERO,
            wander_timer: wander_init,
            is_moving: true,
            walk_anim_timer: 0.0,
            on_ground: false,
            dead: false,
            rng_state: seed,
            stuck_timer: 0.0,
            breath: 15.0,
            max_breath: 15.0,
            drown_timer: 0.0,
        }
    }

    /// Advances the animal's internal PRNG state and returns a uniform pseudo-random f32 in 0.0..1.0.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        if self.rng_state == 0 {
            self.rng_state = 0x853c49e6748fea9b;
        }
        ((self.rng_state >> 32) as u32 as f32) / (u32::MAX as f32)
    }

    pub fn aabb(&self) -> AABB {
        let (half_width, height) = self.kind.dimensions();
        AABB::from_player(self.position, half_width, height)
    }

    pub fn take_damage(&mut self, amount: f32, knockback_dir: Vec3) {
        if self.dead {
            return;
        }
        self.health = (self.health - amount).max(0.0);
        self.hurt_timer = 0.35;
        self.flee_timer = 3.5;

        // Flatten knockback to horizontal plane with a small upward hop
        let horiz_dir = Vec3::new(knockback_dir.x, 0.0, knockback_dir.z).normalize_or_zero();
        let actual_flee = if horiz_dir.length_squared() > 0.01 {
            horiz_dir
        } else {
            Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos())
        };
        self.flee_dir = actual_flee;
        self.velocity = actual_flee * 5.0 + Vec3::new(0.0, 4.0, 0.0);
        self.yaw = actual_flee.x.atan2(actual_flee.z);

        if self.health <= 0.0 {
            self.dead = true;
        }
    }

    pub fn generate_drops(&self) -> Vec<ItemSlot> {
        let mut drops = Vec::new();
        // Seed-based / pseudo-random deterministic drop quantities based on id & health
        let hash = (self.id as u64).wrapping_mul(2654435761);
        let roll1 = ((hash % 3) + 1) as u16; // 1 to 3
        let roll2 = ((hash >> 4) % 2) as u16; // 0 or 1

        match self.kind {
            AnimalKind::Cow => {
                drops.push(ItemSlot::new(ItemId::RAW_BEEF, roll1));
                if roll2 > 0 {
                    drops.push(ItemSlot::new(ItemId::LEATHER, roll2));
                }
            }
            AnimalKind::Pig => {
                drops.push(ItemSlot::new(ItemId::RAW_PORKCHOP, roll1));
            }
            AnimalKind::Chicken => {
                drops.push(ItemSlot::new(ItemId::RAW_CHICKEN, 1));
                if roll2 > 0 {
                    drops.push(ItemSlot::new(ItemId::FEATHER, roll2 + 1));
                }
            }
        }

        drops
    }

    pub fn update(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        if self.dead {
            return;
        }

        if self.position.y < -40.0 {
            self.dead = true;
            return;
        }

        if self.hurt_timer > 0.0 {
            self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        }

        // 0. Water & Drowning Check
        let (_half_width, height) = self.kind.dimensions();
        let head_y = (self.position.y + height * 0.85).floor() as i32;
        let head_x = self.position.x.floor() as i32;
        let head_z = self.position.z.floor() as i32;
        let is_head_submerged = world.get_block(IVec3::new(head_x, head_y, head_z)).is_water();

        let feet_y = self.position.y.floor() as i32;
        let is_in_water = is_head_submerged || world.get_block(IVec3::new(head_x, feet_y, head_z)).is_water();

        if is_head_submerged {
            if self.breath > 0.0 {
                if self.breath >= dt {
                    self.breath -= dt;
                } else {
                    let excess = dt - self.breath;
                    self.breath = 0.0;
                    self.drown_timer += excess;
                    while self.drown_timer >= 1.0 {
                        self.drown_timer -= 1.0;
                        self.take_damage(2.0, Vec3::ZERO);
                    }
                }
            } else {
                self.drown_timer += dt;
                while self.drown_timer >= 1.0 {
                    self.drown_timer -= 1.0;
                    self.take_damage(2.0, Vec3::ZERO);
                }
            }
        } else {
            self.drown_timer = 0.0;
            self.breath = (self.breath + dt * 5.0).min(self.max_breath);
        }

        // 0.5 Cactus Contact Damage Check
        let (half_width, height) = self.kind.dimensions();
        let min_x = (self.position.x - half_width - 0.05).floor() as i32;
        let max_x = (self.position.x + half_width + 0.05).floor() as i32;
        let min_y = (self.position.y.floor() as i32).max(0);
        let max_y = ((self.position.y + height).floor() as i32).min((crate::chunk::VERTICAL_CHUNKS * 16) - 1);
        let min_z = (self.position.z - half_width - 0.05).floor() as i32;
        let max_z = (self.position.z + half_width + 0.05).floor() as i32;
        'cactus_check: for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if world.get_block(IVec3::new(x, y, z)) == BlockId::CACTUS {
                        if self.hurt_timer <= 0.0 {
                            let cactus_center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                            self.take_damage(1.0, cactus_center);
                        }
                        break 'cactus_check;
                    }
                }
            }
        }

        // 1. AI Decision Making
        let mut wish_dir = Vec3::ZERO;

        if self.flee_timer > 0.0 {
            self.flee_timer = (self.flee_timer - dt).max(0.0);
            wish_dir = self.flee_dir;
            self.is_moving = true;
        } else {
            self.wander_timer -= dt;
            if self.wander_timer <= 0.0 {
                let r = self.next_f32();
                if r < 0.60 {
                    // Start moving
                    self.is_moving = true;
                    let change = (self.next_f32() - 0.5) * 1.8;
                    let new_angle = if self.next_f32() < 0.35 {
                        self.next_f32() * 2.0 * PI
                    } else {
                        self.yaw + change
                    };
                    self.yaw = new_angle;
                    self.wander_timer = 2.0 + self.next_f32() * 4.0;
                } else {
                    // Idle in place
                    self.is_moving = false;
                    self.wander_timer = 1.5 + self.next_f32() * 2.5;
                }
            }

            if self.is_moving {
                wish_dir = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
            }
        }

        // 2. Physics & Movement
        let gravity = if is_in_water {
            -6.0 // Substantially reduced gravity in water
        } else if self.kind == AnimalKind::Chicken {
            -12.0 // Chickens fall much slower (wing flapping flutter)
        } else {
            -28.0
        };
        self.velocity.y += gravity * dt;
        if self.kind == AnimalKind::Chicken && self.velocity.y < -2.2 {
            self.velocity.y = -2.2; // Chicken flutter terminal velocity
        }

        // Water drag & swimming buoyancy
        if is_in_water {
            self.velocity.y *= 0.85;
            self.velocity.y += 10.0 * dt; // Instinctively paddle upward
            if self.velocity.y > 2.5 {
                self.velocity.y = 2.5;
            }
        }

        let target_speed = if self.flee_timer > 0.0 {
            self.kind.flee_speed()
        } else if self.is_moving {
            self.kind.walk_speed()
        } else {
            0.0
        };

        let target_vel = wish_dir * target_speed;
        let accel = if self.on_ground { 12.0 } else { 2.0 };
        self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
        self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);

        let horiz_speed =
            (self.velocity.x * self.velocity.x + self.velocity.z * self.velocity.z).sqrt();
        if horiz_speed > 0.05 {
            self.walk_anim_timer += dt * horiz_speed * 4.0;
            if self.flee_timer <= 0.0 {
                self.yaw = self.velocity.x.atan2(self.velocity.z);
            }
        }

        // 3. Block Step/Jump Check & Stuck Resolution
        if self.on_ground && (self.is_moving || self.flee_timer > 0.0) {
            let front_check = self.position + wish_dir * 0.55;
            let check_x = front_check.x.floor() as i32;
            let check_y = self.position.y.floor() as i32;
            let check_z = front_check.z.floor() as i32;

            let block_in_front = world.get_block(IVec3::new(check_x, check_y, check_z));
            let block_above_front = world.get_block(IVec3::new(check_x, check_y + 1, check_z));

            // Jump if obstacle is 1 block high with air above
            if registry.is_solid(block_in_front) && !registry.is_solid(block_above_front) {
                self.velocity.y = 7.0;
                self.on_ground = false;
                self.stuck_timer = 0.0;
            } else if registry.is_solid(block_in_front) && registry.is_solid(block_above_front) {
                // High obstacle (wall/tree): turn away smoothly
                if self.flee_timer <= 0.0 {
                    self.yaw += PI * 0.5 + self.next_f32() * PI;
                    self.wander_timer = 1.5 + self.next_f32() * 2.5;
                }
            }
        }

        // Stuck detection: if trying to walk but moving very slowly, pick a new direction
        if self.is_moving && self.on_ground && horiz_speed < 0.12 {
            self.stuck_timer += dt;
            if self.stuck_timer > 0.75 {
                self.yaw += PI * 0.5 + self.next_f32() * PI;
                self.wander_timer = 2.0 + self.next_f32() * 3.0;
                self.stuck_timer = 0.0;
            }
        } else {
            self.stuck_timer = (self.stuck_timer - dt * 2.0).max(0.0);
        }

        // 4. Swept Axis-Separated Collision Resolution
        self.resolve_movement(dt, world, registry);
    }

    fn resolve_movement(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        let (half_width, height) = self.kind.dimensions();
        crate::physics::resolve_entity_movement(
            &mut self.position,
            &mut self.velocity,
            &mut self.on_ground,
            half_width,
            height,
            dt,
            world,
            registry,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::BlockId,
        chunk::{Chunk, ChunkPos},
    };

    #[test]
    fn test_animal_initialization_and_damage() {
        let mut cow = AnimalEntity::new(1, AnimalKind::Cow, Vec3::new(10.0, 64.0, 10.0));
        assert_eq!(cow.health, 10.0);
        assert!(!cow.dead);

        // Take 4 damage
        cow.take_damage(4.0, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(cow.health, 6.0);
        assert!(cow.hurt_timer > 0.0);
        assert!(cow.flee_timer > 0.0);
        assert!(!cow.dead);

        // Lethal damage
        cow.take_damage(10.0, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(cow.health, 0.0);
        assert!(cow.dead);

        let drops = cow.generate_drops();
        assert!(!drops.is_empty());
        assert_eq!(drops[0].item_id, ItemId::RAW_BEEF);
    }

    #[test]
    fn test_pig_and_chicken_drops() {
        let mut pig = AnimalEntity::new(2, AnimalKind::Pig, Vec3::ZERO);
        pig.take_damage(100.0, Vec3::X);
        let pig_drops = pig.generate_drops();
        assert_eq!(pig_drops[0].item_id, ItemId::RAW_PORKCHOP);

        let mut chicken = AnimalEntity::new(3, AnimalKind::Chicken, Vec3::ZERO);
        chicken.take_damage(100.0, Vec3::Z);
        let chicken_drops = chicken.generate_drops();
        assert_eq!(chicken_drops[0].item_id, ItemId::RAW_CHICKEN);
    }

    #[test]
    fn test_animal_ai_wander_and_no_permanent_freeze() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let chunk_pos = ChunkPos::new(0, 3, 0);
        let mut chunk = Chunk::new(chunk_pos);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 15, z, BlockId::GRASS);
            }
        }
        world.insert_chunk(chunk);

        let mut cow = AnimalEntity::new(4, AnimalKind::Cow, Vec3::new(8.0, 64.0, 8.0));

        // Track state transitions over 200 ticks on solid ground
        let start_pos = cow.position;
        let mut moved = false;

        for _ in 0..200 {
            cow.update(0.05, &world, &registry);
            if cow.position.x != start_pos.x || cow.position.z != start_pos.z {
                moved = true;
            }
        }

        assert!(!cow.dead, "Animal on solid ground should remain alive");
        assert!(moved, "Animal should wander and change horizontal position over 10 seconds");
    }

    #[test]
    fn test_animal_drowning_when_submerged() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let chunk_pos = ChunkPos::new(0, 0, 0);
        let mut chunk = Chunk::new(chunk_pos);
        // Fill chunk with water so animal is completely submerged
        chunk.fill(BlockId::WATER);
        // Floor at y=0..2 and ceiling at y=8..16 so animal stays completely submerged
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..2 {
                    chunk.set(x, y, z, BlockId::STONE);
                }
                for y in 8..16 {
                    chunk.set(x, y, z, BlockId::STONE);
                }
            }
        }
        world.insert_chunk(chunk);

        let mut cow = AnimalEntity::new(5, AnimalKind::Cow, Vec3::new(8.0, 3.0, 8.0));
        cow.is_moving = false;
        cow.wander_timer = 9999.0;
        assert_eq!(cow.breath, 15.0);
        assert_eq!(cow.health, 10.0);

        // Submerge for 15 seconds: breath goes to 0
        for _ in 0..150 {
            cow.update(0.1, &world, &registry);
        }
        assert_eq!(cow.breath, 0.0);
        assert_eq!(cow.health, 10.0); // Not damaged yet until timer ticks

        // 1 more second submerged: drown damage triggers
        for _ in 0..10 {
            cow.update(0.1, &world, &registry);
        }
        assert!(cow.health < 10.0, "Submerged animal should take drowning damage");
    }
}
