use glam::{IVec3, Vec3};
use std::f32::consts::PI;

use crate::{
    block::BlockRegistry,
    item::{ItemId, ItemSlot},
    physics::{aabb_intersects_solid, resolve_entity_movement, AABB},
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonsterKind {
    Zombie,
    Skeleton,
    Spider,
}

impl MonsterKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Zombie => "Zombie",
            Self::Skeleton => "Skeleton",
            Self::Spider => "Spider",
        }
    }

    pub fn dimensions(&self) -> (f32, f32) {
        match self {
            Self::Zombie => (0.35, 1.95),
            Self::Skeleton => (0.35, 1.95),
            Self::Spider => (0.65, 0.90),
        }
    }

    pub fn max_health(&self) -> f32 {
        match self {
            Self::Zombie => 20.0,
            Self::Skeleton => 20.0,
            Self::Spider => 16.0,
        }
    }

    pub fn move_speed(&self) -> f32 {
        match self {
            Self::Zombie => 2.3,
            Self::Skeleton => 2.5,
            Self::Spider => 3.2,
        }
    }

    pub fn attack_damage(&self) -> f32 {
        match self {
            Self::Zombie => 3.0,
            Self::Skeleton => 3.0,
            Self::Spider => 2.0,
        }
    }

    pub fn burns_in_sunlight(&self) -> bool {
        matches!(self, Self::Zombie | Self::Skeleton)
    }
}

#[derive(Debug, Clone)]
pub struct ArrowProjectile {
    pub id: u32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub from_player: bool,
    pub damage: f32,
    pub age: f32,
    pub stuck: bool,
    pub stuck_timer: f32,
    pub dead: bool,
}

impl ArrowProjectile {
    pub fn new(id: u32, position: Vec3, velocity: Vec3, from_player: bool, damage: f32) -> Self {
        Self {
            id,
            position,
            velocity,
            from_player,
            damage,
            age: 0.0,
            stuck: false,
            stuck_timer: 0.0,
            dead: false,
        }
    }

    pub fn update(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        if self.dead {
            return;
        }
        self.age += dt;
        if self.age > 20.0 {
            self.dead = true;
            return;
        }

        if self.stuck {
            self.stuck_timer += dt;
            if self.stuck_timer > 4.0 {
                self.dead = true;
            }
            return;
        }

        // Apply gravity
        self.velocity.y -= 18.0 * dt;
        let next_pos = self.position + self.velocity * dt;

        // Check block collision along step
        let steps = ((next_pos - self.position).length() / 0.25).ceil().max(1.0) as usize;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let probe = self.position.lerp(next_pos, t);
            let bpos = IVec3::new(
                probe.x.floor() as i32,
                probe.y.floor() as i32,
                probe.z.floor() as i32,
            );
            let block = world.get_block(bpos);
            if registry.is_solid(block) {
                self.position = probe;
                self.stuck = true;
                return;
            }
        }

