use crate::item::{Inventory, ItemId, ItemSlot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FishingRodKind {
    Wooden,
    Copper,
    Iron,
    Diamond,
}

impl FishingRodKind {
    pub fn from_item(id: ItemId) -> Option<Self> {
        match id {
            ItemId::WOODEN_FISHING_ROD => Some(Self::Wooden),
            ItemId::COPPER_FISHING_ROD => Some(Self::Copper),
            ItemId::IRON_FISHING_ROD => Some(Self::Iron),
            ItemId::DIAMOND_FISHING_ROD => Some(Self::Diamond),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Wooden => "Wooden Rod",
            Self::Copper => "Copper Rod",
            Self::Iron => "Iron Rod",
            Self::Diamond => "Diamond Rod",
        }
    }

    pub fn max_durability(&self) -> u16 {
        match self {
            Self::Wooden => 64,
            Self::Copper => 150,
            Self::Iron => 280,
            Self::Diamond => 650,
        }
    }

    pub fn bar_size_bonus(&self) -> f32 {
        match self {
            Self::Wooden => 0.0,
            Self::Copper => 0.03,
            Self::Iron => 0.06,
            Self::Diamond => 0.10,
        }
    }

    pub fn bite_speed_mult(&self) -> f32 {
        match self {
            Self::Wooden => 1.0,
            Self::Copper => 1.15,
            Self::Iron => 1.30,
            Self::Diamond => 1.50,
        }
    }

    pub fn rare_chance_bonus(&self) -> f32 {
        match self {
            Self::Wooden => 0.0,
            Self::Copper => 0.05,
            Self::Iron => 0.12,
            Self::Diamond => 0.25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FishingLineKind {
    Braided,
    Fluorocarbon,
    Titanium,
}

impl FishingLineKind {
    pub fn from_item(id: ItemId) -> Option<Self> {
        match id {
            ItemId::BRAIDED_LINE => Some(Self::Braided),
            ItemId::FLUOROCARBON_LINE => Some(Self::Fluorocarbon),
            ItemId::TITANIUM_LINE => Some(Self::Titanium),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Braided => "Braided Line",
            Self::Fluorocarbon => "Fluorocarbon Line",
            Self::Titanium => "Titanium Line",
        }
    }

    pub fn max_durability(&self) -> u16 {
        match self {
            Self::Braided => 50,
            Self::Fluorocarbon => 75,
            Self::Titanium => 120,
        }
    }

    pub fn bar_size_bonus(&self) -> f32 {
        match self {
            Self::Braided => 0.05,
            Self::Fluorocarbon => 0.0,
            Self::Titanium => 0.04,
        }
    }

    pub fn escape_speed_mult(&self) -> f32 {
        match self {
            Self::Braided => 0.85,
            Self::Fluorocarbon => 1.0,
            Self::Titanium => 0.90,
        }
    }

    pub fn bite_speed_mult(&self) -> f32 {
        match self {
            Self::Braided => 1.0,
            Self::Fluorocarbon => 1.30,
            Self::Titanium => 1.05,
        }
    }

    pub fn rare_chance_bonus(&self) -> f32 {
        match self {
            Self::Braided => 0.0,
            Self::Fluorocarbon => 0.12,
            Self::Titanium => 0.05,
        }
    }

    pub fn progress_speed_mult(&self) -> f32 {
        match self {
            Self::Braided => 1.0,
            Self::Fluorocarbon => 1.0,
            Self::Titanium => 1.25,
        }
    }

    pub fn bounce_mult(&self) -> f32 {
        match self {
            Self::Braided => 1.0,
            Self::Fluorocarbon => 1.0,
            Self::Titanium => 0.55,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FishingLureKind {
    CorkBobber,
    Spinner,
    BarbedHook,
    TreasureMagnet,
}

impl FishingLureKind {
    pub fn from_item(id: ItemId) -> Option<Self> {
        match id {
            ItemId::CORK_BOBBER => Some(Self::CorkBobber),
            ItemId::SPINNER_LURE => Some(Self::Spinner),
            ItemId::BARBED_HOOK => Some(Self::BarbedHook),
            ItemId::TREASURE_LURE => Some(Self::TreasureMagnet),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CorkBobber => "Cork Bobber",
            Self::Spinner => "Copper Spinner",
            Self::BarbedHook => "Barbed Hook",
            Self::TreasureMagnet => "Treasure Magnet",
        }
    }

    pub fn max_durability(&self) -> u16 {
        match self {
            Self::CorkBobber => 40,
            Self::Spinner => 45,
            Self::BarbedHook => 50,
            Self::TreasureMagnet => 40,
        }
    }

    pub fn bar_size_bonus(&self) -> f32 {
        match self {
            Self::CorkBobber => 0.09,
            Self::Spinner => 0.0,
            Self::BarbedHook => 0.02,
            Self::TreasureMagnet => 0.0,
        }
    }

    pub fn bite_speed_mult(&self) -> f32 {
        match self {
            Self::CorkBobber => 1.0,
            Self::Spinner => 1.40,
            Self::BarbedHook => 1.05,
            Self::TreasureMagnet => 1.10,
        }
    }

    pub fn escape_speed_mult(&self) -> f32 {
        match self {
            Self::CorkBobber => 1.0,
            Self::Spinner => 1.0,
            Self::BarbedHook => 0.65,
            Self::TreasureMagnet => 1.0,
        }
    }

    pub fn bounce_mult(&self) -> f32 {
        match self {
            Self::CorkBobber => 0.90,
            Self::Spinner => 1.0,
            Self::BarbedHook => 0.50,
            Self::TreasureMagnet => 1.0,
        }
    }

    pub fn rare_chance_bonus(&self) -> f32 {
        match self {
            Self::CorkBobber => 0.0,
            Self::Spinner => 0.05,
            Self::BarbedHook => 0.0,
            Self::TreasureMagnet => 0.30,
        }
    }

    pub fn treasure_chance(&self) -> f32 {
        match self {
            Self::CorkBobber => 0.06,
            Self::Spinner => 0.08,
            Self::BarbedHook => 0.06,
            Self::TreasureMagnet => 0.35,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishBehavior {
    Smooth,
    Mixed,
    Dart,
    Sinker,
    Floater,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FishSpecies {
    pub item_id: ItemId,
    pub name: &'static str,
    /// 1..=100 difficulty (controls speed & erratic jumps)
    pub difficulty: f32,
    pub behavior: FishBehavior,
    pub rarity_weight: f32,
    pub prefers_ocean: bool,
    pub prefers_night: bool,
    pub is_rare: bool,
}

pub const ALL_FISH_SPECIES: &[FishSpecies] = &[
    FishSpecies {
        item_id: ItemId::RAW_CARP,
        name: "Carp",
        difficulty: 18.0,
        behavior: FishBehavior::Smooth,
        rarity_weight: 28.0,
        prefers_ocean: false,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_SARDINE,
        name: "Sardine",
        difficulty: 24.0,
        behavior: FishBehavior::Mixed,
        rarity_weight: 25.0,
        prefers_ocean: true,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_PERCH,
        name: "Perch",
        difficulty: 32.0,
        behavior: FishBehavior::Mixed,
        rarity_weight: 22.0,
        prefers_ocean: false,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_TROUT,
        name: "Rainbow Trout",
        difficulty: 40.0,
        behavior: FishBehavior::Smooth,
        rarity_weight: 18.0,
        prefers_ocean: false,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_BASS,
        name: "Largemouth Bass",
        difficulty: 48.0,
        behavior: FishBehavior::Mixed,
        rarity_weight: 16.0,
        prefers_ocean: false,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_SALMON,
        name: "Salmon",
        difficulty: 56.0,
        behavior: FishBehavior::Dart,
        rarity_weight: 13.0,
        prefers_ocean: true,
        prefers_night: false,
        is_rare: false,
    },
    FishSpecies {
        item_id: ItemId::RAW_CATFISH,
        name: "Catfish",
        difficulty: 65.0,
        behavior: FishBehavior::Sinker,
        rarity_weight: 9.0,
        prefers_ocean: false,
        prefers_night: true,
        is_rare: true,
    },
    FishSpecies {
        item_id: ItemId::PUFFERFISH,
        name: "Pufferfish",
        difficulty: 74.0,
        behavior: FishBehavior::Floater,
        rarity_weight: 7.0,
        prefers_ocean: true,
        prefers_night: false,
        is_rare: true,
    },
    FishSpecies {
        item_id: ItemId::RAW_TUNA,
        name: "Bluefin Tuna",
        difficulty: 80.0,
        behavior: FishBehavior::Dart,
        rarity_weight: 6.0,
        prefers_ocean: true,
        prefers_night: false,
        is_rare: true,
    },
    FishSpecies {
        item_id: ItemId::GOLDEN_KOI,
        name: "Golden Koi",
        difficulty: 90.0,
        behavior: FishBehavior::Dart,
        rarity_weight: 2.5,
        prefers_ocean: false,
        prefers_night: true,
        is_rare: true,
    },
];

#[derive(Debug, Clone)]
pub struct FishingTackleStats {
    pub rod: FishingRodKind,
    pub line: Option<FishingLineKind>,
    pub lure: Option<FishingLureKind>,
    pub line_slot: Option<usize>,
    pub lure_slot: Option<usize>,
    pub bait_slot: Option<usize>,
    pub bait_count: u16,
    pub bar_height: f32,
    pub bite_speed_mult: f32,
    pub progress_speed_mult: f32,
    pub escape_speed_mult: f32,
    pub bounce_mult: f32,
    pub rare_chance_bonus: f32,
    pub treasure_chance: f32,
}

impl FishingTackleStats {
    pub fn from_inventory(rod: FishingRodKind, inventory: &Inventory) -> Self {
        let mut best_line: Option<(usize, FishingLineKind)> = None;
        let mut best_lure: Option<(usize, FishingLureKind)> = None;
        let mut bait_slot: Option<usize> = None;
        let mut bait_count: u16 = 0;

        for idx in 0..inventory.len() {
            let slot = inventory.get(idx);
            if slot.is_empty() {
                continue;
            }
            if let Some(line) = FishingLineKind::from_item(slot.item_id) {
                if best_line.is_none()
                    || line.max_durability() > best_line.unwrap().1.max_durability()
                {
                    best_line = Some((idx, line));
                }
            }
            if let Some(lure) = FishingLureKind::from_item(slot.item_id) {
                if best_lure.is_none()
                    || lure.max_durability() > best_lure.unwrap().1.max_durability()
                {
                    best_lure = Some((idx, lure));
                }
            }
            if slot.item_id == ItemId::FISHING_BAIT {
                if bait_slot.is_none() {
                    bait_slot = Some(idx);
                }
                bait_count = bait_count.saturating_add(slot.count);
            }
        }

        let line = best_line.map(|(_, l)| l);
        let lure = best_lure.map(|(_, l)| l);

        let bar_height = (0.24
            + rod.bar_size_bonus()
            + line.map_or(0.0, |l| l.bar_size_bonus())
            + lure.map_or(0.0, |l| l.bar_size_bonus()))
        .clamp(0.20, 0.50);

        let bite_speed_mult = rod.bite_speed_mult()
            * line.map_or(1.0, |l| l.bite_speed_mult())
            * lure.map_or(1.0, |l| l.bite_speed_mult())
            * if bait_count > 0 { 1.25 } else { 1.0 };

        let progress_speed_mult = line.map_or(1.0, |l| l.progress_speed_mult());
        let escape_speed_mult =
            line.map_or(1.0, |l| l.escape_speed_mult()) * lure.map_or(1.0, |l| l.escape_speed_mult());
        let bounce_mult =
            line.map_or(1.0, |l| l.bounce_mult()) * lure.map_or(1.0, |l| l.bounce_mult());
        let rare_chance_bonus = rod.rare_chance_bonus()
            + line.map_or(0.0, |l| l.rare_chance_bonus())
            + lure.map_or(0.0, |l| l.rare_chance_bonus());
        let treasure_chance = 0.05 + lure.map_or(0.0, |l| l.treasure_chance());

        Self {
            rod,
            line,
            lure,
            line_slot: best_line.map(|(i, _)| i),
            lure_slot: best_lure.map(|(i, _)| i),
            bait_slot,
            bait_count,
            bar_height,
            bite_speed_mult,
            progress_speed_mult,
            escape_speed_mult,
            bounce_mult,
            rare_chance_bonus,
            treasure_chance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinigameStatus {
    Active,
    Caught,
    Escaped,
}

#[derive(Debug, Clone)]
pub struct FishingMinigameState {
    pub species: FishSpecies,
    /// Bottom edge of the green catch bar in 0.0..=(1.0 - bar_height)
    pub bar_pos: f32,
    pub bar_vel: f32,
    pub bar_height: f32,
    /// Fish vertical position in 0.0..=1.0
    pub fish_pos: f32,
    pub fish_vel: f32,
    pub fish_target: f32,
    pub fish_move_timer: f32,
    /// Catch progress 0.0..=1.0
    pub progress: f32,
    pub progress_speed_mult: f32,
    pub escape_speed_mult: f32,
    pub bounce_mult: f32,
    pub fish_in_bar: bool,
    pub perfect: bool,
}

impl FishingMinigameState {
    pub fn new(species: FishSpecies, tackle: &FishingTackleStats) -> Self {
        Self {
            species,
            bar_pos: 0.10,
            bar_vel: 0.0,
            bar_height: tackle.bar_height,
            fish_pos: 0.35,
            fish_vel: 0.0,
            fish_target: 0.55,
            fish_move_timer: 0.4,
            progress: 0.28,
            progress_speed_mult: tackle.progress_speed_mult,
            escape_speed_mult: tackle.escape_speed_mult,
            bounce_mult: tackle.bounce_mult,
            fish_in_bar: false,
            perfect: true,
        }
    }

    pub fn step(&mut self, dt: f32, holding_input: bool, rng: &mut u64) -> MinigameStatus {
        let dt = dt.clamp(0.0, 0.05);

        // 1. Update player green catch bar physics (Stardew-like gravity + thrust + bounce)
        let accel = if holding_input { 2.15 } else { -1.85 };
        self.bar_vel += accel * dt;
        self.bar_vel *= (1.0 - 1.6 * dt).max(0.0);
        self.bar_pos += self.bar_vel * dt;

        let max_bar_pos = (1.0 - self.bar_height).max(0.0);
        if self.bar_pos <= 0.0 {
            self.bar_pos = 0.0;
            if self.bar_vel < -0.15 {
                self.bar_vel = -self.bar_vel * 0.42 * self.bounce_mult;
            } else {
                self.bar_vel = 0.0;
            }
        } else if self.bar_pos >= max_bar_pos {
            self.bar_pos = max_bar_pos;
            if self.bar_vel > 0.0 {
                self.bar_vel = 0.0;
            }
        }

        // 2. Update fish AI target & movement
        self.fish_move_timer -= dt;
        if self.fish_move_timer <= 0.0 {
            let r1 = next_f32(rng);
            let r2 = next_f32(rng);
            let diff_norm = (self.species.difficulty / 100.0).clamp(0.1, 1.0);

            let (min_wait, max_wait) = match self.species.behavior {
                FishBehavior::Smooth => (0.9, 1.8),
                FishBehavior::Mixed => (0.55, 1.35),
                FishBehavior::Dart => (0.30, 0.90),
                FishBehavior::Sinker => (0.45, 1.15),
                FishBehavior::Floater => (0.45, 1.15),
            };
            self.fish_move_timer =
                min_wait + r1 * (max_wait - min_wait) * (1.15 - 0.45 * diff_norm);

            let raw_target = match self.species.behavior {
                FishBehavior::Smooth => {
                    let delta = (r2 - 0.5) * (0.28 + 0.25 * diff_norm);
                    (self.fish_pos + delta).clamp(0.08, 0.92)
                }
                FishBehavior::Mixed => 0.08 + r2 * 0.84,
                FishBehavior::Dart => {
                    let jump = (r2 - 0.5) * (0.55 + 0.45 * diff_norm);
                    (self.fish_pos + jump).clamp(0.06, 0.94)
                }
                FishBehavior::Sinker => (0.05 + r2.powf(1.6) * 0.70).clamp(0.05, 0.85),
                FishBehavior::Floater => (0.95 - r2.powf(1.6) * 0.70).clamp(0.15, 0.95),
            };
            self.fish_target = raw_target;
        }

        let diff_norm = (self.species.difficulty / 100.0).clamp(0.1, 1.0);
        let max_fish_speed = (0.25 + diff_norm * 0.78) * 1.10;
        let stiffness = (2.8 + diff_norm * 4.5) * 1.10;
        let desired_vel = ((self.fish_target - self.fish_pos) * stiffness)
            .clamp(-max_fish_speed, max_fish_speed);
        self.fish_vel += (desired_vel - self.fish_vel) * (8.0 * dt).min(1.0);
        self.fish_pos = (self.fish_pos + self.fish_vel * dt).clamp(0.03, 0.97);

        // 3. Check overlap and update catch progress
        let bar_bottom = self.bar_pos;
        let bar_top = self.bar_pos + self.bar_height;
        // Small generous margin around fish center
        let margin = 0.022;
        self.fish_in_bar = self.fish_pos >= (bar_bottom - margin) && self.fish_pos <= (bar_top + margin);

        if self.fish_in_bar {
            self.progress += 0.215 * self.progress_speed_mult * dt;
        } else {
            self.perfect = false;
            self.progress -= 0.176 * self.escape_speed_mult * dt;
        }

        if self.progress >= 1.0 {
            self.progress = 1.0;
            MinigameStatus::Caught
        } else if self.progress <= 0.0 {
            self.progress = 0.0;
            MinigameStatus::Escaped
        } else {
            MinigameStatus::Active
        }
    }
}

pub fn next_u64(state: &mut u64) -> u64 {
    let mut x = *state;
    if x == 0 {
        x = 0x9E3779B97F4A7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

pub fn next_f32(state: &mut u64) -> f32 {
    (next_u64(state) >> 40) as f32 / (1u64 << 24) as f32
}

pub fn roll_fish_species(
    rng: &mut u64,
    is_ocean: bool,
    is_night: bool,
    tackle: &FishingTackleStats,
) -> FishSpecies {
    let mut weights = [0.0f32; 16];
    let mut total = 0.0f32;
    for (i, sp) in ALL_FISH_SPECIES.iter().enumerate() {
        let mut w = sp.rarity_weight;
        if sp.prefers_ocean == is_ocean {
            w *= 1.45;
        } else {
            w *= 0.65;
        }
        if sp.prefers_night && is_night {
            w *= 1.8;
        }
        if sp.is_rare {
            w *= 1.0 + tackle.rare_chance_bonus * 2.2;
        }
        weights[i] = w;
        total += w;
    }

    let mut pick = next_f32(rng) * total;
    for (i, sp) in ALL_FISH_SPECIES.iter().enumerate() {
        pick -= weights[i];
        if pick <= 0.0 {
            return *sp;
        }
    }
    ALL_FISH_SPECIES[0]
}

pub fn roll_treasure_loot(rng: &mut u64) -> ItemSlot {
    let r = next_f32(rng);
    if r < 0.20 {
        ItemSlot::new(ItemId::COAL, 2 + (next_u64(rng) % 4) as u16)
    } else if r < 0.40 {
        ItemSlot::new(ItemId::IRON_INGOT, 1 + (next_u64(rng) % 3) as u16)
    } else if r < 0.58 {
        ItemSlot::new(ItemId::GOLD_INGOT, 1 + (next_u64(rng) % 2) as u16)
    } else if r < 0.72 {
        ItemSlot::new(ItemId::ARROW, 4 + (next_u64(rng) % 8) as u16)
    } else if r < 0.85 {
        ItemSlot::new(ItemId::FISHING_BAIT, 3 + (next_u64(rng) % 5) as u16)
    } else if r < 0.94 {
        ItemSlot::new(ItemId::PRISMARINE, 1 + (next_u64(rng) % 3) as u16)
    } else {
        ItemSlot::new(ItemId::DIAMOND, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tackle_stats_combine_rod_line_lure_and_bait() {
        let mut inv = Inventory::new(36);
        inv.set(0, ItemSlot::with_durability(ItemId::DIAMOND_FISHING_ROD, 1, 0));
        inv.set(1, ItemSlot::with_durability(ItemId::TITANIUM_LINE, 1, 0));
        inv.set(2, ItemSlot::with_durability(ItemId::CORK_BOBBER, 1, 0));
        inv.set(3, ItemSlot::new(ItemId::FISHING_BAIT, 10));

        let stats = FishingTackleStats::from_inventory(FishingRodKind::Diamond, &inv);
        assert_eq!(stats.line, Some(FishingLineKind::Titanium));
        assert_eq!(stats.lure, Some(FishingLureKind::CorkBobber));
        assert_eq!(stats.bait_count, 10);
        // 0.24 + 0.10 (diamond) + 0.04 (titanium) + 0.09 (cork) = 0.47
        assert!((stats.bar_height - 0.47).abs() < 0.001);
        assert!(stats.bite_speed_mult > 1.8);
    }

    #[test]
    fn test_minigame_can_catch_and_escape() {
        let inv = Inventory::new(36);
        let stats = FishingTackleStats::from_inventory(FishingRodKind::Wooden, &inv);
        let mut game = FishingMinigameState::new(ALL_FISH_SPECIES[0], &stats);
        game.bar_pos = 0.30;
        game.fish_pos = 0.35;

        let mut rng = 42u64;
        let mut caught = false;
        for _ in 0..200 {
            game.fish_pos = game.bar_pos + game.bar_height * 0.5;
            if game.step(0.03, false, &mut rng) == MinigameStatus::Caught {
                caught = true;
                break;
            }
        }
        assert!(caught);

        let mut game2 = FishingMinigameState::new(ALL_FISH_SPECIES[0], &stats);
        let mut escaped = false;
        for _ in 0..200 {
            game2.fish_pos = 0.99;
            game2.bar_pos = 0.0;
            if game2.step(0.03, false, &mut rng) == MinigameStatus::Escaped {
                escaped = true;
                break;
            }
        }
        assert!(escaped);
    }

    #[test]
    fn test_roll_fish_and_treasure() {
        let inv = Inventory::new(36);
        let stats = FishingTackleStats::from_inventory(FishingRodKind::Iron, &inv);
        let mut rng = 123456789u64;
        for _ in 0..50 {
            let sp = roll_fish_species(&mut rng, true, true, &stats);
            assert!(sp.difficulty > 0.0);
            let loot = roll_treasure_loot(&mut rng);
            assert!(!loot.is_empty());
        }
    }
}
