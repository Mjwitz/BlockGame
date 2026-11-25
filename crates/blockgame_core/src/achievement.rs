use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::block::BlockId;
use crate::item::{ItemId, ItemSlot};

/// Current schema version for `achievements.yaml`.
pub const ACHIEVEMENT_PROFILE_SCHEMA_VERSION: u32 = 1;

/// Default local file name for the client-owned global achievement profile.
pub const DEFAULT_ACHIEVEMENTS_PATH: &str = "achievements.yaml";

/// Gameplay categories displayed on the Achievements screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AchievementCategory {
    SurvivalCrafting,
    MiningExploration,
    FarmingCooking,
    Fishing,
    CombatBosses,
    VillagesTrading,
    PowerAutomation,
    MmoSkills,
    Hidden,
}

impl AchievementCategory {
    pub const ALL: [AchievementCategory; 9] = [
        AchievementCategory::SurvivalCrafting,
        AchievementCategory::MiningExploration,
        AchievementCategory::FarmingCooking,
        AchievementCategory::Fishing,
        AchievementCategory::CombatBosses,
        AchievementCategory::VillagesTrading,
        AchievementCategory::PowerAutomation,
        AchievementCategory::MmoSkills,
        AchievementCategory::Hidden,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SurvivalCrafting => "Survival",
            Self::MiningExploration => "Exploration",
            Self::FarmingCooking => "Farming",
            Self::Fishing => "Fishing",
            Self::CombatBosses => "Combat",
            Self::VillagesTrading => "Villages",
            Self::PowerAutomation => "Power",
            Self::MmoSkills => "Skills",
            Self::Hidden => "Hidden",
        }
    }

    pub const fn full_label(self) -> &'static str {
        match self {
            Self::SurvivalCrafting => "Survival & Crafting",
            Self::MiningExploration => "Mining & Exploration",
            Self::FarmingCooking => "Farming & Cooking",
            Self::Fishing => "Fishing",
            Self::CombatBosses => "Combat & Bosses",
            Self::VillagesTrading => "Villages & Trading",
            Self::PowerAutomation => "Power & Automation",
            Self::MmoSkills => "MMO Skills",
            Self::Hidden => "Hidden Achievements",
        }
    }
}

/// How an Achievement's progress is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AchievementRequirement {
    /// Unlocked immediately when the qualifying event occurs once.
    OneShot,
    /// Unlocked when the named Career Stat counter reaches `target`.
    Counter { stat: &'static str, target: u64 },
    /// Unlocked when the named Career Stat unique set reaches `target` entries.
    UniqueSet { stat: &'static str, target: u64 },
}

/// Static definition of a single Achievement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AchievementDef {
    /// Stable `snake_case` identifier mapped 1:1 to Steam achievements.
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub category: AchievementCategory,
    pub hidden: bool,
    /// Existing `ItemId` rendered from the procedural `TextureAtlas`.
    pub icon: ItemId,
    pub requirement: AchievementRequirement,
}