        self.position = next_pos;
        if self.position.y < -40.0 {
            self.dead = true;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MonsterTickOutcome {
    pub damage_to_player: f32,
    pub spawned_arrow: Option<(Vec3, Vec3)>,
}

#[derive(Debug, Clone)]
pub struct MonsterEntity {
    pub id: u32,
    pub kind: MonsterKind,
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub attack_cooldown: f32,
    pub wander_timer: f32,
    pub is_moving: bool,
    pub walk_anim_timer: f32,
    pub on_ground: bool,
    pub dead: bool,
    pub burning_timer: f32,
    pub aggro_in_day: bool,
    pub rng_state: u64,
}

impl MonsterEntity {
    pub fn new(id: u32, kind: MonsterKind, position: Vec3) -> Self {
        let max_hp = kind.max_health();
        let mut seed = ((id as u64) + 1).wrapping_mul(0x9e3779b97f4a7c15);
        seed ^= (position.x.to_bits() as u64).wrapping_mul(0x6c62272e07bb0142);
        seed ^= (position.z.to_bits() as u64).wrapping_mul(0x62b821756295c58d);
        if seed == 0 {
            seed = 0x853c49e6748fea9b;
        }
        Self {
            id,
            kind,
            position,
            velocity: Vec3::ZERO,
            yaw: ((id * 53) % 360) as f32 * (PI / 180.0),
            health: max_hp,
            max_health: max_hp,
            hurt_timer: 0.0,
            attack_cooldown: 0.5,
            wander_timer: 1.5,
            is_moving: false,
            walk_anim_timer: 0.0,
            on_ground: false,
            dead: false,
            burning_timer: 0.0,
            aggro_in_day: false,
            rng_state: seed,
        }
    }

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
        let (hw, h) = self.kind.dimensions();
        AABB::from_player(self.position, hw, h)
    }

    pub fn take_damage(&mut self, amount: f32, knockback_dir: Vec3) {
        if self.dead {
            return;
        }
        self.health = (self.health - amount).max(0.0);
        self.hurt_timer = 0.35;
        self.aggro_in_day = true;

        let horiz_dir = Vec3::new(knockback_dir.x, 0.0, knockback_dir.z).normalize_or_zero();
        if horiz_dir.length_squared() > 0.01 {
            self.velocity = horiz_dir * 4.5 + Vec3::new(0.0, 3.2, 0.0);
        } else {
            self.velocity.y = self.velocity.y.max(3.0);
        }

        if self.health <= 0.0 {
            self.dead = true;
        }
    }

    pub fn generate_drops(&self) -> Vec<ItemSlot> {
        let mut drops = Vec::new();
        let hash = (self.id as u64).wrapping_mul(2654435761);
        let roll1 = ((hash % 2) + 1) as u16; // 1 or 2
        let roll2 = ((hash >> 4) % 2) as u16; // 0 or 1

        match self.kind {
            MonsterKind::Zombie => {
                drops.push(ItemSlot::new(ItemId::ROTTEN_FLESH, roll1));
            }
            MonsterKind::Skeleton => {
                drops.push(ItemSlot::new(ItemId::BONE, roll1));
                drops.push(ItemSlot::new(ItemId::ARROW, 1 + roll2));
            }
            MonsterKind::Spider => {
                drops.push(ItemSlot::new(ItemId::STRING, roll1));
            }
        }
        drops
    }

    pub fn update(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        sun_intensity: f32,
        world: &World,
        registry: &BlockRegistry,
    ) -> MonsterTickOutcome {
        let mut outcome = MonsterTickOutcome::default();
        if self.dead {
            return outcome;
        }

        if self.position.y < -40.0 {
            self.dead = true;
            return outcome;
        }

        if self.hurt_timer > 0.0 {
            self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        }
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }

        // Sunlight burning for Zombies & Skeletons
        if self.kind.burns_in_sunlight() && sun_intensity > 0.55 {
            let bpos = IVec3::new(
                self.position.x.floor() as i32,
                (self.position.y + 1.0).floor() as i32,
                self.position.z.floor() as i32,
            );
            let in_water = world.get_block(bpos).is_water();
            if !in_water && has_sky_access(bpos, world, registry) {
                self.burning_timer = 0.5;
                self.health = (self.health - 2.0 * dt).max(0.0);
                if self.health <= 0.0 {
                    self.dead = true;
                    return outcome;
                }
            }
        } else if self.burning_timer > 0.0 {
            self.burning_timer = (self.burning_timer - dt).max(0.0);
        }

        let to_player = player_pos - self.position;
        let horiz_to_player = Vec3::new(to_player.x, 0.0, to_player.z);
        let dist = horiz_to_player.length();

        let spider_hostile = if self.kind == MonsterKind::Spider {
            sun_intensity <= 0.45 || self.aggro_in_day
        } else {
            true
        };

        let mut target_vel = Vec3::ZERO;
        let speed = self.kind.move_speed();

        if spider_hostile && dist < 18.0 && to_player.y.abs() < 10.0 {
            let dir = horiz_to_player.normalize_or_zero();
            self.yaw = dir.x.atan2(dir.z);

            match self.kind {
                MonsterKind::Zombie | MonsterKind::Spider => {
                    let melee_range = if self.kind == MonsterKind::Spider { 1.8 } else { 1.6 };
                    if dist > melee_range * 0.75 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }
                    if dist <= melee_range && to_player.y.abs() <= 1.8 && self.attack_cooldown <= 0.0 {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = 1.0;
                    }
                }
                MonsterKind::Skeleton => {
                    if dist > 11.0 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else if dist < 4.5 {
                        target_vel = -dir * (speed * 0.8);
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }

                    if dist <= 15.0 && self.attack_cooldown <= 0.0 {
                        let shoot_origin = self.position + Vec3::new(0.0, 1.5, 0.0);
                        let target_aim = player_pos + Vec3::new(0.0, 1.1, 0.0);
                        let delta = target_aim - shoot_origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        // Slight upward arc compensation for gravity
                        let aim_dir = (delta + Vec3::new(0.0, horiz_d * 0.14, 0.0)).normalize_or_zero();
                        outcome.spawned_arrow = Some((shoot_origin + aim_dir * 0.5, aim_dir * 20.0));
                        self.attack_cooldown = 1.8;
                    }
                }
            }
        } else {
            // Idle / Wander
            self.wander_timer -= dt;
            if self.wander_timer <= 0.0 {
                self.is_moving = self.next_f32() < 0.55;
                if self.is_moving {
                    self.yaw = self.next_f32() * std::f32::consts::TAU;
                }
                self.wander_timer = 2.0 + self.next_f32() * 3.0;
            }
            if self.is_moving {
                let dir = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
                target_vel = dir * (speed * 0.5);
            }
        }

