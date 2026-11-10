#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStats {
    pub health: f32,
    pub max_health: f32,
    pub hunger: f32,
    pub max_hunger: f32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub damage_flash: f32,
    pub invulnerability: f32,
    pub regen_timer: f32,
    pub starvation_timer: f32,
    pub void_timer: f32,
    pub is_dead: bool,
    pub breath: f32,
    pub max_breath: f32,
    pub drown_timer: f32,
    pub is_submerged: bool,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerStats {
    pub const MAX_HEALTH: f32 = 20.0;
    pub const MAX_HUNGER: f32 = 20.0;
    pub const MAX_BREATH: f32 = 15.0; // 15 seconds of oxygen (10 bubbles)

    pub fn new() -> Self {
        Self {
            health: Self::MAX_HEALTH,
            max_health: Self::MAX_HEALTH,
            hunger: Self::MAX_HUNGER,
            max_hunger: Self::MAX_HUNGER,
            saturation: 5.0,
            exhaustion: 0.0,
            damage_flash: 0.0,
            invulnerability: 0.0,
            regen_timer: 0.0,
            starvation_timer: 0.0,
            void_timer: 0.0,
            is_dead: false,
            breath: Self::MAX_BREATH,
            max_breath: Self::MAX_BREATH,
            drown_timer: 0.0,
            is_submerged: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Deals damage to player with invulnerability frames and damage flash.
    /// Returns true if damage was applied, false if player is dead or invulnerable.
    pub fn take_damage(&mut self, amount: f32) -> bool {
        if self.is_dead || self.invulnerability > 0.0 || amount <= 0.0 {
            return false;
        }
        self.health = (self.health - amount).max(0.0);
        self.damage_flash = 0.5;
        self.invulnerability = 0.5;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.is_dead = true;
        }
        true
    }

    /// Force damage (e.g. void damage) bypassing invulnerability.
    pub fn take_damage_force(&mut self, amount: f32) -> bool {
        if self.is_dead || amount <= 0.0 {
            return false;
        }
        self.health = (self.health - amount).max(0.0);
        self.damage_flash = 0.4;
        self.invulnerability = 0.4;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.is_dead = true;
        }
        true
    }

    /// Consumes food, restoring hunger and saturation points.
    /// Returns true if food was consumed, false if player is already completely full.
    pub fn eat(&mut self, food_points: u16, saturation_points: f32) -> bool {
        if self.is_dead || self.hunger >= self.max_hunger {
            return false;
        }
        self.hunger = (self.hunger + food_points as f32).min(self.max_hunger);
        self.saturation = (self.saturation + saturation_points).min(self.hunger);
        true
    }

    /// Adds exhaustion from movement, jumping, mining, etc.
    pub fn add_exhaustion(&mut self, amount: f32) {
        if self.is_dead || amount <= 0.0 {
            return;
        }
        self.exhaustion += amount;
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.hunger = (self.hunger - 1.0).max(0.0);
            }
        }
    }

    /// Ticks player regeneration, starvation, damage cooldowns, void damage, and breath/drowning.
    pub fn update(&mut self, dt: f32, player_y: f32, is_submerged: bool) {
        if self.is_dead {
            return;
        }

        if self.damage_flash > 0.0 {
            self.damage_flash = (self.damage_flash - dt).max(0.0);
        }
        if self.invulnerability > 0.0 {
            self.invulnerability = (self.invulnerability - dt).max(0.0);
        }

        // 1. Void Damage: below y = -25.0
        if player_y < -25.0 {
            self.void_timer += dt;
            if self.void_timer >= 0.5 {
                self.void_timer = 0.0;
                self.take_damage_force(4.0); // 2 hearts every 0.5s
                if self.is_dead {
                    return;
                }
            }
        } else {
            self.void_timer = 0.0;
        }

        // 2. Natural Health Regeneration
        // When hunger >= 18.0 and player is hurt, regenerate 1 HP every 3 seconds
        if self.hunger >= 18.0 && self.health < self.max_health {
            self.regen_timer += dt;
            if self.regen_timer >= 3.0 {
                self.regen_timer = 0.0;
                self.health = (self.health + 1.0).min(self.max_health);
                self.add_exhaustion(1.5);
            }
        } else {
            self.regen_timer = 0.0;
        }

        // 3. Starvation Damage
        // When hunger is 0, tick 1 HP damage every 4 seconds
        if self.hunger <= 0.0 {
            self.starvation_timer += dt;
            if self.starvation_timer >= 4.0 {
                self.starvation_timer = 0.0;
                self.take_damage_force(1.0);
            }
        } else {
            self.starvation_timer = 0.0;
        }

        // 4. Breath & Drowning Mechanics
        self.is_submerged = is_submerged;
        if is_submerged {
            if self.breath > 0.0 {
                if self.breath >= dt {
                    self.breath -= dt;
                } else {
                    let excess = dt - self.breath;
                    self.breath = 0.0;
                    self.drown_timer += excess;
                    while self.drown_timer >= 1.0 {
                        self.drown_timer -= 1.0;
                        self.take_damage_force(2.0); // 1 full heart (2 HP) drowning damage
                    }
                }
            } else {
                self.drown_timer += dt;
                while self.drown_timer >= 1.0 {
                    self.drown_timer -= 1.0;
                    self.take_damage_force(2.0);
                }
            }
        } else {
            self.drown_timer = 0.0;
            // Rapid breath regeneration when surfacing (recovers full in 3 seconds)
            self.breath = (self.breath + dt * (self.max_breath / 3.0)).min(self.max_breath);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_stats_initialization() {
        let stats = PlayerStats::new();
        assert_eq!(stats.health, 20.0);
        assert_eq!(stats.hunger, 20.0);
        assert_eq!(stats.saturation, 5.0);
        assert_eq!(stats.breath, 15.0);
        assert!(!stats.is_dead);
    }

    #[test]
    fn test_take_damage_and_death() {
        let mut stats = PlayerStats::new();
        assert!(stats.take_damage(6.0));
        assert_eq!(stats.health, 14.0);
        assert!(stats.damage_flash > 0.0);

        // Immediate subsequent damage blocked by invulnerability frames
        assert!(!stats.take_damage(4.0));
        assert_eq!(stats.health, 14.0);

        // Advance cooldown
        stats.invulnerability = 0.0;
        assert!(stats.take_damage(14.0));
        assert_eq!(stats.health, 0.0);
        assert!(stats.is_dead);

        // Dead player cannot take further damage
        assert!(!stats.take_damage(5.0));
    }

    #[test]
    fn test_void_damage_kills_player() {
        let mut stats = PlayerStats::new();
        // Fall into void below y = -25.0
        for _ in 0..5 {
            stats.update(0.5, -30.0, false);
        }
        // 5 ticks of 4.0 damage = 20.0 damage = death
        assert_eq!(stats.health, 0.0);
        assert!(stats.is_dead);
    }

    #[test]
    fn test_natural_regeneration_and_starvation() {
        let mut stats = PlayerStats::new();
        stats.health = 10.0;
        stats.hunger = 20.0;

        // Advance 3 seconds of regen
        stats.update(3.0, 10.0, false);
        assert_eq!(stats.health, 11.0);

        // Starvation test
        stats.hunger = 0.0;
        stats.update(4.0, 10.0, false);
        assert_eq!(stats.health, 10.0);
    }

    #[test]
    fn test_eating_restores_hunger_and_saturation() {
        let mut stats = PlayerStats::new();
        stats.hunger = 12.0;
        stats.saturation = 0.0;

        assert!(stats.eat(4, 2.4));
        assert_eq!(stats.hunger, 16.0);
        assert_eq!(stats.saturation, 2.4);

        // Eating when full fails
        stats.hunger = 20.0;
        assert!(!stats.eat(4, 2.4));
    }

    #[test]
    fn test_breath_depletion_and_drowning() {
        let mut stats = PlayerStats::new();
        assert_eq!(stats.breath, 15.0);

        // Submerged for 10 seconds
        stats.update(10.0, 10.0, true);
        assert_eq!(stats.breath, 5.0);
        assert_eq!(stats.health, 20.0); // Not drowning yet

        // Submerged for another 5 seconds -> breath reaches 0
        stats.update(5.0, 10.0, true);
        assert_eq!(stats.breath, 0.0);
        assert_eq!(stats.health, 20.0);

        // Submerged for 1 more second -> takes drowning damage (2 HP)
        stats.update(1.0, 10.0, true);
        assert_eq!(stats.health, 18.0);

        // Surface: rapid recovery
        stats.update(1.5, 10.0, false);
        assert!(stats.breath > 7.0);

        stats.update(2.0, 10.0, false);
        assert_eq!(stats.breath, 15.0);
    }
}
