use std::collections::HashMap;

use crate::block::BlockId;
use crate::tool::{ToolTier, ToolType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u16);

impl ItemId {
    pub const EMPTY: Self = Self(0);
    pub const STONE: Self = Self(1);
    pub const DIRT: Self = Self(2);
    pub const GRASS: Self = Self(3);
    pub const COBBLESTONE: Self = Self(4);
    pub const WOOD_LOG: Self = Self(5);
    pub const LEAVES: Self = Self(6);
    pub const SAND: Self = Self(7);
    pub const PLANKS: Self = Self(8);
    pub const GLASS: Self = Self(9);
    pub const TORCH: Self = Self(10);
    pub const STICK: Self = Self(11);
    pub const WOODEN_PICKAXE: Self = Self(12);
    pub const STONE_PICKAXE: Self = Self(13);
    pub const CRAFTING_TABLE: Self = Self(14);
    pub const COAL: Self = Self(15);
    pub const IRON_INGOT: Self = Self(16);
    pub const IRON_PICKAXE: Self = Self(17);
    pub const COAL_ORE: Self = Self(18);
    pub const COPPER_ORE: Self = Self(19);
    pub const IRON_ORE: Self = Self(20);
    pub const GOLD_ORE: Self = Self(21);
    pub const DIAMOND_ORE: Self = Self(22);
    pub const RAW_COPPER: Self = Self(23);
    pub const COPPER_INGOT: Self = Self(24);
    pub const RAW_IRON: Self = Self(25);
    pub const RAW_GOLD: Self = Self(26);
    pub const GOLD_INGOT: Self = Self(27);
    pub const DIAMOND: Self = Self(28);
    pub const DIAMOND_PICKAXE: Self = Self(29);
    pub const WOODEN_AXE: Self = Self(30);
    pub const STONE_AXE: Self = Self(31);
    pub const IRON_AXE: Self = Self(32);
    pub const DIAMOND_AXE: Self = Self(33);
    pub const WOODEN_SHOVEL: Self = Self(34);
    pub const STONE_SHOVEL: Self = Self(35);
    pub const IRON_SHOVEL: Self = Self(36);
    pub const DIAMOND_SHOVEL: Self = Self(37);
    pub const APPLE: Self = Self(38);
    pub const WOODEN_HOE: Self = Self(39);
    pub const STONE_HOE: Self = Self(40);
    pub const IRON_HOE: Self = Self(41);
    pub const DIAMOND_HOE: Self = Self(42);
    pub const WHEAT_SEEDS: Self = Self(43);
    pub const WHEAT: Self = Self(44);
    pub const BREAD: Self = Self(45);
    pub const RAW_BEEF: Self = Self(46);
    pub const COOKED_STEAK: Self = Self(47);
    pub const RAW_PORKCHOP: Self = Self(48);
    pub const COOKED_PORKCHOP: Self = Self(49);
    pub const RAW_CHICKEN: Self = Self(50);
    pub const COOKED_CHICKEN: Self = Self(51);
    pub const LEATHER: Self = Self(52);
    pub const FEATHER: Self = Self(53);
    pub const CHEST: Self = Self(54);
    pub const FURNACE: Self = Self(55);
    pub const BIRCH_LOG: Self = Self(56);
    pub const BIRCH_PLANKS: Self = Self(57);
    pub const BIRCH_LEAVES: Self = Self(58);
    pub const SPRUCE_LOG: Self = Self(59);
    pub const SPRUCE_PLANKS: Self = Self(60);
    pub const SPRUCE_LEAVES: Self = Self(61);
    pub const TALL_GRASS: Self = Self(62);
    pub const SNOW_BLOCK: Self = Self(63);
    pub const ICE: Self = Self(64);
    pub const CLAY: Self = Self(65);
    pub const LILY_PAD: Self = Self(66);
    pub const ACACIA_LOG: Self = Self(67);
    pub const ACACIA_PLANKS: Self = Self(68);
    pub const ACACIA_LEAVES: Self = Self(69);
    pub const CACTUS: Self = Self(70);
    pub const SANDSTONE: Self = Self(71);
    pub const GRAVEL: Self = Self(72);
    pub const JUNGLE_LOG: Self = Self(73);
    pub const JUNGLE_PLANKS: Self = Self(74);
    pub const JUNGLE_LEAVES: Self = Self(75);
    pub const TERRACOTTA: Self = Self(76);
    pub const RED_SAND: Self = Self(77);
    pub const PODZOL: Self = Self(78);
    pub const MOSSY_COBBLESTONE: Self = Self(79);
    pub const MYCELIUM: Self = Self(80);
    pub const RED_MUSHROOM_BLOCK: Self = Self(81);
    pub const BROWN_MUSHROOM_BLOCK: Self = Self(82);
    pub const MUSHROOM_STEM: Self = Self(83);
    pub const DARK_OAK_LOG: Self = Self(84);
    pub const DARK_OAK_PLANKS: Self = Self(85);
    pub const DARK_OAK_LEAVES: Self = Self(86);
    pub const CORAL_BLOCK: Self = Self(87);
    pub const TUBE_CORAL_BLOCK: Self = Self(88);
    pub const PRISMARINE: Self = Self(89);
    pub const SEA_LANTERN: Self = Self(90);
    pub const SEAGRASS: Self = Self(91);
    pub const BUCKET: Self = Self(92);
    pub const WATER_BUCKET: Self = Self(93);
    pub const WOODEN_SWORD: Self = Self(94);
    pub const STONE_SWORD: Self = Self(95);
    pub const COPPER_SWORD: Self = Self(96);
    pub const IRON_SWORD: Self = Self(97);
    pub const DIAMOND_SWORD: Self = Self(98);
    pub const GOLD_SWORD: Self = Self(99);
    pub const COPPER_PICKAXE: Self = Self(100);
    pub const COPPER_AXE: Self = Self(101);
    pub const COPPER_SHOVEL: Self = Self(102);
    pub const COPPER_HOE: Self = Self(103);
    pub const GOLD_PICKAXE: Self = Self(104);
    pub const GOLD_AXE: Self = Self(105);
    pub const GOLD_SHOVEL: Self = Self(106);
    pub const GOLD_HOE: Self = Self(107);
    pub const BRICK: Self = Self(108);
    pub const BRICKS: Self = Self(109);
    pub const WHITE_WOOL: Self = Self(110);
    pub const RED_WOOL: Self = Self(111);
    pub const ORANGE_WOOL: Self = Self(112);
    pub const YELLOW_WOOL: Self = Self(113);
    pub const LIME_WOOL: Self = Self(114);
    pub const GREEN_WOOL: Self = Self(115);
    pub const CYAN_WOOL: Self = Self(116);
    pub const BLUE_WOOL: Self = Self(117);
    pub const PURPLE_WOOL: Self = Self(118);
    pub const PINK_WOOL: Self = Self(119);
    pub const BROWN_WOOL: Self = Self(120);
    pub const GRAY_WOOL: Self = Self(121);
    pub const BLACK_WOOL: Self = Self(122);
    pub const WHITE_DYE: Self = Self(123);
    pub const RED_DYE: Self = Self(124);
    pub const ORANGE_DYE: Self = Self(125);
    pub const YELLOW_DYE: Self = Self(126);
    pub const LIME_DYE: Self = Self(127);
    pub const GREEN_DYE: Self = Self(128);
    pub const CYAN_DYE: Self = Self(129);
    pub const BLUE_DYE: Self = Self(130);
    pub const PURPLE_DYE: Self = Self(131);
    pub const PINK_DYE: Self = Self(132);
    pub const BROWN_DYE: Self = Self(133);
    pub const GRAY_DYE: Self = Self(134);
    pub const BLACK_DYE: Self = Self(135);
    pub const OAK_STAIRS: Self = Self(136);
    pub const COBBLESTONE_STAIRS: Self = Self(137);
    pub const BRICK_STAIRS: Self = Self(138);
    pub const OAK_FENCE: Self = Self(139);
    pub const OAK_DOOR: Self = Self(140);
    pub const BED: Self = Self(141);
    pub const POPPY: Self = Self(142);
    pub const DANDELION: Self = Self(143);
    pub const BLUE_ORCHID: Self = Self(144);
    pub const ALLIUM: Self = Self(145);
    pub const ORANGE_TULIP: Self = Self(146);
    pub const PINK_TULIP: Self = Self(147);
    pub const OXEYE_DAISY: Self = Self(148);
    pub const PUMPKIN: Self = Self(149);
    pub const JACK_O_LANTERN: Self = Self(150);
    pub const MELON: Self = Self(151);
    pub const MELON_SLICE: Self = Self(152);
    pub const BOW: Self = Self(153);
    pub const ARROW: Self = Self(154);
    pub const STRING: Self = Self(155);
    pub const BONE: Self = Self(156);
    pub const ROTTEN_FLESH: Self = Self(157);
    pub const LEATHER_HELMET: Self = Self(158);
    pub const LEATHER_CHESTPLATE: Self = Self(159);
    pub const LEATHER_LEGGINGS: Self = Self(160);
    pub const LEATHER_BOOTS: Self = Self(161);
    pub const IRON_HELMET: Self = Self(162);
    pub const IRON_CHESTPLATE: Self = Self(163);
    pub const IRON_LEGGINGS: Self = Self(164);
    pub const IRON_BOOTS: Self = Self(165);
    pub const GOLD_HELMET: Self = Self(166);
    pub const GOLD_CHESTPLATE: Self = Self(167);
    pub const GOLD_LEGGINGS: Self = Self(168);
    pub const GOLD_BOOTS: Self = Self(169);
    pub const DIAMOND_HELMET: Self = Self(170);
    pub const DIAMOND_CHESTPLATE: Self = Self(171);
    pub const DIAMOND_LEGGINGS: Self = Self(172);
    pub const DIAMOND_BOOTS: Self = Self(173);

    // Fishing Rods, Lines, Lures, Bait, Fish & Crafted Dishes
    pub const WOODEN_FISHING_ROD: Self = Self(174);
    pub const COPPER_FISHING_ROD: Self = Self(175);
    pub const IRON_FISHING_ROD: Self = Self(176);
    pub const DIAMOND_FISHING_ROD: Self = Self(177);
    pub const BRAIDED_LINE: Self = Self(178);
    pub const FLUOROCARBON_LINE: Self = Self(179);
    pub const TITANIUM_LINE: Self = Self(180);
    pub const CORK_BOBBER: Self = Self(181);
    pub const SPINNER_LURE: Self = Self(182);
    pub const BARBED_HOOK: Self = Self(183);
    pub const TREASURE_LURE: Self = Self(184);
    pub const FISHING_BAIT: Self = Self(185);
    pub const RAW_CARP: Self = Self(186);
    pub const RAW_SARDINE: Self = Self(187);
    pub const RAW_PERCH: Self = Self(188);
    pub const RAW_TROUT: Self = Self(189);
    pub const RAW_BASS: Self = Self(190);
    pub const RAW_SALMON: Self = Self(191);
    pub const RAW_CATFISH: Self = Self(192);
    pub const PUFFERFISH: Self = Self(193);
    pub const RAW_TUNA: Self = Self(194);
    pub const GOLDEN_KOI: Self = Self(195);
    pub const COOKED_FISH: Self = Self(196);
    pub const COOKED_SALMON: Self = Self(197);
    pub const SUSHI_ROLL: Self = Self(198);
    pub const FISH_STEW: Self = Self(199);

    // Power & Electric Machine items (Issues #14 & #15)
    pub const COPPER_WIRE: Self = Self(200);
    pub const POWER_CORE: Self = Self(201);
    pub const COPPER_CABLE: Self = Self(202);
    pub const COAL_GENERATOR: Self = Self(203);
    pub const SOLAR_PANEL: Self = Self(204);
    pub const WIND_TURBINE: Self = Self(205);
    pub const BATTERY_BOX: Self = Self(207);
    pub const ELECTRIC_FURNACE: Self = Self(208);
    pub const ELECTRIC_LAMP: Self = Self(209);
    pub const ADVANCED_SOLAR_PANEL: Self = Self(210);
    pub const ADVANCED_BATTERY_BOX: Self = Self(211);
    pub const QUANTUM_BATTERY_BOX: Self = Self(212);
    pub const GOLD_CABLE: Self = Self(213);
    pub const DIAMOND_CABLE: Self = Self(214);

    // Item Storage & Sorting System + Backpacks (Issue #13)
    pub const IRON_CHEST: Self = Self(215);
    pub const GOLD_CHEST: Self = Self(216);
    pub const DIAMOND_CHEST: Self = Self(217);
    pub const STORAGE_TERMINAL: Self = Self(218);
    pub const STORAGE_CABLE: Self = Self(219);
    pub const LEATHER_BACKPACK: Self = Self(220);
    pub const IRON_BACKPACK: Self = Self(221);
    pub const DIAMOND_BACKPACK: Self = Self(222);

