use crate::block::BlockId;

pub const MAX_SKILL_LEVEL: u16 = 1000;
pub const SKILL_COUNT: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SkillKind {
    Combat = 0,
    Woodcutting = 1,
    Mining = 2,
    Fishing = 3,
    Digging = 4,
    Farming = 5,
    Agility = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillMilestone {
    pub level: u16,
    pub name: &'static str,
    pub description: &'static str,
}

const COMBAT_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Warrior Stance",
        description: "+1.0 base bonus attack damage",
    },
    SkillMilestone {
        level: 50,
        name: "Critical Strike",
        description: "12% chance to land 1.5x critical hits",
    },
    SkillMilestone {
        level: 100,
        name: "Lifesteal",
        description: "Restore 1 HP upon slaying a monster",
    },
    SkillMilestone {
        level: 250,
        name: "Berserker Focus",
        description: "Critical hit chance rises to 22%",
    },
    SkillMilestone {
        level: 500,
        name: "Iron Skin",
        description: "+10% permanent incoming damage reduction",
    },
    SkillMilestone {
        level: 750,
        name: "Executioner",
        description: "+2.0 bonus damage and 2 HP lifesteal on kill",
    },
    SkillMilestone {
        level: 1000,
        name: "Warlord",
        description: "35% crit chance, +3.0 bonus damage, 3 HP lifesteal",
    },
];

const WOODCUTTING_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Lumberjack Grip",
        description: "+10% chopping speed",
    },
    SkillMilestone {
        level: 50,
        name: "Double Timber",
        description: "+12% chance for extra log drops",
    },
    SkillMilestone {
        level: 100,
        name: "Leaf Blower",
        description: "Bonus apple & sapling drops from tree leaves",
    },
    SkillMilestone {
        level: 250,
        name: "Tree Feller",
        description: "Axe breaks fell up to 12 connected logs above",
    },
    SkillMilestone {
        level: 500,
        name: "Hardwood Mastery",
        description: "+25% chopping speed & 24-log Tree Feller",
    },
    SkillMilestone {
        level: 750,
        name: "Forest Bounty",
        description: "Chance for triple logs & 48-log Tree Feller",
    },
    SkillMilestone {
        level: 1000,
        name: "Worldtree Blessing",
        description: "Peak chopping speed & 96-log Tree Feller",
    },
];

const MINING_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Stonebreaker",
        description: "+10% mining speed",
    },
    SkillMilestone {
        level: 50,
        name: "Double Drops",
        description: "+12% chance for bonus mined block drops",
    },
    SkillMilestone {
        level: 100,
        name: "Ore Fortune",
        description: "Extra bonus yield rolls on Coal, Iron, Gold & Diamond ores",
    },
    SkillMilestone {
        level: 250,
        name: "Super Breaker",
        description: "+20% mining speed boost",
    },
    SkillMilestone {
        level: 500,
        name: "Vein Resonance",
        description: "+25% mining speed & stronger mineral fortune",
    },
    SkillMilestone {
        level: 750,
        name: "Master Excavator",
        description: "Chance for triple ore yields",
    },
    SkillMilestone {
        level: 1000,
        name: "Earthshaker",
        description: "Peak mining speed & guaranteed bonus mineral mastery",
    },
];

const FISHING_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Patient Angler",
        description: "+10% faster fish bite speed",
    },
    SkillMilestone {
        level: 50,
        name: "Steady Reel",
        description: "Wider green catch bar during minigame",
    },
    SkillMilestone {
        level: 100,
        name: "Treasure Fisher",
        description: "+6% treasure find chance",
    },
    SkillMilestone {
        level: 250,
        name: "Double Catch",
        description: "15% chance to reel in 2 fish at once",
    },
    SkillMilestone {
        level: 500,
        name: "Master Caster",
        description: "+25% faster bites & rarer catch luck",
    },
    SkillMilestone {
        level: 750,
        name: "Deep Sea Hunter",
        description: "30% double catch & +12% treasure bonus",
    },
    SkillMilestone {
        level: 1000,
        name: "Poseidon's Favor",
        description: "50% double catch & peak catch bar control",
    },
];