        let (half_width, height) = self.kind.dimensions();

        // Jump over 1-block obstacles when chasing
        if self.is_moving && self.on_ground && target_vel.length_squared() > 0.1 {
            let dir = target_vel.normalize_or_zero();
            let probe = self.position + dir * (half_width + 0.45);
            if aabb_intersects_solid(probe, half_width, height, 1e-4, world, registry) {
                let step_probe = probe + Vec3::new(0.0, 1.05, 0.0);
                let can_jump_over = self.kind == MonsterKind::Spider
                    || !aabb_intersects_solid(step_probe, half_width, height, 1e-4, world, registry);
                if can_jump_over {
                    self.velocity.y = if self.kind == MonsterKind::Spider { 6.0 } else { 7.5 };
                    self.on_ground = false;
                } else {
                    self.yaw += PI * 0.75;
                }
            }
        }

        // Gravity & horizontal acceleration
        self.velocity.y = (self.velocity.y - 26.0 * dt).max(-45.0);
        let accel = if self.on_ground { 10.0 } else { 3.0 };
        self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
        self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);

        resolve_entity_movement(
            &mut self.position,
            &mut self.velocity,
            &mut self.on_ground,
            half_width,
            height,
            dt,
            world,
            registry,
        );

        let horiz_speed = Vec3::new(self.velocity.x, 0.0, self.velocity.z).length();
        if horiz_speed > 0.1 {
            self.walk_anim_timer += horiz_speed * dt * 2.5;
        }

        outcome
    }
}

/// Returns `true` if there are no solid blocks directly above `pos`.
pub fn has_sky_access(pos: IVec3, world: &World, registry: &BlockRegistry) -> bool {
    let max_y = (crate::chunk::VERTICAL_CHUNKS * 16) - 1;
    for y in (pos.y + 1)..=max_y {
        let b = world.get_block(IVec3::new(pos.x, y, pos.z));
        if registry.is_solid(b) {
            return false;
        }
    }
    true
}

