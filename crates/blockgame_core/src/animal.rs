use glam::{IVec3, Vec3};
use std::f32::consts::PI;

use crate::{
    block::BlockRegistry,
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
}

impl AnimalEntity {
    pub fn new(id: u32, kind: AnimalKind, position: Vec3) -> Self {
        let max_hp = kind.max_health();
        Self {
            id,
            kind,
            position,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            health: max_hp,
            max_health: max_hp,
            hurt_timer: 0.0,
            flee_timer: 0.0,
            flee_dir: Vec3::ZERO,
            wander_timer: 2.0,
            is_moving: false,
            walk_anim_timer: 0.0,
            on_ground: false,
            dead: false,
        }
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

        if self.hurt_timer > 0.0 {
            self.hurt_timer = (self.hurt_timer - dt).max(0.0);
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
                // Change state: moving or standing still
                let pseudo_rand = ((self.position.x * 12.9898 + self.position.z * 78.233)
                    .sin()
                    .fract()
                    + 1.0)
                    * 0.5;
                if pseudo_rand > 0.45 {
                    self.is_moving = true;
                    // Pick a random direction
                    let angle = pseudo_rand * 2.0 * PI;
                    self.yaw = angle;
                    self.wander_timer = 2.0 + pseudo_rand * 3.0;
                } else {
                    self.is_moving = false;
                    self.wander_timer = 1.5 + pseudo_rand * 2.0;
                }
            }

            if self.is_moving {
                wish_dir = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
            }
        }

        // 2. Physics & Movement
        let gravity = -28.0;
        self.velocity.y += gravity * dt;

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

        // 3. Block Step/Jump Check
        if self.on_ground && horiz_speed > 0.2 {
            let front_check = self.position + wish_dir * 0.6;
            let check_x = front_check.x.floor() as i32;
            let check_y = self.position.y.floor() as i32;
            let check_z = front_check.z.floor() as i32;

            let block_in_front = world.get_block(IVec3::new(check_x, check_y, check_z));
            let block_above_front = world.get_block(IVec3::new(check_x, check_y + 1, check_z));

            // Jump if obstacle is 1 block high with air above
            if registry.is_solid(block_in_front) && !registry.is_solid(block_above_front) {
                self.velocity.y = 7.0;
                self.on_ground = false;
            }
        }

        // 4. Swept Axis-Separated Collision Resolution
        self.resolve_movement(dt, world, registry);
    }

    fn resolve_movement(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        let (half_width, height) = self.kind.dimensions();

        // Step Y
        self.position.y += self.velocity.y * dt;
        let mut bbox = self.aabb();

        let min_x = (bbox.min.x.floor() as i32) - 1;
        let max_x = (bbox.max.x.ceil() as i32) + 1;
        let min_y = (bbox.min.y.floor() as i32) - 1;
        let max_y = (bbox.max.y.ceil() as i32) + 1;
        let min_z = (bbox.min.z.floor() as i32) - 1;
        let max_z = (bbox.max.z.ceil() as i32) + 1;

        self.on_ground = false;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if bbox.intersects(&block_box) {
                            if self.velocity.y < 0.0 {
                                self.position.y = block_box.max.y;
                                self.velocity.y = 0.0;
                                self.on_ground = true;
                            } else if self.velocity.y > 0.0 {
                                self.position.y = block_box.min.y - height;
                                self.velocity.y = 0.0;
                            }
                            bbox = self.aabb();
                        }
                    }
                }
            }
        }

        // Step X
        self.position.x += self.velocity.x * dt;
        bbox = self.aabb();
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if bbox.intersects(&block_box) {
                            if self.velocity.x > 0.0 {
                                self.position.x = block_box.min.x - half_width;
                            } else if self.velocity.x < 0.0 {
                                self.position.x = block_box.max.x + half_width;
                            }
                            self.velocity.x = 0.0;
                            bbox = self.aabb();
                        }
                    }
                }
            }
        }

        // Step Z
        self.position.z += self.velocity.z * dt;
        bbox = self.aabb();
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let block = world.get_block(IVec3::new(x, y, z));
                    if registry.is_solid(block) {
                        let block_box = AABB::from_block(IVec3::new(x, y, z));
                        if bbox.intersects(&block_box) {
                            if self.velocity.z > 0.0 {
                                self.position.z = block_box.min.z - half_width;
                            } else if self.velocity.z < 0.0 {
                                self.position.z = block_box.max.z + half_width;
                            }
                            self.velocity.z = 0.0;
                            bbox = self.aabb();
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
}