const DIGGING_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Quick Shovel",
        description: "+10% digging speed",
    },
    SkillMilestone {
        level: 50,
        name: "Archaeologist",
        description: "Uncover Coal, Flint & Clay while digging soil",
    },
    SkillMilestone {
        level: 100,
        name: "Sand Sifter",
        description: "+15% double drop chance on dirt, sand & gravel",
    },
    SkillMilestone {
        level: 250,
        name: "Treasure Seeker",
        description: "Discover Raw Iron, String & Apples buried underground",
    },
    SkillMilestone {
        level: 500,
        name: "Giga Drill",
        description: "+30% digging speed boost",
    },
    SkillMilestone {
        level: 750,
        name: "Relic Hunter",
        description: "Chance to unearth Gold Ingots & rare Diamonds",
    },
    SkillMilestone {
        level: 1000,
        name: "Terra Sculptor",
        description: "Peak excavation speed & treasure mastery",
    },
];

const FARMING_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Green Thumb",
        description: "+15% bonus seed & crop yield chance",
    },
    SkillMilestone {
        level: 50,
        name: "Bountiful Harvest",
        description: "Extra wheat bundles from ripe crops",
    },
    SkillMilestone {
        level: 100,
        name: "Nutrient Rich",
        description: "Eating food restores +15% extra hunger & health",
    },
    SkillMilestone {
        level: 250,
        name: "Rapid Sprout",
        description: "Harvesting ripe wheat auto-replants new seeds",
    },
    SkillMilestone {
        level: 500,
        name: "Golden Harvest",
        description: "High chance for double & triple wheat harvests",
    },
    SkillMilestone {
        level: 750,
        name: "Feast Master",
        description: "Food restores +35% extra hunger & health",
    },
    SkillMilestone {
        level: 1000,
        name: "Nature's Avatar",
        description: "Peak crop yields & food nutrition mastery",
    },
];

const AGILITY_MILESTONES: [SkillMilestone; 7] = [
    SkillMilestone {
        level: 25,
        name: "Light Footed",
        description: "-10% fall damage taken",
    },
    SkillMilestone {
        level: 50,
        name: "Swift Stride",
        description: "+6% movement speed",
    },
    SkillMilestone {
        level: 100,
        name: "Graceful Roll",
        description: "-25% total fall damage reduction",
    },
    SkillMilestone {
        level: 250,
        name: "Parkourist",
        description: "+8% jump height boost",
    },
    SkillMilestone {
        level: 500,
        name: "Wind Runner",
        description: "+15% movement speed & -40% fall damage",
    },
    SkillMilestone {
        level: 750,
        name: "Feather Step",
        description: "-55% total fall damage reduction",
    },
    SkillMilestone {
        level: 1000,
        name: "Phantom Step",
        description: "Peak sprint speed, jump boost & fall resistance",
    },
];

impl SkillKind {
    pub const ALL: [SkillKind; SKILL_COUNT] = [
        SkillKind::Combat,
        SkillKind::Woodcutting,
        SkillKind::Mining,
        SkillKind::Fishing,
        SkillKind::Digging,
        SkillKind::Farming,
        SkillKind::Agility,
    ];