    // Biome Boss Temple blocks, weapons, armor, and trophy cores (Issue #9)
    pub const TEMPLE_STONE: Self = Self(223);
    pub const TEMPLE_SANDSTONE: Self = Self(224);
    pub const TEMPLE_ICE_BRICK: Self = Self(225);
    pub const TEMPLE_MOSS_BRICK: Self = Self(226);
    pub const TEMPLE_OBSIDIAN: Self = Self(227);
    pub const TEMPLE_BRAZIER: Self = Self(228);

    pub const SUNBLADE: Self = Self(229);
    pub const FROSTBLADE: Self = Self(230);
    pub const VERDANT_GREATBLADE: Self = Self(231);
    pub const SHADOWFANG_BLADE: Self = Self(232);

    pub const PHARAOH_CROWN: Self = Self(233);
    pub const GLACIAL_CHESTPLATE: Self = Self(234);
    pub const GOLEM_GREAVES: Self = Self(235);
    pub const LICH_BOOTS: Self = Self(236);

    pub const SUN_CORE: Self = Self(237);
    pub const FROST_HEART: Self = Self(238);
    pub const ANCIENT_VINE_CORE: Self = Self(239);
    pub const SOUL_PHYLACTERY: Self = Self(240);

    // Farming, Cooking, and Hunger Revamp (Issue #12)
    pub const TOMATO_SEEDS: Self = Self(241);
    pub const MELON_SEEDS: Self = Self(242);
    pub const PUMPKIN_SEEDS: Self = Self(243);
    pub const SUGAR_CANE: Self = Self(244);
    pub const COMPOST: Self = Self(245);
    pub const BONE_FERTILIZER: Self = Self(246);

    pub const CARROT: Self = Self(247);
    pub const POTATO: Self = Self(248);
    pub const TOMATO: Self = Self(249);
    pub const SUGAR: Self = Self(250);
    pub const EGG: Self = Self(251);
    pub const MILK_BUCKET: Self = Self(252);
    pub const RAW_MUTTON: Self = Self(253);

    pub const BAKED_POTATO: Self = Self(254);
    pub const COOKED_MUTTON: Self = Self(255);
    pub const COOKIE: Self = Self(256);
    pub const PUMPKIN_PIE: Self = Self(257);

    pub const TOMATO_SOUP: Self = Self(258);
    pub const VEGETABLE_STEW: Self = Self(259);
    pub const HEARTY_MEAT_STEW: Self = Self(260);
    pub const OMELETTE: Self = Self(261);
    pub const FRUIT_SALAD: Self = Self(262);
    pub const SWEET_CAKE: Self = Self(263);
    pub const HERBAL_TEA: Self = Self(264);

    pub const COOKING_POT: Self = Self(265);

    // NPC Villages & Trading (Issue #11)
    pub const GOLD_COIN: Self = Self(266);
    pub const MINERS_LANTERN: Self = Self(267);
    pub const VANGUARD_BLADE: Self = Self(268);
    pub const MASTER_ANGLER_LURE: Self = Self(269);
    pub const GOLDEN_APPLE_PIE: Self = Self(270);
    pub const ROYAL_FEAST: Self = Self(271);
    pub const SPICED_ELIXIR: Self = Self(272);

    // Underground & Caves (Issue #23)
    pub const DEEPSTONE: Self = Self(273);
    pub const DRIPSTONE: Self = Self(274);
    pub const GLOW_CRYSTAL: Self = Self(275);
    pub const OBSIDIAN: Self = Self(276);
    pub const LAVA_BUCKET: Self = Self(277);

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns true if this item is any crop seed (used as Chicken Breeding Feed).
    #[inline]
    pub const fn is_seed(&self) -> bool {
        matches!(
            *self,
            Self::WHEAT_SEEDS | Self::TOMATO_SEEDS | Self::MELON_SEEDS | Self::PUMPKIN_SEEDS
        )
    }

    /// Returns the passive FertilizerTier for fertilizer items, if applicable.
    #[inline]
    pub const fn fertilizer_tier(&self) -> Option<crate::block::FertilizerTier> {
        match *self {
            Self::COMPOST => Some(crate::block::FertilizerTier::Compost),
            Self::BONE_FERTILIZER => Some(crate::block::FertilizerTier::Bone),
            _ => None,
        }
    }

    /// Returns the stage-0 farmland crop block planted when right-clicking farmland with this item.
    #[inline]
    pub const fn plantable_farmland_crop(&self) -> Option<BlockId> {
        match *self {
            Self::WHEAT_SEEDS => Some(BlockId::WHEAT_0),
            Self::CARROT => Some(BlockId::CARROTS_0),
            Self::POTATO => Some(BlockId::POTATOES_0),
            Self::TOMATO_SEEDS => Some(BlockId::TOMATOES_0),
            Self::MELON_SEEDS => Some(BlockId::MELON_STEM_0),
            Self::PUMPKIN_SEEDS => Some(BlockId::PUMPKIN_STEM_0),
            _ => None,
        }
    }

    /// Extra base melee damage for unique boss or shop weapons.
    #[inline]
    pub const fn boss_weapon_damage(&self) -> Option<f32> {
        match *self {
            Self::SUNBLADE => Some(11.0),
            Self::FROSTBLADE => Some(11.5),
            Self::VERDANT_GREATBLADE => Some(10.5),
            Self::SHADOWFANG_BLADE => Some(12.5),
            Self::VANGUARD_BLADE => Some(10.0),
            _ => None,
        }
    }

    /// Bonus lifesteal fraction granted by unique boss weapons.
    #[inline]
    pub const fn boss_weapon_lifesteal(&self) -> f32 {
        match *self {
            Self::VERDANT_GREATBLADE => 0.15,
            Self::SHADOWFANG_BLADE => 0.08,
            _ => 0.0,
        }
    }
}

/// Shop-exclusive Enhancement applied to a single tool, weapon, armor piece, or fishing rod (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Enhancement {
    Reinforced = 1,
    Haste = 2,
    Prospector = 3,
    Keen = 4,
    Bulwark = 5,
    TreasureHunter = 6,
    LuckyCast = 7,
}

impl Enhancement {
    pub const ALL: [Self; 7] = [
        Self::Reinforced,
        Self::Haste,
        Self::Prospector,
        Self::Keen,
        Self::Bulwark,
        Self::TreasureHunter,
        Self::LuckyCast,
    ];

    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Reinforced),
            2 => Some(Self::Haste),
            3 => Some(Self::Prospector),
            4 => Some(Self::Keen),
            5 => Some(Self::Bulwark),
            6 => Some(Self::TreasureHunter),
            7 => Some(Self::LuckyCast),
            _ => None,
        }
    }

    #[inline]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Reinforced => "Reinforced",
            Self::Haste => "Haste",
            Self::Prospector => "Prospector",
            Self::Keen => "Keen",
            Self::Bulwark => "Bulwark",
            Self::TreasureHunter => "Treasure Hunter",
            Self::LuckyCast => "Lucky Cast",
        }
    }

    #[inline]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Reinforced => "+100% Max Durability",
            Self::Haste => "+35% Mining Speed",
            Self::Prospector => "+25% Bonus Ore Drops",
            Self::Keen => "+25% Weapon Damage",
            Self::Bulwark => "+5% Damage Reduction",
            Self::TreasureHunter => "+50% Treasure Odds",
            Self::LuckyCast => "+35% Rare Fish Odds",
        }
    }

    /// Returns true if this Enhancement can be applied to `item_id`.
    pub fn is_compatible_with(self, item_id: ItemId, registry: &ItemRegistry) -> bool {
        let Some(def) = registry.get(item_id) else {
            return false;
        };
        let is_fishing_rod = crate::fishing::FishingRodKind::from_item(item_id).is_some();
        let is_armor = registry.get_armor(item_id).is_some();
        match self {
            Self::Reinforced => def.durability.is_some(),
            Self::Haste | Self::Prospector => matches!(
                def.tool_type,
                Some(ToolType::Pickaxe | ToolType::Axe | ToolType::Shovel)
            ),
            Self::Keen => matches!(def.tool_type, Some(ToolType::Sword)) || item_id == ItemId::BOW,
            Self::Bulwark => is_armor,
            Self::TreasureHunter | Self::LuckyCast => is_fishing_rod,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: &'static str,
    pub display_name: &'static str,
    pub max_stack: u16,
    pub placeable_block: Option<BlockId>,
    pub durability: Option<u16>,
    pub tool_type: Option<ToolType>,
    pub tool_tier: Option<ToolTier>,
    pub icon_tile: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSlot {
    pub item_id: ItemId,
    pub count: u16,
    pub durability: u16,
    pub enhancement: Option<Enhancement>,
}

impl Default for ItemSlot {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl ItemSlot {
    pub const EMPTY: Self = Self {
        item_id: ItemId::EMPTY,
        count: 0,
        durability: 0,
        enhancement: None,
    };

    pub const fn new(item_id: ItemId, count: u16) -> Self {
        Self {
            item_id,
            count,
            durability: 0,
            enhancement: None,
        }
    }

    pub const fn with_durability(item_id: ItemId, count: u16, durability: u16) -> Self {
        Self {
            item_id,
            count,
            durability,
            enhancement: None,
        }
    }

    pub const fn with_enhancement(
        item_id: ItemId,
        count: u16,
        durability: u16,
        enhancement: Option<Enhancement>,
    ) -> Self {
        Self {
            item_id,
            count,
            durability,
            enhancement,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.item_id.is_empty() || self.count == 0
    }

    #[inline(always)]
    pub fn is_enhanced(&self) -> bool {
        self.enhancement.is_some()
    }

    /// Formats a tooltip display label including any shop-exclusive Enhancement name and effect.
    pub fn tooltip_label(&self, registry: &ItemRegistry) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let def = registry.get(self.item_id)?;
        if let Some(enh) = self.enhancement {
            Some(format!(
                "{} [{} - {}]",
                def.display_name,
                enh.display_name(),
                enh.description()
            ))
        } else {
            Some(def.display_name.to_string())
        }
    }

    /// Applies an Enhancement to an unenhanced item and restores it to full durability.
    /// Returns `false` if the slot is empty or already enhanced.
    pub fn apply_enhancement(&mut self, enhancement: Enhancement) -> bool {
        if self.is_empty() || self.enhancement.is_some() {
            return false;
        }
        self.enhancement = Some(enhancement);
        self.durability = 0; // Restore to pristine full durability
        true
    }

    #[inline]
    pub fn can_stack_with(&self, other: &Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.item_id == other.item_id
            && self.durability == other.durability
            && self.enhancement == other.enhancement
    }

    /// Takes up to `amount` items from this slot and returns a new slot with them.
    pub fn take(&mut self, amount: u16) -> ItemSlot {
        if self.is_empty() {
            return ItemSlot::EMPTY;
        }
        let take_amount = amount.min(self.count);
        self.count -= take_amount;
        let taken = ItemSlot {
            item_id: self.item_id,
            count: take_amount,
            durability: self.durability,
            enhancement: self.enhancement,
        };
        if self.count == 0 {
            *self = ItemSlot::EMPTY;
        }
        taken
    }

    /// Splits the slot into half (rounded up for taken half).
    pub fn split_half(&mut self) -> ItemSlot {
        if self.is_empty() {
            return ItemSlot::EMPTY;
        }
        let half = (self.count + 1) / 2;
        self.take(half)
    }

    /// Returns the effective max durability, accounting for the `Reinforced` Enhancement (+100%).
    #[inline]
    pub const fn effective_max_durability(&self, base_max_durability: u16) -> u16 {
        if matches!(self.enhancement, Some(Enhancement::Reinforced)) {
            base_max_durability.saturating_mul(2)
        } else {
            base_max_durability
        }
    }

    /// Returns the mining speed multiplier granted by this slot's Enhancement.
    #[inline]
    pub fn mining_speed_multiplier(&self) -> f32 {
        if matches!(self.enhancement, Some(Enhancement::Haste)) {
            1.35
        } else {
            1.0
        }
    }

    /// Returns the bonus ore/block drop chance granted by this slot's Enhancement.
    #[inline]
    pub fn prospector_bonus_chance(&self) -> f32 {
        if matches!(self.enhancement, Some(Enhancement::Prospector)) {
            0.25
        } else {
            0.0
        }
    }

    /// Returns the weapon damage multiplier granted by this slot's Enhancement.
    #[inline]
    pub fn weapon_damage_multiplier(&self) -> f32 {
        if matches!(self.enhancement, Some(Enhancement::Keen)) {
            1.25
        } else {
            1.0
        }
    }

    /// Returns the remaining durability given the item's base max durability.
    /// If self.durability == 0, the tool is pristine (full effective durability).
    /// Otherwise, self.durability stores the remaining durability points.
    #[inline]
    pub fn remaining_durability(&self, max_durability: u16) -> u16 {
        let eff_max = self.effective_max_durability(max_durability);
        if self.durability == 0 {
            eff_max
        } else {
            self.durability.min(eff_max)
        }
    }

    /// Returns true if this slot contains a tool that has taken damage.
    #[inline]
    pub fn is_damaged(&self, max_durability: u16) -> bool {
        let eff_max = self.effective_max_durability(max_durability);
        self.durability > 0 && self.durability < eff_max
    }

    /// Returns the fraction of remaining durability in the range [0.0, 1.0].
    #[inline]
    pub fn durability_fraction(&self, max_durability: u16) -> f32 {
        let eff_max = self.effective_max_durability(max_durability);
        if eff_max == 0 {
            return 1.0;
        }
        (self.remaining_durability(max_durability) as f32) / (eff_max as f32)
    }

    /// Damages the tool by `amount` points.
    /// Returns `true` if the tool broke (durability reached 0), resetting this slot to `EMPTY`.
    /// Returns `false` if the tool took damage but did not break.
    pub fn damage_tool(&mut self, amount: u16, max_durability: u16) -> bool {
        let eff_max = self.effective_max_durability(max_durability);
        if eff_max == 0 || self.is_empty() {
            return false;
        }
        let current = self.remaining_durability(max_durability);
        if current <= amount {
            *self = ItemSlot::EMPTY;
            true
        } else {
            self.durability = current - amount;
            false
        }
    }
}

/// Status effects applied by unsafe or specialty foods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FoodEffectKind {
    FoodPoisoning = 1,
    Poison = 2,
    Speed = 3,
    Regeneration = 4,
    MiningHaste = 5,
    Burning = 6,
}

impl FoodEffectKind {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::FoodPoisoning),
            2 => Some(Self::Poison),
            3 => Some(Self::Speed),
            4 => Some(Self::Regeneration),
            5 => Some(Self::MiningHaste),
            6 => Some(Self::Burning),
            _ => None,
        }
    }

    #[inline]
    pub const fn is_negative(self) -> bool {
        matches!(self, Self::FoodPoisoning | Self::Poison)
    }
}

