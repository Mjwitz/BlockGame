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

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
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
    };

    pub const fn new(item_id: ItemId, count: u16) -> Self {
        Self {
            item_id,
            count,
            durability: 0,
        }
    }

    pub const fn with_durability(item_id: ItemId, count: u16, durability: u16) -> Self {
        Self {
            item_id,
            count,
            durability,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.item_id.is_empty() || self.count == 0
    }

    #[inline]
    pub fn can_stack_with(&self, other: &Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        self.item_id == other.item_id && self.durability == other.durability
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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoodDef {
    pub food_points: u16,
    pub saturation: f32,
}

impl FoodDef {
    pub const fn new(food_points: u16, saturation: f32) -> Self {
        Self {
            food_points,
            saturation,
        }
    }
}

pub struct ItemRegistry {
    items: Vec<Option<ItemDef>>,
    by_name: HashMap<String, ItemId>,
    block_to_item: HashMap<BlockId, ItemId>,
    food_properties: HashMap<ItemId, FoodDef>,
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
        self.register_food(ItemId::RAW_CHICKEN, FoodDef::new(2, 1.2));

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
    }
}

/// Standard Minecraft inventory: 9 hotbar slots + 27 backpack slots = 36 slots total.
#[derive(Debug, Clone)]
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
                if slot.count < max_stack {
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
}