    pub const fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Combat),
            1 => Some(Self::Woodcutting),
            2 => Some(Self::Mining),
            3 => Some(Self::Fishing),
            4 => Some(Self::Digging),
            5 => Some(Self::Farming),
            6 => Some(Self::Agility),
            _ => None,
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            Self::Combat => "Combat",
            Self::Woodcutting => "Woodcutting",
            Self::Mining => "Mining",
            Self::Fishing => "Fishing",
            Self::Digging => "Digging",
            Self::Farming => "Farming",
            Self::Agility => "Agility",
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Combat => "Slay monsters with swords or bows to increase attack power, crits, lifesteal, and armor.",
            Self::Woodcutting => "Chop tree logs to increase axe speed, log yields, and unlock Tree Feller.",
            Self::Mining => "Mine stone and ores to increase pickaxe speed and unlock bonus mineral fortune.",
            Self::Fishing => "Catch fish to shorten bite waits, widen the catch bar, and unlock double catches.",
            Self::Digging => "Excavate dirt, sand, and gravel to boost shovel speed and uncover buried treasure.",
            Self::Farming => "Tend soil, crops, and animals to boost harvest yields, nutrition, and auto-replanting.",
            Self::Agility => "Sprint, jump, and swim to increase movement speed, jump height, and reduce fall damage.",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "combat" | "fight" | "fighting" | "sword" | "melee" => Some(Self::Combat),
            "woodcutting" | "wood" | "wc" | "lumber" | "chop" => Some(Self::Woodcutting),
            "mining" | "mine" => Some(Self::Mining),
            "fishing" | "fish" => Some(Self::Fishing),
            "digging" | "dig" | "excavation" => Some(Self::Digging),
            "farming" | "farm" | "herbalism" => Some(Self::Farming),
            "agility" | "agi" | "acrobatics" | "speed" => Some(Self::Agility),
            _ => None,
        }
    }

    pub const fn milestones(&self) -> &'static [SkillMilestone; 7] {
        match self {
            Self::Combat => &COMBAT_MILESTONES,
            Self::Woodcutting => &WOODCUTTING_MILESTONES,
            Self::Mining => &MINING_MILESTONES,
            Self::Fishing => &FISHING_MILESTONES,
            Self::Digging => &DIGGING_MILESTONES,
            Self::Farming => &FARMING_MILESTONES,
            Self::Agility => &AGILITY_MILESTONES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillProgress {
    pub level: u16,
    pub xp: u64,
}

impl Default for SkillProgress {
    fn default() -> Self {
        Self { level: 1, xp: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLevelUp {
    pub skill: SkillKind,
    pub old_level: u16,
    pub new_level: u16,
    pub unlocked_milestones: Vec<SkillMilestone>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSkills {
    pub entries: [SkillProgress; SKILL_COUNT],
}

impl Default for PlayerSkills {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerSkills {
    pub fn new() -> Self {
        Self {
            entries: [SkillProgress::default(); SKILL_COUNT],
        }
    }

    #[inline]
    pub fn get(&self, skill: SkillKind) -> SkillProgress {
        self.entries[skill as usize]
    }

    #[inline]
    pub fn level(&self, skill: SkillKind) -> u16 {
        self.entries[skill as usize].level.clamp(1, MAX_SKILL_LEVEL)
    }

    #[inline]
    pub fn xp(&self, skill: SkillKind) -> u64 {
        self.entries[skill as usize].xp
    }

    pub fn total_level(&self) -> u32 {
        self.entries.iter().map(|e| e.level as u32).sum()
    }

    /// XP required to advance from `level` to `level + 1`.
    pub fn xp_for_next_level(level: u16) -> u64 {
        let lvl = level.max(1) as u64;
        if lvl >= MAX_SKILL_LEVEL as u64 {
            return 0;
        }
        let d = lvl - 1;
        50 + d * 14 + (d * d) / 8
    }

    pub fn add_xp(&mut self, skill: SkillKind, amount: u64) -> Option<SkillLevelUp> {
        if amount == 0 {
            return None;
        }
        let entry = &mut self.entries[skill as usize];
        entry.level = entry.level.clamp(1, MAX_SKILL_LEVEL);
        if entry.level >= MAX_SKILL_LEVEL {
            entry.xp = 0;
            return None;
        }

        let old_level = entry.level;
        entry.xp = entry.xp.saturating_add(amount);

        while entry.level < MAX_SKILL_LEVEL {
            let needed = Self::xp_for_next_level(entry.level);
            if entry.xp >= needed {
                entry.xp -= needed;
                entry.level += 1;
            } else {
                break;
            }
        }
        if entry.level >= MAX_SKILL_LEVEL {
            entry.xp = 0;
        }

        if entry.level > old_level {
            let new_level = entry.level;
            let unlocked_milestones = skill
                .milestones()
                .iter()
                .copied()
                .filter(|m| m.level > old_level && m.level <= new_level)
                .collect();
            Some(SkillLevelUp {
                skill,
                old_level,
                new_level,
                unlocked_milestones,
            })
        } else {
            None
        }
    }

    pub fn set_level(&mut self, skill: SkillKind, level: u16) {
        let entry = &mut self.entries[skill as usize];
        entry.level = level.clamp(1, MAX_SKILL_LEVEL);
        entry.xp = 0;
    }

    #[inline]
    fn norm(level: u16) -> f32 {
        ((level.saturating_sub(1)) as f32 / (MAX_SKILL_LEVEL - 1) as f32).clamp(0.0, 1.0)
    }

    // --- Mining Perks ---

    pub fn mining_speed_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Mining);
        let t = Self::norm(lvl).powf(0.85);
        let mut mult = 1.0 + 0.85 * t;
        if lvl >= 25 {
            mult += 0.10;
        }
        if lvl >= 250 {
            mult += 0.20;
        }
        if lvl >= 500 {
            mult += 0.25;
        }
        if lvl >= 1000 {
            mult += 0.35;
        }
        mult
    }

    pub fn mining_bonus_drop_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Mining);
        if lvl <= 1 {
            return 0.0;
        }
        let mut chance = 0.35 * Self::norm(lvl).powf(0.90);
        if lvl >= 50 {
            chance += 0.12;
        }
        if lvl >= 100 {
            chance += 0.10;
        }
        if lvl >= 500 {
            chance += 0.10;
        }
        if lvl >= 1000 {
            chance += 0.20;
        }
        chance.clamp(0.0, 1.0)
    }

    pub fn mining_triple_ore_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Mining);
        if lvl < 750 {
            0.0
        } else if lvl >= 1000 {
            0.35
        } else {
            0.18
        }
    }

    // --- Woodcutting Perks ---

    pub fn woodcutting_speed_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Woodcutting);
        let t = Self::norm(lvl).powf(0.85);
        let mut mult = 1.0 + 0.85 * t;
        if lvl >= 25 {
            mult += 0.10;
        }
        if lvl >= 500 {
            mult += 0.25;
        }
        if lvl >= 1000 {
            mult += 0.35;
        }
        mult
    }

    pub fn woodcutting_bonus_drop_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Woodcutting);
        if lvl <= 1 {
            return 0.0;
        }
        let mut chance = 0.35 * Self::norm(lvl).powf(0.90);
        if lvl >= 50 {
            chance += 0.12;
        }
        if lvl >= 750 {
            chance += 0.15;
        }
        if lvl >= 1000 {
            chance += 0.20;
        }
        chance.clamp(0.0, 1.0)
    }

    pub fn woodcutting_leaf_bonus_unlocked(&self) -> bool {
        self.level(SkillKind::Woodcutting) >= 100
    }

    pub fn tree_feller_max_logs(&self) -> usize {
        let lvl = self.level(SkillKind::Woodcutting);
        if lvl >= 1000 {
            96
        } else if lvl >= 750 {
            48
        } else if lvl >= 500 {
            24
        } else if lvl >= 250 {
            12
        } else {
            0
        }
    }

    // --- Digging Perks ---

    pub fn digging_speed_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Digging);
        let t = Self::norm(lvl).powf(0.85);
        let mut mult = 1.0 + 0.85 * t;
        if lvl >= 25 {
            mult += 0.10;
        }
        if lvl >= 500 {
            mult += 0.30;
        }
        if lvl >= 1000 {
            mult += 0.35;
        }
        mult
    }

    pub fn digging_double_drop_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Digging);
        if lvl <= 1 {
            return 0.0;
        }
        let mut chance = 0.30 * Self::norm(lvl).powf(0.90);
        if lvl >= 100 {
            chance += 0.15;
        }
        if lvl >= 1000 {
            chance += 0.20;
        }
        chance.clamp(0.0, 1.0)
    }

    pub fn digging_treasure_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Digging);
        if lvl < 10 {
            return 0.0;
        }
        let mut chance = 0.06 * Self::norm(lvl);
        if lvl >= 50 {
            chance += 0.04;
        }
        if lvl >= 250 {
            chance += 0.04;
        }
        if lvl >= 750 {
            chance += 0.04;
        }
        chance.clamp(0.0, 0.22)
    }

    // --- Combat Perks ---

    pub fn combat_damage_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Combat);
        1.0 + 0.85 * Self::norm(lvl).powf(0.85)
    }

    pub fn combat_flat_bonus(&self) -> f32 {
        let lvl = self.level(SkillKind::Combat);
        let mut bonus = 0.0;
        if lvl >= 25 {
            bonus += 1.0;
        }
        if lvl >= 750 {
            bonus += 1.0;
        }
        if lvl >= 1000 {
            bonus += 1.0;
        }
        bonus
    }

    pub fn combat_crit_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Combat);
        if lvl >= 1000 {
            0.35
        } else if lvl >= 250 {
            0.22
        } else if lvl >= 50 {
            0.12
        } else {
            0.0
        }
    }

    pub fn combat_lifesteal_on_kill(&self) -> f32 {
        let lvl = self.level(SkillKind::Combat);
        if lvl >= 1000 {
            3.0
        } else if lvl >= 750 {
            2.0
        } else if lvl >= 100 {
            1.0
        } else {
            0.0
        }
    }

    pub fn combat_damage_reduction(&self) -> f32 {
        let lvl = self.level(SkillKind::Combat);
        let mut red = 0.12 * Self::norm(lvl);
        if lvl >= 500 {
            red += 0.10;
        }
        red.clamp(0.0, 0.28)
    }

    // --- Fishing Perks ---

    pub fn fishing_bite_speed_mult(&self) -> f32 {
        let lvl = self.level(SkillKind::Fishing);
        let mut mult = 1.0 + 0.45 * Self::norm(lvl).powf(0.85);
        if lvl >= 25 {
            mult += 0.10;
        }
        if lvl >= 500 {
            mult += 0.25;
        }
        mult
    }

    pub fn fishing_bar_height_bonus(&self) -> f32 {
        let lvl = self.level(SkillKind::Fishing);
        let mut bonus = 0.045 * Self::norm(lvl);
        if lvl >= 50 {
            bonus += 0.025;
        }
        if lvl >= 1000 {
            bonus += 0.03;
        }
        bonus
    }

    pub fn fishing_treasure_chance_bonus(&self) -> f32 {
        let lvl = self.level(SkillKind::Fishing);
        let mut bonus = 0.08 * Self::norm(lvl);
        if lvl >= 100 {
            bonus += 0.06;
        }
        if lvl >= 750 {
            bonus += 0.06;
        }
        bonus
    }

    pub fn fishing_double_catch_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Fishing);
        if lvl >= 1000 {
            0.50
        } else if lvl >= 750 {
            0.30
        } else if lvl >= 250 {
            0.15
        } else {
            0.0
        }
    }

    // --- Farming Perks ---

    pub fn farming_bonus_crop_chance(&self) -> f32 {
        let lvl = self.level(SkillKind::Farming);
        if lvl <= 1 {
            return 0.0;
        }
        let mut chance = 0.40 * Self::norm(lvl).powf(0.9);
        if lvl >= 25 {
            chance += 0.15;
        }
        if lvl >= 50 {
            chance += 0.10;
        }
        if lvl >= 500 {
            chance += 0.15;
        }
        if lvl >= 1000 {
            chance += 0.20;
        }
        chance.clamp(0.0, 1.0)
    }

    pub fn farming_auto_replant(&self) -> bool {
        self.level(SkillKind::Farming) >= 250
    }

    pub fn farming_food_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Farming);
        let mut mult = 1.0 + 0.20 * Self::norm(lvl);
        if lvl >= 100 {
            mult += 0.15;
        }
        if lvl >= 750 {
            mult += 0.20;
        }
        mult
    }

    // --- Agility Perks ---

    pub fn agility_speed_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Agility);
        let mut mult = 1.0 + 0.18 * Self::norm(lvl).powf(0.85);
        if lvl >= 50 {
            mult += 0.06;
        }
        if lvl >= 500 {
            mult += 0.09;
        }
        if lvl >= 1000 {
            mult += 0.07;
        }
        mult
    }

    pub fn agility_jump_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Agility);
        let mut mult = 1.0 + 0.08 * Self::norm(lvl);
        if lvl >= 250 {
            mult += 0.08;
        }
        if lvl >= 1000 {
            mult += 0.06;
        }
        mult
    }

    pub fn agility_fall_damage_multiplier(&self) -> f32 {
        let lvl = self.level(SkillKind::Agility);
        let mut red = 0.20 * Self::norm(lvl);
        if lvl >= 25 {
            red += 0.10;
        }
        if lvl >= 100 {
            red += 0.15;
        }
        if lvl >= 500 {
            red += 0.15;
        }
        if lvl >= 750 {
            red += 0.15;
        }
        (1.0 - red.clamp(0.0, 0.70)).max(0.30)
    }

    pub fn perks_summary(&self, skill: SkillKind) -> String {
        match skill {
            SkillKind::Combat => format!(
                "+{:.0}% Dmg (+{:.0}), {:.0}% Crit, {:.0}% Def",
                (self.combat_damage_multiplier() - 1.0) * 100.0,
                self.combat_flat_bonus(),
                self.combat_crit_chance() * 100.0,
                self.combat_damage_reduction() * 100.0
            ),
            SkillKind::Woodcutting => format!(
                "+{:.0}% Speed, {:.0}% Bonus Logs, Feller: {}",
                (self.woodcutting_speed_multiplier() - 1.0) * 100.0,
                self.woodcutting_bonus_drop_chance() * 100.0,
                self.tree_feller_max_logs()
            ),
            SkillKind::Mining => format!(
                "+{:.0}% Speed, {:.0}% Bonus Drops",
                (self.mining_speed_multiplier() - 1.0) * 100.0,
                self.mining_bonus_drop_chance() * 100.0
            ),
            SkillKind::Fishing => format!(
                "+{:.0}% Bite Speed, +{:.0}% Bar, {:.0}% Dbl Catch",
                (self.fishing_bite_speed_mult() - 1.0) * 100.0,
                self.fishing_bar_height_bonus() * 100.0,
                self.fishing_double_catch_chance() * 100.0
            ),
            SkillKind::Digging => format!(
                "+{:.0}% Speed, {:.0}% Dbl, {:.1}% Treasure",
                (self.digging_speed_multiplier() - 1.0) * 100.0,
                self.digging_double_drop_chance() * 100.0,
                self.digging_treasure_chance() * 100.0
            ),
            SkillKind::Farming => format!(
                "{:.0}% Bonus Yield, +{:.0}% Food{}",
                self.farming_bonus_crop_chance() * 100.0,
                (self.farming_food_multiplier() - 1.0) * 100.0,
                if self.farming_auto_replant() {
                    ", Auto-Replant"
                } else {
                    ""
                }
            ),
            SkillKind::Agility => format!(
                "+{:.0}% Move, +{:.0}% Jump, -{:.0}% Fall Dmg",
                (self.agility_speed_multiplier() - 1.0) * 100.0,
                (self.agility_jump_multiplier() - 1.0) * 100.0,
                (1.0 - self.agility_fall_damage_multiplier()) * 100.0
            ),
        }
    }
}