/// Canonical 42-achievement launch roster across all 8 gameplay pillars + 5 Hidden Achievements.
pub const ACHIEVEMENTS: &[AchievementDef] = &[
    // --- Survival & Crafting (5) ---
    AchievementDef {
        id: "first_timber",
        title: "Timber!",
        description: "Chop your first log.",
        category: AchievementCategory::SurvivalCrafting,
        hidden: false,
        icon: ItemId::WOOD_LOG,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "craft_table",
        title: "Getting to Work",
        description: "Craft a Crafting Table.",
        category: AchievementCategory::SurvivalCrafting,
        hidden: false,
        icon: ItemId::CRAFTING_TABLE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "first_smelt",
        title: "Into the Fire",
        description: "Smelt an ingot in a Furnace.",
        category: AchievementCategory::SurvivalCrafting,
        hidden: false,
        icon: ItemId::IRON_INGOT,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "sleep_bed",
        title: "Sweet Dreams",
        description: "Sleep through the night in a bed.",
        category: AchievementCategory::SurvivalCrafting,
        hidden: false,
        icon: ItemId::BED,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "full_diamond_armor",
        title: "Shining Knight",
        description: "Equip a full set of Diamond Armor.",
        category: AchievementCategory::SurvivalCrafting,
        hidden: false,
        icon: ItemId::DIAMOND_CHESTPLATE,
        requirement: AchievementRequirement::OneShot,
    },
    // --- Mining & Exploration (5) ---
    AchievementDef {
        id: "mine_diamond",
        title: "Diamonds!",
        description: "Mine your first Diamond Ore.",
        category: AchievementCategory::MiningExploration,
        hidden: false,
        icon: ItemId::DIAMOND,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "mine_blocks_1000",
        title: "Earthmover",
        description: "Mine 1,000 blocks.",
        category: AchievementCategory::MiningExploration,
        hidden: false,
        icon: ItemId::IRON_PICKAXE,
        requirement: AchievementRequirement::Counter {
            stat: "blocks_mined",
            target: 1_000,
        },
    },
    AchievementDef {
        id: "mine_blocks_10000",
        title: "Mountain Breaker",
        description: "Mine 10,000 blocks.",
        category: AchievementCategory::MiningExploration,
        hidden: false,
        icon: ItemId::DIAMOND_PICKAXE,
        requirement: AchievementRequirement::Counter {
            stat: "blocks_mined",
            target: 10_000,
        },
    },
    AchievementDef {
        id: "visit_biomes_10",
        title: "Wanderer",
        description: "Visit 10 different biomes.",
        category: AchievementCategory::MiningExploration,
        hidden: false,
        icon: ItemId::GRASS,
        requirement: AchievementRequirement::UniqueSet {
            stat: "biomes_visited",
            target: 10,
        },
    },
    AchievementDef {
        id: "visit_biomes_all",
        title: "World Explorer",
        description: "Visit all 20 biomes.",
        category: AchievementCategory::MiningExploration,
        hidden: false,
        icon: ItemId::CORAL_BLOCK,
        requirement: AchievementRequirement::UniqueSet {
            stat: "biomes_visited",
            target: 20,
        },
    },
    // --- Farming, Ranching & Cooking (5) ---
    AchievementDef {
        id: "harvest_crops_50",
        title: "Green Thumb",
        description: "Harvest 50 mature crops.",
        category: AchievementCategory::FarmingCooking,
        hidden: false,
        icon: ItemId::WHEAT,
        requirement: AchievementRequirement::Counter {
            stat: "crops_harvested",
            target: 50,
        },
    },
    AchievementDef {
        id: "use_fertilizer",
        title: "Rich Soil",
        description: "Enrich farmland with Fertilizer.",
        category: AchievementCategory::FarmingCooking,
        hidden: false,
        icon: ItemId::BONE_FERTILIZER,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "breed_animals_5",
        title: "Matchmaker",
        description: "Breed 5 animals using Breeding Feed.",
        category: AchievementCategory::FarmingCooking,
        hidden: false,
        icon: ItemId::CARROT,
        requirement: AchievementRequirement::Counter {
            stat: "animals_bred",
            target: 5,
        },
    },
    AchievementDef {
        id: "cook_first_meal",
        title: "Home Cook",
        description: "Prepare a meal in a Cooking Pot.",
        category: AchievementCategory::FarmingCooking,
        hidden: false,
        icon: ItemId::COOKING_POT,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "cook_all_dishes",
        title: "Master Chef",
        description: "Prepare all 8 Cooking Pot dishes.",
        category: AchievementCategory::FarmingCooking,
        hidden: false,
        icon: ItemId::HEARTY_MEAT_STEW,
        requirement: AchievementRequirement::UniqueSet {
            stat: "dishes_cooked",
            target: 8,
        },
    },
    // --- Fishing (4) ---
    AchievementDef {
        id: "catch_first_fish",
        title: "Hook, Line & Sinker",
        description: "Catch your first fish.",
        category: AchievementCategory::Fishing,
        hidden: false,
        icon: ItemId::RAW_TROUT,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "catch_fish_50",
        title: "Seasoned Angler",
        description: "Catch 50 fish.",
        category: AchievementCategory::Fishing,
        hidden: false,
        icon: ItemId::RAW_SALMON,
        requirement: AchievementRequirement::Counter {
            stat: "fish_caught",
            target: 50,
        },
    },
    AchievementDef {
        id: "catch_all_fish",
        title: "Master Angler",
        description: "Catch all 10 fish species.",
        category: AchievementCategory::Fishing,
        hidden: false,
        icon: ItemId::GOLDEN_KOI,
        requirement: AchievementRequirement::UniqueSet {
            stat: "fish_species_caught",
            target: 10,
        },
    },
    AchievementDef {
        id: "diamond_rod",
        title: "Top of the Line",
        description: "Craft or obtain a Diamond Fishing Rod.",
        category: AchievementCategory::Fishing,
        hidden: false,
        icon: ItemId::DIAMOND_FISHING_ROD,
        requirement: AchievementRequirement::OneShot,
    },
    // --- Combat & Biome Bosses (7) ---
    AchievementDef {
        id: "kill_first_monster",
        title: "Monster Hunter",
        description: "Defeat your first hostile monster.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::IRON_SWORD,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "kill_monsters_100",
        title: "Night Watch",
        description: "Defeat 100 hostile monsters.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::DIAMOND_SWORD,
        requirement: AchievementRequirement::Counter {
            stat: "monsters_killed",
            target: 100,
        },
    },
    AchievementDef {
        id: "boss_sun_pharaoh",
        title: "Sun Slayer",
        description: "Defeat the Sun Pharaoh in the Pyramid of the Blazing Sun.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::SUNBLADE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "boss_frost_titan",
        title: "Icebreaker",
        description: "Defeat the Frost Titan in the Citadel of Eternal Frost.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::FROSTBLADE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "boss_overgrown_golem",
        title: "Weed Whacker",
        description: "Defeat the Overgrown Golem in the Ziggurat of the Wilds.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::VERDANT_GREATBLADE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "boss_shadow_lich",
        title: "Light in the Dark",
        description: "Defeat the Shadow Lich in the Cathedral of Whispers.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::SHADOWFANG_BLADE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "boss_all_four",
        title: "Conqueror of the Realms",
        description: "Defeat all 4 Biome Bosses.",
        category: AchievementCategory::CombatBosses,
        hidden: false,
        icon: ItemId::PHARAOH_CROWN,
        requirement: AchievementRequirement::UniqueSet {
            stat: "bosses_defeated",
            target: 4,
        },
    },
    // --- Villages & Trading (4) ---
    AchievementDef {
        id: "first_trade",
        title: "Open for Business",
        description: "Complete your first trade with a Shopkeeper.",
        category: AchievementCategory::VillagesTrading,
        hidden: false,
        icon: ItemId::GOLD_COIN,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "spend_coins_500",
        title: "Big Spender",
        description: "Spend 500 Gold Coins with Shopkeepers.",
        category: AchievementCategory::VillagesTrading,
        hidden: false,
        icon: ItemId::GOLD_COIN,
        requirement: AchievementRequirement::Counter {
            stat: "gold_coins_spent",
            target: 500,
        },
    },
    AchievementDef {
        id: "shop_tier_5",
        title: "VIP Customer",
        description: "Reach Shop Tier 5 with any Shopkeeper.",
        category: AchievementCategory::VillagesTrading,
        hidden: false,
        icon: ItemId::ROYAL_FEAST,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "first_enhancement",
        title: "Upgraded",
        description: "Apply an Enhancement to a piece of gear.",
        category: AchievementCategory::VillagesTrading,
        hidden: false,
        icon: ItemId::VANGUARD_BLADE,
        requirement: AchievementRequirement::OneShot,
    },
    // --- Power & Automation (4) ---
    AchievementDef {
        id: "first_generator",
        title: "Power Up",
        description: "Place your first EU generator.",
        category: AchievementCategory::PowerAutomation,
        hidden: false,
        icon: ItemId::COAL_GENERATOR,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "electric_smelt",
        title: "High Voltage",
        description: "Smelt an item in an Electric Furnace.",
        category: AchievementCategory::PowerAutomation,
        hidden: false,
        icon: ItemId::ELECTRIC_FURNACE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "storage_network",
        title: "Digital Hoarder",
        description: "Open a powered Storage Terminal connected to a chest.",
        category: AchievementCategory::PowerAutomation,
        hidden: false,
        icon: ItemId::STORAGE_TERMINAL,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "diamond_backpack",
        title: "Pack Rat",
        description: "Craft a Diamond Backpack.",
        category: AchievementCategory::PowerAutomation,
        hidden: false,
        icon: ItemId::DIAMOND_BACKPACK,
        requirement: AchievementRequirement::OneShot,
    },
    // --- MMO Skills (3) ---
    AchievementDef {
        id: "skill_level_25",
        title: "Apprentice",
        description: "Reach level 25 in any skill.",
        category: AchievementCategory::MmoSkills,
        hidden: false,
        icon: ItemId::STONE_PICKAXE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "skill_level_100",
        title: "Journeyman",
        description: "Reach level 100 in any skill.",
        category: AchievementCategory::MmoSkills,
        hidden: false,
        icon: ItemId::GOLD_PICKAXE,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "skill_level_500",
        title: "Grandmaster",
        description: "Reach level 500 in any skill.",
        category: AchievementCategory::MmoSkills,
        hidden: false,
        icon: ItemId::MINERS_LANTERN,
        requirement: AchievementRequirement::OneShot,
    },
    // --- Hidden Achievements (5) ---
    AchievementDef {
        id: "hidden_cure_poison",
        title: "Doctor's Orders",
        description: "Cure Food Poisoning or Poison by drinking Herbal Tea.",
        category: AchievementCategory::Hidden,
        hidden: true,
        icon: ItemId::HERBAL_TEA,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "hidden_anger_guard",
        title: "Wanted",
        description: "Provoke a Village Guard by attacking a Village NPC.",
        category: AchievementCategory::Hidden,
        hidden: true,
        icon: ItemId::BOW,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "hidden_milk_cow",
        title: "Got Milk?",
        description: "Milk a cow with a Bucket.",
        category: AchievementCategory::Hidden,
        hidden: true,
        icon: ItemId::MILK_BUCKET,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "hidden_stem_harvest",
        title: "Bountiful Vine",
        description: "Harvest a Melon or Pumpkin grown from a Stem Crop.",
        category: AchievementCategory::Hidden,
        hidden: true,
        icon: ItemId::PUMPKIN,
        requirement: AchievementRequirement::OneShot,
    },
    AchievementDef {
        id: "hidden_boss_trophy",
        title: "Trophy Collector",
        description: "Collect all 4 Boss Trophy Cores.",
        category: AchievementCategory::Hidden,
        hidden: true,
        icon: ItemId::SUN_CORE,
        requirement: AchievementRequirement::UniqueSet {
            stat: "boss_trophy_cores",
            target: 4,
        },
    },
];

