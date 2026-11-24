use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use glam::{IVec3, Vec3};

use crate::{
    block::{BlockId, BlockRegistry},
    character::{
        BodyFrame, CharacterAppearance, EyeStyle, FacialHairStyle, HairStyle, PantsStyle, RgbColor,
        ShirtStyle, ShoeStyle,
    },
    item::{Enhancement, Inventory, ItemId, ItemRegistry, ItemSlot},
    monster::MonsterEntity,
    tool::ToolTier,
    world::World,
};

/// Coarse overworld grid spacing (in blocks) for deterministic Village placement.
pub const VILLAGE_GRID_SIZE: i32 = 384;
/// Percentage chance (0..=100) that an eligible Village grid cell spawns a Village.
pub const VILLAGE_SPAWN_CHANCE_PERCENT: u64 = 65;
/// Minimum horizontal distance from world origin `(0, 0)` before Villages may generate.
pub const VILLAGE_MIN_ORIGIN_DIST: i32 = 160;
/// Horizontal exclusion radius around any Boss Temple where Villages will not spawn.
pub const VILLAGE_TEMPLE_EXCLUSION_RADIUS: i32 = 320;
/// In-game ticks per full day/night cycle.
pub const TICKS_PER_DAY: u64 = 24_000;
/// Stock restock cadence: every 3 in-game days.
pub const VILLAGE_RESTOCK_TICKS: u64 = TICKS_PER_DAY * 3;
/// Maximum per-player Shop Tier on a Shopkeeper.
pub const MAX_SHOP_TIER: u8 = 5;
/// Minimum Shop Tier required to unlock the Enhance tab.
pub const ENHANCE_UNLOCK_TIER: u8 = 4;
/// Cumulative Gold Coins spent thresholds for Shop Tiers 1..=5.
pub const SHOP_TIER_THRESHOLDS: [u32; 5] = [0, 50, 150, 400, 900];
/// Key offset for Enhancement stock entries inside `ShopkeeperEntity::shared_stock`.
const ENHANCE_STOCK_KEY_OFFSET: u16 = 1000;