/// Classifies a broken block into the skill and base XP earned for breaking it.
pub fn skill_xp_for_broken_block(block: BlockId) -> Option<(SkillKind, u64)> {
    if block.is_log() {
        return Some((SkillKind::Woodcutting, 22));
    }
    if block.is_leaves() {
        return Some((SkillKind::Woodcutting, 3));
    }
    match block {
        BlockId::STONE
        | BlockId::COBBLESTONE
        | BlockId::MOSSY_COBBLESTONE
        | BlockId::DEEPSTONE
        | BlockId::DEEPSTONE_COBBLESTONE
        | BlockId::MOSSY_DEEPSTONE_COBBLESTONE
        | BlockId::DRIPSTONE => Some((SkillKind::Mining, 8)),
        BlockId::SANDSTONE
        | BlockId::TERRACOTTA
        | BlockId::BRICKS
        | BlockId::COBBLESTONE_STAIRS
        | BlockId::BRICK_STAIRS
        | BlockId::FURNACE
        | BlockId::FURNACE_LIT => Some((SkillKind::Mining, 10)),
        BlockId::COAL_ORE | BlockId::DEEPSTONE_COAL_ORE => Some((SkillKind::Mining, 18)),
        BlockId::COPPER_ORE | BlockId::DEEPSTONE_COPPER_ORE => Some((SkillKind::Mining, 24)),
        BlockId::IRON_ORE | BlockId::DEEPSTONE_IRON_ORE => Some((SkillKind::Mining, 30)),
        BlockId::GOLD_ORE | BlockId::DEEPSTONE_GOLD_ORE => Some((SkillKind::Mining, 45)),
        BlockId::OBSIDIAN => Some((SkillKind::Mining, 60)),
        BlockId::DIAMOND_ORE | BlockId::DEEPSTONE_DIAMOND_ORE => Some((SkillKind::Mining, 95)),
        BlockId::DIRT
        | BlockId::GRASS
        | BlockId::PODZOL
        | BlockId::MYCELIUM
        | BlockId::SAND
        | BlockId::RED_SAND
        | BlockId::GRAVEL
        | BlockId::FARMLAND
        | BlockId::FARMLAND_MOIST
        | BlockId::SNOW_BLOCK => Some((SkillKind::Digging, 8)),
        BlockId::CLAY => Some((SkillKind::Digging, 14)),
        BlockId::WHEAT_3 => Some((SkillKind::Farming, 25)),
        BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 => Some((SkillKind::Farming, 4)),
        _ => None,
    }
}