/// Look up an Achievement definition by its stable `snake_case` ID.
pub fn achievement_by_id(id: &str) -> Option<&'static AchievementDef> {
    ACHIEVEMENTS.iter().find(|def| def.id == id)
}

/// Returns all Achievement definitions in the given category.
pub fn achievements_in_category(category: AchievementCategory) -> Vec<&'static AchievementDef> {
    ACHIEVEMENTS
        .iter()
        .filter(|def| def.category == category)
        .collect()
}

/// Returns true if `block` is any tree log block.
#[inline]
pub const fn is_log_block(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::WOOD_LOG
            | BlockId::BIRCH_LOG
            | BlockId::SPRUCE_LOG
            | BlockId::ACACIA_LOG
            | BlockId::JUNGLE_LOG
            | BlockId::DARK_OAK_LOG
    )
}

/// Returns true if `block` is a mature harvestable crop block.
#[inline]
pub const fn is_mature_crop_block(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::WHEAT_3
            | BlockId::CARROTS_3
            | BlockId::POTATOES_3
            | BlockId::TOMATOES_3
            | BlockId::MELON
            | BlockId::PUMPKIN
            | BlockId::SUGAR_CANE
    )
}

/// Returns true if `block` is a fruit block produced by a Stem Crop.
#[inline]
pub const fn is_stem_fruit_block(block: BlockId) -> bool {
    matches!(block, BlockId::MELON | BlockId::PUMPKIN)
}

/// Returns true if `block` is an EU power generator block.
#[inline]
pub const fn is_eu_generator_block(block: BlockId) -> bool {
    matches!(
        block,
        BlockId::COAL_GENERATOR
            | BlockId::SOLAR_PANEL
            | BlockId::ADVANCED_SOLAR_PANEL
            | BlockId::WIND_TURBINE
    )
}

/// Returns true if `item` is a smelted metal ingot.
#[inline]
pub const fn is_smelted_ingot_item(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::COPPER_INGOT | ItemId::IRON_INGOT | ItemId::GOLD_INGOT
    )
}

/// Returns a canonical key if `item` is one of the 8 Cooking Pot dishes.
pub const fn cooking_pot_dish_key(item: ItemId) -> Option<&'static str> {
    match item {
        ItemId::TOMATO_SOUP => Some("tomato_soup"),
        ItemId::VEGETABLE_STEW => Some("vegetable_stew"),
        ItemId::HEARTY_MEAT_STEW => Some("hearty_meat_stew"),
        ItemId::OMELETTE => Some("omelette"),
        ItemId::FISH_STEW => Some("fish_stew"),
        ItemId::FRUIT_SALAD => Some("fruit_salad"),
        ItemId::SWEET_CAKE => Some("sweet_cake"),
        ItemId::HERBAL_TEA => Some("herbal_tea"),
        _ => None,
    }
}

/// Returns a canonical key if `item` is one of the 4 Boss Trophy Cores.
pub const fn boss_trophy_core_key(item: ItemId) -> Option<&'static str> {
    match item {
        ItemId::SUN_CORE => Some("sun_core"),
        ItemId::FROST_HEART => Some("frost_heart"),
        ItemId::ANCIENT_VINE_CORE => Some("ancient_vine_core"),
        ItemId::SOUL_PHYLACTERY => Some("soul_phylactery"),
        _ => None,
    }
}

/// Returns a canonical boss key if `kind` is one of the 4 Biome Bosses.
pub const fn boss_achievement_key(kind: crate::monster::MonsterKind) -> Option<&'static str> {
    match kind {
        crate::monster::MonsterKind::SunPharaoh => Some("sun_pharaoh"),
        crate::monster::MonsterKind::FrostTitan => Some("frost_titan"),
        crate::monster::MonsterKind::OvergrownGolem => Some("overgrown_golem"),
        crate::monster::MonsterKind::ShadowLich => Some("shadow_lich"),
        _ => None,
    }
}