/// Probabilistic status effect applied when consuming an unsafe food item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoodEffectSpec {
    pub kind: FoodEffectKind,
    /// Probability in `0.0..=1.0`.
    pub chance: f32,
    pub duration_secs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoodDef {
    pub food_points: u16,
    pub saturation: f32,
    /// Duration in seconds the player must hold right-click to consume this item.
    pub eat_duration: f32,
    /// Whether this consumable can be used even when hunger is already full (e.g., Herbal Tea).
    pub always_edible: bool,
    /// Whether consuming this item clears active negative food effects (Food Poisoning / Poison).
    pub cures_negative_effects: bool,
    /// Optional unsafe food negative effect.
    pub effect: Option<FoodEffectSpec>,
}

impl FoodDef {
    pub const DEFAULT_EAT_DURATION: f32 = 1.2;

    pub const fn new(food_points: u16, saturation: f32) -> Self {
        Self {
            food_points,
            saturation,
            eat_duration: Self::DEFAULT_EAT_DURATION,
            always_edible: false,
            cures_negative_effects: false,
            effect: None,
        }
    }

    pub const fn with_eat_duration(mut self, secs: f32) -> Self {
        self.eat_duration = secs;
        self
    }

    pub const fn with_effect(mut self, kind: FoodEffectKind, chance: f32, duration_secs: f32) -> Self {
        self.effect = Some(FoodEffectSpec {
            kind,
            chance,
            duration_secs,
        });
        self
    }

    pub const fn with_always_edible(mut self) -> Self {
        self.always_edible = true;
        self
    }

    pub const fn with_cure(mut self) -> Self {
        self.cures_negative_effects = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSlot {
    Helmet = 0,
    Chestplate = 1,
    Leggings = 2,
    Boots = 3,
}

impl ArmorSlot {
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArmorEquipment {
    pub slots: [ItemSlot; 4],
}

impl ArmorEquipment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_defense(&self, registry: &ItemRegistry) -> u8 {
        let mut total: u8 = 0;
        for slot in &self.slots {
            if !slot.is_empty() {
                if let Some((_, def)) = registry.get_armor(slot.item_id) {
                    total = total.saturating_add(def);
                }
            }
        }
        total.min(20)
    }

    /// Returns the damage multiplier (1.0 with 0 armor, down to 0.20 at 20 armor points,
    /// plus 5% extra damage reduction per `Bulwark`-enhanced armor piece down to 0.15).
    pub fn damage_multiplier(&self, registry: &ItemRegistry) -> f32 {
        let defense = self.total_defense(registry) as f32;
        let bulwark_pieces = self
            .slots
            .iter()
            .filter(|s| !s.is_empty() && matches!(s.enhancement, Some(Enhancement::Bulwark)))
            .count() as f32;
        (1.0 - defense * 0.04 - bulwark_pieces * 0.05).clamp(0.15, 1.0)
    }

    /// Applies durability wear to all equipped armor pieces when taking damage.
    /// Returns `true` if any piece broke.
    pub fn damage_armor(&mut self, raw_damage: f32, registry: &ItemRegistry) -> bool {
        if raw_damage <= 0.0 {
            return false;
        }
        let wear = (raw_damage / 4.0).ceil().max(1.0) as u16;
        let mut any_broke = false;
        for slot in &mut self.slots {
            if !slot.is_empty() {
                if let Some(def) = registry.get(slot.item_id) {
                    if let Some(max_dur) = def.durability {
                        if slot.damage_tool(wear, max_dur) {
                            any_broke = true;
                        }
                    }
                }
            }
        }
        any_broke
    }
}

pub struct ItemRegistry {
    items: Vec<Option<ItemDef>>,
    by_name: HashMap<String, ItemId>,
    block_to_item: HashMap<BlockId, ItemId>,
    food_properties: HashMap<ItemId, FoodDef>,
    armor_properties: HashMap<ItemId, (ArmorSlot, u8)>,
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            items: Vec::new(),
            by_name: HashMap::new(),
            block_to_item: HashMap::new(),
            food_properties: HashMap::new(),
            armor_properties: HashMap::new(),
        };
        reg.register_default_items();
        reg
    }

    pub fn register(&mut self, def: ItemDef) {
        let idx = def.id.0 as usize;
        if idx >= self.items.len() {
            self.items.resize(idx + 1, None);
        }
        self.by_name.insert(def.name.to_string(), def.id);
        if let Some(block_id) = def.placeable_block {
            self.block_to_item.insert(block_id, def.id);
        }
        self.items[idx] = Some(def);
    }

    pub fn register_food(&mut self, id: ItemId, food: FoodDef) {
        self.food_properties.insert(id, food);
    }

    pub fn get_food(&self, id: ItemId) -> Option<FoodDef> {
        self.food_properties.get(&id).copied()
    }

    pub fn is_food(&self, id: ItemId) -> bool {
        self.food_properties.contains_key(&id)
    }

    pub fn register_armor(&mut self, id: ItemId, slot: ArmorSlot, defense: u8) {
        self.armor_properties.insert(id, (slot, defense));
    }

    pub fn get_armor(&self, id: ItemId) -> Option<(ArmorSlot, u8)> {
        self.armor_properties.get(&id).copied()
    }