/// Returns which skill speeds up breaking `block`.
pub fn break_speed_skill_for_block(block: BlockId) -> Option<SkillKind> {
    if block.is_log() {
        return Some(SkillKind::Woodcutting);
    }
    match block {
        BlockId::STONE
        | BlockId::COBBLESTONE
        | BlockId::MOSSY_COBBLESTONE
        | BlockId::DEEPSTONE
        | BlockId::DEEPSTONE_COBBLESTONE
        | BlockId::MOSSY_DEEPSTONE_COBBLESTONE
        | BlockId::DRIPSTONE
        | BlockId::OBSIDIAN
        | BlockId::SANDSTONE
        | BlockId::TERRACOTTA
        | BlockId::BRICKS
        | BlockId::COBBLESTONE_STAIRS
        | BlockId::BRICK_STAIRS
        | BlockId::FURNACE
        | BlockId::FURNACE_LIT
        | BlockId::COAL_ORE
        | BlockId::DEEPSTONE_COAL_ORE
        | BlockId::COPPER_ORE
        | BlockId::DEEPSTONE_COPPER_ORE
        | BlockId::IRON_ORE
        | BlockId::DEEPSTONE_IRON_ORE
        | BlockId::GOLD_ORE
        | BlockId::DEEPSTONE_GOLD_ORE
        | BlockId::DIAMOND_ORE
        | BlockId::DEEPSTONE_DIAMOND_ORE => Some(SkillKind::Mining),
        BlockId::DIRT
        | BlockId::GRASS
        | BlockId::PODZOL
        | BlockId::MYCELIUM
        | BlockId::SAND
        | BlockId::RED_SAND
        | BlockId::GRAVEL
        | BlockId::CLAY
        | BlockId::FARMLAND
        | BlockId::FARMLAND_MOIST
        | BlockId::SNOW_BLOCK => Some(SkillKind::Digging),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skills_initialization_and_level_1000_cap() {
        let mut skills = PlayerSkills::new();
        assert_eq!(skills.total_level(), 7);
        for s in SkillKind::ALL {
            assert_eq!(skills.level(s), 1);
            assert_eq!(skills.xp(s), 0);
        }

        // Level 1 perks start minimal (no bonuses)
        assert!((skills.mining_speed_multiplier() - 1.0).abs() < 1e-6);
        assert_eq!(skills.mining_bonus_drop_chance(), 0.0);
        assert_eq!(skills.tree_feller_max_logs(), 0);
        assert_eq!(skills.combat_crit_chance(), 0.0);
        assert!(!skills.farming_auto_replant());

        // Gain huge XP -> caps cleanly at 1000
        let lvlup = skills.add_xp(SkillKind::Mining, 100_000_000).unwrap();
        assert_eq!(lvlup.old_level, 1);
        assert_eq!(lvlup.new_level, 1000);
        assert_eq!(lvlup.unlocked_milestones.len(), 7);
        assert_eq!(skills.level(SkillKind::Mining), 1000);
        assert_eq!(skills.xp(SkillKind::Mining), 0);
        assert!(skills.mining_speed_multiplier() > 2.5);
    }

    #[test]
    fn test_skill_gradual_scaling_and_milestones() {
        let mut skills = PlayerSkills::new();
        let res = skills.add_xp(SkillKind::Woodcutting, 50).unwrap();
        assert_eq!(res.new_level, 2);
        assert!(res.unlocked_milestones.is_empty());

        // Lv 2 is minimal increment over Lv 1
        assert!(skills.woodcutting_speed_multiplier() > 1.0);
        assert!(skills.woodcutting_speed_multiplier() < 1.02);

        skills.set_level(SkillKind::Woodcutting, 250);
        assert_eq!(skills.tree_feller_max_logs(), 12);
        skills.set_level(SkillKind::Woodcutting, 1000);
        assert_eq!(skills.tree_feller_max_logs(), 96);
    }
}