/// Returns true if all 4 armor slots are equipped with Diamond Armor.
pub fn is_full_diamond_armor(armor: &[ItemSlot; 4]) -> bool {
    armor[0].item_id == ItemId::DIAMOND_HELMET
        && armor[0].count > 0
        && armor[1].item_id == ItemId::DIAMOND_CHESTPLATE
        && armor[1].count > 0
        && armor[2].item_id == ItemId::DIAMOND_LEGGINGS
        && armor[2].count > 0
        && armor[3].item_id == ItemId::DIAMOND_BOOTS
        && armor[3].count > 0
}

/// Typed gameplay events fed into [`AchievementEngine`], both locally in singleplayer
/// and via `ServerPacket::AchievementEvent` in multiplayer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AchievementEvent {
    BlockMined { block_id: u8 },
    StemFruitHarvested,
    ItemCrafted { item_id: u16 },
    ItemObtained { item_id: u16 },
    ItemSmelted { item_id: u16, electric: bool },
    SleptInBed,
    EquippedFullDiamondArmor,
    BiomeVisited { biome_key: String },
    FertilizerApplied,
    AnimalBred,
    MealCooked { item_id: u16 },
    FishCaught { species_key: String },
    MonsterKilled { is_boss: bool, boss_key: Option<String> },
    ShopTradeCompleted { coins_spent: u64, new_shop_tier: u8 },
    EnhancementApplied,
    GeneratorPlaced,
    PoweredStorageTerminalOpened,
    SkillLevelReached { level: u32 },
    CuredPoisonWithHerbalTea,
    VillageGuardProvoked,
    CowMilked,
}

impl AchievementEvent {
    /// Encodes this event into a compact binary stream for network transmission.
    pub fn encode<W: Write>(&self, w: &mut W) -> io::Result<()> {
        match self {
            Self::BlockMined { block_id } => {
                w.write_all(&[1, *block_id])?;
            }
            Self::StemFruitHarvested => w.write_all(&[2])?,
            Self::ItemCrafted { item_id } => {
                w.write_all(&[3])?;
                w.write_all(&item_id.to_le_bytes())?;
            }
            Self::ItemObtained { item_id } => {
                w.write_all(&[4])?;
                w.write_all(&item_id.to_le_bytes())?;
            }
            Self::ItemSmelted { item_id, electric } => {
                w.write_all(&[5])?;
                w.write_all(&item_id.to_le_bytes())?;
                w.write_all(&[u8::from(*electric)])?;
            }
            Self::SleptInBed => w.write_all(&[6])?,
            Self::EquippedFullDiamondArmor => w.write_all(&[7])?,
            Self::BiomeVisited { biome_key } => {
                w.write_all(&[8])?;
                write_short_str(w, biome_key)?;
            }
            Self::FertilizerApplied => w.write_all(&[9])?,
            Self::AnimalBred => w.write_all(&[10])?,
            Self::MealCooked { item_id } => {
                w.write_all(&[11])?;
                w.write_all(&item_id.to_le_bytes())?;
            }
            Self::FishCaught { species_key } => {
                w.write_all(&[12])?;
                write_short_str(w, species_key)?;
            }
            Self::MonsterKilled { is_boss, boss_key } => {
                w.write_all(&[13, u8::from(*is_boss)])?;
                match boss_key {
                    Some(key) => {
                        w.write_all(&[1])?;
                        write_short_str(w, key)?;
                    }
                    None => w.write_all(&[0])?,
                }
            }
            Self::ShopTradeCompleted {
                coins_spent,
                new_shop_tier,
            } => {
                w.write_all(&[14])?;
                w.write_all(&coins_spent.to_le_bytes())?;
                w.write_all(&[*new_shop_tier])?;
            }
            Self::EnhancementApplied => w.write_all(&[15])?,
            Self::GeneratorPlaced => w.write_all(&[16])?,
            Self::PoweredStorageTerminalOpened => w.write_all(&[17])?,
            Self::SkillLevelReached { level } => {
                w.write_all(&[18])?;
                w.write_all(&level.to_le_bytes())?;
            }
            Self::CuredPoisonWithHerbalTea => w.write_all(&[19])?,
            Self::VillageGuardProvoked => w.write_all(&[20])?,
            Self::CowMilked => w.write_all(&[21])?,
        }
        Ok(())
    }

    /// Decodes an event previously written by [`Self::encode`].
    pub fn decode<R: Read>(r: &mut R) -> io::Result<Self> {
        let tag = read_u8(r)?;
        match tag {
            1 => Ok(Self::BlockMined {
                block_id: read_u8(r)?,
            }),
            2 => Ok(Self::StemFruitHarvested),
            3 => Ok(Self::ItemCrafted {
                item_id: read_u16(r)?,
            }),
            4 => Ok(Self::ItemObtained {
                item_id: read_u16(r)?,
            }),
            5 => Ok(Self::ItemSmelted {
                item_id: read_u16(r)?,
                electric: read_u8(r)? != 0,
            }),
            6 => Ok(Self::SleptInBed),
            7 => Ok(Self::EquippedFullDiamondArmor),
            8 => Ok(Self::BiomeVisited {
                biome_key: read_short_str(r)?,
            }),
            9 => Ok(Self::FertilizerApplied),
            10 => Ok(Self::AnimalBred),
            11 => Ok(Self::MealCooked {
                item_id: read_u16(r)?,
            }),
            12 => Ok(Self::FishCaught {
                species_key: read_short_str(r)?,
            }),
            13 => {
                let is_boss = read_u8(r)? != 0;
                let has_key = read_u8(r)? != 0;
                let boss_key = if has_key {
                    Some(read_short_str(r)?)
                } else {
                    None
                };
                Ok(Self::MonsterKilled { is_boss, boss_key })
            }
            14 => Ok(Self::ShopTradeCompleted {
                coins_spent: read_u64(r)?,
                new_shop_tier: read_u8(r)?,
            }),
            15 => Ok(Self::EnhancementApplied),
            16 => Ok(Self::GeneratorPlaced),
            17 => Ok(Self::PoweredStorageTerminalOpened),
            18 => Ok(Self::SkillLevelReached {
                level: read_u32(r)?,
            }),
            19 => Ok(Self::CuredPoisonWithHerbalTea),
            20 => Ok(Self::VillageGuardProvoked),
            21 => Ok(Self::CowMilked),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown AchievementEvent tag: {other}"),
            )),
        }
    }
}