    pub fn is_armor(&self, id: ItemId) -> bool {
        self.armor_properties.contains_key(&id)
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemDef> {
        self.items.get(id.0 as usize).and_then(|x| x.as_ref())
    }

    pub fn get_by_name(&self, name: &str) -> Option<&ItemDef> {
        self.by_name.get(name).and_then(|&id| self.get(id))
    }

    pub fn item_for_block(&self, block: BlockId) -> Option<ItemId> {
        self.block_to_item.get(&block).copied()
    }

    pub fn max_stack_size(&self, id: ItemId) -> u16 {
        self.get(id).map(|d| d.max_stack).unwrap_or(64)
    }

    /// Returns all registered items in definition order.
    pub fn all_items(&self) -> Vec<&ItemDef> {
        self.items.iter().filter_map(|d| d.as_ref()).collect()
    }

    fn register_default_items(&mut self) {
        // Blocks as Placeable Items
        self.register(ItemDef {
            id: ItemId::STONE,
            name: "blockgame:stone",
            display_name: "Stone",
            max_stack: 64,
            placeable_block: Some(BlockId::STONE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 1,
        });

        self.register(ItemDef {
            id: ItemId::DIRT,
            name: "blockgame:dirt",
            display_name: "Dirt",
            max_stack: 64,
            placeable_block: Some(BlockId::DIRT),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 2,
        });

        self.register(ItemDef {
            id: ItemId::GRASS,
            name: "blockgame:grass_block",
            display_name: "Grass Block",
            max_stack: 64,
            placeable_block: Some(BlockId::GRASS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 4,
        });

        self.register(ItemDef {
            id: ItemId::COBBLESTONE,
            name: "blockgame:cobblestone",
            display_name: "Cobblestone",
            max_stack: 64,
            placeable_block: Some(BlockId::COBBLESTONE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 5,
        });

        self.register(ItemDef {
            id: ItemId::WOOD_LOG,
            name: "blockgame:oak_log",
            display_name: "Oak Log",
            max_stack: 64,
            placeable_block: Some(BlockId::WOOD_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 6,
        });

        self.register(ItemDef {
            id: ItemId::LEAVES,
            name: "blockgame:leaves",
            display_name: "Oak Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 8,
        });

        self.register(ItemDef {
            id: ItemId::SAND,
            name: "blockgame:sand",
            display_name: "Sand",
            max_stack: 64,
            placeable_block: Some(BlockId::SAND),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 9,
        });

        self.register(ItemDef {
            id: ItemId::PLANKS,
            name: "blockgame:planks",
            display_name: "Oak Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 10,
        });

        self.register(ItemDef {
            id: ItemId::GLASS,
            name: "blockgame:glass",
            display_name: "Glass",
            max_stack: 64,
            placeable_block: Some(BlockId::GLASS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 11,
        });

        self.register(ItemDef {
            id: ItemId::TORCH,
            name: "blockgame:torch",
            display_name: "Torch",
            max_stack: 64,
            placeable_block: Some(BlockId::TORCH),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 13,
        });

        // Crafting & Resource Items
        self.register(ItemDef {
            id: ItemId::STICK,
            name: "blockgame:stick",
            display_name: "Stick",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 14,
        });

        self.register(ItemDef {
            id: ItemId::WOODEN_PICKAXE,
            name: "blockgame:wooden_pickaxe",
            display_name: "Wooden Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(59),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 15,
        });

        self.register(ItemDef {
            id: ItemId::STONE_PICKAXE,
            name: "blockgame:stone_pickaxe",
            display_name: "Stone Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(131),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 16,
        });

        self.register(ItemDef {
            id: ItemId::CRAFTING_TABLE,
            name: "blockgame:crafting_table",
            display_name: "Crafting Table",
            max_stack: 64,
            placeable_block: Some(BlockId::CRAFTING_TABLE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 17,
        });

        self.register(ItemDef {
            id: ItemId::COAL,
            name: "blockgame:coal",
            display_name: "Coal",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 18,
        });

        self.register(ItemDef {
            id: ItemId::IRON_INGOT,
            name: "blockgame:iron_ingot",
            display_name: "Iron Ingot",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 19,
        });

        self.register(ItemDef {
            id: ItemId::IRON_PICKAXE,
            name: "blockgame:iron_pickaxe",
            display_name: "Iron Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(250),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 20,
        });

        // Placeable Ore Items
        self.register(ItemDef {
            id: ItemId::COAL_ORE,
            name: "blockgame:coal_ore",
            display_name: "Coal Ore",
            max_stack: 64,
            placeable_block: Some(BlockId::COAL_ORE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 129,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_ORE,
            name: "blockgame:copper_ore",
            display_name: "Copper Ore",
            max_stack: 64,
            placeable_block: Some(BlockId::COPPER_ORE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 130,
        });

        self.register(ItemDef {
            id: ItemId::IRON_ORE,
            name: "blockgame:iron_ore",
            display_name: "Iron Ore",
            max_stack: 64,
            placeable_block: Some(BlockId::IRON_ORE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 131,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_ORE,
            name: "blockgame:gold_ore",
            display_name: "Gold Ore",
            max_stack: 64,
            placeable_block: Some(BlockId::GOLD_ORE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 132,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND_ORE,
            name: "blockgame:diamond_ore",
            display_name: "Diamond Ore",
            max_stack: 64,
            placeable_block: Some(BlockId::DIAMOND_ORE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 133,
        });

        // Raw metals, ingots, gems
        self.register(ItemDef {
            id: ItemId::RAW_COPPER,
            name: "blockgame:raw_copper",
            display_name: "Raw Copper",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 134,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_INGOT,
            name: "blockgame:copper_ingot",
            display_name: "Copper Ingot",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 135,
        });

        self.register(ItemDef {
            id: ItemId::RAW_IRON,
            name: "blockgame:raw_iron",
            display_name: "Raw Iron",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 136,
        });

        self.register(ItemDef {
            id: ItemId::RAW_GOLD,
            name: "blockgame:raw_gold",
            display_name: "Raw Gold",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 137,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_INGOT,
            name: "blockgame:gold_ingot",
            display_name: "Gold Ingot",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 138,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND,
            name: "blockgame:diamond",
            display_name: "Diamond",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 139,
        });

        // Diamond Pickaxe
        self.register(ItemDef {
            id: ItemId::DIAMOND_PICKAXE,
            name: "blockgame:diamond_pickaxe",
            display_name: "Diamond Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(1561),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 140,
        });

        // Axes
        self.register(ItemDef {
            id: ItemId::WOODEN_AXE,
            name: "blockgame:wooden_axe",
            display_name: "Wooden Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(59),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 141,
        });

        self.register(ItemDef {
            id: ItemId::STONE_AXE,
            name: "blockgame:stone_axe",
            display_name: "Stone Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(131),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 142,
        });

        self.register(ItemDef {
            id: ItemId::IRON_AXE,
            name: "blockgame:iron_axe",
            display_name: "Iron Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(250),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 143,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND_AXE,
            name: "blockgame:diamond_axe",
            display_name: "Diamond Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(1561),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 144,
        });

        // Shovels
        self.register(ItemDef {
            id: ItemId::WOODEN_SHOVEL,
            name: "blockgame:wooden_shovel",
            display_name: "Wooden Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(59),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 145,
        });

        self.register(ItemDef {
            id: ItemId::STONE_SHOVEL,
            name: "blockgame:stone_shovel",
            display_name: "Stone Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(131),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 146,
        });

        self.register(ItemDef {
            id: ItemId::IRON_SHOVEL,
            name: "blockgame:iron_shovel",
            display_name: "Iron Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(250),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 147,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND_SHOVEL,
            name: "blockgame:diamond_shovel",
            display_name: "Diamond Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(1561),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 148,
        });

        self.register(ItemDef {
            id: ItemId::APPLE,
            name: "blockgame:apple",
            display_name: "Apple",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 31,
        });
        self.register_food(ItemId::APPLE, FoodDef::new(4, 2.4));

        // Hoes
        self.register(ItemDef {
            id: ItemId::WOODEN_HOE,
            name: "blockgame:wooden_hoe",
            display_name: "Wooden Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(59),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 155,
        });

        self.register(ItemDef {
            id: ItemId::STONE_HOE,
            name: "blockgame:stone_hoe",
            display_name: "Stone Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(131),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 156,
        });

        self.register(ItemDef {
            id: ItemId::IRON_HOE,
            name: "blockgame:iron_hoe",
            display_name: "Iron Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(250),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 157,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND_HOE,
            name: "blockgame:diamond_hoe",
            display_name: "Diamond Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(1561),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 158,
        });

        // Farming and Food
        self.register(ItemDef {
            id: ItemId::WHEAT_SEEDS,
            name: "blockgame:wheat_seeds",
            display_name: "Wheat Seeds",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 159,
        });

        self.register(ItemDef {
            id: ItemId::WHEAT,
            name: "blockgame:wheat",
            display_name: "Wheat",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 160,
        });

        self.register(ItemDef {
            id: ItemId::BREAD,
            name: "blockgame:bread",
            display_name: "Bread",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 161,
        });
        self.register_food(ItemId::BREAD, FoodDef::new(5, 6.0));

        // Animal Meats and Drops
        self.register(ItemDef {
            id: ItemId::RAW_BEEF,
            name: "blockgame:raw_beef",
            display_name: "Raw Beef",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 162,
        });
        self.register_food(ItemId::RAW_BEEF, FoodDef::new(3, 1.8));

        self.register(ItemDef {
            id: ItemId::COOKED_STEAK,
            name: "blockgame:cooked_steak",
            display_name: "Steak",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 163,
        });
        self.register_food(ItemId::COOKED_STEAK, FoodDef::new(8, 12.8));

        self.register(ItemDef {
            id: ItemId::RAW_PORKCHOP,
            name: "blockgame:raw_porkchop",
            display_name: "Raw Porkchop",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 164,
        });
        self.register_food(ItemId::RAW_PORKCHOP, FoodDef::new(3, 1.8));

        self.register(ItemDef {
            id: ItemId::COOKED_PORKCHOP,
            name: "blockgame:cooked_porkchop",
            display_name: "Cooked Porkchop",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 165,
        });
        self.register_food(ItemId::COOKED_PORKCHOP, FoodDef::new(8, 12.8));

        self.register(ItemDef {
            id: ItemId::RAW_CHICKEN,
            name: "blockgame:raw_chicken",
            display_name: "Raw Chicken",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 166,
        });
        self.register_food(
            ItemId::RAW_CHICKEN,
            FoodDef::new(2, 1.2).with_effect(FoodEffectKind::FoodPoisoning, 0.30, 15.0),
        );

        self.register(ItemDef {
            id: ItemId::COOKED_CHICKEN,
            name: "blockgame:cooked_chicken",
            display_name: "Cooked Chicken",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 167,
        });
        self.register_food(ItemId::COOKED_CHICKEN, FoodDef::new(6, 7.2));

        self.register(ItemDef {
            id: ItemId::LEATHER,
            name: "blockgame:leather",
            display_name: "Leather",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 168,
        });

        self.register(ItemDef {
            id: ItemId::FEATHER,
            name: "blockgame:feather",
            display_name: "Feather",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 169,
        });

        self.register(ItemDef {
            id: ItemId::CHEST,
            name: "blockgame:chest",
            display_name: "Chest",
            max_stack: 64,
            placeable_block: Some(BlockId::CHEST),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 177,
        });

        self.register(ItemDef {
            id: ItemId::FURNACE,
            name: "blockgame:furnace",
            display_name: "Furnace",
            max_stack: 64,
            placeable_block: Some(BlockId::FURNACE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 182,
        });

        self.register(ItemDef {
            id: ItemId::BIRCH_LOG,
            name: "blockgame:birch_log",
            display_name: "Birch Log",
            max_stack: 64,
            placeable_block: Some(BlockId::BIRCH_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 192,
        });

        self.register(ItemDef {
            id: ItemId::BIRCH_PLANKS,
            name: "blockgame:birch_planks",
            display_name: "Birch Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::BIRCH_PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 194,
        });

        self.register(ItemDef {
            id: ItemId::BIRCH_LEAVES,
            name: "blockgame:birch_leaves",
            display_name: "Birch Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::BIRCH_LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 195,
        });

        self.register(ItemDef {
            id: ItemId::SPRUCE_LOG,
            name: "blockgame:spruce_log",
            display_name: "Spruce Log",
            max_stack: 64,
            placeable_block: Some(BlockId::SPRUCE_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 196,
        });

        self.register(ItemDef {
            id: ItemId::SPRUCE_PLANKS,
            name: "blockgame:spruce_planks",
            display_name: "Spruce Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::SPRUCE_PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 198,
        });

        self.register(ItemDef {
            id: ItemId::SPRUCE_LEAVES,
            name: "blockgame:spruce_leaves",
            display_name: "Spruce Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::SPRUCE_LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 199,
        });

        self.register(ItemDef {
            id: ItemId::TALL_GRASS,
            name: "blockgame:tall_grass",
            display_name: "Tall Grass",
            max_stack: 64,
            placeable_block: Some(BlockId::TALL_GRASS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 200,
        });

        self.register(ItemDef {
            id: ItemId::SNOW_BLOCK,
            name: "blockgame:snow_block",
            display_name: "Snow Block",
            max_stack: 64,
            placeable_block: Some(BlockId::SNOW_BLOCK),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 203,
        });

        self.register(ItemDef {
            id: ItemId::ICE,
            name: "blockgame:ice",
            display_name: "Ice",
            max_stack: 64,
            placeable_block: Some(BlockId::ICE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 204,
        });

        self.register(ItemDef {
            id: ItemId::CLAY,
            name: "blockgame:clay",
            display_name: "Clay",
            max_stack: 64,
            placeable_block: Some(BlockId::CLAY),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 205,
        });

        self.register(ItemDef {
            id: ItemId::LILY_PAD,
            name: "blockgame:lily_pad",
            display_name: "Lily Pad",
            max_stack: 64,
            placeable_block: Some(BlockId::LILY_PAD),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 206,
        });

        self.register(ItemDef {
            id: ItemId::ACACIA_LOG,
            name: "blockgame:acacia_log",
            display_name: "Acacia Log",
            max_stack: 64,
            placeable_block: Some(BlockId::ACACIA_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 207,
        });

        self.register(ItemDef {
            id: ItemId::ACACIA_PLANKS,
            name: "blockgame:acacia_planks",
            display_name: "Acacia Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::ACACIA_PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 209,
        });

        self.register(ItemDef {
            id: ItemId::ACACIA_LEAVES,
            name: "blockgame:acacia_leaves",
            display_name: "Acacia Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::ACACIA_LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 210,
        });

        self.register(ItemDef {
            id: ItemId::CACTUS,
            name: "blockgame:cactus",
            display_name: "Cactus",
            max_stack: 64,
            placeable_block: Some(BlockId::CACTUS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 211,
        });

        self.register(ItemDef {
            id: ItemId::SANDSTONE,
            name: "blockgame:sandstone",
            display_name: "Sandstone",
            max_stack: 64,
            placeable_block: Some(BlockId::SANDSTONE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 213,
        });

        self.register(ItemDef {
            id: ItemId::GRAVEL,
            name: "blockgame:gravel",
            display_name: "Gravel",
            max_stack: 64,
            placeable_block: Some(BlockId::GRAVEL),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 216,
        });

        // Jungle Wood Items
        self.register(ItemDef {
            id: ItemId::JUNGLE_LOG,
            name: "blockgame:jungle_log",
            display_name: "Jungle Log",
            max_stack: 64,
            placeable_block: Some(BlockId::JUNGLE_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 217,
        });

        self.register(ItemDef {
            id: ItemId::JUNGLE_PLANKS,
            name: "blockgame:jungle_planks",
            display_name: "Jungle Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::JUNGLE_PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 219,
        });

        self.register(ItemDef {
            id: ItemId::JUNGLE_LEAVES,
            name: "blockgame:jungle_leaves",
            display_name: "Jungle Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::JUNGLE_LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 220,
        });

        // Badlands Items
        self.register(ItemDef {
            id: ItemId::TERRACOTTA,
            name: "blockgame:terracotta",
            display_name: "Terracotta",
            max_stack: 64,
            placeable_block: Some(BlockId::TERRACOTTA),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 221,
        });

        self.register(ItemDef {
            id: ItemId::RED_SAND,
            name: "blockgame:red_sand",
            display_name: "Red Sand",
            max_stack: 64,
            placeable_block: Some(BlockId::RED_SAND),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 222,
        });

        // Phase 4 Items: Taiga & Mushroom Fields
        self.register(ItemDef {
            id: ItemId::PODZOL,
            name: "blockgame:podzol",
            display_name: "Podzol",
            max_stack: 64,
            placeable_block: Some(BlockId::PODZOL),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 224,
        });

        self.register(ItemDef {
            id: ItemId::MOSSY_COBBLESTONE,
            name: "blockgame:mossy_cobblestone",
            display_name: "Mossy Cobblestone",
            max_stack: 64,
            placeable_block: Some(BlockId::MOSSY_COBBLESTONE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 225,
        });

        self.register(ItemDef {
            id: ItemId::MYCELIUM,
            name: "blockgame:mycelium",
            display_name: "Mycelium",
            max_stack: 64,
            placeable_block: Some(BlockId::MYCELIUM),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 227,
        });

        self.register(ItemDef {
            id: ItemId::RED_MUSHROOM_BLOCK,
            name: "blockgame:red_mushroom_block",
            display_name: "Red Mushroom Block",
            max_stack: 64,
            placeable_block: Some(BlockId::RED_MUSHROOM_BLOCK),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 228,
        });

        self.register(ItemDef {
            id: ItemId::BROWN_MUSHROOM_BLOCK,
            name: "blockgame:brown_mushroom_block",
            display_name: "Brown Mushroom Block",
            max_stack: 64,
            placeable_block: Some(BlockId::BROWN_MUSHROOM_BLOCK),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 229,
        });

        self.register(ItemDef {
            id: ItemId::MUSHROOM_STEM,
            name: "blockgame:mushroom_stem",
            display_name: "Mushroom Stem",
            max_stack: 64,
            placeable_block: Some(BlockId::MUSHROOM_STEM),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 230,
        });

        // Phase 5 Items: Dark Forest & Coral Reefs
        self.register(ItemDef {
            id: ItemId::DARK_OAK_LOG,
            name: "blockgame:dark_oak_log",
            display_name: "Dark Oak Log",
            max_stack: 64,
            placeable_block: Some(BlockId::DARK_OAK_LOG),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 231,
        });

        self.register(ItemDef {
            id: ItemId::DARK_OAK_PLANKS,
            name: "blockgame:dark_oak_planks",
            display_name: "Dark Oak Planks",
            max_stack: 64,
            placeable_block: Some(BlockId::DARK_OAK_PLANKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 233,
        });

        self.register(ItemDef {
            id: ItemId::DARK_OAK_LEAVES,
            name: "blockgame:dark_oak_leaves",
            display_name: "Dark Oak Leaves",
            max_stack: 64,
            placeable_block: Some(BlockId::DARK_OAK_LEAVES),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 234,
        });

        self.register(ItemDef {
            id: ItemId::CORAL_BLOCK,
            name: "blockgame:coral_block",
            display_name: "Brain Coral Block",
            max_stack: 64,
            placeable_block: Some(BlockId::CORAL_BLOCK),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 235,
        });

        self.register(ItemDef {
            id: ItemId::TUBE_CORAL_BLOCK,
            name: "blockgame:tube_coral_block",
            display_name: "Tube Coral Block",
            max_stack: 64,
            placeable_block: Some(BlockId::TUBE_CORAL_BLOCK),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 236,
        });

        self.register(ItemDef {
            id: ItemId::PRISMARINE,
            name: "blockgame:prismarine",
            display_name: "Prismarine",
            max_stack: 64,
            placeable_block: Some(BlockId::PRISMARINE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 237,
        });

        self.register(ItemDef {
            id: ItemId::SEA_LANTERN,
            name: "blockgame:sea_lantern",
            display_name: "Sea Lantern",
            max_stack: 64,
            placeable_block: Some(BlockId::SEA_LANTERN),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 238,
        });

        self.register(ItemDef {
            id: ItemId::SEAGRASS,
            name: "blockgame:seagrass",
            display_name: "Seagrass",
            max_stack: 64,
            placeable_block: Some(BlockId::SEAGRASS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 239,
        });

        self.register(ItemDef {
            id: ItemId::BUCKET,
            name: "blockgame:bucket",
            display_name: "Bucket",
            max_stack: 16,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 242,
        });

        self.register(ItemDef {
            id: ItemId::WATER_BUCKET,
            name: "blockgame:water_bucket",
            display_name: "Water Bucket",
            max_stack: 1,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 243,
        });

        // Swords (Wood: 59, Stone: 131, Copper: 190, Iron: 250, Diamond: 1561, Gold: 32)
        self.register(ItemDef {
            id: ItemId::WOODEN_SWORD,
            name: "blockgame:wooden_sword",
            display_name: "Wooden Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Wood.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Wood),
            icon_tile: 256,
        });

        self.register(ItemDef {
            id: ItemId::STONE_SWORD,
            name: "blockgame:stone_sword",
            display_name: "Stone Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Stone.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Stone),
            icon_tile: 257,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_SWORD,
            name: "blockgame:copper_sword",
            display_name: "Copper Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Copper.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Copper),
            icon_tile: 258,
        });

        self.register(ItemDef {
            id: ItemId::IRON_SWORD,
            name: "blockgame:iron_sword",
            display_name: "Iron Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Iron.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Iron),
            icon_tile: 259,
        });

        self.register(ItemDef {
            id: ItemId::DIAMOND_SWORD,
            name: "blockgame:diamond_sword",
            display_name: "Diamond Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Diamond.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 260,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_SWORD,
            name: "blockgame:gold_sword",
            display_name: "Golden Sword",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Gold.max_durability()),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Gold),
            icon_tile: 261,
        });

        // Copper Tools (Durability: 190)
        self.register(ItemDef {
            id: ItemId::COPPER_PICKAXE,
            name: "blockgame:copper_pickaxe",
            display_name: "Copper Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Copper.max_durability()),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Copper),
            icon_tile: 262,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_AXE,
            name: "blockgame:copper_axe",
            display_name: "Copper Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Copper.max_durability()),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Copper),
            icon_tile: 263,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_SHOVEL,
            name: "blockgame:copper_shovel",
            display_name: "Copper Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Copper.max_durability()),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Copper),
            icon_tile: 264,
        });

        self.register(ItemDef {
            id: ItemId::COPPER_HOE,
            name: "blockgame:copper_hoe",
            display_name: "Copper Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Copper.max_durability()),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Copper),
            icon_tile: 265,
        });

        // Gold Tools (Durability: 32)
        self.register(ItemDef {
            id: ItemId::GOLD_PICKAXE,
            name: "blockgame:gold_pickaxe",
            display_name: "Golden Pickaxe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Gold.max_durability()),
            tool_type: Some(ToolType::Pickaxe),
            tool_tier: Some(ToolTier::Gold),
            icon_tile: 266,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_AXE,
            name: "blockgame:gold_axe",
            display_name: "Golden Axe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Gold.max_durability()),
            tool_type: Some(ToolType::Axe),
            tool_tier: Some(ToolTier::Gold),
            icon_tile: 267,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_SHOVEL,
            name: "blockgame:gold_shovel",
            display_name: "Golden Shovel",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Gold.max_durability()),
            tool_type: Some(ToolType::Shovel),
            tool_tier: Some(ToolTier::Gold),
            icon_tile: 268,
        });

        self.register(ItemDef {
            id: ItemId::GOLD_HOE,
            name: "blockgame:gold_hoe",
            display_name: "Golden Hoe",
            max_stack: 1,
            placeable_block: None,
            durability: Some(ToolTier::Gold.max_durability()),
            tool_type: Some(ToolType::Hoe),
            tool_tier: Some(ToolTier::Gold),
            icon_tile: 269,
        });

        // Bricks
        self.register(ItemDef {
            id: ItemId::BRICK,
            name: "blockgame:brick",
            display_name: "Brick",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 315,
        });

        self.register(ItemDef {
            id: ItemId::BRICKS,
            name: "blockgame:bricks",
            display_name: "Bricks",
            max_stack: 64,
            placeable_block: Some(BlockId::BRICKS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 288,
        });

        // Wool (13 colors)
        let wool_defs = [
            (ItemId::WHITE_WOOL, BlockId::WHITE_WOOL, "blockgame:white_wool", "White Wool", 289),
            (ItemId::RED_WOOL, BlockId::RED_WOOL, "blockgame:red_wool", "Red Wool", 290),
            (ItemId::ORANGE_WOOL, BlockId::ORANGE_WOOL, "blockgame:orange_wool", "Orange Wool", 291),
            (ItemId::YELLOW_WOOL, BlockId::YELLOW_WOOL, "blockgame:yellow_wool", "Yellow Wool", 292),
            (ItemId::LIME_WOOL, BlockId::LIME_WOOL, "blockgame:lime_wool", "Lime Wool", 293),
            (ItemId::GREEN_WOOL, BlockId::GREEN_WOOL, "blockgame:green_wool", "Green Wool", 294),
            (ItemId::CYAN_WOOL, BlockId::CYAN_WOOL, "blockgame:cyan_wool", "Cyan Wool", 295),
            (ItemId::BLUE_WOOL, BlockId::BLUE_WOOL, "blockgame:blue_wool", "Blue Wool", 296),
            (ItemId::PURPLE_WOOL, BlockId::PURPLE_WOOL, "blockgame:purple_wool", "Purple Wool", 297),
            (ItemId::PINK_WOOL, BlockId::PINK_WOOL, "blockgame:pink_wool", "Pink Wool", 298),
            (ItemId::BROWN_WOOL, BlockId::BROWN_WOOL, "blockgame:brown_wool", "Brown Wool", 299),
            (ItemId::GRAY_WOOL, BlockId::GRAY_WOOL, "blockgame:gray_wool", "Gray Wool", 300),
            (ItemId::BLACK_WOOL, BlockId::BLACK_WOOL, "blockgame:black_wool", "Black Wool", 301),
        ];
        for (id, block_id, name, display_name, icon_tile) in wool_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block: Some(block_id),
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Dyes (13 colors)
        let dye_defs = [
            (ItemId::WHITE_DYE, "blockgame:white_dye", "White Dye", 302),
            (ItemId::RED_DYE, "blockgame:red_dye", "Red Dye", 303),
            (ItemId::ORANGE_DYE, "blockgame:orange_dye", "Orange Dye", 304),
            (ItemId::YELLOW_DYE, "blockgame:yellow_dye", "Yellow Dye", 305),
            (ItemId::LIME_DYE, "blockgame:lime_dye", "Lime Dye", 306),
            (ItemId::GREEN_DYE, "blockgame:green_dye", "Green Dye", 307),
            (ItemId::CYAN_DYE, "blockgame:cyan_dye", "Cyan Dye", 308),
            (ItemId::BLUE_DYE, "blockgame:blue_dye", "Blue Dye", 309),
            (ItemId::PURPLE_DYE, "blockgame:purple_dye", "Purple Dye", 310),
            (ItemId::PINK_DYE, "blockgame:pink_dye", "Pink Dye", 311),
            (ItemId::BROWN_DYE, "blockgame:brown_dye", "Brown Dye", 312),
            (ItemId::GRAY_DYE, "blockgame:gray_dye", "Gray Dye", 313),
            (ItemId::BLACK_DYE, "blockgame:black_dye", "Black Dye", 314),
        ];
        for (id, name, display_name, icon_tile) in dye_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Stairs, Fence, Door, Bed
        self.register(ItemDef {
            id: ItemId::OAK_STAIRS,
            name: "blockgame:oak_stairs",
            display_name: "Oak Stairs",
            max_stack: 64,
            placeable_block: Some(BlockId::OAK_STAIRS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 335,
        });

        self.register(ItemDef {
            id: ItemId::COBBLESTONE_STAIRS,
            name: "blockgame:cobblestone_stairs",
            display_name: "Cobblestone Stairs",
            max_stack: 64,
            placeable_block: Some(BlockId::COBBLESTONE_STAIRS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 336,
        });

        self.register(ItemDef {
            id: ItemId::BRICK_STAIRS,
            name: "blockgame:brick_stairs",
            display_name: "Brick Stairs",
            max_stack: 64,
            placeable_block: Some(BlockId::BRICK_STAIRS),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 337,
        });

        self.register(ItemDef {
            id: ItemId::OAK_FENCE,
            name: "blockgame:oak_fence",
            display_name: "Oak Fence",
            max_stack: 64,
            placeable_block: Some(BlockId::OAK_FENCE),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 338,
        });

        self.register(ItemDef {
            id: ItemId::OAK_DOOR,
            name: "blockgame:oak_door",
            display_name: "Oak Door",
            max_stack: 64,
            placeable_block: Some(BlockId::OAK_DOOR_CLOSED),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 316,
        });

        self.register(ItemDef {
            id: ItemId::BED,
            name: "blockgame:bed",
            display_name: "Red Bed",
            max_stack: 1,
            placeable_block: Some(BlockId::BED),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 332,
        });

        // Flowers
        let flower_defs = [
            (ItemId::POPPY, BlockId::POPPY, "blockgame:poppy", "Poppy", 319),
            (ItemId::DANDELION, BlockId::DANDELION, "blockgame:dandelion", "Dandelion", 320),
            (ItemId::BLUE_ORCHID, BlockId::BLUE_ORCHID, "blockgame:blue_orchid", "Blue Orchid", 321),
            (ItemId::ALLIUM, BlockId::ALLIUM, "blockgame:allium", "Allium", 322),
            (ItemId::ORANGE_TULIP, BlockId::ORANGE_TULIP, "blockgame:orange_tulip", "Orange Tulip", 323),
            (ItemId::PINK_TULIP, BlockId::PINK_TULIP, "blockgame:pink_tulip", "Pink Tulip", 324),
            (ItemId::OXEYE_DAISY, BlockId::OXEYE_DAISY, "blockgame:oxeye_daisy", "Oxeye Daisy", 325),
        ];
        for (id, block_id, name, display_name, icon_tile) in flower_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block: Some(block_id),
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Pumpkins and Melons
        self.register(ItemDef {
            id: ItemId::PUMPKIN,
            name: "blockgame:pumpkin",
            display_name: "Pumpkin",
            max_stack: 64,
            placeable_block: Some(BlockId::PUMPKIN),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 326,
        });

        self.register(ItemDef {
            id: ItemId::JACK_O_LANTERN,
            name: "blockgame:jack_o_lantern",
            display_name: "Jack o'Lantern",
            max_stack: 64,
            placeable_block: Some(BlockId::JACK_O_LANTERN),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 328,
        });

        self.register(ItemDef {
            id: ItemId::MELON,
            name: "blockgame:melon",
            display_name: "Melon",
            max_stack: 64,
            placeable_block: Some(BlockId::MELON),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 329,
        });

        self.register(ItemDef {
            id: ItemId::MELON_SLICE,
            name: "blockgame:melon_slice",
            display_name: "Melon Slice",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 331,
        });
        self.register_food(ItemId::MELON_SLICE, FoodDef::new(2, 1.2));

        self.register(ItemDef {
            id: ItemId::BOW,
            name: "blockgame:bow",
            display_name: "Bow",
            max_stack: 1,
            placeable_block: None,
            durability: Some(384),
            tool_type: None,
            tool_tier: None,
            icon_tile: 344,
        });

        self.register(ItemDef {
            id: ItemId::ARROW,
            name: "blockgame:arrow",
            display_name: "Arrow",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 345,
        });

        self.register(ItemDef {
            id: ItemId::STRING,
            name: "blockgame:string",
            display_name: "String",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 346,
        });

        self.register(ItemDef {
            id: ItemId::BONE,
            name: "blockgame:bone",
            display_name: "Bone",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 347,
        });

        self.register(ItemDef {
            id: ItemId::ROTTEN_FLESH,
            name: "blockgame:rotten_flesh",
            display_name: "Rotten Flesh",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 348,
        });
        self.register_food(
            ItemId::ROTTEN_FLESH,
            FoodDef::new(4, 0.8).with_effect(FoodEffectKind::FoodPoisoning, 0.80, 20.0),
        );

        let armor_defs: [(ItemId, &'static str, &'static str, u16, ArmorSlot, u8, u32); 16] = [
            (ItemId::LEATHER_HELMET, "blockgame:leather_helmet", "Leather Cap", 55, ArmorSlot::Helmet, 1, 356),
            (ItemId::LEATHER_CHESTPLATE, "blockgame:leather_chestplate", "Leather Tunic", 80, ArmorSlot::Chestplate, 3, 357),
            (ItemId::LEATHER_LEGGINGS, "blockgame:leather_leggings", "Leather Pants", 75, ArmorSlot::Leggings, 2, 358),
            (ItemId::LEATHER_BOOTS, "blockgame:leather_boots", "Leather Boots", 65, ArmorSlot::Boots, 1, 359),
            (ItemId::IRON_HELMET, "blockgame:iron_helmet", "Iron Helmet", 165, ArmorSlot::Helmet, 2, 360),
            (ItemId::IRON_CHESTPLATE, "blockgame:iron_chestplate", "Iron Chestplate", 240, ArmorSlot::Chestplate, 6, 361),
            (ItemId::IRON_LEGGINGS, "blockgame:iron_leggings", "Iron Leggings", 225, ArmorSlot::Leggings, 5, 362),
            (ItemId::IRON_BOOTS, "blockgame:iron_boots", "Iron Boots", 195, ArmorSlot::Boots, 2, 363),
            (ItemId::GOLD_HELMET, "blockgame:gold_helmet", "Golden Helmet", 77, ArmorSlot::Helmet, 2, 364),
            (ItemId::GOLD_CHESTPLATE, "blockgame:gold_chestplate", "Golden Chestplate", 112, ArmorSlot::Chestplate, 5, 365),
            (ItemId::GOLD_LEGGINGS, "blockgame:gold_leggings", "Golden Leggings", 105, ArmorSlot::Leggings, 3, 366),
            (ItemId::GOLD_BOOTS, "blockgame:gold_boots", "Golden Boots", 91, ArmorSlot::Boots, 1, 367),
            (ItemId::DIAMOND_HELMET, "blockgame:diamond_helmet", "Diamond Helmet", 363, ArmorSlot::Helmet, 3, 368),
            (ItemId::DIAMOND_CHESTPLATE, "blockgame:diamond_chestplate", "Diamond Chestplate", 528, ArmorSlot::Chestplate, 8, 369),
            (ItemId::DIAMOND_LEGGINGS, "blockgame:diamond_leggings", "Diamond Leggings", 495, ArmorSlot::Leggings, 6, 370),
            (ItemId::DIAMOND_BOOTS, "blockgame:diamond_boots", "Diamond Boots", 429, ArmorSlot::Boots, 3, 371),
        ];

        for (id, name, display_name, dur, slot, defense, icon_tile) in armor_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
            self.register_armor(id, slot, defense);
        }

        // Fishing Rods (tiles 384..=387)
        let rod_defs = [
            (ItemId::WOODEN_FISHING_ROD, "blockgame:wooden_fishing_rod", "Wooden Fishing Rod", 64, 384),
            (ItemId::COPPER_FISHING_ROD, "blockgame:copper_fishing_rod", "Copper Fishing Rod", 150, 385),
            (ItemId::IRON_FISHING_ROD, "blockgame:iron_fishing_rod", "Iron Fishing Rod", 280, 386),
            (ItemId::DIAMOND_FISHING_ROD, "blockgame:diamond_fishing_rod", "Diamond Fishing Rod", 650, 387),
        ];
        for (id, name, display_name, dur, icon_tile) in rod_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Fishing Lines (tiles 388..=390)
        let line_defs = [
            (ItemId::BRAIDED_LINE, "blockgame:braided_line", "Braided Fishing Line", 50, 388),
            (ItemId::FLUOROCARBON_LINE, "blockgame:fluorocarbon_line", "Fluorocarbon Line", 75, 389),
            (ItemId::TITANIUM_LINE, "blockgame:titanium_line", "Titanium-Weave Line", 120, 390),
        ];
        for (id, name, display_name, dur, icon_tile) in line_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Fishing Lures (tiles 391..=394)
        let lure_defs = [
            (ItemId::CORK_BOBBER, "blockgame:cork_bobber", "Cork Bobber Lure", 40, 391),
            (ItemId::SPINNER_LURE, "blockgame:spinner_lure", "Copper Spinner Lure", 45, 392),
            (ItemId::BARBED_HOOK, "blockgame:barbed_hook", "Barbed Hook Lure", 50, 393),
            (ItemId::TREASURE_LURE, "blockgame:treasure_lure", "Gold Treasure Magnet", 40, 394),
        ];
        for (id, name, display_name, dur, icon_tile) in lure_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Fishing Bait (tile 395)
        self.register(ItemDef {
            id: ItemId::FISHING_BAIT,
            name: "blockgame:fishing_bait",
            display_name: "Fishing Bait",
            max_stack: 64,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 395,
        });

        // Raw Fish & Edible/Crafted Fish Foods (tiles 396..=409)
        let fish_food_defs = [
            (ItemId::RAW_CARP, "blockgame:raw_carp", "Raw Carp", 396, Some((2, 1.2))),
            (ItemId::RAW_SARDINE, "blockgame:raw_sardine", "Raw Sardine", 397, Some((2, 1.0))),
            (ItemId::RAW_PERCH, "blockgame:raw_perch", "Raw Perch", 398, Some((2, 1.2))),
            (ItemId::RAW_TROUT, "blockgame:raw_trout", "Raw Rainbow Trout", 399, Some((3, 1.8))),
            (ItemId::RAW_BASS, "blockgame:raw_bass", "Raw Bass", 400, Some((3, 1.8))),
            (ItemId::RAW_SALMON, "blockgame:raw_salmon", "Raw Salmon", 401, Some((3, 2.0))),
            (ItemId::RAW_CATFISH, "blockgame:raw_catfish", "Raw Catfish", 402, Some((3, 2.0))),
            (ItemId::PUFFERFISH, "blockgame:pufferfish", "Pufferfish", 403, Some((1, 0.4))),
            (ItemId::RAW_TUNA, "blockgame:raw_tuna", "Raw Bluefin Tuna", 404, Some((4, 2.4))),
            (ItemId::GOLDEN_KOI, "blockgame:golden_koi", "Golden Koi", 405, Some((5, 4.0))),
            (ItemId::COOKED_FISH, "blockgame:cooked_fish", "Cooked Fish Fillet", 406, Some((6, 7.2))),
            (ItemId::COOKED_SALMON, "blockgame:cooked_salmon", "Cooked Salmon", 407, Some((8, 11.2))),
            (ItemId::SUSHI_ROLL, "blockgame:sushi_roll", "Sushi Roll", 408, Some((7, 9.6))),
            (ItemId::FISH_STEW, "blockgame:fish_stew", "Fisherman's Stew", 409, Some((10, 14.0))),
        ];
        for (id, name, display_name, icon_tile, food) in fish_food_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
            if let Some((fp, sat)) = food {
                let mut def = FoodDef::new(fp, sat);
                if id == ItemId::PUFFERFISH {
                    def = def.with_effect(FoodEffectKind::Poison, 1.0, 10.0);
                }
                self.register_food(id, def);
            }
        }

        // Power & Electric Machine items (Issues #14 & #15)
        let power_item_defs = [
            (ItemId::COPPER_WIRE, None, "blockgame:copper_wire", "Copper Wire", 426),
            (ItemId::POWER_CORE, None, "blockgame:power_core", "Power Core", 427),
            (ItemId::COPPER_CABLE, Some(BlockId::COPPER_CABLE), "blockgame:copper_cable", "Copper Cable (200 EU/s)", 410),
            (ItemId::GOLD_CABLE, Some(BlockId::GOLD_CABLE), "blockgame:gold_cable", "Gold Cable (1,000 EU/s)", 434),
            (ItemId::DIAMOND_CABLE, Some(BlockId::DIAMOND_CABLE), "blockgame:diamond_cable", "Diamond Cable (5,000 EU/s)", 435),
            (ItemId::COAL_GENERATOR, Some(BlockId::COAL_GENERATOR), "blockgame:coal_generator", "Coal Generator (+40 EU/s)", 411),
            (ItemId::SOLAR_PANEL, Some(BlockId::SOLAR_PANEL), "blockgame:solar_panel", "Solar Panel (+25 EU/s)", 413),
            (ItemId::WIND_TURBINE, Some(BlockId::WIND_TURBINE), "blockgame:wind_turbine", "Wind Turbine (+15-60 EU/s)", 415),
            (ItemId::BATTERY_BOX, Some(BlockId::BATTERY_BOX), "blockgame:battery_box", "Battery Box (20,000 EU)", 419),
            (ItemId::ELECTRIC_FURNACE, Some(BlockId::ELECTRIC_FURNACE), "blockgame:electric_furnace", "Electric Furnace (-30 EU/s)", 421),
            (ItemId::ELECTRIC_LAMP, Some(BlockId::ELECTRIC_LAMP), "blockgame:electric_lamp", "Electric Lamp (-5 EU/s)", 424),
            (ItemId::ADVANCED_SOLAR_PANEL, Some(BlockId::ADVANCED_SOLAR_PANEL), "blockgame:advanced_solar_panel", "Advanced Solar Panel (+75 EU/s)", 428),
            (ItemId::ADVANCED_BATTERY_BOX, Some(BlockId::ADVANCED_BATTERY_BOX), "blockgame:advanced_battery_box", "Advanced Battery Box (100,000 EU)", 430),
            (ItemId::QUANTUM_BATTERY_BOX, Some(BlockId::QUANTUM_BATTERY_BOX), "blockgame:quantum_battery_box", "Quantum Battery Box (500,000 EU)", 432),
            // Item Storage & Sorting System (Issue #13)
            (ItemId::IRON_CHEST, Some(BlockId::IRON_CHEST), "blockgame:iron_chest", "Iron Chest (45 Slots)", 436),
            (ItemId::GOLD_CHEST, Some(BlockId::GOLD_CHEST), "blockgame:gold_chest", "Gold Chest (63 Slots)", 438),
            (ItemId::DIAMOND_CHEST, Some(BlockId::DIAMOND_CHEST), "blockgame:diamond_chest", "Diamond Chest (81 Slots)", 440),
            (ItemId::STORAGE_TERMINAL, Some(BlockId::STORAGE_TERMINAL), "blockgame:storage_terminal", "Storage Terminal (-15+ EU/s)", 442),
            (ItemId::STORAGE_CABLE, Some(BlockId::STORAGE_CABLE), "blockgame:storage_cable", "Storage Cable (-1 EU/s)", 444),
        ];
        for (id, placeable_block, name, display_name, icon_tile) in power_item_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Wearable Backpacks (max_stack = 1)
        let backpack_defs = [
            (ItemId::LEATHER_BACKPACK, "blockgame:leather_backpack", "Leather Backpack (18 Slots)", 445),
            (ItemId::IRON_BACKPACK, "blockgame:iron_backpack", "Iron Backpack (27 Slots)", 446),
            (ItemId::DIAMOND_BACKPACK, "blockgame:diamond_backpack", "Diamond Backpack (36 Slots)", 447),
        ];
        for (id, name, display_name, icon_tile) in backpack_defs {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Biome Boss Temple placeable blocks (Issue #9)
        let temple_block_items = [
            (ItemId::TEMPLE_STONE, Some(BlockId::TEMPLE_STONE), "blockgame:temple_stone", "Temple Stone", 448),
            (ItemId::TEMPLE_SANDSTONE, Some(BlockId::TEMPLE_SANDSTONE), "blockgame:temple_sandstone", "Temple Sandstone", 449),
            (ItemId::TEMPLE_ICE_BRICK, Some(BlockId::TEMPLE_ICE_BRICK), "blockgame:temple_ice_brick", "Temple Ice Brick", 450),
            (ItemId::TEMPLE_MOSS_BRICK, Some(BlockId::TEMPLE_MOSS_BRICK), "blockgame:temple_moss_brick", "Temple Moss Brick", 451),
            (ItemId::TEMPLE_OBSIDIAN, Some(BlockId::TEMPLE_OBSIDIAN), "blockgame:temple_obsidian", "Temple Obsidian", 452),
            (ItemId::TEMPLE_BRAZIER, Some(BlockId::TEMPLE_BRAZIER), "blockgame:temple_brazier", "Temple Brazier", 456),
        ];
        for (id, placeable_block, name, display_name, icon_tile) in temple_block_items {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 64,
                placeable_block,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Unique Boss Weapons (Issue #9)
        let boss_weapons = [
            (ItemId::SUNBLADE, "blockgame:sunblade", "Sunblade (11 DMG)", 2048, 457),
            (ItemId::FROSTBLADE, "blockgame:frostblade", "Frostblade (11.5 DMG)", 2048, 458),
            (ItemId::VERDANT_GREATBLADE, "blockgame:verdant_greatblade", "Verdant Greatblade (10.5 DMG, +15% Lifesteal)", 2048, 459),
            (ItemId::SHADOWFANG_BLADE, "blockgame:shadowfang_blade", "Shadowfang Blade (12.5 DMG, +8% Lifesteal)", 2048, 460),
        ];
        for (id, name, display_name, dur, icon_tile) in boss_weapons {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: Some(ToolType::Sword),
                tool_tier: Some(ToolTier::Diamond),
                icon_tile,
            });
        }

        // Unique Boss Armor Set (Issue #9: 4+9+7+4 = 24 raw, capped at 20 = 80% max DR)
        let boss_armor = [
            (ItemId::PHARAOH_CROWN, "blockgame:pharaoh_crown", "Pharaoh's Crown", 600, ArmorSlot::Helmet, 4, 461),
            (ItemId::GLACIAL_CHESTPLATE, "blockgame:glacial_chestplate", "Glacial Chestplate", 850, ArmorSlot::Chestplate, 9, 462),
            (ItemId::GOLEM_GREAVES, "blockgame:golem_greaves", "Golem Greaves", 780, ArmorSlot::Leggings, 7, 463),
            (ItemId::LICH_BOOTS, "blockgame:lich_boots", "Lich's Boots", 680, ArmorSlot::Boots, 4, 464),
        ];
        for (id, name, display_name, dur, slot, defense, icon_tile) in boss_armor {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 1,
                placeable_block: None,
                durability: Some(dur),
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
            self.register_armor(id, slot, defense);
        }

        // Boss Trophy Cores (Issue #9)
        let boss_cores = [
            (ItemId::SUN_CORE, "blockgame:sun_core", "Sun Core", 465),
            (ItemId::FROST_HEART, "blockgame:frost_heart", "Frost Heart", 466),
            (ItemId::ANCIENT_VINE_CORE, "blockgame:ancient_vine_core", "Ancient Vine Core", 467),
            (ItemId::SOUL_PHYLACTERY, "blockgame:soul_phylactery", "Soul Phylactery", 468),
        ];
        for (id, name, display_name, icon_tile) in boss_cores {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack: 16,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // Farming, Cooking, and Hunger Revamp (Issue #12)
        let farming_non_food_items: [(ItemId, &'static str, &'static str, u16, Option<BlockId>, u32); 10] = [
            (ItemId::TOMATO_SEEDS, "blockgame:tomato_seeds", "Tomato Seeds", 64, None, 542),
            (ItemId::MELON_SEEDS, "blockgame:melon_seeds", "Melon Seeds", 64, None, 543),
            (ItemId::PUMPKIN_SEEDS, "blockgame:pumpkin_seeds", "Pumpkin Seeds", 64, None, 544),
            (ItemId::SUGAR_CANE, "blockgame:sugar_cane", "Sugar Cane", 64, Some(BlockId::SUGAR_CANE), 545),
            (ItemId::COMPOST, "blockgame:compost", "Compost", 64, None, 546),
            (ItemId::BONE_FERTILIZER, "blockgame:bone_fertilizer", "Bone Fertilizer", 64, None, 547),
            (ItemId::SUGAR, "blockgame:sugar", "Sugar", 64, None, 551),
            (ItemId::EGG, "blockgame:egg", "Egg", 16, None, 552),
            (ItemId::MILK_BUCKET, "blockgame:milk_bucket", "Milk Bucket", 1, None, 553),
            (ItemId::COOKING_POT, "blockgame:cooking_pot", "Cooking Pot", 64, Some(BlockId::COOKING_POT), 566),
        ];
        for (id, name, display_name, max_stack, placeable_block, icon_tile) in farming_non_food_items {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack,
                placeable_block,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }

        // New Foods across 4 clear hunger/saturation tiers + Herbal Tea cure
        let new_foods: [(ItemId, &'static str, &'static str, u16, u32, FoodDef); 15] = [
            // Tier 1: Raw crops & raw meat
            (ItemId::CARROT, "blockgame:carrot", "Carrot", 64, 548, FoodDef::new(3, 3.6)),
            (ItemId::POTATO, "blockgame:potato", "Potato", 64, 549, FoodDef::new(1, 0.6)),
            (ItemId::TOMATO, "blockgame:tomato", "Tomato", 64, 550, FoodDef::new(2, 2.0)),
            (ItemId::RAW_MUTTON, "blockgame:raw_mutton", "Raw Mutton", 64, 554, FoodDef::new(2, 1.2)),
            // Tier 2: Simple furnace / crafted foods
            (ItemId::BAKED_POTATO, "blockgame:baked_potato", "Baked Potato", 64, 555, FoodDef::new(5, 6.0)),
            (ItemId::COOKED_MUTTON, "blockgame:cooked_mutton", "Cooked Mutton", 64, 556, FoodDef::new(6, 9.6)),
            (ItemId::COOKIE, "blockgame:cookie", "Cookie", 64, 557, FoodDef::new(2, 1.2).with_eat_duration(0.8)),
            (ItemId::PUMPKIN_PIE, "blockgame:pumpkin_pie", "Pumpkin Pie", 64, 558, FoodDef::new(8, 7.2)),
            // Tier 3-4: Cooking Pot meals (stack to 16, no Bowls required, high hunger + high saturation)
            (ItemId::TOMATO_SOUP, "blockgame:tomato_soup", "Tomato Soup", 16, 559, FoodDef::new(8, 10.4)),
            (ItemId::VEGETABLE_STEW, "blockgame:vegetable_stew", "Vegetable Stew", 16, 560, FoodDef::new(9, 12.0)),
            (ItemId::HEARTY_MEAT_STEW, "blockgame:hearty_meat_stew", "Hearty Meat Stew", 16, 561, FoodDef::new(12, 16.0)),
            (ItemId::OMELETTE, "blockgame:omelette", "Omelette", 16, 562, FoodDef::new(9, 12.6)),
            (ItemId::FRUIT_SALAD, "blockgame:fruit_salad", "Fruit Salad", 16, 563, FoodDef::new(7, 8.4)),
            (ItemId::SWEET_CAKE, "blockgame:sweet_cake", "Sweet Cake", 16, 564, FoodDef::new(10, 12.0)),
            // Thematic status-effect cure brewed in the Cooking Pot
            (
                ItemId::HERBAL_TEA,
                "blockgame:herbal_tea",
                "Herbal Tea",
                16,
                565,
                FoodDef::new(2, 2.0).with_always_edible().with_cure(),
            ),
        ];
        for (id, name, display_name, max_stack, icon_tile, food) in new_foods {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
            self.register_food(id, food);
        }

        // NPC Villages & Trading (Issue #11)
        self.register(ItemDef {
            id: ItemId::GOLD_COIN,
            name: "blockgame:gold_coin",
            display_name: "Gold Coin",
            max_stack: 1000,
            placeable_block: None,
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 567,
        });
        self.register(ItemDef {
            id: ItemId::MINERS_LANTERN,
            name: "blockgame:miners_lantern",
            display_name: "Miner's Lantern",
            max_stack: 16,
            placeable_block: Some(BlockId::SEA_LANTERN),
            durability: None,
            tool_type: None,
            tool_tier: None,
            icon_tile: 568,
        });
        self.register(ItemDef {
            id: ItemId::VANGUARD_BLADE,
            name: "blockgame:vanguard_blade",
            display_name: "Vanguard Blade",
            max_stack: 1,
            placeable_block: None,
            durability: Some(2000),
            tool_type: Some(ToolType::Sword),
            tool_tier: Some(ToolTier::Diamond),
            icon_tile: 569,
        });
        self.register(ItemDef {
            id: ItemId::MASTER_ANGLER_LURE,
            name: "blockgame:master_angler_lure",
            display_name: "Master Angler's Lure",
            max_stack: 1,
            placeable_block: None,
            durability: Some(120),
            tool_type: None,
            tool_tier: None,
            icon_tile: 570,
        });

        let village_foods: [(ItemId, &'static str, &'static str, u16, u32, FoodDef); 3] = [
            (
                ItemId::GOLDEN_APPLE_PIE,
                "blockgame:golden_apple_pie",
                "Golden Apple Pie",
                16,
                571,
                FoodDef::new(14, 18.0)
                    .with_always_edible()
                    .with_cure()
                    .with_effect(FoodEffectKind::Regeneration, 1.0, 30.0),
            ),
            (
                ItemId::ROYAL_FEAST,
                "blockgame:royal_feast",
                "Royal Feast",
                16,
                572,
                FoodDef::new(18, 22.0)
                    .with_always_edible()
                    .with_effect(FoodEffectKind::MiningHaste, 1.0, 45.0),
            ),
            (
                ItemId::SPICED_ELIXIR,
                "blockgame:spiced_elixir",
                "Spiced Elixir",
                16,
                573,
                FoodDef::new(6, 12.0)
                    .with_eat_duration(0.8)
                    .with_always_edible()
                    .with_cure()
                    .with_effect(FoodEffectKind::Speed, 1.0, 45.0),
            ),
        ];
        for (id, name, display_name, max_stack, icon_tile, food) in village_foods {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack,
                placeable_block: None,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
            self.register_food(id, food);
        }

        // Underground & Caves items (Issue #23)
        let cave_items: [(ItemId, Option<BlockId>, &'static str, &'static str, u16, u32); 5] = [
            (
                ItemId::DEEPSTONE,
                Some(BlockId::DEEPSTONE),
                "blockgame:deepstone",
                "Deepstone",
                64,
                574,
            ),
            (
                ItemId::DRIPSTONE,
                Some(BlockId::DRIPSTONE),
                "blockgame:dripstone",
                "Dripstone",
                64,
                575,
            ),
            (
                ItemId::GLOW_CRYSTAL,
                Some(BlockId::GLOW_CRYSTAL),
                "blockgame:glow_crystal",
                "Glow Crystal",
                64,
                577,
            ),
            (
                ItemId::OBSIDIAN,
                Some(BlockId::OBSIDIAN),
                "blockgame:obsidian",
                "Obsidian",
                64,
                578,
            ),
            (
                ItemId::LAVA_BUCKET,
                None,
                "blockgame:lava_bucket",
                "Lava Bucket",
                1,
                580,
            ),
        ];
        for (id, placeable_block, name, display_name, max_stack, icon_tile) in cave_items {
            self.register(ItemDef {
                id,
                name,
                display_name,
                max_stack,
                placeable_block,
                durability: None,
                tool_type: None,
                tool_tier: None,
                icon_tile,
            });
        }
    }
}

/// Standard Minecraft inventory: 9 hotbar slots + 27 backpack slots = 36 slots total.
#[derive(Debug, Clone, PartialEq)]
pub struct Inventory {
    pub slots: Vec<ItemSlot>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(36)
    }
}

impl Inventory {
    pub const HOTBAR_COUNT: usize = 9;
    pub const TOTAL_COUNT: usize = 36;

    pub fn new(size: usize) -> Self {
        Self {
            slots: vec![ItemSlot::EMPTY; size],
        }
    }

    /// Pre-populates a starter survival hotbar.
    pub fn starter_inventory() -> Self {
        let mut inv = Self::new(Self::TOTAL_COUNT);
        inv.slots[0] = ItemSlot::new(ItemId::STONE_PICKAXE, 1);
        inv.slots[1] = ItemSlot::new(ItemId::WOOD_LOG, 16);
        inv.slots[2] = ItemSlot::new(ItemId::COBBLESTONE, 32);
        inv.slots[3] = ItemSlot::new(ItemId::TORCH, 16);
        inv.slots[4] = ItemSlot::new(ItemId::DIRT, 64);
        inv.slots[5] = ItemSlot::new(ItemId::PLANKS, 32);
        inv.slots[6] = ItemSlot::new(ItemId::STICK, 8);
        inv.slots[7] = ItemSlot::new(ItemId::GLASS, 16);
        inv.slots[8] = ItemSlot::new(ItemId::CRAFTING_TABLE, 1);
        inv.slots[9] = ItemSlot::new(ItemId::IRON_HOE, 1);
        inv.slots[10] = ItemSlot::new(ItemId::WHEAT_SEEDS, 16);
        inv.slots[11] = ItemSlot::new(ItemId::BREAD, 4);
        inv.slots[12] = ItemSlot::new(ItemId::BUCKET, 1);
        inv.slots[13] = ItemSlot::new(ItemId::WATER_BUCKET, 1);
        inv
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    #[inline]
    pub fn get(&self, idx: usize) -> ItemSlot {
        self.slots.get(idx).copied().unwrap_or(ItemSlot::EMPTY)
    }

    #[inline]
    pub fn set(&mut self, idx: usize, slot: ItemSlot) {
        if idx < self.slots.len() {
            self.slots[idx] = slot;
        }
    }

    /// Counts total items matching `item_id` across all slots.
    pub fn count_item(&self, item_id: ItemId) -> u32 {
        self.slots
            .iter()
            .filter(|s| !s.is_empty() && s.item_id == item_id)
            .map(|s| s.count as u32)
            .sum()
    }

    /// Removes up to `amount` items matching `item_id` across inventory slots.
    /// Returns the actual number of items removed.
    pub fn remove_item(&mut self, item_id: ItemId, mut amount: u32) -> u32 {
        let mut removed = 0u32;
        for s in self.slots.iter_mut() {
            if amount == 0 {
                break;
            }
            if !s.is_empty() && s.item_id == item_id {
                let take = (s.count as u32).min(amount) as u16;
                s.take(take);
                removed += take as u32;
                amount -= take as u32;
            }
        }
        removed
    }

    /// Attempts to add items into inventory. Fills existing stacks first, then empty slots.
    /// Returns any remaining items that could not fit.
    pub fn add_item(&mut self, mut slot: ItemSlot, registry: &ItemRegistry) -> Option<ItemSlot> {
        if slot.is_empty() {
            return None;
        }

        let max_stack = registry.max_stack_size(slot.item_id);

        // 1. Fill existing matching stacks
        for s in self.slots.iter_mut() {
            if s.can_stack_with(&slot) && s.count < max_stack {
                let space = max_stack - s.count;
                let add_amt = space.min(slot.count);
                s.count += add_amt;
                slot.count -= add_amt;
                if slot.count == 0 {
                    return None;
                }
            }
        }

        // 2. Place into first empty slots
        for s in self.slots.iter_mut() {
            if s.is_empty() {
                let add_amt = max_stack.min(slot.count);
                *s = ItemSlot {
                    item_id: slot.item_id,
                    count: add_amt,
                    durability: slot.durability,
                    enhancement: slot.enhancement,
                };
                slot.count -= add_amt;
                if slot.count == 0 {
                    return None;
                }
            }
        }

        Some(slot)
    }

    /// Swaps the contents of slot `a` and slot `b`.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() && a != b {
            self.slots.swap(a, b);
        }
    }

    /// Transfers items between two slots, stacking if identical or swapping if different.
    pub fn click_slot(
        &mut self,
        slot_idx: usize,
        held_cursor: &mut ItemSlot,
        split_half: bool,
        registry: &ItemRegistry,
    ) {
        if slot_idx >= self.slots.len() {
            return;
        }

        let slot = &mut self.slots[slot_idx];

        if held_cursor.is_empty() {
            // Picking up from slot
            if !slot.is_empty() {
                if split_half {
                    *held_cursor = slot.split_half();
                } else {
                    *held_cursor = *slot;
                    *slot = ItemSlot::EMPTY;
                }
            }
        } else {
            // Placing held item into slot
            if slot.is_empty() {
                if split_half {
                    // Place 1 single item
                    *slot = held_cursor.take(1);
                } else {
                    // Place entire held stack
                    let max_stack = registry.max_stack_size(held_cursor.item_id);
                    let transfer = max_stack.min(held_cursor.count);
                    *slot = held_cursor.take(transfer);
                }
            } else if slot.can_stack_with(held_cursor) {
                let max_stack = registry.max_stack_size(slot.item_id);
                if max_stack <= 1 {
                    if !split_half {
                        std::mem::swap(slot, held_cursor);
                    }
                } else if slot.count < max_stack {
                    if split_half {
                        // Place 1 item into stack
                        slot.count += 1;
                        held_cursor.take(1);
                    } else {
                        // Fill stack
                        let space = max_stack - slot.count;
                        let transfer = space.min(held_cursor.count);
                        slot.count += transfer;
                        held_cursor.take(transfer);
                    }
                }
            } else {
                // Different items: swap held with slot
                if !split_half {
                    std::mem::swap(slot, held_cursor);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_slot_stacking_and_taking() {
        let mut slot = ItemSlot::new(ItemId::COBBLESTONE, 10);
        assert_eq!(slot.count, 10);

        let taken = slot.take(4);
        assert_eq!(taken.count, 4);
        assert_eq!(slot.count, 6);

        let half = slot.split_half();
        assert_eq!(half.count, 3);
        assert_eq!(slot.count, 3);
    }

    #[test]
    fn test_inventory_add_and_stack() {
        let registry = ItemRegistry::new();
        let mut inv = Inventory::new(9);

        // Add 40 logs
        let overflow = inv.add_item(ItemSlot::new(ItemId::WOOD_LOG, 40), &registry);
        assert!(overflow.is_none());
        assert_eq!(inv.get(0).count, 40);

        // Add 30 more logs (should top up slot 0 to 64 and place 6 in slot 1)
        let overflow = inv.add_item(ItemSlot::new(ItemId::WOOD_LOG, 30), &registry);
        assert!(overflow.is_none());
        assert_eq!(inv.get(0).count, 64);
        assert_eq!(inv.get(1).count, 6);
    }

    #[test]
    fn test_tool_durability_tiers_and_breakage() {
        let registry = ItemRegistry::new();

        // 1. Verify strict durability order requested: Gold < Wood < Stone < Copper < Iron < Diamond
        let gold_dur = registry.get(ItemId::GOLD_PICKAXE).unwrap().durability.unwrap();
        let wood_dur = registry.get(ItemId::WOODEN_PICKAXE).unwrap().durability.unwrap();
        let stone_dur = registry.get(ItemId::STONE_PICKAXE).unwrap().durability.unwrap();
        let copper_dur = registry.get(ItemId::COPPER_PICKAXE).unwrap().durability.unwrap();
        let iron_dur = registry.get(ItemId::IRON_PICKAXE).unwrap().durability.unwrap();
        let diamond_dur = registry.get(ItemId::DIAMOND_PICKAXE).unwrap().durability.unwrap();

        assert_eq!(gold_dur, 32);
        assert_eq!(wood_dur, 59);
        assert_eq!(stone_dur, 131);
        assert_eq!(copper_dur, 190);
        assert_eq!(iron_dur, 250);
        assert_eq!(diamond_dur, 1561);

        assert!(gold_dur < wood_dur);
        assert!(wood_dur < stone_dur);
        assert!(stone_dur < copper_dur);
        assert!(copper_dur < iron_dur);
        assert!(iron_dur < diamond_dur);

        // 2. Verify all tool types (Pickaxe, Axe, Shovel, Sword, Hoe) have matching durability per tier
        for (axe, shovel, sword, hoe, expected) in [
            (ItemId::GOLD_AXE, ItemId::GOLD_SHOVEL, ItemId::GOLD_SWORD, ItemId::GOLD_HOE, 32),
            (ItemId::WOODEN_AXE, ItemId::WOODEN_SHOVEL, ItemId::WOODEN_SWORD, ItemId::WOODEN_HOE, 59),
            (ItemId::STONE_AXE, ItemId::STONE_SHOVEL, ItemId::STONE_SWORD, ItemId::STONE_HOE, 131),
            (ItemId::COPPER_AXE, ItemId::COPPER_SHOVEL, ItemId::COPPER_SWORD, ItemId::COPPER_HOE, 190),
            (ItemId::IRON_AXE, ItemId::IRON_SHOVEL, ItemId::IRON_SWORD, ItemId::IRON_HOE, 250),
            (ItemId::DIAMOND_AXE, ItemId::DIAMOND_SHOVEL, ItemId::DIAMOND_SWORD, ItemId::DIAMOND_HOE, 1561),
        ] {
            assert_eq!(registry.get(axe).unwrap().durability, Some(expected));
            assert_eq!(registry.get(shovel).unwrap().durability, Some(expected));
            assert_eq!(registry.get(sword).unwrap().durability, Some(expected));
            assert_eq!(registry.get(hoe).unwrap().durability, Some(expected));
        }

        // 3. Test tool damage, fraction, and breakage
        let mut gold_pick = ItemSlot::new(ItemId::GOLD_PICKAXE, 1);
        assert_eq!(gold_pick.remaining_durability(32), 32);
        assert!(!gold_pick.is_damaged(32));
        assert_eq!(gold_pick.durability_fraction(32), 1.0);

        // Apply 1 damage
        let broke = gold_pick.damage_tool(1, 32);
        assert!(!broke);
        assert!(gold_pick.is_damaged(32));
        assert_eq!(gold_pick.remaining_durability(32), 31);
        assert_eq!(gold_pick.durability, 31);

        // Apply 30 more damage
        let broke = gold_pick.damage_tool(30, 32);
        assert!(!broke);
        assert_eq!(gold_pick.remaining_durability(32), 1);

        // Final hit breaks the tool
        let broke = gold_pick.damage_tool(1, 32);
        assert!(broke);
        assert!(gold_pick.is_empty());
    }

    #[test]
    fn test_armor_defense_and_durability() {
        let registry = ItemRegistry::new();
        let mut armor = ArmorEquipment::new();
        assert_eq!(armor.total_defense(&registry), 0);
        assert!((armor.damage_multiplier(&registry) - 1.0).abs() < 1e-5);

        // Full Diamond Armor = 3 + 8 + 6 + 3 = 20 defense -> 80% reduction (0.2x damage)
        armor.slots[ArmorSlot::Helmet.index()] = ItemSlot::new(ItemId::DIAMOND_HELMET, 1);
        armor.slots[ArmorSlot::Chestplate.index()] = ItemSlot::new(ItemId::DIAMOND_CHESTPLATE, 1);
        armor.slots[ArmorSlot::Leggings.index()] = ItemSlot::new(ItemId::DIAMOND_LEGGINGS, 1);
        armor.slots[ArmorSlot::Boots.index()] = ItemSlot::new(ItemId::DIAMOND_BOOTS, 1);
        assert_eq!(armor.total_defense(&registry), 20);
        assert!((armor.damage_multiplier(&registry) - 0.2).abs() < 1e-5);

        // Damage wears durability
        let broke = armor.damage_armor(8.0, &registry);
        assert!(!broke);
        assert_eq!(armor.slots[ArmorSlot::Helmet.index()].durability, 361);
    }
}