/// Deterministic unique Village ID from world seed and grid cell.
pub fn village_id_from_grid(seed: u32, gx: i32, gz: i32) -> u64 {
    let mut x = (seed as u64)
        .wrapping_add(0x517C_C1B7_2722_0A95)
        .wrapping_add((gx as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((gz as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VillageTheme {
    Plains = 0,
    Desert = 1,
    Taiga = 2,
    Savanna = 3,
}

impl VillageTheme {
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Plains),
            1 => Some(Self::Desert),
            2 => Some(Self::Taiga),
            3 => Some(Self::Savanna),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Plains => "Plains Village",
            Self::Desert => "Desert Oasis Village",
            Self::Taiga => "Taiga Timber Village",
            Self::Savanna => "Savanna Acacia Village",
        }
    }

    pub const fn wall_block(self) -> BlockId {
        match self {
            Self::Plains => BlockId::PLANKS,
            Self::Desert => BlockId::SANDSTONE,
            Self::Taiga => BlockId::SPRUCE_PLANKS,
            Self::Savanna => BlockId::ACACIA_PLANKS,
        }
    }

    pub const fn pillar_block(self) -> BlockId {
        match self {
            Self::Plains => BlockId::WOOD_LOG,
            Self::Desert => BlockId::TERRACOTTA,
            Self::Taiga => BlockId::SPRUCE_LOG,
            Self::Savanna => BlockId::ACACIA_LOG,
        }
    }

    pub const fn roof_block(self) -> BlockId {
        match self {
            Self::Plains => BlockId::COBBLESTONE,
            Self::Desert => BlockId::TERRACOTTA,
            Self::Taiga => BlockId::SPRUCE_LOG,
            Self::Savanna => BlockId::TERRACOTTA,
        }
    }

    pub const fn floor_block(self) -> BlockId {
        match self {
            Self::Plains => BlockId::COBBLESTONE,
            Self::Desert => BlockId::SANDSTONE,
            Self::Taiga => BlockId::COBBLESTONE,
            Self::Savanna => BlockId::SANDSTONE,
        }
    }

    pub const fn path_block(self) -> BlockId {
        match self {
            Self::Plains => BlockId::GRAVEL,
            Self::Desert => BlockId::SANDSTONE,
            Self::Taiga => BlockId::COBBLESTONE,
            Self::Savanna => BlockId::TERRACOTTA,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ShopkeeperProfession {
    Armorer = 0,
    Toolsmith = 1,
    Angler = 2,
    Baker = 3,
    Chef = 4,
}

impl ShopkeeperProfession {
    pub const ALL: [Self; 5] = [
        Self::Armorer,
        Self::Toolsmith,
        Self::Angler,
        Self::Baker,
        Self::Chef,
    ];

    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Armorer),
            1 => Some(Self::Toolsmith),
            2 => Some(Self::Angler),
            3 => Some(Self::Baker),
            4 => Some(Self::Chef),
            _ => None,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Armorer => "Armorer",
            Self::Toolsmith => "Toolsmith",
            Self::Angler => "Angler",
            Self::Baker => "Baker",
            Self::Chef => "Chef",
        }
    }

    pub const fn has_enhancements(self) -> bool {
        matches!(self, Self::Armorer | Self::Toolsmith | Self::Angler)
    }

    pub fn held_item(self) -> ItemId {
        match self {
            Self::Armorer => ItemId::IRON_SWORD,
            Self::Toolsmith => ItemId::IRON_PICKAXE,
            Self::Angler => ItemId::IRON_FISHING_ROD,
            Self::Baker => ItemId::BREAD,
            Self::Chef => ItemId::HEARTY_MEAT_STEW,
        }
    }

    /// Daily Gold Coin buyback budget per player, scaling modestly with Shop Tier.
    pub const fn daily_buyback_budget(shop_tier: u8) -> u32 {
        match shop_tier {
            1 => 60,
            2 => 100,
            3 => 150,
            4 => 220,
            _ => 320,
        }
    }

    /// Returns the catalog of items this Shopkeeper sells across Shop Tiers 1..=5.
    /// Tiers 1-3 use per-player stock; Tiers 4-5 use scarce shared stock per NPC.
    pub fn buy_offers(self) -> Vec<ShopOffer> {
        match self {
            Self::Armorer => vec![
                // Tier 1 (generous personal stock)
                ShopOffer::new(1, ItemId::ARROW, 16, 8, 24),
                ShopOffer::new(1, ItemId::LEATHER_CHESTPLATE, 1, 14, 16),
                ShopOffer::new(1, ItemId::STONE_SWORD, 1, 12, 16),
                // Tier 2 (moderate personal stock)
                ShopOffer::new(2, ItemId::BOW, 1, 24, 8),
                ShopOffer::new(2, ItemId::IRON_SWORD, 1, 32, 8),
                ShopOffer::new(2, ItemId::IRON_HELMET, 1, 28, 6),
                // Tier 3 (smaller personal stock)
                ShopOffer::new(3, ItemId::IRON_CHESTPLATE, 1, 48, 5),
                ShopOffer::new(3, ItemId::IRON_LEGGINGS, 1, 42, 5),
                // Tier 4 (scarce shared stock)
                ShopOffer::new(4, ItemId::DIAMOND_SWORD, 1, 130, 2),
                ShopOffer::new(4, ItemId::DIAMOND_BOOTS, 1, 120, 2),
                // Tier 5 (ultra-rare shared stock: 1 per restock)
                ShopOffer::new(5, ItemId::VANGUARD_BLADE, 1, 320, 1),
            ],
            Self::Toolsmith => vec![
                // Tier 1
                ShopOffer::new(1, ItemId::TORCH, 16, 6, 32),
                ShopOffer::new(1, ItemId::COAL, 8, 10, 24),
                ShopOffer::new(1, ItemId::STONE_PICKAXE, 1, 10, 16),
                // Tier 2
                ShopOffer::new(2, ItemId::IRON_PICKAXE, 1, 30, 8),
                ShopOffer::new(2, ItemId::IRON_SHOVEL, 1, 22, 8),
                ShopOffer::new(2, ItemId::BUCKET, 1, 20, 8),
                // Tier 3
                ShopOffer::new(3, ItemId::IRON_AXE, 1, 30, 6),
                ShopOffer::new(3, ItemId::LEATHER_BACKPACK, 1, 65, 4),
                // Tier 4 (shared)
                ShopOffer::new(4, ItemId::MINERS_LANTERN, 2, 95, 2),
                ShopOffer::new(4, ItemId::DIAMOND_PICKAXE, 1, 140, 2),
                // Tier 5 (shared)
                ShopOffer::new(5, ItemId::DIAMOND_BACKPACK, 1, 260, 1),
            ],
            Self::Angler => vec![
                // Tier 1
                ShopOffer::new(1, ItemId::FISHING_BAIT, 8, 8, 32),
                ShopOffer::new(1, ItemId::WOODEN_FISHING_ROD, 1, 12, 16),
                ShopOffer::new(1, ItemId::COOKED_FISH, 4, 10, 20),
                // Tier 2
                ShopOffer::new(2, ItemId::COPPER_FISHING_ROD, 1, 28, 8),
                ShopOffer::new(2, ItemId::CORK_BOBBER, 1, 22, 8),
                ShopOffer::new(2, ItemId::BRAIDED_LINE, 1, 24, 8),
                // Tier 3
                ShopOffer::new(3, ItemId::IRON_FISHING_ROD, 1, 55, 5),
                ShopOffer::new(3, ItemId::TREASURE_LURE, 1, 50, 4),
                // Tier 4 (shared)
                ShopOffer::new(4, ItemId::DIAMOND_FISHING_ROD, 1, 145, 2),
                ShopOffer::new(4, ItemId::TITANIUM_LINE, 1, 90, 2),
                // Tier 5 (shared)
                ShopOffer::new(5, ItemId::MASTER_ANGLER_LURE, 1, 240, 1),
            ],
            Self::Baker => vec![
                // Tier 1
                ShopOffer::new(1, ItemId::BREAD, 4, 8, 32),
                ShopOffer::new(1, ItemId::WHEAT_SEEDS, 8, 6, 32),
                ShopOffer::new(1, ItemId::APPLE, 4, 8, 24),
                // Tier 2
                ShopOffer::new(2, ItemId::COOKIE, 8, 12, 10),
                ShopOffer::new(2, ItemId::BAKED_POTATO, 4, 12, 10),
                ShopOffer::new(2, ItemId::COMPOST, 4, 16, 8),
                // Tier 3
                ShopOffer::new(3, ItemId::PUMPKIN_PIE, 2, 22, 6),
                ShopOffer::new(3, ItemId::BONE_FERTILIZER, 4, 28, 6),
                // Tier 4 (shared)
                ShopOffer::new(4, ItemId::SWEET_CAKE, 2, 55, 2),
                // Tier 5 (shared)
                ShopOffer::new(5, ItemId::GOLDEN_APPLE_PIE, 1, 125, 1),
            ],
            Self::Chef => vec![
                // Tier 1
                ShopOffer::new(1, ItemId::COOKED_STEAK, 2, 10, 24),
                ShopOffer::new(1, ItemId::COOKED_PORKCHOP, 2, 10, 24),
                ShopOffer::new(1, ItemId::COOKED_CHICKEN, 3, 9, 24),
                // Tier 2
                ShopOffer::new(2, ItemId::COOKED_MUTTON, 2, 12, 10),
                ShopOffer::new(2, ItemId::TOMATO_SOUP, 2, 18, 8),
                ShopOffer::new(2, ItemId::OMELETTE, 2, 20, 8),
                // Tier 3
                ShopOffer::new(3, ItemId::HEARTY_MEAT_STEW, 2, 32, 6),
                ShopOffer::new(3, ItemId::HERBAL_TEA, 2, 26, 6),
                // Tier 4 (shared)
                ShopOffer::new(4, ItemId::SPICED_ELIXIR, 2, 75, 2),
                // Tier 5 (shared)
                ShopOffer::new(5, ItemId::ROYAL_FEAST, 1, 140, 1),
            ],
        }
    }

    /// Profession-matched items this Shopkeeper buys from players for Gold Coins.
    pub fn sell_offers(self) -> Vec<SellOffer> {
        match self {
            Self::Armorer => vec![
                SellOffer::new(ItemId::ROTTEN_FLESH, 8, 6),
                SellOffer::new(ItemId::BONE, 6, 8),
                SellOffer::new(ItemId::STRING, 6, 8),
                SellOffer::new(ItemId::LEATHER, 4, 10),
                SellOffer::new(ItemId::IRON_INGOT, 4, 14),
            ],
            Self::Toolsmith => vec![
                SellOffer::new(ItemId::COAL, 8, 6),
                SellOffer::new(ItemId::COPPER_INGOT, 4, 10),
                SellOffer::new(ItemId::IRON_INGOT, 4, 14),
                SellOffer::new(ItemId::GOLD_INGOT, 2, 18),
                SellOffer::new(ItemId::DIAMOND, 1, 35),
            ],
            Self::Angler => vec![
                SellOffer::new(ItemId::RAW_CARP, 4, 8),
                SellOffer::new(ItemId::RAW_TROUT, 3, 10),
                SellOffer::new(ItemId::RAW_SALMON, 2, 12),
                SellOffer::new(ItemId::RAW_TUNA, 2, 16),
                SellOffer::new(ItemId::GOLDEN_KOI, 1, 45),
            ],
            Self::Baker => vec![
                SellOffer::new(ItemId::WHEAT, 8, 8),
                SellOffer::new(ItemId::CARROT, 8, 9),
                SellOffer::new(ItemId::POTATO, 8, 9),
                SellOffer::new(ItemId::EGG, 6, 10),
                SellOffer::new(ItemId::PUMPKIN, 4, 12),
            ],
            Self::Chef => vec![
                SellOffer::new(ItemId::RAW_BEEF, 4, 9),
                SellOffer::new(ItemId::RAW_PORKCHOP, 4, 9),
                SellOffer::new(ItemId::RAW_MUTTON, 4, 10),
                SellOffer::new(ItemId::TOMATO, 6, 10),
                SellOffer::new(ItemId::COOKED_STEAK, 4, 16),
            ],
        }
    }

    /// Shop-exclusive Enhancements offered at Tiers 4-5 (shared per NPC).
    pub fn enhance_offers(self) -> Vec<EnhanceOffer> {
        match self {
            Self::Toolsmith => vec![
                EnhanceOffer::new(4, Enhancement::Haste, 90, 2),
                EnhanceOffer::new(4, Enhancement::Reinforced, 110, 2),
                EnhanceOffer::new(5, Enhancement::Prospector, 140, 1),
            ],
            Self::Armorer => vec![
                EnhanceOffer::new(4, Enhancement::Reinforced, 110, 2),
                EnhanceOffer::new(4, Enhancement::Bulwark, 120, 2),
                EnhanceOffer::new(5, Enhancement::Keen, 140, 1),
            ],
            Self::Angler => vec![
                EnhanceOffer::new(4, Enhancement::Reinforced, 95, 2),
                EnhanceOffer::new(4, Enhancement::LuckyCast, 115, 2),
                EnhanceOffer::new(5, Enhancement::TreasureHunter, 140, 1),
            ],
            Self::Baker | Self::Chef => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopOffer {
    pub tier: u8,
    pub item_id: ItemId,
    pub count: u16,
    pub price_coins: u32,
    pub max_stock: u16,
}

impl ShopOffer {
    pub const fn new(
        tier: u8,
        item_id: ItemId,
        count: u16,
        price_coins: u32,
        max_stock: u16,
    ) -> Self {
        Self {
            tier,
            item_id,
            count,
            price_coins,
            max_stock,
        }
    }

    #[inline]
    pub const fn is_shared_stock(&self) -> bool {
        self.tier >= ENHANCE_UNLOCK_TIER
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellOffer {
    pub item_id: ItemId,
    pub count: u16,
    pub payout_coins: u32,
}

impl SellOffer {
    pub const fn new(item_id: ItemId, count: u16, payout_coins: u32) -> Self {
        Self {
            item_id,
            count,
            payout_coins,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnhanceOffer {
    pub tier: u8,
    pub enhancement: Enhancement,
    pub base_price_coins: u32,
    pub max_stock: u16,
}

impl EnhanceOffer {
    pub const fn new(
        tier: u8,
        enhancement: Enhancement,
        base_price_coins: u32,
        max_stock: u16,
    ) -> Self {
        Self {
            tier,
            enhancement,
            base_price_coins,
            max_stock,
        }
    }

    /// Computes the Gold Coin fee to enhance `item_id`, scaling sharply for Iron/Diamond/Unique gear.
    pub fn price_for_item(&self, item_id: ItemId, registry: &ItemRegistry) -> u32 {
        let mult_percent: u32 = if let Some(def) = registry.get(item_id) {
            match def.tool_tier {
                Some(ToolTier::Diamond) => 220,
                Some(ToolTier::Iron | ToolTier::Gold) => 160,
                Some(ToolTier::Copper | ToolTier::Stone) => 125,
                Some(ToolTier::Wood) => 100,
                _ => {
                    if let Some((_, defense)) = registry.get_armor(item_id) {
                        if defense >= 6 {
                            210
                        } else if defense >= 4 {
                            155
                        } else {
                            115
                        }
                    } else if item_id == ItemId::DIAMOND_FISHING_ROD {
                        220
                    } else if item_id == ItemId::IRON_FISHING_ROD {
                        160
                    } else if item_id == ItemId::COPPER_FISHING_ROD {
                        125
                    } else {
                        140
                    }
                }
            }
        } else {
            150
        };
        (self.base_price_coins * mult_percent) / 100
    }
}

/// Per-player progression, Tiers 1-3 personal stock, and daily sell budget for a single Shopkeeper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerShopState {
    pub shop_tier: u8,
    pub coins_spent: u32,
    pub personal_stock: HashMap<u16, u16>,
    pub last_restock_cycle: u64,
    pub daily_coins_paid_out: u32,
    pub last_sell_day: u64,
}

impl PlayerShopState {
    pub fn new(profession: ShopkeeperProfession, world_ticks: u64) -> Self {
        let mut s = Self {
            shop_tier: 1,
            coins_spent: 0,
            personal_stock: HashMap::new(),
            last_restock_cycle: world_ticks / VILLAGE_RESTOCK_TICKS,
            daily_coins_paid_out: 0,
            last_sell_day: world_ticks / TICKS_PER_DAY,
        };
        s.refill_personal_stock(profession);
        s
    }

    pub fn refill_personal_stock(&mut self, profession: ShopkeeperProfession) {
        self.personal_stock.clear();
        for (idx, offer) in profession.buy_offers().iter().enumerate() {
            if !offer.is_shared_stock() {
                self.personal_stock.insert(idx as u16, offer.max_stock);
            }
        }
    }

    pub fn ensure_up_to_date(&mut self, profession: ShopkeeperProfession, world_ticks: u64) {
        let cycle = world_ticks / VILLAGE_RESTOCK_TICKS;
        if cycle > self.last_restock_cycle {
            self.last_restock_cycle = cycle;
            self.refill_personal_stock(profession);
        }
        let day = world_ticks / TICKS_PER_DAY;
        if day > self.last_sell_day {
            self.last_sell_day = day;
            self.daily_coins_paid_out = 0;
        }
    }

    pub fn add_spent_coins(&mut self, coins: u32) -> bool {
        self.coins_spent = self.coins_spent.saturating_add(coins);
        let old_tier = self.shop_tier;
        let mut new_tier = 1u8;
        for (i, &threshold) in SHOP_TIER_THRESHOLDS.iter().enumerate() {
            if self.coins_spent >= threshold {
                new_tier = (i + 1) as u8;
            }
        }
        self.shop_tier = new_tier.min(MAX_SHOP_TIER);
        self.shop_tier > old_tier
    }

    pub fn daily_sell_limit(&self) -> u32 {
        ShopkeeperProfession::daily_buyback_budget(self.shop_tier)
    }

    pub fn daily_sell_remaining(&self) -> u32 {
        self.daily_sell_limit()
            .saturating_sub(self.daily_coins_paid_out)
    }
}

#[derive(Debug, Clone)]
pub struct ShopkeeperEntity {
    pub id: u32,
    pub village_id: u64,
    pub theme: VillageTheme,
    pub profession: ShopkeeperProfession,
    pub name: String,
    pub appearance: CharacterAppearance,
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub walk_anim: f32,
    pub on_ground: bool,
    pub home_pos: Vec3,
    pub courtyard_pos: Vec3,
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub dead: bool,
    pub wander_target: Vec3,
    pub wander_timer: f32,
    pub path: Vec<Vec3>,
    pub path_recalc_timer: f32,
    pub shared_stock: HashMap<u16, u16>,
    pub last_shared_restock_cycle: u64,
    pub player_states: HashMap<String, PlayerShopState>,
}

impl ShopkeeperEntity {
    pub const HALF_WIDTH: f32 = 0.30;
    pub const HEIGHT: f32 = 1.80;
    pub const MAX_HEALTH: f32 = 30.0;

    pub fn new(
        id: u32,
        village_id: u64,
        theme: VillageTheme,
        profession: ShopkeeperProfession,
        home_pos: Vec3,
        courtyard_pos: Vec3,
        world_ticks: u64,
    ) -> Self {
        let name = generate_npc_name(village_id, id, profession.display_name());
        let appearance = build_shopkeeper_appearance(theme, profession, village_id ^ (id as u64));
        let mut entity = Self {
            id,
            village_id,
            theme,
            profession,
            name,
            appearance,
            position: courtyard_pos,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            walk_anim: 0.0,
            on_ground: true,
            home_pos,
            courtyard_pos,
            health: Self::MAX_HEALTH,
            max_health: Self::MAX_HEALTH,
            hurt_timer: 0.0,
            dead: false,
            wander_target: courtyard_pos,
            wander_timer: 2.0,
            path: Vec::new(),
            path_recalc_timer: 0.0,
            shared_stock: HashMap::new(),
            last_shared_restock_cycle: world_ticks / VILLAGE_RESTOCK_TICKS,
            player_states: HashMap::new(),
        };
        entity.refill_shared_stock();
        entity
    }

    pub fn refill_shared_stock(&mut self) {
        self.shared_stock.clear();
        for (idx, offer) in self.profession.buy_offers().iter().enumerate() {
            if offer.is_shared_stock() {
                self.shared_stock.insert(idx as u16, offer.max_stock);
            }
        }
        for (idx, offer) in self.profession.enhance_offers().iter().enumerate() {
            self.shared_stock
                .insert(ENHANCE_STOCK_KEY_OFFSET + idx as u16, offer.max_stock);
        }
    }

    pub fn ensure_shared_stock_up_to_date(&mut self, world_ticks: u64) {
        let cycle = world_ticks / VILLAGE_RESTOCK_TICKS;
        if cycle > self.last_shared_restock_cycle {
            self.last_shared_restock_cycle = cycle;
            self.refill_shared_stock();
        }
    }

    pub fn player_state_mut(&mut self, player_key: &str, world_ticks: u64) -> &mut PlayerShopState {
        let key = player_key.to_ascii_lowercase();
        let prof = self.profession;
        let state = self
            .player_states
            .entry(key)
            .or_insert_with(|| PlayerShopState::new(prof, world_ticks));
        state.ensure_up_to_date(prof, world_ticks);
        state
    }

    pub fn take_damage(&mut self, amount: f32, knockback_dir: Vec3) -> bool {
        if self.dead || amount <= 0.0 {
            return false;
        }
        self.health = (self.health - amount).max(0.0);
        self.hurt_timer = 0.35;
        let kb = Vec3::new(knockback_dir.x, 0.0, knockback_dir.z).normalize_or_zero();
        self.velocity.x += kb.x * 4.5;
        self.velocity.z += kb.z * 4.5;
        self.velocity.y = self.velocity.y.max(3.0);
        if self.health <= 0.0 {
            self.dead = true;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct VillageGuardEntity {
    pub id: u32,
    pub village_id: u64,
    pub theme: VillageTheme,
    pub name: String,
    pub appearance: CharacterAppearance,
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub walk_anim: f32,
    pub swing_anim: f32,
    pub on_ground: bool,
    pub home_pos: Vec3,
    pub patrol_target: Vec3,
    pub patrol_timer: f32,
    pub path: Vec<Vec3>,
    pub path_recalc_timer: f32,
    pub health: f32,
    pub max_health: f32,
    pub hurt_timer: f32,
    pub attack_cooldown: f32,
    pub out_of_combat_timer: f32,
    pub dead: bool,
}

impl VillageGuardEntity {
    pub const HALF_WIDTH: f32 = 0.32;
    pub const HEIGHT: f32 = 1.85;
    pub const MAX_HEALTH: f32 = 100.0;
    pub const ATTACK_DAMAGE: f32 = 10.0;
    pub const AGGRO_RANGE: f32 = 18.0;
    /// Maximum horizontal distance from the village before a Guard loses interest and returns home.
    pub const MAX_CHASE_DIST_FROM_VILLAGE: f32 = 30.0;
    pub const MELEE_RANGE: f32 = 2.5;
    /// Built-in armor damage multiplier (35% damage reduction).
    pub const ARMOR_DAMAGE_MULT: f32 = 0.65;

    pub fn new(id: u32, village_id: u64, theme: VillageTheme, home_pos: Vec3) -> Self {
        let name = format!("Village Guard");
        let appearance = build_guard_appearance(theme, village_id ^ (id as u64));
        Self {
            id,
            village_id,
            theme,
            name,
            appearance,
            position: home_pos,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            walk_anim: 0.0,
            swing_anim: 0.0,
            on_ground: true,
            home_pos,
            patrol_target: home_pos,
            patrol_timer: 1.5,
            path: Vec::new(),
            path_recalc_timer: 0.0,
            health: Self::MAX_HEALTH,
            max_health: Self::MAX_HEALTH,
            hurt_timer: 0.0,
            attack_cooldown: 0.0,
            out_of_combat_timer: 0.0,
            dead: false,
        }
    }

    pub fn take_damage(&mut self, amount: f32, knockback_dir: Vec3) -> bool {
        if self.dead || amount <= 0.0 {
            return false;
        }
        let mitigated = (amount * Self::ARMOR_DAMAGE_MULT).max(0.5);
        self.health = (self.health - mitigated).max(0.0);
        self.hurt_timer = 0.35;
        self.out_of_combat_timer = 0.0;
        let kb = Vec3::new(knockback_dir.x, 0.0, knockback_dir.z).normalize_or_zero();
        self.velocity.x += kb.x * 2.5;
        self.velocity.z += kb.z * 2.5;
        self.velocity.y = self.velocity.y.max(2.0);
        if self.health <= 0.0 {
            self.dead = true;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardHostility {
    pub last_near_village_tick: u64,
}

#[derive(Debug, Clone)]
pub struct VillageRecord {
    pub village_id: u64,
    pub theme: VillageTheme,
    pub origin: IVec3,
    pub spawned: bool,
    pub guard_hostility: HashMap<String, GuardHostility>,
}

/// Spawn descriptor produced by `blockgame_gen` so `VillageManager` can populate a Village once.
#[derive(Debug, Clone)]
pub struct VillageSpawnBlueprint {
    pub village_id: u64,
    pub theme: VillageTheme,
    pub origin: IVec3,
    pub shops: Vec<(ShopkeeperProfession, Vec3, Vec3)>,
    pub guard_positions: Vec<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeError {
    NpcNotFound,
    NpcDead,
    InvalidOffer,
    TierLocked { required: u8, current: u8 },
    OutOfStock,
    InsufficientCoins { required: u32, available: u32 },
    InsufficientItems,
    DailySellLimitReached { remaining: u32 },
    InventoryFull,
    ItemNotEligible,
    AlreadyEnhanced,
}

impl TradeError {
    pub fn message(&self) -> String {
        match self {
            Self::NpcNotFound => "Shopkeeper not found.".to_string(),
            Self::NpcDead => "This Shopkeeper is no longer alive.".to_string(),
            Self::InvalidOffer => "Invalid shop offer.".to_string(),
            Self::TierLocked { required, current } => {
                format!("Requires Shop Tier {} (currently Tier {}).", required, current)
            }
            Self::OutOfStock => "Out of stock until the next restock.".to_string(),
            Self::InsufficientCoins {
                required,
                available,
            } => format!(
                "Need {} Gold Coins (you have {}).",
                required, available
            ),
            Self::InsufficientItems => "You don't have enough items to sell.".to_string(),
            Self::DailySellLimitReached { remaining } => format!(
                "Shopkeeper's daily coin budget reached ({} coins left today).",
                remaining
            ),
            Self::InventoryFull => "Your inventory is full.".to_string(),
            Self::ItemNotEligible => "That item cannot receive this Enhancement.".to_string(),
            Self::AlreadyEnhanced => "That item already has an Enhancement.".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeSuccess {
    pub coins_delta: i32,
    pub leveled_up_to: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopBuyEntry {
    pub offer_idx: u16,
    pub tier: u8,
    pub item_id: ItemId,
    pub count: u16,
    pub price_coins: u32,
    pub stock_remaining: u16,
    pub max_stock: u16,
    pub is_shared: bool,
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopSellEntry {
    pub offer_idx: u16,
    pub item_id: ItemId,
    pub count: u16,
    pub payout_coins: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopEnhanceEntry {
    pub offer_idx: u16,
    pub tier: u8,
    pub enhancement: Enhancement,
    pub base_price_coins: u32,
    pub stock_remaining: u16,
    pub max_stock: u16,
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopUiSnapshot {
    pub npc_id: u32,
    pub name: String,
    pub profession: ShopkeeperProfession,
    pub shop_tier: u8,
    pub coins_spent: u32,
    pub next_tier_coins: Option<u32>,
    pub ticks_until_restock: u64,
    pub daily_sell_remaining: u32,
    pub daily_sell_limit: u32,
    pub buy_offers: Vec<ShopBuyEntry>,
    pub sell_offers: Vec<ShopSellEntry>,
    pub enhance_offers: Vec<ShopEnhanceEntry>,
}

impl ShopEnhanceEntry {
    /// Computes the Gold Coin fee to apply this Enhancement to `item_id`.
    pub fn price_for_item(&self, item_id: ItemId, registry: &ItemRegistry) -> u32 {
        EnhanceOffer::new(
            self.tier,
            self.enhancement,
            self.base_price_coins,
            self.max_stock,
        )
        .price_for_item(item_id, registry)
    }
}

#[derive(Debug, Default)]
pub struct VillageTickOutcome {
    /// Damage dealt by Village Guards to specific players: `(player_key, damage, knockback_dir)`.
    pub guard_damage_to_players: Vec<(String, f32, Vec3)>,
}

#[derive(Debug, Clone, Default)]
pub struct VillageManager {
    pub villages: HashMap<u64, VillageRecord>,
    pub shopkeepers: Vec<ShopkeeperEntity>,
    pub guards: Vec<VillageGuardEntity>,
    pub next_npc_id: u32,
    /// Monotonic world tick counter (never wraps at 24,000) driving daily buyback resets,
    /// 3-day stock restocks, and 1-day Guard aggro decay.
    pub total_world_ticks: u64,
    last_day_time: Option<f32>,
}

impl VillageManager {
    pub fn new() -> Self {
        Self {
            villages: HashMap::new(),
            shopkeepers: Vec::new(),
            guards: Vec::new(),
            next_npc_id: 1,
            total_world_ticks: 0,
            last_day_time: None,
        }
    }

    /// Advances the monotonic `total_world_ticks` clock from the wrapped `day_night_time` (0..24000).
    /// Correctly handles day wraparound and sleeping skips without ever wrapping backwards.
    pub fn advance_clock(&mut self, day_night_time: f32) -> u64 {
        let cur = day_night_time.rem_euclid(TICKS_PER_DAY as f32);
        let cur_u64 = (cur as u64).min(TICKS_PER_DAY - 1);
        let mut day = self.total_world_ticks / TICKS_PER_DAY;
        if let Some(prev) = self.last_day_time {
            if cur < prev {
                // Wrapped past 24,000 (midnight/new day or sleep skip)
                day = day.saturating_add(1);
            }
        }
        self.total_world_ticks = day.saturating_mul(TICKS_PER_DAY).saturating_add(cur_u64);
        self.last_day_time = Some(cur);
        self.total_world_ticks
    }

    /// Idempotently spawns a Village's Shopkeepers and Village Guards on first discovery.
    /// Once marked `spawned`, killed NPCs are never respawned.
    pub fn ensure_village_spawned(
        &mut self,
        blueprint: &VillageSpawnBlueprint,
        world_ticks: u64,
    ) -> bool {
        if let Some(rec) = self.villages.get(&blueprint.village_id) {
            if rec.spawned {
                return false;
            }
        }

        self.villages.insert(
            blueprint.village_id,
            VillageRecord {
                village_id: blueprint.village_id,
                theme: blueprint.theme,
                origin: blueprint.origin,
                spawned: true,
                guard_hostility: HashMap::new(),
            },
        );

        for &(profession, home_pos, courtyard_pos) in &blueprint.shops {
            let id = self.alloc_npc_id();
            self.shopkeepers.push(ShopkeeperEntity::new(
                id,
                blueprint.village_id,
                blueprint.theme,
                profession,
                home_pos,
                courtyard_pos,
                world_ticks,
            ));
        }

        for &gpos in &blueprint.guard_positions {
            let id = self.alloc_npc_id();
            self.guards.push(VillageGuardEntity::new(
                id,
                blueprint.village_id,
                blueprint.theme,
                gpos,
            ));
        }

        true
    }

    fn alloc_npc_id(&mut self) -> u32 {
        let id = self.next_npc_id.max(1);
        self.next_npc_id = id.wrapping_add(1).max(1);
        id
    }

    pub fn is_player_hostile_to_village(
        &self,
        village_id: u64,
        player_key: &str,
        world_ticks: u64,
    ) -> bool {
        let key = player_key.to_ascii_lowercase();
        let Some(rec) = self.villages.get(&village_id) else {
            return false;
        };
        let Some(h) = rec.guard_hostility.get(&key) else {
            return false;
        };
        world_ticks < h.last_near_village_tick.saturating_add(TICKS_PER_DAY)
    }

    /// Clears all Village Guard hostility toward `player_key` when the player dies.
    pub fn clear_guard_hostility_on_player_death(&mut self, player_key: &str) {
        let key = player_key.to_ascii_lowercase();
        for rec in self.villages.values_mut() {
            rec.guard_hostility.remove(&key);
        }
    }

    /// Applies damage to either a Shopkeeper or a Village Guard by NPC ID.
    /// If `attacker_player` is `Some`, immediately flags that player as hostile to that Village's Guards.
    /// Returns `Some((was_killed, village_id))` if an NPC was hit.
    pub fn damage_npc(
        &mut self,
        npc_id: u32,
        amount: f32,
        knockback_dir: Vec3,
        attacker_player: Option<&str>,
        world_ticks: u64,
    ) -> Option<(bool, u64)> {
        let mut hit: Option<(bool, u64)> = None;
        if let Some(sk) = self
            .shopkeepers
            .iter_mut()
            .find(|s| s.id == npc_id && !s.dead)
        {
            let killed = sk.take_damage(amount, knockback_dir);
            hit = Some((killed, sk.village_id));
        } else if let Some(g) = self.guards.iter_mut().find(|g| g.id == npc_id && !g.dead) {
            let killed = g.take_damage(amount, knockback_dir);
            hit = Some((killed, g.village_id));
        }

        if let Some((_, village_id)) = hit {
            if let Some(player_key) = attacker_player {
                let key = player_key.to_ascii_lowercase();
                if let Some(rec) = self.villages.get_mut(&village_id) {
                    rec.guard_hostility.insert(
                        key,
                        GuardHostility {
                            last_near_village_tick: world_ticks,
                        },
                    );
                }
            }
        }
        hit
    }

    /// Builds a `ShopUiSnapshot` for `player_key` viewing `npc_id`.
    pub fn build_shop_snapshot(
        &mut self,
        npc_id: u32,
        player_key: &str,
        world_ticks: u64,
    ) -> Option<ShopUiSnapshot> {
        let sk = self
            .shopkeepers
            .iter_mut()
            .find(|s| s.id == npc_id && !s.dead)?;
        sk.ensure_shared_stock_up_to_date(world_ticks);
        let shared_stock = sk.shared_stock.clone();
        let profession = sk.profession;
        let name = sk.name.clone();
        let pstate = sk.player_state_mut(player_key, world_ticks);
        let shop_tier = pstate.shop_tier;
        let coins_spent = pstate.coins_spent;
        let next_tier_coins = if (shop_tier as usize) < SHOP_TIER_THRESHOLDS.len() {
            Some(SHOP_TIER_THRESHOLDS[shop_tier as usize])
        } else {
            None
        };
        let daily_sell_limit = pstate.daily_sell_limit();
        let daily_sell_remaining = pstate.daily_sell_remaining();
        let personal_stock = pstate.personal_stock.clone();

        let ticks_until_restock = VILLAGE_RESTOCK_TICKS - (world_ticks % VILLAGE_RESTOCK_TICKS);

        let buy_offers = profession
            .buy_offers()
            .into_iter()
            .enumerate()
            .map(|(idx, offer)| {
                let is_shared = offer.is_shared_stock();
                let stock_remaining = if is_shared {
                    shared_stock.get(&(idx as u16)).copied().unwrap_or(0)
                } else {
                    personal_stock.get(&(idx as u16)).copied().unwrap_or(0)
                };
                ShopBuyEntry {
                    offer_idx: idx as u16,
                    tier: offer.tier,
                    item_id: offer.item_id,
                    count: offer.count,
                    price_coins: offer.price_coins,
                    stock_remaining,
                    max_stock: offer.max_stock,
                    is_shared,
                    unlocked: shop_tier >= offer.tier,
                }
            })
            .collect();

        let sell_offers = profession
            .sell_offers()
            .into_iter()
            .enumerate()
            .map(|(idx, offer)| ShopSellEntry {
                offer_idx: idx as u16,
                item_id: offer.item_id,
                count: offer.count,
                payout_coins: offer.payout_coins,
            })
            .collect();

        let enhance_offers = profession
            .enhance_offers()
            .into_iter()
            .enumerate()
            .map(|(idx, offer)| {
                let stock_remaining = shared_stock
                    .get(&(ENHANCE_STOCK_KEY_OFFSET + idx as u16))
                    .copied()
                    .unwrap_or(0);
                ShopEnhanceEntry {
                    offer_idx: idx as u16,
                    tier: offer.tier,
                    enhancement: offer.enhancement,
                    base_price_coins: offer.base_price_coins,
                    stock_remaining,
                    max_stock: offer.max_stock,
                    unlocked: shop_tier >= offer.tier,
                }
            })
            .collect();

        Some(ShopUiSnapshot {
            npc_id,
            name,
            profession,
            shop_tier,
            coins_spent,
            next_tier_coins,
            ticks_until_restock,
            daily_sell_remaining,
            daily_sell_limit,
            buy_offers,
            sell_offers,
            enhance_offers,
        })
    }

    /// Executes a `Buy` transaction for `player_key` at `npc_id`.
    pub fn buy_item(
        &mut self,
        npc_id: u32,
        player_key: &str,
        offer_idx: u16,
        inventory: &mut Inventory,
        registry: &ItemRegistry,
        world_ticks: u64,
    ) -> Result<TradeSuccess, TradeError> {
        let sk = self
            .shopkeepers
            .iter_mut()
            .find(|s| s.id == npc_id)
            .ok_or(TradeError::NpcNotFound)?;
        if sk.dead {
            return Err(TradeError::NpcDead);
        }
        sk.ensure_shared_stock_up_to_date(world_ticks);
        let offers = sk.profession.buy_offers();
        let offer = *offers
            .get(offer_idx as usize)
            .ok_or(TradeError::InvalidOffer)?;

        let current_tier = sk.player_state_mut(player_key, world_ticks).shop_tier;
        if current_tier < offer.tier {
            return Err(TradeError::TierLocked {
                required: offer.tier,
                current: current_tier,
            });
        }

        let stock_remaining = if offer.is_shared_stock() {
            sk.shared_stock.get(&offer_idx).copied().unwrap_or(0)
        } else {
            sk.player_state_mut(player_key, world_ticks)
                .personal_stock
                .get(&offer_idx)
                .copied()
                .unwrap_or(0)
        };
        if stock_remaining == 0 {
            return Err(TradeError::OutOfStock);
        }

        let available_coins = inventory.count_item(ItemId::GOLD_COIN);
        if available_coins < offer.price_coins {
            return Err(TradeError::InsufficientCoins {
                required: offer.price_coins,
                available: available_coins,
            });
        }

        // Verify inventory can fit the purchased item before mutating state
        let mut test_inv = inventory.clone();
        test_inv.remove_item(ItemId::GOLD_COIN, offer.price_coins);
        if test_inv
            .add_item(ItemSlot::new(offer.item_id, offer.count), registry)
            .is_some()
        {
            return Err(TradeError::InventoryFull);
        }

        // Commit inventory & stock mutation
        *inventory = test_inv;
        if offer.is_shared_stock() {
            if let Some(s) = sk.shared_stock.get_mut(&offer_idx) {
                *s = s.saturating_sub(1);
            }
        } else if let Some(s) = sk
            .player_state_mut(player_key, world_ticks)
            .personal_stock
            .get_mut(&offer_idx)
        {
            *s = s.saturating_sub(1);
        }

        let pstate = sk.player_state_mut(player_key, world_ticks);
        let leveled = pstate.add_spent_coins(offer.price_coins);
        let leveled_up_to = if leveled { Some(pstate.shop_tier) } else { None };

        Ok(TradeSuccess {
            coins_delta: -(offer.price_coins as i32),
            leveled_up_to,
        })
    }

    /// Executes a `Sell` transaction for `player_key` at `npc_id`, enforcing the daily buyback limit.
    pub fn sell_item(
        &mut self,
        npc_id: u32,
        player_key: &str,
        offer_idx: u16,
        inventory: &mut Inventory,
        registry: &ItemRegistry,
        world_ticks: u64,
    ) -> Result<TradeSuccess, TradeError> {
        let sk = self
            .shopkeepers
            .iter_mut()
            .find(|s| s.id == npc_id)
            .ok_or(TradeError::NpcNotFound)?;
        if sk.dead {
            return Err(TradeError::NpcDead);
        }
        let offers = sk.profession.sell_offers();
        let offer = *offers
            .get(offer_idx as usize)
            .ok_or(TradeError::InvalidOffer)?;

        let pstate = sk.player_state_mut(player_key, world_ticks);
        let remaining_budget = pstate.daily_sell_remaining();
        if remaining_budget < offer.payout_coins {
            return Err(TradeError::DailySellLimitReached {
                remaining: remaining_budget,
            });
        }

        if inventory.count_item(offer.item_id) < offer.count as u32 {
            return Err(TradeError::InsufficientItems);
        }

        let mut test_inv = inventory.clone();
        test_inv.remove_item(offer.item_id, offer.count as u32);
        if test_inv
            .add_item(
                ItemSlot::new(ItemId::GOLD_COIN, offer.payout_coins as u16),
                registry,
            )
            .is_some()
        {
            return Err(TradeError::InventoryFull);
        }

        *inventory = test_inv;
        pstate.daily_coins_paid_out = pstate
            .daily_coins_paid_out
            .saturating_add(offer.payout_coins);

        Ok(TradeSuccess {
            coins_delta: offer.payout_coins as i32,
            leveled_up_to: None,
        })
    }

    /// Applies a shop-exclusive `Enhancement` to `inventory.slots[inv_slot_idx]`.
    /// Requires Tier 4+, remaining shared stock, an unenhanced eligible item, and Gold Coins.
    /// Restores the item to full durability on success.
    pub fn enhance_item(
        &mut self,
        npc_id: u32,
        player_key: &str,
        inv_slot_idx: usize,
        offer_idx: u16,
        inventory: &mut Inventory,
        registry: &ItemRegistry,
        world_ticks: u64,
    ) -> Result<TradeSuccess, TradeError> {
        let sk = self
            .shopkeepers
            .iter_mut()
            .find(|s| s.id == npc_id)
            .ok_or(TradeError::NpcNotFound)?;
        if sk.dead {
            return Err(TradeError::NpcDead);
        }
        sk.ensure_shared_stock_up_to_date(world_ticks);
        let offers = sk.profession.enhance_offers();
        let offer = *offers
            .get(offer_idx as usize)
            .ok_or(TradeError::InvalidOffer)?;

        let current_tier = sk.player_state_mut(player_key, world_ticks).shop_tier;
        if current_tier < offer.tier {
            return Err(TradeError::TierLocked {
                required: offer.tier,
                current: current_tier,
            });
        }

        let stock_key = ENHANCE_STOCK_KEY_OFFSET + offer_idx;
        let stock_remaining = sk.shared_stock.get(&stock_key).copied().unwrap_or(0);
        if stock_remaining == 0 {
            return Err(TradeError::OutOfStock);
        }

        let target_slot = inventory.get(inv_slot_idx);
        if target_slot.is_empty() {
            return Err(TradeError::ItemNotEligible);
        }
        if target_slot.is_enhanced() {
            return Err(TradeError::AlreadyEnhanced);
        }
        if !offer
            .enhancement
            .is_compatible_with(target_slot.item_id, registry)
        {
            return Err(TradeError::ItemNotEligible);
        }

        let price = offer.price_for_item(target_slot.item_id, registry);
        let available_coins = inventory.count_item(ItemId::GOLD_COIN);
        if available_coins < price {
            return Err(TradeError::InsufficientCoins {
                required: price,
                available: available_coins,
            });
        }

        // Commit: consume Gold Coins, apply Enhancement + restore full durability, decrement shared stock
        inventory.remove_item(ItemId::GOLD_COIN, price);
        let mut updated = inventory.get(inv_slot_idx);
        updated.apply_enhancement(offer.enhancement);
        inventory.set(inv_slot_idx, updated);

        if let Some(s) = sk.shared_stock.get_mut(&stock_key) {
            *s = s.saturating_sub(1);
        }

        let pstate = sk.player_state_mut(player_key, world_ticks);
        let leveled = pstate.add_spent_coins(price);
        let leveled_up_to = if leveled { Some(pstate.shop_tier) } else { None };

        Ok(TradeSuccess {
            coins_delta: -(price as i32),
            leveled_up_to,
        })
    }

    /// Returns all living NPC target positions `(npc_id, position)` for monster targeting.
    pub fn living_npc_targets(&self) -> Vec<(u32, Vec3)> {
        let mut out = Vec::with_capacity(self.shopkeepers.len() + self.guards.len());
        for sk in &self.shopkeepers {
            if !sk.dead {
                out.push((sk.id, sk.position));
            }
        }
        for g in &self.guards {
            if !g.dead {
                out.push((g.id, g.position));
            }
        }
        out
    }

    /// Ticks Shopkeeper daily shelter/wandering AI, Village Guard patrol/defense/retaliation AI,
    /// and Guard hostility decay.
    pub fn tick(
        &mut self,
        dt: f32,
        world_ticks: u64,
        players: &[(String, Vec3)],
        monsters: &mut [MonsterEntity],
        world: &World,
        registry: &BlockRegistry,
    ) -> VillageTickOutcome {
        let mut outcome = VillageTickOutcome::default();
        let time_of_day = world_ticks % TICKS_PER_DAY;
        // Sun sets around 12_000 (dusk starts ~11_500) and rises around 0 (dawn clears ~600)
        let is_night_or_dusk = time_of_day >= 11_500 || time_of_day <= 600;

        // Refresh guard hostility timers while hostile players remain near the village;
        // expire entries once they stay away for 1 full in-game day.
        for rec in self.villages.values_mut() {
            let vpos = rec.origin.as_vec3();
            for (pkey, ppos) in players {
                let key = pkey.to_ascii_lowercase();
                if let Some(h) = rec.guard_hostility.get_mut(&key) {
                    let dist_sq = (*ppos - vpos).length_squared();
                    if dist_sq <= 64.0 * 64.0 {
                        h.last_near_village_tick = world_ticks;
                    }
                }
            }
            rec.guard_hostility.retain(|_, h| {
                world_ticks < h.last_near_village_tick.saturating_add(TICKS_PER_DAY)
            });
        }

        // 1. Tick Shopkeepers: wander near courtyard by safe day, shelter at home_pos at night or when monsters are nearby.
        for sk in &mut self.shopkeepers {
            if sk.dead {
                continue;
            }
            if sk.hurt_timer > 0.0 {
                sk.hurt_timer = (sk.hurt_timer - dt).max(0.0);
            }

            let monster_nearby = monsters.iter().any(|m| {
                !m.dead
                    && (m.position.distance_squared(sk.position) <= 18.0 * 18.0
                        || m.position.distance_squared(sk.home_pos) <= 18.0 * 18.0)
            });
            let should_shelter = is_night_or_dusk || monster_nearby;

            let goal = if should_shelter {
                sk.home_pos
            } else {
                sk.wander_timer -= dt;
                if sk.wander_timer <= 0.0 {
                    sk.wander_timer = 3.5 + ((sk.id as f32 * 1.73) % 3.0);
                    let angle = ((world_ticks as f32) * 0.013 + sk.id as f32 * 2.1).sin() * 3.14;
                    let radius = 2.5;
                    sk.wander_target = sk.courtyard_pos
                        + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
                }
                sk.wander_target
            };

            let goal_dist = Vec3::new(goal.x - sk.position.x, 0.0, goal.z - sk.position.z).length();
            let speed = if should_shelter { 2.8 } else { 1.6 };
            if goal_dist > 0.45 {
                let steer_target = steer_along_path(
                    sk.position,
                    goal,
                    &mut sk.path,
                    &mut sk.path_recalc_timer,
                    dt,
                    world,
                    registry,
                );
                let to_steer = Vec3::new(
                    steer_target.x - sk.position.x,
                    0.0,
                    steer_target.z - sk.position.z,
                );
                let steer_dist = to_steer.length();
                if steer_dist > 0.05 {
                    let dir = to_steer / steer_dist;
                    sk.yaw = dir.x.atan2(dir.z);
                    sk.velocity.x = dir.x * speed;
                    sk.velocity.z = dir.z * speed;
                    sk.walk_anim += dt * speed * 2.4;
                }
            } else {
                sk.path.clear();
                sk.velocity.x *= (1.0 - 8.0 * dt).max(0.0);
                sk.velocity.z *= (1.0 - 8.0 * dt).max(0.0);
            }

            step_npc_physics(
                &mut sk.position,
                &mut sk.velocity,
                &mut sk.on_ground,
                ShopkeeperEntity::HALF_WIDTH,
                ShopkeeperEntity::HEIGHT,
                dt,
                world,
                registry,
            );
        }

        // 2. Tick Village Guards: fight hostile monsters and retaliate against hostile players.
        for g in &mut self.guards {
            if g.dead {
                continue;
            }
            if g.hurt_timer > 0.0 {
                g.hurt_timer = (g.hurt_timer - dt).max(0.0);
            }
            if g.attack_cooldown > 0.0 {
                g.attack_cooldown = (g.attack_cooldown - dt).max(0.0);
            }
            if g.swing_anim > 0.0 {
                g.swing_anim = (g.swing_anim - dt * 3.5).max(0.0);
            }

            // Distance-based interest decay: the farther from the village a Guard roams,
            // the less interested they are in chasing targets, until they lose interest
            // at MAX_CHASE_DIST_FROM_VILLAGE and return home.
            let village_center = self
                .villages
                .get(&g.village_id)
                .map(|r| Vec3::new(r.origin.x as f32 + 0.5, g.home_pos.y, r.origin.z as f32 + 0.5))
                .unwrap_or(g.home_pos);
            let home_dist = Vec3::new(
                g.position.x - village_center.x,
                0.0,
                g.position.z - village_center.z,
            )
            .length();
            let interest =
                (1.0 - (home_dist / VillageGuardEntity::MAX_CHASE_DIST_FROM_VILLAGE)).clamp(0.0, 1.0);
            let effective_aggro = VillageGuardEntity::AGGRO_RANGE * interest;

            // Find nearest hostile monster within decayed aggro range
            let mut target_monster: Option<(usize, Vec3, f32)> = None;
            if effective_aggro > 0.5 {
                for (idx, m) in monsters.iter().enumerate() {
                    if m.dead {
                        continue;
                    }
                    let d = (m.position - g.position).length();
                    if d <= effective_aggro
                        && target_monster.map_or(true, |(_, _, best)| d < best)
                    {
                        target_monster = Some((idx, m.position, d));
                    }
                }
            }

            // Find nearest hostile player for this village within decayed aggro range
            let mut target_player: Option<(String, Vec3, f32)> = None;
            if effective_aggro > 0.5 {
                if let Some(rec) = self.villages.get(&g.village_id) {
                    for (pkey, ppos) in players {
                        let key = pkey.to_ascii_lowercase();
                        if rec.guard_hostility.contains_key(&key) {
                            let d = (*ppos - g.position).length();
                            if d <= effective_aggro
                                && target_player.as_ref().map_or(true, |(_, _, best)| d < *best)
                            {
                                target_player = Some((pkey.clone(), *ppos, d));
                            }
                        }
                    }
                }
            }

            // Prioritize whichever valid threat is closer
            let monster_closer = match (&target_monster, &target_player) {
                (Some((_, _, md)), Some((_, _, pd))) => *md <= *pd,
                (Some(_), None) => true,
                _ => false,
            };

            let mut in_combat = false;
            if monster_closer {
                if let Some((m_idx, mpos, dist)) = target_monster {
                    in_combat = true;
                    let steer = steer_along_path(
                        g.position,
                        mpos,
                        &mut g.path,
                        &mut g.path_recalc_timer,
                        dt,
                        world,
                        registry,
                    );
                    let horiz = Vec3::new(steer.x - g.position.x, 0.0, steer.z - g.position.z);
                    let dir = horiz.normalize_or_zero();
                    if dir.length_squared() > 0.001 {
                        g.yaw = dir.x.atan2(dir.z);
                    }
                    if dist > VillageGuardEntity::MELEE_RANGE * 0.8 {
                        g.velocity.x = dir.x * 4.2;
                        g.velocity.z = dir.z * 4.2;
                        g.walk_anim += dt * 7.5;
                    } else {
                        g.velocity.x *= (1.0 - 8.0 * dt).max(0.0);
                        g.velocity.z *= (1.0 - 8.0 * dt).max(0.0);
                    }
                    if dist <= VillageGuardEntity::MELEE_RANGE && g.attack_cooldown <= 0.0 {
                        let kb_dir = Vec3::new(mpos.x - g.position.x, 0.0, mpos.z - g.position.z)
                            .normalize_or_zero();
                        monsters[m_idx].take_damage(VillageGuardEntity::ATTACK_DAMAGE, kb_dir);
                        g.attack_cooldown = 0.75;
                        g.swing_anim = 1.0;
                    }
                }
            } else if let Some((pkey, ppos, dist)) = target_player {
                in_combat = true;
                let steer = steer_along_path(
                    g.position,
                    ppos,
                    &mut g.path,
                    &mut g.path_recalc_timer,
                    dt,
                    world,
                    registry,
                );
                let horiz = Vec3::new(steer.x - g.position.x, 0.0, steer.z - g.position.z);
                let dir = horiz.normalize_or_zero();
                if dir.length_squared() > 0.001 {
                    g.yaw = dir.x.atan2(dir.z);
                }
                if dist > VillageGuardEntity::MELEE_RANGE * 0.8 {
                    g.velocity.x = dir.x * 4.4;
                    g.velocity.z = dir.z * 4.4;
                    g.walk_anim += dt * 8.0;
                } else {
                    g.velocity.x *= (1.0 - 8.0 * dt).max(0.0);
                    g.velocity.z *= (1.0 - 8.0 * dt).max(0.0);
                }
                if dist <= VillageGuardEntity::MELEE_RANGE && g.attack_cooldown <= 0.0 {
                    let kb_dir = Vec3::new(ppos.x - g.position.x, 0.0, ppos.z - g.position.z)
                        .normalize_or_zero();
                    outcome.guard_damage_to_players.push((
                        pkey,
                        VillageGuardEntity::ATTACK_DAMAGE,
                        kb_dir,
                    ));
                    g.attack_cooldown = 0.85;
                    g.swing_anim = 1.0;
                }
            }

            if in_combat {
                g.out_of_combat_timer = 0.0;
            } else {
                // Out-of-combat health regeneration (2.5 HP/sec after 3.5s out of combat)
                g.out_of_combat_timer += dt;
                if g.out_of_combat_timer >= 3.5 && g.health < g.max_health {
                    g.health = (g.health + 2.5 * dt).min(g.max_health);
                }

                // If the Guard wandered far from the village (e.g. lost interest during a chase),
                // return briskly to the central square using A* pathfinding; otherwise patrol gently.
                if home_dist > 9.0 {
                    let steer = steer_along_path(
                        g.position,
                        g.home_pos,
                        &mut g.path,
                        &mut g.path_recalc_timer,
                        dt,
                        world,
                        registry,
                    );
                    let to_home = Vec3::new(steer.x - g.position.x, 0.0, steer.z - g.position.z);
                    let dir = to_home.normalize_or_zero();
                    if dir.length_squared() > 0.001 {
                        g.yaw = dir.x.atan2(dir.z);
                        g.velocity.x = dir.x * 3.4;
                        g.velocity.z = dir.z * 3.4;
                        g.walk_anim += dt * 6.5;
                    }
                } else {
                    g.patrol_timer -= dt;
                    if g.patrol_timer <= 0.0 {
                        g.patrol_timer = 4.0 + ((g.id as f32 * 1.37) % 3.0);
                        let ang = ((world_ticks as f32) * 0.009 + g.id as f32 * 2.7).sin() * 3.14;
                        g.patrol_target =
                            g.home_pos + Vec3::new(ang.cos() * 4.5, 0.0, ang.sin() * 4.5);
                    }
                    let dist = Vec3::new(
                        g.patrol_target.x - g.position.x,
                        0.0,
                        g.patrol_target.z - g.position.z,
                    )
                    .length();
                    if dist > 0.6 {
                        let steer = steer_along_path(
                            g.position,
                            g.patrol_target,
                            &mut g.path,
                            &mut g.path_recalc_timer,
                            dt,
                            world,
                            registry,
                        );
                        let to_patrol =
                            Vec3::new(steer.x - g.position.x, 0.0, steer.z - g.position.z);
                        let sdist = to_patrol.length();
                        if sdist > 0.05 {
                            let dir = to_patrol / sdist;
                            g.yaw = dir.x.atan2(dir.z);
                            g.velocity.x = dir.x * 2.1;
                            g.velocity.z = dir.z * 2.1;
                            g.walk_anim += dt * 4.5;
                        }
                    } else {
                        g.path.clear();
                        g.velocity.x *= (1.0 - 8.0 * dt).max(0.0);
                        g.velocity.z *= (1.0 - 8.0 * dt).max(0.0);
                    }
                }
            }

            step_npc_physics(
                &mut g.position,
                &mut g.velocity,
                &mut g.on_ground,
                VillageGuardEntity::HALF_WIDTH,
                VillageGuardEntity::HEIGHT,
                dt,
                world,
                registry,
            );
        }

        outcome
    }
}

#[inline]
fn is_block_solid_for_npc(pos: IVec3, world: &World, registry: &BlockRegistry) -> bool {
    let b = world.get_block(pos);
    if b.is_air() || b.is_water() {
        return false;
    }
    registry.get(b).map_or(false, |d| d.is_solid)
}

#[inline]
fn is_block_passable_for_npc(pos: IVec3, world: &World, registry: &BlockRegistry) -> bool {
    let b = world.get_block(pos);
    if b.is_water() {
        return false;
    }
    !registry.get(b).map_or(false, |d| d.is_solid)
}

/// Finds a walkable standing `y` (feet block) at column `(x, z)` within `ref_y - 2 ..= ref_y + 1`.
fn walkable_y_near(
    x: i32,
    z: i32,
    ref_y: i32,
    world: &World,
    registry: &BlockRegistry,
) -> Option<i32> {
    for dy in [0, 1, -1, -2] {
        let y = ref_y + dy;
        if is_block_solid_for_npc(IVec3::new(x, y - 1, z), world, registry)
            && is_block_passable_for_npc(IVec3::new(x, y, z), world, registry)
            && is_block_passable_for_npc(IVec3::new(x, y + 1, z), world, registry)
        {
            return Some(y);
        }
    }
    None
}

/// Bounded 4-neighbor voxel A* pathfinder that routes NPCs around walls and through doorways.
/// Returns a sequence of tile-center waypoints from the step after `start` to `goal` (or the
/// closest reachable tile if `max_expansions` is hit).
pub fn find_voxel_path(
    start: Vec3,
    goal: Vec3,
    world: &World,
    registry: &BlockRegistry,
    max_expansions: usize,
) -> Vec<Vec3> {
    let sx = start.x.floor() as i32;
    let sz = start.z.floor() as i32;
    let start_ref_y = start.y.floor() as i32;
    let Some(sy) = walkable_y_near(sx, sz, start_ref_y, world, registry) else {
        return Vec::new();
    };

    let gx = goal.x.floor() as i32;
    let gz = goal.z.floor() as i32;
    let gy = walkable_y_near(gx, gz, goal.y.floor() as i32, world, registry)
        .unwrap_or(goal.y.floor() as i32);
    if sx == gx && sz == gz && (sy - gy).abs() <= 1 {
        return Vec::new();
    }

    let heuristic = |x: i32, y: i32, z: i32| (x - gx).abs() + (z - gz).abs() + (y - gy).abs() * 2;

    // BinaryHeap stores Reverse<(f_score, g_score, x, y, z)>
    let mut open: BinaryHeap<Reverse<(i32, i32, i32, i32, i32)>> = BinaryHeap::new();
    // (x, y, z) -> (best_g, parent_xyz)
    let mut visited: HashMap<(i32, i32, i32), (i32, (i32, i32, i32))> = HashMap::new();

    let start_key = (sx, sy, sz);
    open.push(Reverse((heuristic(sx, sy, sz), 0, sx, sy, sz)));
    visited.insert(start_key, (0, start_key));

    let mut best_node = start_key;
    let mut best_h = heuristic(sx, sy, sz);
    let mut expansions = 0usize;

    while let Some(Reverse((_, g, cx, cy, cz))) = open.pop() {
        let cur_key = (cx, cy, cz);
        if let Some(&(known_g, _)) = visited.get(&cur_key) {
            if g > known_g {
                continue;
            }
        }
        let h = heuristic(cx, cy, cz);
        if h < best_h {
            best_h = h;
            best_node = cur_key;
        }
        if cx == gx && cz == gz && (cy - gy).abs() <= 1 {
            best_node = cur_key;
            break;
        }
        expansions += 1;
        if expansions >= max_expansions {
            break;
        }

        for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = cx + dx;
            let nz = cz + dz;
            let Some(ny) = walkable_y_near(nx, nz, cy, world, registry) else {
                continue;
            };
            // Ensure headroom at current column when stepping up 1 block
            if ny > cy && !is_block_passable_for_npc(IVec3::new(cx, cy + 2, cz), world, registry) {
                continue;
            }
            let next_key = (nx, ny, nz);
            let next_g = g + 1;
            let is_better = visited
                .get(&next_key)
                .map_or(true, |&(prev_g, _)| next_g < prev_g);
            if is_better {
                visited.insert(next_key, (next_g, cur_key));
                let f = next_g + heuristic(nx, ny, nz);
                open.push(Reverse((f, next_g, nx, ny, nz)));
            }
        }
    }

    if best_node == start_key {
        return Vec::new();
    }

    let mut rev_path = Vec::new();
    let mut cur = best_node;
    while cur != start_key {
        let Some(&(_, parent)) = visited.get(&cur) else {
            break;
        };
        rev_path.push(Vec3::new(cur.0 as f32 + 0.5, cur.1 as f32, cur.2 as f32 + 0.5));
        if parent == cur {
            break;
        }
        cur = parent;
    }
    rev_path.reverse();
    rev_path
}

/// Maintains a cached A* waypoint path for an NPC and returns the immediate steering target.
/// Falls back cleanly to `goal` when no voxel path is needed or the world is empty.
fn steer_along_path(
    pos: Vec3,
    goal: Vec3,
    path: &mut Vec<Vec3>,
    recalc_timer: &mut f32,
    dt: f32,
    world: &World,
    registry: &BlockRegistry,
) -> Vec3 {
    *recalc_timer = (*recalc_timer - dt).max(0.0);

    // Pop waypoints we have already reached horizontally
    while let Some(&wp) = path.first() {
        let d_sq = (wp.x - pos.x).powi(2) + (wp.z - pos.z).powi(2);
        if d_sq <= 0.42 * 0.42 {
            path.remove(0);
        } else {
            break;
        }
    }

    let goal_moved = path.last().map_or(true, |&last| {
        (last.x - goal.x).powi(2) + (last.z - goal.z).powi(2) > 2.25
    });
    let goal_dist_sq = (goal.x - pos.x).powi(2) + (goal.z - pos.z).powi(2);

    if *recalc_timer <= 0.0 && goal_dist_sq > 1.0 && (path.is_empty() || goal_moved) {
        let computed = find_voxel_path(pos, goal, world, registry, 180);
        if !computed.is_empty() {
            *path = computed;
        } else {
            path.clear();
        }
        *recalc_timer = 0.55;
    }

    path.first().copied().unwrap_or(goal)
}

fn step_npc_physics(
    pos: &mut Vec3,
    vel: &mut Vec3,
    on_ground: &mut bool,
    half_width: f32,
    height: f32,
    dt: f32,
    world: &World,
    registry: &BlockRegistry,
) {
    vel.y = (vel.y - 26.0 * dt).max(-40.0);

    // Horizontal step with 1-block auto-hop and jump-assist over obstacles
    let mut blocked_horiz = false;
    let next_x = pos.x + vel.x * dt;
    if !npc_collides(Vec3::new(next_x, pos.y, pos.z), half_width, height, world, registry) {
        pos.x = next_x;
    } else if *on_ground
        && !npc_collides(
            Vec3::new(next_x, pos.y + 1.05, pos.z),
            half_width,
            height,
            world,
            registry,
        )
    {
        pos.y += 1.02;
        pos.x = next_x;
    } else {
        if vel.x.abs() > 0.5 {
            blocked_horiz = true;
        }
        vel.x = 0.0;
    }

    let next_z = pos.z + vel.z * dt;
    if !npc_collides(Vec3::new(pos.x, pos.y, next_z), half_width, height, world, registry) {
        pos.z = next_z;
    } else if *on_ground
        && !npc_collides(
            Vec3::new(pos.x, pos.y + 1.05, next_z),
            half_width,
            height,
            world,
            registry,
        )
    {
        pos.y += 1.02;
        pos.z = next_z;
    } else {
        if vel.z.abs() > 0.5 {
            blocked_horiz = true;
        }
        vel.z = 0.0;
    }

    if blocked_horiz && *on_ground {
        vel.y = 7.6;
        *on_ground = false;
    }

    let next_y = pos.y + vel.y * dt;
    if !npc_collides(Vec3::new(pos.x, next_y, pos.z), half_width, height, world, registry) {
        pos.y = next_y;
        *on_ground = false;
    } else {
        if vel.y < 0.0 {
            pos.y = pos.y.floor();
            *on_ground = true;
        }
        vel.y = 0.0;
    }
}

fn npc_collides(
    pos: Vec3,
    half_width: f32,
    height: f32,
    world: &World,
    registry: &BlockRegistry,
) -> bool {
    let min_x = (pos.x - half_width).floor() as i32;
    let max_x = (pos.x + half_width).floor() as i32;
    let min_y = pos.y.floor() as i32;
    let max_y = (pos.y + height - 0.05).floor() as i32;
    let min_z = (pos.z - half_width).floor() as i32;
    let max_z = (pos.z + half_width).floor() as i32;

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let b = world.get_block(IVec3::new(x, y, z));
                if b != BlockId::AIR && registry.is_solid(b) {
                    return true;
                }
            }
        }
    }
    false
}

fn generate_npc_name(village_id: u64, npc_id: u32, role: &str) -> String {
    const FIRST_NAMES: [&str; 16] = [
        "Alden", "Bram", "Cora", "Daria", "Eldon", "Freya", "Garrick", "Hana", "Ivor", "Jora",
        "Kael", "Linnea", "Maren", "Orin", "Rowan", "Talia",
    ];
    let idx = (village_id.wrapping_add((npc_id as u64).wrapping_mul(31)) as usize)
        % FIRST_NAMES.len();
    format!("{} the {}", FIRST_NAMES[idx], role)
}

pub fn build_shopkeeper_appearance(
    theme: VillageTheme,
    profession: ShopkeeperProfession,
    seed: u64,
) -> CharacterAppearance {
    let skin_tones = [
        RgbColor::new(236, 198, 168),
        RgbColor::new(212, 168, 134),
        RgbColor::new(178, 128, 92),
        RgbColor::new(132, 88, 60),
    ];
    let hair_colors = [
        RgbColor::new(48, 36, 28),
        RgbColor::new(112, 74, 42),
        RgbColor::new(186, 142, 74),
        RgbColor::new(146, 62, 42),
        RgbColor::new(182, 184, 192),
    ];

    let shirt_color = match profession {
        ShopkeeperProfession::Armorer => RgbColor::new(72, 82, 98),
        ShopkeeperProfession::Toolsmith => RgbColor::new(142, 86, 52),
        ShopkeeperProfession::Angler => RgbColor::new(46, 118, 152),
        ShopkeeperProfession::Baker => RgbColor::new(224, 206, 174),
        ShopkeeperProfession::Chef => RgbColor::new(176, 58, 52),
    };
    let pants_color = match theme {
        VillageTheme::Plains => RgbColor::new(68, 92, 58),
        VillageTheme::Desert => RgbColor::new(196, 166, 112),
        VillageTheme::Taiga => RgbColor::new(56, 68, 86),
        VillageTheme::Savanna => RgbColor::new(146, 86, 52),
    };

    let frame = match seed % 3 {
        0 => BodyFrame::Masculine,
        1 => BodyFrame::Feminine,
        _ => BodyFrame::Neutral,
    };
    let hair_style = HairStyle::ALL[(seed as usize / 3) % HairStyle::ALL.len()];

    CharacterAppearance {
        body_frame: frame,
        skin_tone: skin_tones[(seed as usize) % skin_tones.len()],
        eye_style: EyeStyle::Standard,
        eye_color: RgbColor::new(54, 88, 126),
        hair_style,
        hair_color: hair_colors[(seed as usize / 5) % hair_colors.len()],
        facial_hair: if frame == BodyFrame::Masculine && seed % 2 == 0 {
            FacialHairStyle::ShortBeard
        } else {
            FacialHairStyle::None
        },
        shirt_style: ShirtStyle::Tunic,
        shirt_primary: shirt_color,
        shirt_accent: RgbColor::new(220, 196, 132),
        pants_style: PantsStyle::Trousers,
        pants_color,
        shoe_style: ShoeStyle::Boots,
        shoe_color: RgbColor::new(68, 48, 34),
    }
}

pub fn build_guard_appearance(theme: VillageTheme, seed: u64) -> CharacterAppearance {
    let accent = match theme {
        VillageTheme::Plains => RgbColor::new(52, 128, 72),
        VillageTheme::Desert => RgbColor::new(212, 164, 52),
        VillageTheme::Taiga => RgbColor::new(62, 118, 176),
        VillageTheme::Savanna => RgbColor::new(186, 82, 46),
    };
    CharacterAppearance {
        body_frame: BodyFrame::Masculine,
        skin_tone: RgbColor::new(212, 168, 134),
        eye_style: EyeStyle::Sharp,
        eye_color: RgbColor::new(42, 62, 82),
        hair_style: if seed % 2 == 0 {
            HairStyle::ShortCrop
        } else {
            HairStyle::SweptBangs
        },
        hair_color: RgbColor::new(56, 44, 34),
        facial_hair: FacialHairStyle::Stubble,
        shirt_style: ShirtStyle::Vest,
        shirt_primary: RgbColor::new(148, 156, 168),
        shirt_accent: accent,
        pants_style: PantsStyle::Trousers,
        pants_color: RgbColor::new(98, 106, 118),
        shoe_style: ShoeStyle::Boots,
        shoe_color: RgbColor::new(58, 62, 72),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monster::MonsterKind;

    fn make_test_manager() -> (VillageManager, u32, u32) {
        let mut mgr = VillageManager::new();
        let bp = VillageSpawnBlueprint {
            village_id: 42,
            theme: VillageTheme::Plains,
            origin: IVec3::new(300, 45, 300),
            shops: vec![(
                ShopkeeperProfession::Toolsmith,
                Vec3::new(300.0, 45.0, 300.0),
                Vec3::new(304.0, 45.0, 300.0),
            )],
            guard_positions: vec![Vec3::new(306.0, 45.0, 300.0)],
        };
        assert!(mgr.ensure_village_spawned(&bp, 1000));
        // Idempotent second call must not duplicate or respawn NPCs
        assert!(!mgr.ensure_village_spawned(&bp, 2000));
        let sk_id = mgr.shopkeepers[0].id;
        let guard_id = mgr.guards[0].id;
        (mgr, sk_id, guard_id)
    }

    #[test]
    fn test_buy_sell_daily_budget_and_per_player_tier_progression() {
        let registry = ItemRegistry::new();
        let (mut mgr, sk_id, _) = make_test_manager();

        // Give Alice coal to sell and verify daily buyback cap works
        let mut alice_inv = Inventory::default();
        alice_inv.add_item(ItemSlot::new(ItemId::DIAMOND, 10), &registry);
        // Toolsmith sell offer 4 is 1 Diamond -> 35 Gold Coins; Tier 1 daily budget = 60
        let r1 = mgr.sell_item(sk_id, "alice", 4, &mut alice_inv, &registry, 1000);
        assert!(r1.is_ok());
        assert_eq!(alice_inv.count_item(ItemId::GOLD_COIN), 35);

        // Second diamond (35 coins) exceeds remaining 25-coin daily budget
        let r2 = mgr.sell_item(sk_id, "alice", 4, &mut alice_inv, &registry, 1000);
        assert_eq!(
            r2,
            Err(TradeError::DailySellLimitReached { remaining: 25 })
        );

        // Next in-game day resets the buyback budget
        let r3 = mgr.sell_item(
            sk_id,
            "alice",
            4,
            &mut alice_inv,
            &registry,
            1000 + TICKS_PER_DAY,
        );
        assert!(r3.is_ok());
        assert_eq!(alice_inv.count_item(ItemId::GOLD_COIN), 70);

        // Spending 50+ coins levels Alice to Tier 2, while Bob stays at Tier 1
        // Offer 1 is 8 Coal for 10 Gold Coins (5 purchases = 50 coins -> Tier 2)
        for _ in 0..5 {
            mgr.buy_item(
                sk_id,
                "alice",
                1,
                &mut alice_inv,
                &registry,
                1000 + TICKS_PER_DAY,
            )
            .unwrap();
        }
        let alice_snap = mgr
            .build_shop_snapshot(sk_id, "alice", 1000 + TICKS_PER_DAY)
            .unwrap();
        let bob_snap = mgr
            .build_shop_snapshot(sk_id, "bob", 1000 + TICKS_PER_DAY)
            .unwrap();
        assert_eq!(alice_snap.shop_tier, 2);
        assert_eq!(bob_snap.shop_tier, 1);

        // Alice can still buy Tier 1 items after reaching Tier 2
        assert!(alice_snap.buy_offers[0].unlocked);
        // Tier 1 stock is per-player: Bob still has full stock on offer 1
        assert_eq!(alice_snap.buy_offers[1].stock_remaining, 19);
        assert_eq!(bob_snap.buy_offers[1].stock_remaining, 24);
    }

    #[test]
    fn test_shared_tier4_tier5_stock_enhancements_and_restock() {
        let registry = ItemRegistry::new();
        let (mut mgr, sk_id, _) = make_test_manager();

        // Advance both Alice and Bob to Tier 5
        for user in ["alice", "bob"] {
            let sk = &mut mgr.shopkeepers[0];
            sk.player_state_mut(user, 0).add_spent_coins(1000);
        }

        // Give Alice a damaged Diamond Pickaxe + 500 Gold Coins (stacks up to 1000!)
        let mut alice_inv = Inventory::default();
        alice_inv.set(
            0,
            ItemSlot::with_durability(ItemId::DIAMOND_PICKAXE, 1, 100),
        );
        alice_inv.add_item(ItemSlot::new(ItemId::GOLD_COIN, 500), &registry);
        assert_eq!(alice_inv.get(1).count, 500);

        // Offer 2 on Toolsmith is Tier 5 Prospector (shared stock = 1)
        mgr.enhance_item(sk_id, "alice", 0, 2, &mut alice_inv, &registry, 0)
            .unwrap();
        let enhanced_pick = alice_inv.get(0);
        assert_eq!(enhanced_pick.enhancement, Some(Enhancement::Prospector));
        // Durability restored to full (0 = pristine)
        assert_eq!(enhanced_pick.durability, 0);

        // Cannot enhance an already-enhanced item
        assert_eq!(
            mgr.enhance_item(sk_id, "alice", 0, 0, &mut alice_inv, &registry, 0),
            Err(TradeError::AlreadyEnhanced)
        );

        // Bob sees Prospector out of stock because Tier 4-5 stock is shared per NPC!
        let mut bob_inv = Inventory::default();
        bob_inv.set(0, ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1));
        bob_inv.add_item(ItemSlot::new(ItemId::GOLD_COIN, 500), &registry);
        assert_eq!(
            mgr.enhance_item(sk_id, "bob", 0, 2, &mut bob_inv, &registry, 0),
            Err(TradeError::OutOfStock)
        );

        // After 3 in-game days, shared stock restocks and Bob can enhance!
        mgr.enhance_item(
            sk_id,
            "bob",
            0,
            2,
            &mut bob_inv,
            &registry,
            VILLAGE_RESTOCK_TICKS,
        )
        .unwrap();
        assert_eq!(bob_inv.get(0).enhancement, Some(Enhancement::Prospector));
    }

    #[test]
    fn test_guard_retaliation_per_player_death_clear_and_permanent_mortality() {
        let (mut mgr, sk_id, guard_id) = make_test_manager();
        let world = World::new();
        let blocks = BlockRegistry::new();

        // Alice attacks the Shopkeeper -> Guards immediately turn hostile to Alice only
        mgr.damage_npc(sk_id, 5.0, Vec3::X, Some("alice"), 1000);
        assert!(mgr.is_player_hostile_to_village(42, "alice", 1000));
        assert!(!mgr.is_player_hostile_to_village(42, "bob", 1000));

        // Guard attacks Alice when in melee range, ignores Bob
        let players = vec![
            ("alice".to_string(), Vec3::new(307.0, 45.0, 300.0)),
            ("bob".to_string(), Vec3::new(306.5, 45.0, 300.0)),
        ];
        let mut monsters = Vec::new();
        let out = mgr.tick(0.05, 1001, &players, &mut monsters, &world, &blocks);
        assert_eq!(out.guard_damage_to_players.len(), 1);
        assert_eq!(out.guard_damage_to_players[0].0, "alice");

        // Clearing on player death forgives Alice immediately
        mgr.clear_guard_hostility_on_player_death("alice");
        assert!(!mgr.is_player_hostile_to_village(42, "alice", 1005));

        // Guard fights hostile monsters
        let zombie = MonsterEntity::new(1, MonsterKind::Zombie, Vec3::new(307.0, 45.0, 300.0));
        let start_hp = zombie.health;
        mgr.guards[0].attack_cooldown = 0.0;
        let mut ms = vec![zombie];
        mgr.tick(0.05, 1010, &[], &mut ms, &world, &blocks);
        assert!(ms[0].health < start_hp);

        // Permanent NPC death
        let (killed, _) = mgr
            .damage_npc(guard_id, 999.0, Vec3::X, None, 1020)
            .unwrap();
        assert!(killed);
        assert!(mgr.guards[0].dead);
    }

    #[test]
    fn test_villages_bin_save_load_roundtrip_and_enhancement_effects() {
        let registry = ItemRegistry::new();
        let (mut mgr, sk_id, guard_id) = make_test_manager();

        // Mutate state: Alice levels up, kills guard, flags hostility
        mgr.shopkeepers[0]
            .player_state_mut("alice", 500)
            .add_spent_coins(420);
        mgr.damage_npc(guard_id, 999.0, Vec3::X, Some("alice"), 500);

        let dir = std::env::temp_dir().join(format!(
            "blockgame_village_save_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        crate::save::save_villages_file(&dir, &mgr).expect("save_villages_file");
        let loaded = crate::save::load_villages_file(&dir)
            .expect("load_villages_file")
            .expect("should load villages.bin");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(loaded.shopkeepers.len(), 1);
        assert_eq!(loaded.shopkeepers[0].id, sk_id);
        assert_eq!(loaded.shopkeepers[0].player_states["alice"].shop_tier, 4);
        assert!(loaded.guards[0].dead);
        assert!(loaded.is_player_hostile_to_village(42, "alice", 500));

        // Verify Enhancement gameplay effects
        let mut pick = ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1);
        assert!(pick.apply_enhancement(Enhancement::Reinforced));
        assert_eq!(pick.effective_max_durability(1561), 3122);
        assert!(!pick.apply_enhancement(Enhancement::Haste)); // max 1 enhancement

        let mut armor = crate::item::ArmorEquipment::new();
        let mut chest = ItemSlot::new(ItemId::IRON_CHESTPLATE, 1);
        let base_mult = {
            armor.slots[1] = chest;
            armor.damage_multiplier(&registry)
        };
        chest.apply_enhancement(Enhancement::Bulwark);
        armor.slots[1] = chest;
        assert!(armor.damage_multiplier(&registry) < base_mult);

        let inv = Inventory::default();
        let base_tackle =
            crate::fishing::FishingTackleStats::from_inventory(crate::fishing::FishingRodKind::Diamond, &inv);
        let th_tackle = base_tackle
            .clone()
            .with_rod_enhancement(Some(Enhancement::TreasureHunter));
        let lc_tackle = base_tackle
            .clone()
            .with_rod_enhancement(Some(Enhancement::LuckyCast));
        assert!(th_tackle.treasure_chance > base_tackle.treasure_chance);
        assert!(lc_tackle.rare_chance_bonus > base_tackle.rare_chance_bonus);
    }

    #[test]
    fn test_monster_targets_and_damages_village_npc() {
        let (mgr, sk_id, _) = make_test_manager();
        let world = World::new();
        let blocks = BlockRegistry::new();

        let sk_pos = mgr.shopkeepers[0].position;
        let mut zombie = MonsterEntity::new(9, MonsterKind::Zombie, sk_pos + Vec3::new(1.0, 0.0, 0.0));
        zombie.attack_cooldown = 0.0;
        let targets = mgr.living_npc_targets();
        // Player is far away, Shopkeeper is right next to the Zombie
        let outcome = zombie.update_with_npc_targets(
            0.05,
            Vec3::new(999.0, 45.0, 999.0),
            &targets,
            0.0,
            &world,
            &blocks,
        );
        assert_eq!(outcome.damage_to_player, 0.0);
        let (hit_id, dmg, _) = outcome
            .damage_to_npc
            .expect("Zombie should attack nearby Shopkeeper");
        assert_eq!(hit_id, sk_id);
        assert!(dmg > 0.0);
    }

    #[test]
    fn test_village_network_packets_roundtrip() {
        use crate::net::{ClientPacket, ServerPacket, SyncedVillageNpcState};

        let (mut mgr, sk_id, _) = make_test_manager();
        let snap = mgr.build_shop_snapshot(sk_id, "alice", 1000).unwrap();

        let client_pkts = vec![
            ClientPacket::InteractShopkeeper { npc_id: sk_id },
            ClientPacket::ShopBuy {
                npc_id: sk_id,
                offer_idx: 2,
            },
            ClientPacket::ShopSell {
                npc_id: sk_id,
                offer_idx: 1,
            },
            ClientPacket::ShopEnhance {
                npc_id: sk_id,
                inv_slot_idx: 0,
                offer_idx: 1,
            },
            ClientPacket::AttackVillageNpc {
                npc_id: sk_id,
                damage: 7.5,
                knockback: Vec3::X,
            },
            ClientPacket::RegisterVillageBlueprint {
                village_id: 99,
                theme: VillageTheme::Desert,
                origin: IVec3::new(500, 45, 500),
                shops: vec![(
                    ShopkeeperProfession::Angler,
                    Vec3::new(500.0, 45.0, 490.0),
                    Vec3::new(500.0, 45.0, 495.0),
                )],
                guard_positions: vec![Vec3::new(504.0, 45.0, 504.0)],
            },
        ];
        for pkt in client_pkts {
            let framed = pkt.encode().expect("encode client pkt");
            let decoded = ClientPacket::decode(&framed[8..]).expect("decode client pkt");
            assert_eq!(decoded, pkt);
        }

        let server_pkts = vec![
            ServerPacket::VillageNpcsSync {
                npcs: vec![SyncedVillageNpcState {
                    id: sk_id,
                    village_id: 42,
                    is_guard: false,
                    profession: Some(ShopkeeperProfession::Toolsmith),
                    name: "Bram the Toolsmith".to_string(),
                    appearance: CharacterAppearance::default(),
                    position: Vec3::new(300.0, 45.0, 300.0),
                    yaw: 1.0,
                    walk_anim: 0.5,
                    swing_anim: 0.0,
                    hurt_timer: 0.0,
                    health: 30.0,
                    max_health: 30.0,
                    held_item: ItemId::IRON_PICKAXE,
                }],
            },
            ServerPacket::ShopUiSync {
                snapshot: snap,
                status_message: "Purchase complete!".to_string(),
            },
            ServerPacket::InventorySync {
                slots: vec![
                    ItemSlot::with_enhancement(
                        ItemId::DIAMOND_PICKAXE,
                        1,
                        0,
                        Some(Enhancement::Reinforced),
                    ),
                    ItemSlot::new(ItemId::GOLD_COIN, 1000),
                ],
            },
        ];
        for pkt in server_pkts {
            let framed = pkt.encode().expect("encode server pkt");
            let decoded = ServerPacket::decode(&framed[8..]).expect("decode server pkt");
            assert_eq!(decoded, pkt);
        }
    }

    #[test]
    fn test_monotonic_clock_wraparound_drives_restock_buyback_and_aggro_decay() {
        let (mut mgr, sk_id, _guard_id) = make_test_manager();

        // Day 0 afternoon -> midnight wrap -> Day 1 morning
        let t0 = mgr.advance_clock(12_000.0);
        assert_eq!(t0, 12_000);
        let t1 = mgr.advance_clock(23_500.0);
        assert_eq!(t1, 23_500);

        // Trigger guard retaliation on Day 0 at tick 23_500
        let _ = mgr.damage_npc(sk_id, 2.0, Vec3::ZERO, Some("alice"), t1);
        assert!(mgr.is_player_hostile_to_village(42, "alice", t1));

        // Wrap past midnight into Day 1 (day_night_time resets near 0)
        let t2 = mgr.advance_clock(500.0);
        assert_eq!(t2, 24_500);
        // Step 1 full day forward across another midnight wrap -> Day 2
        let _ = mgr.advance_clock(12_500.0);
        let t3 = mgr.advance_clock(500.0);
        assert_eq!(t3, 48_500);
        assert!(
            !mgr.is_player_hostile_to_village(42, "alice", t3),
            "Guard aggro should expire after 1 full in-game day across midnight wraparounds"
        );

        // Step 1 more full day -> Day 3 (72_500 ticks >= 3-day restock interval)
        let _ = mgr.advance_clock(12_500.0);
        let t4 = mgr.advance_clock(500.0);
        assert_eq!(t4, 72_500);
        assert!(t4 / VILLAGE_RESTOCK_TICKS >= 1);
    }

    #[test]
    fn test_enhanced_tooltip_and_specialty_food_buffs() {
        use crate::item::{FoodEffectKind, ItemRegistry};
        use crate::player::PlayerStats;

        let registry = ItemRegistry::new();
        let plain = ItemSlot::new(ItemId::DIAMOND_PICKAXE, 1);
        assert_eq!(
            plain.tooltip_label(&registry).as_deref(),
            Some("Diamond Pickaxe")
        );

        let enhanced = ItemSlot::with_enhancement(
            ItemId::DIAMOND_PICKAXE,
            1,
            0,
            Some(Enhancement::Reinforced),
        );
        let label = enhanced.tooltip_label(&registry).expect("enhanced tooltip");
        assert!(label.contains("Diamond Pickaxe"));
        assert!(label.contains("Reinforced"));

        // Verify specialty food buffs and that curing negative effects preserves positive buffs
        let mut stats = PlayerStats::default();
        stats.health = 10.0;
        stats.add_status_effect(FoodEffectKind::Poison, 10.0);
        stats.add_status_effect(FoodEffectKind::Regeneration, 30.0);
        stats.add_status_effect(FoodEffectKind::Speed, 45.0);
        stats.add_status_effect(FoodEffectKind::MiningHaste, 45.0);

        stats.clear_negative_effects();
        assert!(!stats.has_status_effect(FoodEffectKind::Poison));
        assert!(stats.has_status_effect(FoodEffectKind::Regeneration));
        assert!(stats.has_status_effect(FoodEffectKind::Speed));
        assert!(stats.has_status_effect(FoodEffectKind::MiningHaste));
        assert!(stats.speed_buff_multiplier() > 1.0);
        assert!(stats.mining_haste_multiplier() > 1.0);

        let health_before = stats.health;
        stats.update(2.0, 50.0, false);
        assert!(
            stats.health > health_before,
            "Regeneration buff should restore health over time"
        );
    }

    #[test]
    fn test_guard_loses_interest_far_from_village_and_returns_home() {
        let (mut mgr, sk_id, guard_id) = make_test_manager();
        let world = World::new();
        let reg = BlockRegistry::new();

        // Make alice hostile to the village
        let _ = mgr.damage_npc(sk_id, 2.0, Vec3::ZERO, Some("alice"), 1000);

        // Move guard 32 blocks away from village center (beyond MAX_CHASE_DIST_FROM_VILLAGE)
        let g = mgr.guards.iter_mut().find(|g| g.id == guard_id).unwrap();
        g.position = Vec3::new(332.5, 45.0, 300.5);
        g.on_ground = true;

        // Place hostile player right next to the far-away guard
        let players = vec![("alice".to_string(), Vec3::new(334.0, 45.0, 300.5))];
        let mut monsters = Vec::new();
        let out = mgr.tick(0.2, 1000, &players, &mut monsters, &world, &reg);

        // Guard should have lost interest (no melee hit) and be heading back toward village (negative X)
        assert!(
            out.guard_damage_to_players.is_empty(),
            "Guard far from village should lose interest in chasing"
        );
        let g_after = mgr.guards.iter().find(|g| g.id == guard_id).unwrap();
        assert!(
            g_after.velocity.x < -1.0,
            "Guard should head back toward village center, got vel {:?}",
            g_after.velocity
        );
    }

    #[test]
    fn test_shopkeeper_paths_around_hut_wall_through_doorway() {
        use crate::chunk::{Chunk, ChunkPos};

        let mut world = World::new();
        // Insert chunk (0, 0, 0) covering x=0..16, y=0..16, z=0..16
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        // Solid ground floor at y = 4
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 4, z, BlockId::COBBLESTONE);
            }
        }
        // Build a 7x7 shop hut centered at (8, 5, 8) -> walls at x=5..=11, z=5..=11, y=5..=7
        // with a single 1-wide doorway on the South wall at (8, 5..=6, 11)
        for x in 5..=11 {
            for z in 5..=11 {
                let is_wall = x == 5 || x == 11 || z == 5 || z == 11;
                if is_wall {
                    let is_door = x == 8 && z == 11;
                    for y in 5..=7 {
                        if !(is_door && y <= 6) {
                            chunk.set(x, y, z, BlockId::PLANKS);
                        }
                    }
                }
            }
        }
        world.insert_chunk(chunk);

        let reg = BlockRegistry::new();
        let home_pos = Vec3::new(8.5, 5.0, 8.5);
        let behind_north_wall = Vec3::new(8.5, 5.0, 3.5);

        // Verify A* finds a complete path from behind the North wall into home_pos
        let path = find_voxel_path(behind_north_wall, home_pos, &world, &reg, 250);
        assert!(
            !path.is_empty(),
            "Expected A* to find a path around the shop walls and through the South doorway"
        );
        let last = *path.last().unwrap();
        assert!(
            (last.x - home_pos.x).abs() < 0.6 && (last.z - home_pos.z).abs() < 0.6,
            "Path should end at home_pos {:?}, got {:?}",
            home_pos,
            last
        );

        // Simulate a Shopkeeper behind the North wall at night walking back into their hut
        let mut mgr = VillageManager::new();
        let bp = VillageSpawnBlueprint {
            village_id: 77,
            theme: VillageTheme::Plains,
            origin: IVec3::new(8, 4, 13),
            shops: vec![(
                ShopkeeperProfession::Baker,
                home_pos,
                Vec3::new(8.5, 5.0, 13.5),
            )],
            guard_positions: vec![],
        };
        mgr.ensure_village_spawned(&bp, 0);
        mgr.shopkeepers[0].position = behind_north_wall;

        let mut monsters = Vec::new();
        // Tick 12 seconds of night (tick 18_000) in 0.1s steps
        for _ in 0..120 {
            mgr.tick(0.1, 18_000, &[], &mut monsters, &world, &reg);
        }
        let final_pos = mgr.shopkeepers[0].position;
        let dist_home = Vec3::new(final_pos.x - home_pos.x, 0.0, final_pos.z - home_pos.z).length();
        assert!(
            dist_home < 1.2,
            "Shopkeeper should have navigated around the walls and through the doorway to {:?}, ended at {:?} (dist {:.2})",
            home_pos,
            final_pos,
            dist_home
        );
    }

    #[test]
    fn test_clock_no_phase_drift_and_shopkeeper_shelters_from_night_and_monsters() {
        use crate::monster::MonsterKind;

        let (mut mgr, _sk_id, _guard_id) = make_test_manager();

        // Simulate 1,200 fractional 60-FPS frames (~0.3333 ticks/frame = 400 ticks)
        let mut day_time = 11_600.0_f32;
        for _ in 0..1_200 {
            day_time = (day_time + (1.0 / 3.0)).rem_euclid(TICKS_PER_DAY as f32);
            mgr.advance_clock(day_time);
        }
        let expected_tod = day_time as u64;
        assert_eq!(
            mgr.total_world_ticks % TICKS_PER_DAY,
            expected_tod,
            "total_world_ticks % 24000 must stay locked in phase with day_night_time"
        );

        // Even at midday (6_000), if a living monster is within 18 blocks, Shopkeeper shelters at home_pos
        let world = World::new();
        let reg = BlockRegistry::new();
        let sk_home = mgr.shopkeepers[0].home_pos;
        mgr.shopkeepers[0].position = mgr.shopkeepers[0].courtyard_pos; // outside (304, 45, 300)
        let mut monsters = vec![MonsterEntity::new(
            1,
            MonsterKind::Zombie,
            sk_home + Vec3::new(8.0, 0.0, 0.0),
        )];
        mgr.tick(0.2, 6_000, &[], &mut monsters, &world, &reg);
        assert!(
            mgr.shopkeepers[0].velocity.x < -1.0,
            "Shopkeeper should retreat toward home_pos when a monster is nearby, got vel {:?}",
            mgr.shopkeepers[0].velocity
        );
    }
}