fn write_short_str<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    let len = u16::try_from(bytes.len()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "AchievementEvent string too long")
    })?;
    w.write_all(&len.to_le_bytes())?;
    w.write_all(bytes)
}

fn read_short_str<R: Read>(r: &mut R) -> io::Result<String> {
    let len = read_u16(r)? as usize;
    if len > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AchievementEvent string exceeds limit",
        ));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

/// Pluggable platform sink for Achievement unlocks and Career Stat updates.
///
/// Future Steamworks (`steamworks-rs`) integration implements this trait without
/// modifying gameplay, networking, or UI code (ADR 0007).
pub trait AchievementBackend: Send {
    fn on_achievement_unlocked(&mut self, achievement_id: &str);
    fn on_stat_updated(&mut self, stat_id: &str, value: u64);
}

/// Default local-only backend used until Steamworks is integrated.
#[derive(Debug, Default)]
pub struct LocalAchievementBackend;

impl AchievementBackend for LocalAchievementBackend {
    fn on_achievement_unlocked(&mut self, _achievement_id: &str) {}
    fn on_stat_updated(&mut self, _stat_id: &str, _value: u64) {}
}

/// Persisted global player profile stored in `achievements.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AchievementProfile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Map of `achievement_id` -> Unix timestamp (seconds) when unlocked.
    #[serde(default)]
    pub unlocked: BTreeMap<String, u64>,
    /// Map of `stat_id` -> lifetime cumulative Career Stat count.
    #[serde(default)]
    pub counters: BTreeMap<String, u64>,
    /// Map of `stat_id` -> set of unique keys collected/visited/defeated.
    #[serde(default)]
    pub sets: BTreeMap<String, BTreeSet<String>>,
}

const fn default_schema_version() -> u32 {
    ACHIEVEMENT_PROFILE_SCHEMA_VERSION
}

impl Default for AchievementProfile {
    fn default() -> Self {
        Self {
            schema_version: ACHIEVEMENT_PROFILE_SCHEMA_VERSION,
            unlocked: BTreeMap::new(),
            counters: BTreeMap::new(),
            sets: BTreeMap::new(),
        }
    }
}

impl AchievementProfile {
    /// Loads `achievements.yaml` from `path`, returning a clean default profile if
    /// the file does not exist or cannot be parsed.
    pub fn load(path: &Path) -> Self {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_yaml::from_str::<Self>(&contents) {
            Ok(mut profile) => {
                if profile.schema_version == 0 {
                    profile.schema_version = ACHIEVEMENT_PROFILE_SCHEMA_VERSION;
                }
                profile
            }
            Err(err) => {
                log::warn!(
                    "Failed to parse {}: {}. Starting with fresh AchievementProfile.",
                    path.display(),
                    err
                );
                Self::default()
            }
        }
    }

    /// Atomically persists this profile via a sibling `.tmp` file + rename so crashes
    /// never leave a truncated `achievements.yaml`.
    pub fn save_atomic(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let tmp_path = path.with_extension("yaml.tmp");
        std::fs::write(&tmp_path, yaml)?;
        if let Err(err) = std::fs::rename(&tmp_path, path) {
            // Fallback on platforms where rename over an existing file can fail.
            if path.exists() {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&tmp_path, path)?;
            } else {
                return Err(err);
            }
        }
        Ok(())
    }
}

/// Snapshot of a single Achievement's progress for UI rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AchievementProgressView {
    pub unlocked: bool,
    pub unlocked_at: Option<u64>,
    pub current: u64,
    pub target: u64,
}

/// Shared evaluation engine for Achievements and Career Stats.
pub struct AchievementEngine {
    profile: AchievementProfile,
    eligible: bool,
    dirty: bool,
    backend: Box<dyn AchievementBackend>,
}

impl Default for AchievementEngine {
    fn default() -> Self {
        Self::new(AchievementProfile::default(), Box::new(LocalAchievementBackend))
    }
}

impl AchievementEngine {
    pub fn new(profile: AchievementProfile, backend: Box<dyn AchievementBackend>) -> Self {
        Self {
            profile,
            eligible: true,
            dirty: false,
            backend,
        }
    }

    pub fn load_default(path: &Path) -> Self {
        Self::new(
            AchievementProfile::load(path),
            Box::new(LocalAchievementBackend),
        )
    }

    #[inline]
    pub fn profile(&self) -> &AchievementProfile {
        &self.profile
    }

    #[inline]
    pub fn is_eligible(&self) -> bool {
        self.eligible
    }

    #[inline]
    pub fn set_eligible(&mut self, eligible: bool) {
        self.eligible = eligible;
    }

    #[inline]
    pub fn is_unlocked(&self, id: &str) -> bool {
        self.profile.unlocked.contains_key(id)
    }

    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Saves `achievements.yaml` atomically if any unlock or Career Stat changed.
    pub fn save_if_dirty(&mut self, path: &Path) -> io::Result<bool> {
        if !self.dirty {
            return Ok(false);
        }
        self.profile.save_atomic(path)?;
        self.dirty = false;
        Ok(true)
    }

    /// Returns `(unlocked_count, total_count)`.
    pub fn completion_counts(&self) -> (usize, usize) {
        let unlocked = ACHIEVEMENTS
            .iter()
            .filter(|def| self.profile.unlocked.contains_key(def.id))
            .count();
        (unlocked, ACHIEVEMENTS.len())
    }

    /// Computes progress for UI display of `def`.
    pub fn progress_for(&self, def: &AchievementDef) -> AchievementProgressView {
        let unlocked_at = self.profile.unlocked.get(def.id).copied();
        let unlocked = unlocked_at.is_some();
        let (current, target) = match def.requirement {
            AchievementRequirement::OneShot => (u64::from(unlocked), 1),
            AchievementRequirement::Counter { stat, target } => {
                let raw = self.profile.counters.get(stat).copied().unwrap_or(0);
                (raw.min(target), target)
            }
            AchievementRequirement::UniqueSet { stat, target } => {
                let raw = self
                    .profile
                    .sets
                    .get(stat)
                    .map_or(0, |s| s.len() as u64);
                (raw.min(target), target)
            }
        };
        AchievementProgressView {
            unlocked,
            unlocked_at,
            current,
            target,
        }
    }

