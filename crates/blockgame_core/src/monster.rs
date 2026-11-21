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
    // Temple guardian minions (Issue #9)
    SandMummy,
    SunArcher,
    FrostWraith,
    JungleStalker,
    ShadowKnight,
    // Biome Bosses (Issue #9)
    SunPharaoh,
    FrostTitan,
    OvergrownGolem,
    ShadowLich,
}

impl MonsterKind {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Zombie => 0,
            Self::Skeleton => 1,
            Self::Spider => 2,
            Self::SandMummy => 3,
            Self::SunArcher => 4,
            Self::FrostWraith => 5,
            Self::JungleStalker => 6,
            Self::ShadowKnight => 7,
            Self::SunPharaoh => 8,
            Self::FrostTitan => 9,
            Self::OvergrownGolem => 10,
            Self::ShadowLich => 11,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Zombie),
            1 => Some(Self::Skeleton),
            2 => Some(Self::Spider),
            3 => Some(Self::SandMummy),
            4 => Some(Self::SunArcher),
            5 => Some(Self::FrostWraith),
            6 => Some(Self::JungleStalker),
            7 => Some(Self::ShadowKnight),
            8 => Some(Self::SunPharaoh),
            9 => Some(Self::FrostTitan),
            10 => Some(Self::OvergrownGolem),
            11 => Some(Self::ShadowLich),
            _ => None,
        }
    }

    #[inline]
    pub fn is_boss(self) -> bool {
        matches!(
            self,
            Self::SunPharaoh | Self::FrostTitan | Self::OvergrownGolem | Self::ShadowLich
        )
    }

    #[inline]
    pub fn is_temple_minion(self) -> bool {
        matches!(
            self,
            Self::SandMummy
                | Self::SunArcher
                | Self::FrostWraith
                | Self::JungleStalker
                | Self::ShadowKnight
        )
    }

    #[inline]
    pub fn is_temple_mob(self) -> bool {
        self.is_boss() || self.is_temple_minion()
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Zombie => "Zombie",
            Self::Skeleton => "Skeleton",
            Self::Spider => "Spider",
            Self::SandMummy => "Sand Mummy",
            Self::SunArcher => "Sun Archer",
            Self::FrostWraith => "Frost Wraith",
            Self::JungleStalker => "Jungle Stalker",
            Self::ShadowKnight => "Shadow Knight",
            Self::SunPharaoh => "Sun Pharaoh",
            Self::FrostTitan => "Frost Titan",
            Self::OvergrownGolem => "Overgrown Golem",
            Self::ShadowLich => "Shadow Lich",
        }
    }

    /// Accent RGBA color for the boss HUD health bar.
    pub fn boss_theme_color(&self) -> [f32; 4] {
        match self {
            Self::SunPharaoh => [0.96, 0.72, 0.16, 1.0],     // Radiant solar gold
            Self::FrostTitan => [0.24, 0.78, 0.98, 1.0],     // Glacial cyan
            Self::OvergrownGolem => [0.26, 0.80, 0.36, 1.0], // Vibrant jungle emerald
            Self::ShadowLich => [0.68, 0.28, 0.94, 1.0],     // Cursed shadow violet
            _ => [0.85, 0.22, 0.22, 1.0],
        }
    }

    pub fn dimensions(&self) -> (f32, f32) {
        match self {
            Self::Zombie | Self::Skeleton => (0.35, 1.95),
            Self::Spider => (0.65, 0.90),
            Self::SandMummy => (0.40, 2.05),
            Self::SunArcher => (0.36, 1.95),
            Self::FrostWraith => (0.38, 2.00),
            Self::JungleStalker => (0.70, 1.05),
            Self::ShadowKnight => (0.42, 2.10),
            Self::SunPharaoh => (0.60, 2.90),
            Self::FrostTitan => (0.80, 3.40),
            Self::OvergrownGolem => (0.85, 3.20),
            Self::ShadowLich => (0.55, 2.80),
        }
    }

    pub fn max_health(&self) -> f32 {
        match self {
            Self::Zombie => 20.0,
            Self::Skeleton => 20.0,
            Self::Spider => 16.0,
            Self::SandMummy => 45.0,
            Self::SunArcher => 38.0,
            Self::FrostWraith => 42.0,
            Self::JungleStalker => 40.0,
            Self::ShadowKnight => 55.0,
            Self::SunPharaoh => 300.0,
            Self::FrostTitan => 380.0,
            Self::OvergrownGolem => 350.0,
            Self::ShadowLich => 320.0,
        }
    }

    pub fn move_speed(&self) -> f32 {
        match self {
            Self::Zombie => 2.3,
            Self::Skeleton => 2.5,
            Self::Spider => 3.2,
            Self::SandMummy => 2.6,
            Self::SunArcher => 2.7,
            Self::FrostWraith => 3.1,
            Self::JungleStalker => 3.6,
            Self::ShadowKnight => 2.8,
            Self::SunPharaoh => 3.2,
            Self::FrostTitan => 2.8,
            Self::OvergrownGolem => 2.6,
            Self::ShadowLich => 3.4,
        }
    }

    pub fn attack_damage(&self) -> f32 {
        match self {
            Self::Zombie => 3.0,
            Self::Skeleton => 3.0,
            Self::Spider => 2.0,
            Self::SandMummy => 5.0,
            Self::SunArcher => 4.5,
            Self::FrostWraith => 5.0,
            Self::JungleStalker => 4.5,
            Self::ShadowKnight => 6.0,
            Self::SunPharaoh => 8.0,
            Self::FrostTitan => 10.0,
            Self::OvergrownGolem => 9.5,
            Self::ShadowLich => 8.5,
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
    /// Additional projectiles with explicit `(origin, velocity, damage)` (used by bosses/temple archers).
    pub extra_arrows: Vec<(Vec3, Vec3, f32)>,
    /// Radial AoE blast `(center, radius, damage)` triggered by boss special attacks.
    pub aoe_blast: Option<(Vec3, f32, f32)>,
    /// Minions summoned by a boss during Phase 2/3.
    pub summon_minions: Vec<(MonsterKind, Vec3)>,
    /// Set to `Some(new_phase)` on the tick a boss transitions to Phase 2 or 3.
    pub phase_changed: Option<u8>,
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
    // Boss & temple encounter state (Issue #9)
    pub in_combat: bool,
    pub phase: u8,
    pub special_cooldown: f32,
    pub summon_cooldown: f32,
    pub temple_id: Option<u64>,
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
            aggro_in_day: kind.is_temple_mob(),
            rng_state: seed,
            in_combat: false,
            phase: 1,
            special_cooldown: 4.0,
            summon_cooldown: 8.0,
            temple_id: None,
        }
    }

    pub fn new_temple_mob(id: u32, kind: MonsterKind, position: Vec3, temple_id: u64) -> Self {
        let mut m = Self::new(id, kind, position);
        m.temple_id = Some(temple_id);
        m.aggro_in_day = true;
        m
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
        self.in_combat = true;

        // Bosses have heavy knockback resistance so they cannot be trivially stunlocked.
        let kb_scale = if self.kind.is_boss() { 0.35 } else { 1.0 };
        let horiz_dir = Vec3::new(knockback_dir.x, 0.0, knockback_dir.z).normalize_or_zero();
        if horiz_dir.length_squared() > 0.01 {
            self.velocity = horiz_dir * (4.5 * kb_scale) + Vec3::new(0.0, 3.2 * kb_scale, 0.0);
        } else {
            self.velocity.y = self.velocity.y.max(3.0 * kb_scale);
        }

        if self.health <= 0.0 {
            self.dead = true;
            self.in_combat = false;
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
            // Temple minions drop useful combat/crafting resources
            MonsterKind::SandMummy => {
                drops.push(ItemSlot::new(ItemId::GOLD_INGOT, 1 + roll2));
                drops.push(ItemSlot::new(ItemId::BONE, roll1));
            }
            MonsterKind::SunArcher => {
                drops.push(ItemSlot::new(ItemId::ARROW, 4 + roll1 * 2));
                drops.push(ItemSlot::new(ItemId::GOLD_INGOT, 1));
            }
            MonsterKind::FrostWraith => {
                drops.push(ItemSlot::new(ItemId::IRON_INGOT, 1 + roll2));
                drops.push(ItemSlot::new(ItemId::DIAMOND, roll2.max(1)));
            }
            MonsterKind::JungleStalker => {
                drops.push(ItemSlot::new(ItemId::STRING, 2 + roll1));
                drops.push(ItemSlot::new(ItemId::IRON_INGOT, 1));
            }
            MonsterKind::ShadowKnight => {
                drops.push(ItemSlot::new(ItemId::IRON_INGOT, 2));
                drops.push(ItemSlot::new(ItemId::BONE, 2 + roll2));
            }
            // Unique Boss Drops: Signature Weapon + Boss Armor Piece + Trophy Core + Diamonds
            MonsterKind::SunPharaoh => {
                drops.push(ItemSlot::new(ItemId::SUNBLADE, 1));
                drops.push(ItemSlot::new(ItemId::PHARAOH_CROWN, 1));
                drops.push(ItemSlot::new(ItemId::SUN_CORE, 1));
                drops.push(ItemSlot::new(ItemId::GOLD_INGOT, 8));
            }
            MonsterKind::FrostTitan => {
                drops.push(ItemSlot::new(ItemId::FROSTBLADE, 1));
                drops.push(ItemSlot::new(ItemId::GLACIAL_CHESTPLATE, 1));
                drops.push(ItemSlot::new(ItemId::FROST_HEART, 1));
                drops.push(ItemSlot::new(ItemId::DIAMOND, 4));
            }
            MonsterKind::OvergrownGolem => {
                drops.push(ItemSlot::new(ItemId::VERDANT_GREATBLADE, 1));
                drops.push(ItemSlot::new(ItemId::GOLEM_GREAVES, 1));
                drops.push(ItemSlot::new(ItemId::ANCIENT_VINE_CORE, 1));
                drops.push(ItemSlot::new(ItemId::DIAMOND, 4));
            }
            MonsterKind::ShadowLich => {
                drops.push(ItemSlot::new(ItemId::SHADOWFANG_BLADE, 1));
                drops.push(ItemSlot::new(ItemId::LICH_BOOTS, 1));
                drops.push(ItemSlot::new(ItemId::SOUL_PHYLACTERY, 1));
                drops.push(ItemSlot::new(ItemId::DIAMOND, 5));
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
            self.in_combat = false;
            return outcome;
        }

        if self.hurt_timer > 0.0 {
            self.hurt_timer = (self.hurt_timer - dt).max(0.0);
        }
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }
        if self.special_cooldown > 0.0 {
            self.special_cooldown = (self.special_cooldown - dt).max(0.0);
        }
        if self.summon_cooldown > 0.0 {
            self.summon_cooldown = (self.summon_cooldown - dt).max(0.0);
        }

        // Update boss phase based on remaining health
        if self.kind.is_boss() && self.max_health > 0.0 {
            let hp_ratio = self.health / self.max_health;
            let new_phase = if hp_ratio <= 0.33 {
                3
            } else if hp_ratio <= 0.66 {
                2
            } else {
                1
            };
            if new_phase > self.phase {
                self.phase = new_phase;
                outcome.phase_changed = Some(new_phase);
                // Reset special cooldown slightly so phase transitions feel dramatic
                self.special_cooldown = self.special_cooldown.min(1.5);
            }
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

        let aggro_range = if self.kind.is_boss() {
            36.0
        } else if self.kind.is_temple_minion() {
            24.0
        } else {
            18.0
        };
        let vertical_range = if self.kind.is_boss() { 18.0 } else { 10.0 };

        let mut target_vel = Vec3::ZERO;
        let base_speed = self.kind.move_speed();
        let speed = if self.kind.is_boss() && self.phase == 3 {
            base_speed * 1.25
        } else if self.kind.is_boss() && self.phase == 2 {
            base_speed * 1.10
        } else {
            base_speed
        };

        if spider_hostile && dist < aggro_range && to_player.y.abs() < vertical_range {
            self.in_combat = true;
            let dir = horiz_to_player.normalize_or_zero();
            self.yaw = dir.x.atan2(dir.z);

            match self.kind {
                MonsterKind::Zombie
                | MonsterKind::Spider
                | MonsterKind::SandMummy
                | MonsterKind::JungleStalker
                | MonsterKind::ShadowKnight => {
                    let melee_range = match self.kind {
                        MonsterKind::Spider | MonsterKind::JungleStalker => 1.9,
                        MonsterKind::ShadowKnight => 2.0,
                        _ => 1.65,
                    };
                    if dist > melee_range * 0.75 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }
                    // Jungle Stalker leaps periodically toward the player
                    if self.kind == MonsterKind::JungleStalker
                        && self.on_ground
                        && dist > 3.5
                        && dist < 10.0
                        && self.special_cooldown <= 0.0
                    {
                        self.velocity = dir * 9.5 + Vec3::new(0.0, 5.5, 0.0);
                        self.on_ground = false;
                        self.special_cooldown = 4.0;
                    }
                    if dist <= melee_range
                        && to_player.y.abs() <= 2.0
                        && self.attack_cooldown <= 0.0
                    {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = if self.kind == MonsterKind::JungleStalker {
                            0.85
                        } else {
                            1.0
                        };
                    }
                }
                MonsterKind::Skeleton | MonsterKind::SunArcher | MonsterKind::FrostWraith => {
                    let keep_max = if self.kind == MonsterKind::FrostWraith {
                        9.0
                    } else {
                        11.5
                    };
                    if dist > keep_max {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else if dist < 4.5 {
                        target_vel = -dir * (speed * 0.8);
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }

                    if dist <= 18.0 && self.attack_cooldown <= 0.0 {
                        let shoot_origin = self.position + Vec3::new(0.0, 1.5, 0.0);
                        let target_aim = player_pos + Vec3::new(0.0, 1.1, 0.0);
                        let delta = target_aim - shoot_origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        let aim_dir =
                            (delta + Vec3::new(0.0, horiz_d * 0.14, 0.0)).normalize_or_zero();
                        let proj_speed = if self.kind == MonsterKind::Skeleton {
                            20.0
                        } else {
                            23.0
                        };
                        if self.kind == MonsterKind::Skeleton {
                            outcome.spawned_arrow =
                                Some((shoot_origin + aim_dir * 0.5, aim_dir * proj_speed));
                        } else {
                            outcome.extra_arrows.push((
                                shoot_origin + aim_dir * 0.5,
                                aim_dir * proj_speed,
                                self.kind.attack_damage(),
                            ));
                        }
                        self.attack_cooldown = if self.kind == MonsterKind::FrostWraith {
                            1.4
                        } else {
                            1.7
                        };
                    }
                }
                // --- MULTI-PHASE BOSS AI ---
                MonsterKind::SunPharaoh => {
                    let melee_range = 2.6;
                    if dist > 7.5 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else if dist < 3.5 && self.phase < 3 {
                        target_vel = -dir * (speed * 0.65);
                        self.is_moving = true;
                    } else if self.phase == 3 && dist > melee_range * 0.8 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }

                    // Primary melee or solar bolt
                    if dist <= melee_range && to_player.y.abs() <= 2.8 && self.attack_cooldown <= 0.0
                    {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = 1.1;
                    } else if dist <= 26.0 && self.attack_cooldown <= 0.0 {
                        let origin = self.position + Vec3::new(0.0, 2.2, 0.0);
                        let delta = (player_pos + Vec3::new(0.0, 1.0, 0.0)) - origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        let aim = (delta + Vec3::new(0.0, horiz_d * 0.11, 0.0)).normalize_or_zero();
                        outcome.extra_arrows.push((origin + aim * 0.7, aim * 25.0, 6.5));
                        self.attack_cooldown = if self.phase == 3 { 1.15 } else { 1.65 };
                    }

                    // Phase 2+: Solar fan spread / Phase 3: Dash + Solar Slam AoE
                    if self.phase >= 2 && self.special_cooldown <= 0.0 {
                        if self.phase == 3 && dist < 12.0 {
                            self.velocity = dir * 13.0 + Vec3::new(0.0, 4.5, 0.0);
                            outcome.aoe_blast = Some((self.position, 5.5, 10.0));
                            self.special_cooldown = 5.5;
                        } else {
                            let origin = self.position + Vec3::new(0.0, 2.1, 0.0);
                            let base_angle = dir.x.atan2(dir.z);
                            for step in -2..=2 {
                                let ang = base_angle + (step as f32) * 0.22;
                                let vdir = Vec3::new(ang.sin(), 0.08, ang.cos()).normalize();
                                outcome.extra_arrows.push((origin + vdir * 0.8, vdir * 22.0, 6.0));
                            }
                            self.special_cooldown = if self.phase == 3 { 5.0 } else { 7.0 };
                        }
                    }

                    // Phase 2+: Summon Sand Mummies
                    if self.phase >= 2 && self.summon_cooldown <= 0.0 {
                        let side = Vec3::new(-dir.z, 0.0, dir.x);
                        outcome.summon_minions.push((
                            MonsterKind::SandMummy,
                            self.position + side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        outcome.summon_minions.push((
                            MonsterKind::SunArcher,
                            self.position - side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        self.summon_cooldown = 20.0;
                    }
                }
                MonsterKind::FrostTitan => {
                    let melee_range = 3.1;
                    if dist > melee_range * 0.8 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }

                    if dist <= melee_range && to_player.y.abs() <= 3.2 && self.attack_cooldown <= 0.0
                    {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = if self.phase == 3 { 1.0 } else { 1.35 };
                    } else if dist > 5.0 && dist <= 24.0 && self.attack_cooldown <= 0.0 {
                        let origin = self.position + Vec3::new(0.0, 2.6, 0.0);
                        let delta = (player_pos + Vec3::new(0.0, 1.0, 0.0)) - origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        let aim = (delta + Vec3::new(0.0, horiz_d * 0.12, 0.0)).normalize_or_zero();
                        let count = if self.phase == 3 { 3 } else { 1 };
                        let base_angle = aim.x.atan2(aim.z);
                        for i in 0..count {
                            let offset = (i as f32 - (count as f32 - 1.0) * 0.5) * 0.18;
                            let ang = base_angle + offset;
                            let vdir = Vec3::new(ang.sin(), aim.y, ang.cos()).normalize();
                            outcome.extra_arrows.push((origin + vdir * 0.9, vdir * 24.0, 7.0));
                        }
                        self.attack_cooldown = 2.0;
                    }

                    // Phase 2+: Frost Nova AoE / Phase 3: Leap Slam
                    if self.phase >= 2 && self.special_cooldown <= 0.0 {
                        if self.phase == 3 && dist > 4.0 && self.on_ground {
                            self.velocity = dir * 12.0 + Vec3::new(0.0, 7.5, 0.0);
                            self.on_ground = false;
                        }
                        outcome.aoe_blast = Some((self.position, 6.0, 11.0));
                        self.special_cooldown = if self.phase == 3 { 5.5 } else { 7.5 };
                    }

                    if self.phase >= 2 && self.summon_cooldown <= 0.0 {
                        let side = Vec3::new(-dir.z, 0.0, dir.x);
                        outcome.summon_minions.push((
                            MonsterKind::FrostWraith,
                            self.position + side * 3.5 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        outcome.summon_minions.push((
                            MonsterKind::FrostWraith,
                            self.position - side * 3.5 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        self.summon_cooldown = 22.0;
                    }
                }
                MonsterKind::OvergrownGolem => {
                    let melee_range = 3.0;
                    if dist > melee_range * 0.8 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else {
                        self.is_moving = false;
                    }

                    if dist <= melee_range && to_player.y.abs() <= 3.0 && self.attack_cooldown <= 0.0
                    {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = 1.25;
                    } else if dist > 5.0 && dist <= 24.0 && self.attack_cooldown <= 0.0 {
                        let origin = self.position + Vec3::new(0.0, 2.4, 0.0);
                        let delta = (player_pos + Vec3::new(0.0, 1.0, 0.0)) - origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        let aim = (delta + Vec3::new(0.0, horiz_d * 0.14, 0.0)).normalize_or_zero();
                        let count = if self.phase == 3 { 3 } else { 1 };
                        let base_angle = aim.x.atan2(aim.z);
                        for i in 0..count {
                            let offset = (i as f32 - (count as f32 - 1.0) * 0.5) * 0.2;
                            let ang = base_angle + offset;
                            let vdir = Vec3::new(ang.sin(), aim.y, ang.cos()).normalize();
                            outcome.extra_arrows.push((origin + vdir * 0.9, vdir * 21.0, 7.5));
                        }
                        self.attack_cooldown = 2.1;
                    }

                    // Phase 2+: Ground-pound AoE + minor nature regen; Phase 3: Charge dash
                    if self.phase >= 2 && self.special_cooldown <= 0.0 {
                        outcome.aoe_blast = Some((self.position, 6.5, 10.5));
                        if self.phase == 2 {
                            self.health = (self.health + 6.0).min(self.max_health);
                        } else {
                            self.velocity = dir * 13.5 + Vec3::new(0.0, 4.0, 0.0);
                        }
                        self.special_cooldown = if self.phase == 3 { 5.0 } else { 7.5 };
                    }

                    if self.phase >= 2 && self.summon_cooldown <= 0.0 {
                        let side = Vec3::new(-dir.z, 0.0, dir.x);
                        outcome.summon_minions.push((
                            MonsterKind::JungleStalker,
                            self.position + side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        outcome.summon_minions.push((
                            MonsterKind::JungleStalker,
                            self.position - side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        self.summon_cooldown = 20.0;
                    }
                }
                MonsterKind::ShadowLich => {
                    let melee_range = 2.4;
                    if dist > 11.0 {
                        target_vel = dir * speed;
                        self.is_moving = true;
                    } else if dist < 5.5 {
                        target_vel = -dir * (speed * 0.9);
                        self.is_moving = true;
                    } else {
                        // Strafe laterally
                        let strafe = Vec3::new(-dir.z, 0.0, dir.x);
                        target_vel = strafe * (speed * 0.65);
                        self.is_moving = true;
                    }

                    if dist <= melee_range && to_player.y.abs() <= 2.6 && self.attack_cooldown <= 0.0
                    {
                        outcome.damage_to_player = self.kind.attack_damage();
                        self.attack_cooldown = 1.0;
                    } else if dist <= 28.0 && self.attack_cooldown <= 0.0 {
                        let origin = self.position + Vec3::new(0.0, 2.2, 0.0);
                        let delta = (player_pos + Vec3::new(0.0, 1.1, 0.0)) - origin;
                        let horiz_d = Vec3::new(delta.x, 0.0, delta.z).length();
                        let aim = (delta + Vec3::new(0.0, horiz_d * 0.10, 0.0)).normalize_or_zero();
                        let count = if self.phase == 3 { 5 } else if self.phase == 2 { 3 } else { 1 };
                        let base_angle = aim.x.atan2(aim.z);
                        for i in 0..count {
                            let offset = (i as f32 - (count as f32 - 1.0) * 0.5) * 0.17;
                            let ang = base_angle + offset;
                            let vdir = Vec3::new(ang.sin(), aim.y, ang.cos()).normalize();
                            outcome.extra_arrows.push((origin + vdir * 0.7, vdir * 26.0, 6.5));
                        }
                        self.attack_cooldown = if self.phase == 3 { 1.35 } else { 1.75 };
                    }

                    // Shadow-step dash + Dark Nova AoE
                    if self.special_cooldown <= 0.0 {
                        let side = Vec3::new(-dir.z, 0.0, dir.x);
                        let sign = if self.next_f32() < 0.5 { 1.0 } else { -1.0 };
                        self.velocity = side * (sign * 12.0) + Vec3::new(0.0, 4.5, 0.0);
                        if self.phase >= 2 {
                            outcome.aoe_blast = Some((self.position, 5.5, 9.5));
                        }
                        self.special_cooldown = if self.phase == 3 { 4.2 } else { 6.5 };
                    }

                    if self.phase >= 2 && self.summon_cooldown <= 0.0 {
                        let side = Vec3::new(-dir.z, 0.0, dir.x);
                        outcome.summon_minions.push((
                            MonsterKind::ShadowKnight,
                            self.position + side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        outcome.summon_minions.push((
                            MonsterKind::ShadowKnight,
                            self.position - side * 3.0 + Vec3::new(0.0, 0.5, 0.0),
                        ));
                        self.summon_cooldown = 21.0;
                    }
                }
            }
        } else {
            // Drop boss combat state only if player is far away (> 64 blocks)
            if self.kind.is_boss() && dist > 64.0 {
                self.in_combat = false;
            }
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
                let can_jump_over =
                    matches!(self.kind, MonsterKind::Spider | MonsterKind::JungleStalker)
                        || !aabb_intersects_solid(
                            step_probe,
                            half_width,
                            height,
                            1e-4,
                            world,
                            registry,
                        );
                if can_jump_over {
                    self.velocity.y =
                        if matches!(self.kind, MonsterKind::Spider | MonsterKind::JungleStalker) {
                            6.5
                        } else {
                            7.5
                        };
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
/// Sky light passes through leaves with mild attenuation (1 level per leaf block), preventing daytime spawning under trees.
pub fn compute_light_at(
    pos: IVec3,
    sun_intensity: f32,
    world: &World,
    registry: &BlockRegistry,
) -> f32 {
    let max_y = (crate::chunk::VERTICAL_CHUNKS * 16) - 1;
    let sun = sun_intensity.clamp(0.0, 1.0);
    let mut sky_light = if sun > 0.05 {
        10.0 + 5.0 * ((sun - 0.05) / 0.95)
    } else {
        10.0 * (sun / 0.05)
    };
    if sky_light > 0.0 {
        for y in (pos.y + 1)..=max_y {
            let b = world.get_block(IVec3::new(pos.x, y, pos.z));
            if b.is_leaves() {
                sky_light = (sky_light - 1.0).max(0.0);
                if sky_light <= 0.0 {
                    break;
                }
            } else if b.is_water() {
                sky_light = (sky_light - 2.0).max(0.0);
                if sky_light <= 0.0 {
                    break;
                }
            } else if registry.is_solid(b) {
                sky_light = 0.0;
                break;
            }
        }
    }

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

    #[test]
    fn test_sky_light_passes_through_leaves_stops_daytime_spawn() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, BlockId::GRASS);
            }
        }
        world.insert_chunk(chunk);

        // Place a 2-layer leaf canopy above (8, 1, 8)
        world.set_block(IVec3::new(8, 4, 8), BlockId::LEAVES);
        world.set_block(IVec3::new(8, 5, 8), BlockId::BIRCH_LEAVES);

        // Daytime under tree canopy: sky light passes through leaves -> no spawn
        assert!(
            !can_monster_spawn_at(IVec3::new(8, 1, 8), MonsterKind::Zombie, 1.0, &world, &registry),
            "Sky light through leaves should prevent daytime monster spawning under trees"
        );
        // Nighttime under tree canopy: dark -> spawns allowed
        assert!(can_monster_spawn_at(
            IVec3::new(8, 1, 8),
            MonsterKind::Zombie,
            0.0,
            &world,
            &registry
        ));

        // Solid stone roof blocks sky light -> dark cave spawns even during daytime
        world.set_block(IVec3::new(4, 4, 4), BlockId::STONE);
        assert!(can_monster_spawn_at(
            IVec3::new(4, 1, 4),
            MonsterKind::Zombie,
            1.0,
            &world,
            &registry
        ));
    }
}