/// Computes effective light level (0.0..=15.0) at `pos` combining scaled sky light and nearby emissive block light.
pub fn compute_light_at(
    pos: IVec3,
    sun_intensity: f32,
    world: &World,
    registry: &BlockRegistry,
) -> f32 {
    let sky_light = if has_sky_access(pos, world, registry) {
        15.0 * sun_intensity.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let mut block_light: f32 = 0.0;
    for (rel_pos, lum) in world.get_light_sources_near(pos, 15) {
        let dist = rel_pos.x.abs() + rel_pos.y.abs() + rel_pos.z.abs();
        let contrib = (lum as i32 - dist).max(0) as f32;
        if contrib > block_light {
            block_light = contrib;
        }
    }

    sky_light.max(block_light)
}

/// Returns `true` if `kind` can spawn at `pos` (solid floor below, clear space, and effective light <= 7.0).
pub fn can_monster_spawn_at(
    pos: IVec3,
    kind: MonsterKind,
    sun_intensity: f32,
    world: &World,
    registry: &BlockRegistry,
) -> bool {
    let floor = world.get_block(pos - IVec3::Y);
    if !registry.is_solid(floor) || floor.is_water() {
        return false;
    }
    let at = world.get_block(pos);
    if !at.is_air() {
        return false;
    }
    if kind != MonsterKind::Spider && !world.get_block(pos + IVec3::Y).is_air() {
        return false;
    }

    compute_light_at(pos, sun_intensity, world, registry) <= 7.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        block::BlockId,
        chunk::{Chunk, ChunkPos},
    };

    #[test]
    fn test_torch_light_prevents_monster_spawn() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, BlockId::STONE);
            }
        }
        world.insert_chunk(chunk);

        let spawn_pos = IVec3::new(8, 1, 8);
        // At night (sun_intensity = 0.0), dark floor allows monster spawning
        assert!(can_monster_spawn_at(
            spawn_pos,
            MonsterKind::Zombie,
            0.0,
            &world,
            &registry
        ));

        // Placing a Torch nearby raises light > 7 and stops monster spawning
        world.set_block(IVec3::new(6, 1, 8), BlockId::TORCH);
        assert!(
            !can_monster_spawn_at(spawn_pos, MonsterKind::Zombie, 0.0, &world, &registry),
            "Torch nearby must raise light > 7 and stop monster spawning"
        );
    }

    #[test]
    fn test_daytime_sunlight_stops_spawn_and_burns_zombies() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, BlockId::GRASS);
            }
        }
        world.insert_chunk(chunk);

        assert!(!can_monster_spawn_at(
            IVec3::new(8, 1, 8),
            MonsterKind::Skeleton,
            1.0,
            &world,
            &registry
        ));

        let mut zombie = MonsterEntity::new(1, MonsterKind::Zombie, Vec3::new(8.5, 1.0, 8.5));
        let start_hp = zombie.health;
        zombie.update(1.0, Vec3::new(50.0, 1.0, 50.0), 1.0, &world, &registry);
        assert!(zombie.health < start_hp, "Zombie should burn in direct daytime sunlight");
    }

    #[test]
    fn test_monster_drops() {
        let z = MonsterEntity::new(1, MonsterKind::Zombie, Vec3::ZERO);
        let sk = MonsterEntity::new(2, MonsterKind::Skeleton, Vec3::ZERO);
        let sp = MonsterEntity::new(3, MonsterKind::Spider, Vec3::ZERO);

        assert_eq!(z.generate_drops()[0].item_id, ItemId::ROTTEN_FLESH);
        let sk_drops = sk.generate_drops();
        assert_eq!(sk_drops[0].item_id, ItemId::BONE);
        assert_eq!(sk_drops[1].item_id, ItemId::ARROW);
        assert_eq!(sp.generate_drops()[0].item_id, ItemId::STRING);
    }
}