    /// Processes a gameplay event, updating Career Stats and returning any newly
    /// unlocked Achievements in deterministic order.
    ///
    /// If [`Self::is_eligible`] is `false`, the event is ignored completely.
    pub fn record_event(&mut self, event: &AchievementEvent) -> Vec<&'static AchievementDef> {
        if !self.eligible {
            return Vec::new();
        }
        let mut unlocked_now = Vec::new();
        match event {
            AchievementEvent::BlockMined { block_id } => {
                let block = BlockId(*block_id);
                if block != BlockId::AIR {
                    self.increment_counter("blocks_mined", 1);
                    if is_log_block(block) {
                        self.try_unlock("first_timber", &mut unlocked_now);
                    }
                    if block == BlockId::DIAMOND_ORE {
                        self.try_unlock("mine_diamond", &mut unlocked_now);
                    }
                    if is_mature_crop_block(block) {
                        self.increment_counter("crops_harvested", 1);
                        self.check_counter_achievements("crops_harvested", &mut unlocked_now);
                    }
                    self.check_counter_achievements("blocks_mined", &mut unlocked_now);
                }
            }
            AchievementEvent::StemFruitHarvested => {
                self.try_unlock("hidden_stem_harvest", &mut unlocked_now);
            }
            AchievementEvent::ItemCrafted { item_id } => {
                let item = ItemId(*item_id);
                if item == ItemId::CRAFTING_TABLE {
                    self.try_unlock("craft_table", &mut unlocked_now);
                }
                if item == ItemId::DIAMOND_FISHING_ROD {
                    self.try_unlock("diamond_rod", &mut unlocked_now);
                }
                if item == ItemId::DIAMOND_BACKPACK {
                    self.try_unlock("diamond_backpack", &mut unlocked_now);
                }
                self.check_obtained_item(item, &mut unlocked_now);
            }
            AchievementEvent::ItemObtained { item_id } => {
                self.check_obtained_item(ItemId(*item_id), &mut unlocked_now);
            }
            AchievementEvent::ItemSmelted { item_id, electric } => {
                let item = ItemId(*item_id);
                if is_smelted_ingot_item(item) {
                    self.try_unlock("first_smelt", &mut unlocked_now);
                }
                if *electric {
                    self.try_unlock("electric_smelt", &mut unlocked_now);
                }
            }
            AchievementEvent::SleptInBed => {
                self.try_unlock("sleep_bed", &mut unlocked_now);
            }
            AchievementEvent::EquippedFullDiamondArmor => {
                self.try_unlock("full_diamond_armor", &mut unlocked_now);
            }
            AchievementEvent::BiomeVisited { biome_key } => {
                if !biome_key.is_empty() && self.insert_set_item("biomes_visited", biome_key) {
                    self.check_set_achievements("biomes_visited", &mut unlocked_now);
                }
            }
            AchievementEvent::FertilizerApplied => {
                self.try_unlock("use_fertilizer", &mut unlocked_now);
            }
            AchievementEvent::AnimalBred => {
                self.increment_counter("animals_bred", 1);
                self.check_counter_achievements("animals_bred", &mut unlocked_now);
            }
            AchievementEvent::MealCooked { item_id } => {
                let item = ItemId(*item_id);
                self.try_unlock("cook_first_meal", &mut unlocked_now);
                if let Some(dish_key) = cooking_pot_dish_key(item) {
                    if self.insert_set_item("dishes_cooked", dish_key) {
                        self.check_set_achievements("dishes_cooked", &mut unlocked_now);
                    }
                }
            }
            AchievementEvent::FishCaught { species_key } => {
                self.try_unlock("catch_first_fish", &mut unlocked_now);
                self.increment_counter("fish_caught", 1);
                self.check_counter_achievements("fish_caught", &mut unlocked_now);
                if !species_key.is_empty()
                    && self.insert_set_item("fish_species_caught", species_key)
                {
                    self.check_set_achievements("fish_species_caught", &mut unlocked_now);
                }
            }
            AchievementEvent::MonsterKilled { is_boss, boss_key } => {
                self.try_unlock("kill_first_monster", &mut unlocked_now);
                self.increment_counter("monsters_killed", 1);
                self.check_counter_achievements("monsters_killed", &mut unlocked_now);
                if *is_boss {
                    if let Some(key) = boss_key.as_deref() {
                        match key {
                            "sun_pharaoh" => {
                                self.try_unlock("boss_sun_pharaoh", &mut unlocked_now);
                            }
                            "frost_titan" => {
                                self.try_unlock("boss_frost_titan", &mut unlocked_now);
                            }
                            "overgrown_golem" => {
                                self.try_unlock("boss_overgrown_golem", &mut unlocked_now);
                            }
                            "shadow_lich" => {
                                self.try_unlock("boss_shadow_lich", &mut unlocked_now);
                            }
                            _ => {}
                        }
                        if matches!(
                            key,
                            "sun_pharaoh" | "frost_titan" | "overgrown_golem" | "shadow_lich"
                        ) && self.insert_set_item("bosses_defeated", key)
                        {
                            self.check_set_achievements("bosses_defeated", &mut unlocked_now);
                        }
                    }
                }
            }
            AchievementEvent::ShopTradeCompleted {
                coins_spent,
                new_shop_tier,
            } => {
                self.try_unlock("first_trade", &mut unlocked_now);
                if *coins_spent > 0 {
                    self.increment_counter("gold_coins_spent", *coins_spent);
                    self.check_counter_achievements("gold_coins_spent", &mut unlocked_now);
                }
                if *new_shop_tier >= 5 {
                    self.try_unlock("shop_tier_5", &mut unlocked_now);
                }
            }
            AchievementEvent::EnhancementApplied => {
                self.try_unlock("first_enhancement", &mut unlocked_now);
            }
            AchievementEvent::GeneratorPlaced => {
                self.try_unlock("first_generator", &mut unlocked_now);
            }
            AchievementEvent::PoweredStorageTerminalOpened => {
                self.try_unlock("storage_network", &mut unlocked_now);
            }
            AchievementEvent::SkillLevelReached { level } => {
                if *level >= 25 {
                    self.try_unlock("skill_level_25", &mut unlocked_now);
                }
                if *level >= 100 {
                    self.try_unlock("skill_level_100", &mut unlocked_now);
                }
                if *level >= 500 {
                    self.try_unlock("skill_level_500", &mut unlocked_now);
                }
            }
            AchievementEvent::CuredPoisonWithHerbalTea => {
                self.try_unlock("hidden_cure_poison", &mut unlocked_now);
            }
            AchievementEvent::VillageGuardProvoked => {
                self.try_unlock("hidden_anger_guard", &mut unlocked_now);
            }
            AchievementEvent::CowMilked => {
                self.try_unlock("hidden_milk_cow", &mut unlocked_now);
            }
        }
        unlocked_now
    }

    fn check_obtained_item(&mut self, item: ItemId, unlocked_now: &mut Vec<&'static AchievementDef>) {
        if item == ItemId::DIAMOND_FISHING_ROD {
            self.try_unlock("diamond_rod", unlocked_now);
        }
        if let Some(core_key) = boss_trophy_core_key(item) {
            if self.insert_set_item("boss_trophy_cores", core_key) {
                self.check_set_achievements("boss_trophy_cores", unlocked_now);
            }
        }
    }

    fn increment_counter(&mut self, stat: &'static str, delta: u64) {
        if delta == 0 {
            return;
        }
        let entry = self.profile.counters.entry(stat.to_string()).or_insert(0);
        *entry = entry.saturating_add(delta);
        let new_val = *entry;
        self.dirty = true;
        self.backend.on_stat_updated(stat, new_val);
    }

    fn insert_set_item(&mut self, stat: &'static str, key: &str) -> bool {
        let set = self.profile.sets.entry(stat.to_string()).or_default();
        if set.insert(key.to_string()) {
            let new_len = set.len() as u64;
            self.dirty = true;
            self.backend.on_stat_updated(stat, new_len);
            true
        } else {
            false
        }
    }

    fn check_counter_achievements(
        &mut self,
        stat: &'static str,
        unlocked_now: &mut Vec<&'static AchievementDef>,
    ) {
        let value = self.profile.counters.get(stat).copied().unwrap_or(0);
        for def in ACHIEVEMENTS {
            if let AchievementRequirement::Counter {
                stat: req_stat,
                target,
            } = def.requirement
            {
                if req_stat == stat && value >= target {
                    self.try_unlock(def.id, unlocked_now);
                }
            }
        }
    }

    fn check_set_achievements(
        &mut self,
        stat: &'static str,
        unlocked_now: &mut Vec<&'static AchievementDef>,
    ) {
        let count = self
            .profile
            .sets
            .get(stat)
            .map_or(0, |s| s.len() as u64);
        for def in ACHIEVEMENTS {
            if let AchievementRequirement::UniqueSet {
                stat: req_stat,
                target,
            } = def.requirement
            {
                if req_stat == stat && count >= target {
                    self.try_unlock(def.id, unlocked_now);
                }
            }
        }
    }

    fn try_unlock(&mut self, id: &str, unlocked_now: &mut Vec<&'static AchievementDef>) {
        if self.profile.unlocked.contains_key(id) {
            return;
        }
        let Some(def) = achievement_by_id(id) else {
            return;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.profile.unlocked.insert(def.id.to_string(), now);
        self.dirty = true;
        self.backend.on_achievement_unlocked(def.id);
        unlocked_now.push(def);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct MockBackend {
        unlocks: Arc<Mutex<Vec<String>>>,
        stats: Arc<Mutex<Vec<(String, u64)>>>,
    }

    impl AchievementBackend for MockBackend {
        fn on_achievement_unlocked(&mut self, achievement_id: &str) {
            self.unlocks
                .lock()
                .unwrap()
                .push(achievement_id.to_string());
        }

        fn on_stat_updated(&mut self, stat_id: &str, value: u64) {
            self.stats
                .lock()
                .unwrap()
                .push((stat_id.to_string(), value));
        }
    }

    #[test]
    fn roster_has_42_unique_snake_case_achievements() {
        assert_eq!(ACHIEVEMENTS.len(), 42);
        let mut seen = HashSet::new();
        for def in ACHIEVEMENTS {
            assert!(
                seen.insert(def.id),
                "Duplicate achievement ID found: {}",
                def.id
            );
            assert!(
                def.id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "Achievement ID must be snake_case: {}",
                def.id
            );
            assert!(!def.title.is_empty());
            assert!(!def.description.is_empty());
            assert!(!def.icon.is_empty());
        }
    }

    #[test]
    fn ineligible_engine_ignores_all_events_and_stats() {
        let backend = MockBackend::default();
        let mut engine = AchievementEngine::new(AchievementProfile::default(), Box::new(backend.clone()));
        engine.set_eligible(false);

        let unlocked = engine.record_event(&AchievementEvent::BlockMined {
            block_id: BlockId::WOOD_LOG.0,
        });
        assert!(unlocked.is_empty());
        assert_eq!(engine.completion_counts().0, 0);
        assert!(engine.profile().counters.is_empty());
        assert!(backend.unlocks.lock().unwrap().is_empty());
        assert!(backend.stats.lock().unwrap().is_empty());
    }

    #[test]
    fn unlock_is_idempotent_and_notifies_backend_once() {
        let backend = MockBackend::default();
        let mut engine = AchievementEngine::new(AchievementProfile::default(), Box::new(backend.clone()));

        let first = engine.record_event(&AchievementEvent::SleptInBed);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "sleep_bed");

        let second = engine.record_event(&AchievementEvent::SleptInBed);
        assert!(second.is_empty());
        assert_eq!(backend.unlocks.lock().unwrap().as_slice(), &["sleep_bed"]);
    }

    #[test]
    fn all_42_achievements_can_be_unlocked_via_events() {
        let backend = MockBackend::default();
        let mut engine = AchievementEngine::new(AchievementProfile::default(), Box::new(backend.clone()));

        // Survival & Crafting
        engine.record_event(&AchievementEvent::BlockMined {
            block_id: BlockId::WOOD_LOG.0,
        });
        engine.record_event(&AchievementEvent::ItemCrafted {
            item_id: ItemId::CRAFTING_TABLE.0,
        });
        engine.record_event(&AchievementEvent::ItemSmelted {
            item_id: ItemId::IRON_INGOT.0,
            electric: false,
        });
        engine.record_event(&AchievementEvent::SleptInBed);
        engine.record_event(&AchievementEvent::EquippedFullDiamondArmor);

        // Mining & Exploration
        engine.record_event(&AchievementEvent::BlockMined {
            block_id: BlockId::DIAMOND_ORE.0,
        });
        for _ in 0..9_998 {
            engine.record_event(&AchievementEvent::BlockMined {
                block_id: BlockId::STONE.0,
            });
        }
        for i in 0..20 {
            engine.record_event(&AchievementEvent::BiomeVisited {
                biome_key: format!("biome_{i}"),
            });
        }

        // Farming, Ranching & Cooking
        for _ in 0..50 {
            engine.record_event(&AchievementEvent::BlockMined {
                block_id: BlockId::WHEAT_3.0,
            });
        }
        engine.record_event(&AchievementEvent::FertilizerApplied);
        for _ in 0..5 {
            engine.record_event(&AchievementEvent::AnimalBred);
        }
        for dish in [
            ItemId::TOMATO_SOUP,
            ItemId::VEGETABLE_STEW,
            ItemId::HEARTY_MEAT_STEW,
            ItemId::OMELETTE,
            ItemId::FISH_STEW,
            ItemId::FRUIT_SALAD,
            ItemId::SWEET_CAKE,
            ItemId::HERBAL_TEA,
        ] {
            engine.record_event(&AchievementEvent::MealCooked { item_id: dish.0 });
        }

        // Fishing
        for i in 0..50 {
            engine.record_event(&AchievementEvent::FishCaught {
                species_key: format!("species_{}", i % 10),
            });
        }
        engine.record_event(&AchievementEvent::ItemObtained {
            item_id: ItemId::DIAMOND_FISHING_ROD.0,
        });

        // Combat & Bosses
        for _ in 0..96 {
            engine.record_event(&AchievementEvent::MonsterKilled {
                is_boss: false,
                boss_key: None,
            });
        }
        for boss in ["sun_pharaoh", "frost_titan", "overgrown_golem", "shadow_lich"] {
            engine.record_event(&AchievementEvent::MonsterKilled {
                is_boss: true,
                boss_key: Some(boss.to_string()),
            });
        }

        // Villages & Trading
        engine.record_event(&AchievementEvent::ShopTradeCompleted {
            coins_spent: 500,
            new_shop_tier: 5,
        });
        engine.record_event(&AchievementEvent::EnhancementApplied);

        // Power & Automation
        engine.record_event(&AchievementEvent::GeneratorPlaced);
        engine.record_event(&AchievementEvent::ItemSmelted {
            item_id: ItemId::COPPER_INGOT.0,
            electric: true,
        });
        engine.record_event(&AchievementEvent::PoweredStorageTerminalOpened);
        engine.record_event(&AchievementEvent::ItemCrafted {
            item_id: ItemId::DIAMOND_BACKPACK.0,
        });

        // MMO Skills
        engine.record_event(&AchievementEvent::SkillLevelReached { level: 500 });

        // Hidden
        engine.record_event(&AchievementEvent::CuredPoisonWithHerbalTea);
        engine.record_event(&AchievementEvent::VillageGuardProvoked);
        engine.record_event(&AchievementEvent::CowMilked);
        engine.record_event(&AchievementEvent::StemFruitHarvested);
        for core in [
            ItemId::SUN_CORE,
            ItemId::FROST_HEART,
            ItemId::ANCIENT_VINE_CORE,
            ItemId::SOUL_PHYLACTERY,
        ] {
            engine.record_event(&AchievementEvent::ItemObtained { item_id: core.0 });
        }

        let (unlocked, total) = engine.completion_counts();
        assert_eq!(unlocked, 42);
        assert_eq!(total, 42);
        assert_eq!(backend.unlocks.lock().unwrap().len(), 42);
    }

    #[test]
    fn profile_atomic_yaml_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "blockgame_ach_test_{}",
            std::process::id()
        ));
        let path = dir.join("achievements.yaml");
        let _ = std::fs::remove_dir_all(&dir);

        let mut engine = AchievementEngine::default();
        engine.record_event(&AchievementEvent::BlockMined {
            block_id: BlockId::WOOD_LOG.0,
        });
        engine.record_event(&AchievementEvent::BiomeVisited {
            biome_key: "plains".to_string(),
        });
        assert!(engine.save_if_dirty(&path).unwrap());
        assert!(!engine.is_dirty());

        let reloaded = AchievementEngine::load_default(&path);
        assert!(reloaded.is_unlocked("first_timber"));
        assert_eq!(
            reloaded.profile().counters.get("blocks_mined").copied(),
            Some(1)
        );
        assert!(reloaded
            .profile()
            .sets
            .get("biomes_visited")
            .unwrap()
            .contains("plains"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn achievement_event_binary_roundtrip() {
        let events = vec![
            AchievementEvent::BlockMined {
                block_id: BlockId::DIAMOND_ORE.0,
            },
            AchievementEvent::StemFruitHarvested,
            AchievementEvent::ItemCrafted {
                item_id: ItemId::CRAFTING_TABLE.0,
            },
            AchievementEvent::ItemObtained {
                item_id: ItemId::SUN_CORE.0,
            },
            AchievementEvent::ItemSmelted {
                item_id: ItemId::IRON_INGOT.0,
                electric: true,
            },
            AchievementEvent::SleptInBed,
            AchievementEvent::EquippedFullDiamondArmor,
            AchievementEvent::BiomeVisited {
                biome_key: "coral_reef".to_string(),
            },
            AchievementEvent::FertilizerApplied,
            AchievementEvent::AnimalBred,
            AchievementEvent::MealCooked {
                item_id: ItemId::TOMATO_SOUP.0,
            },
            AchievementEvent::FishCaught {
                species_key: "golden_koi".to_string(),
            },
            AchievementEvent::MonsterKilled {
                is_boss: true,
                boss_key: Some("sun_pharaoh".to_string()),
            },
            AchievementEvent::ShopTradeCompleted {
                coins_spent: 250,
                new_shop_tier: 4,
            },
            AchievementEvent::EnhancementApplied,
            AchievementEvent::GeneratorPlaced,
            AchievementEvent::PoweredStorageTerminalOpened,
            AchievementEvent::SkillLevelReached { level: 100 },
            AchievementEvent::CuredPoisonWithHerbalTea,
            AchievementEvent::VillageGuardProvoked,
            AchievementEvent::CowMilked,
        ];

        for ev in events {
            let mut buf = Vec::new();
            ev.encode(&mut buf).unwrap();
            let decoded = AchievementEvent::decode(&mut buf.as_slice()).unwrap();
            assert_eq!(ev, decoded);
        }
    }
}
