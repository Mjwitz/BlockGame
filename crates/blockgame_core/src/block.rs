use std::collections::HashSet;

use glam::IVec3;

use crate::direction::Direction;
use crate::item::{ItemId, ItemSlot};
use crate::tool::{ToolTier, ToolType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BlockId(pub u8);

impl BlockId {
    pub const AIR: Self = Self(0);
    pub const STONE: Self = Self(1);
    pub const DIRT: Self = Self(2);
    pub const GRASS: Self = Self(3);
    pub const COBBLESTONE: Self = Self(4);
    pub const WOOD_LOG: Self = Self(5);
    pub const LEAVES: Self = Self(6);
    pub const SAND: Self = Self(7);
    pub const PLANKS: Self = Self(8);
    pub const GLASS: Self = Self(9);
    pub const BEDROCK: Self = Self(10);
    pub const TORCH: Self = Self(13);
    pub const CRAFTING_TABLE: Self = Self(14);
    pub const COAL_ORE: Self = Self(15);
    pub const COPPER_ORE: Self = Self(16);
    pub const IRON_ORE: Self = Self(17);
    pub const GOLD_ORE: Self = Self(18);
    pub const DIAMOND_ORE: Self = Self(19);
    pub const FARMLAND: Self = Self(20);
    pub const WHEAT_0: Self = Self(21);
    pub const WHEAT_1: Self = Self(22);
    pub const WHEAT_2: Self = Self(23);
    pub const WHEAT_3: Self = Self(24);
    pub const CHEST: Self = Self(25);
    pub const FURNACE: Self = Self(26);
    pub const FURNACE_LIT: Self = Self(27);
    pub const WATER: Self = Self(11);
    pub const TALL_GRASS: Self = Self(28);
    pub const BIRCH_LOG: Self = Self(29);
    pub const BIRCH_PLANKS: Self = Self(30);
    pub const BIRCH_LEAVES: Self = Self(31);
    pub const SPRUCE_LOG: Self = Self(32);
    pub const SPRUCE_PLANKS: Self = Self(33);
    pub const SPRUCE_LEAVES: Self = Self(34);
    pub const SNOW_BLOCK: Self = Self(35);
    pub const ICE: Self = Self(36);
    pub const CLAY: Self = Self(37);
    pub const LILY_PAD: Self = Self(38);
    pub const ACACIA_LOG: Self = Self(39);
    pub const ACACIA_PLANKS: Self = Self(40);
    pub const ACACIA_LEAVES: Self = Self(41);
    pub const CACTUS: Self = Self(42);
    pub const SANDSTONE: Self = Self(43);
    pub const GRAVEL: Self = Self(44);
    pub const JUNGLE_LOG: Self = Self(45);
    pub const JUNGLE_PLANKS: Self = Self(46);
    pub const JUNGLE_LEAVES: Self = Self(47);
    pub const TERRACOTTA: Self = Self(48);
    pub const RED_SAND: Self = Self(49);
    pub const PODZOL: Self = Self(50);
    pub const MOSSY_COBBLESTONE: Self = Self(51);
    pub const MYCELIUM: Self = Self(52);
    pub const RED_MUSHROOM_BLOCK: Self = Self(53);
    pub const BROWN_MUSHROOM_BLOCK: Self = Self(54);
    pub const MUSHROOM_STEM: Self = Self(55);
    pub const DARK_OAK_LOG: Self = Self(56);
    pub const DARK_OAK_PLANKS: Self = Self(57);
    pub const DARK_OAK_LEAVES: Self = Self(58);
    pub const CORAL_BLOCK: Self = Self(59);
    pub const TUBE_CORAL_BLOCK: Self = Self(60);
    pub const PRISMARINE: Self = Self(61);
    pub const SEA_LANTERN: Self = Self(62);
    pub const SEAGRASS: Self = Self(63);

    #[inline]
    pub const fn is_air(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_water(&self) -> bool {
        self.0 == Self::WATER.0 || self.0 == Self::SEAGRASS.0
    }

    #[inline]
    pub const fn luminance(&self) -> u8 {
        match self.0 {
            13 => 14, // TORCH
            27 => 13, // FURNACE_LIT
            62 => 15, // SEA_LANTERN
            _ => 0,
        }
    }

    #[inline]
    pub const fn is_emissive(&self) -> bool {
        self.luminance() > 0
    }

    /// Whether this block requires attachment or substrate support to remain intact.
    #[inline]
    pub const fn requires_support(&self) -> bool {
        matches!(
            *self,
            Self::TALL_GRASS
                | Self::WHEAT_0
                | Self::WHEAT_1
                | Self::WHEAT_2
                | Self::WHEAT_3
                | Self::CACTUS
                | Self::TORCH
                | Self::LILY_PAD
                | Self::SEAGRASS
        )
    }

    /// Whether this block can exist submerged in water.
    #[inline]
    pub const fn can_exist_in_water(&self) -> bool {
        match *self {
            Self::TORCH
            | Self::TALL_GRASS
            | Self::CACTUS
            | Self::WHEAT_0
            | Self::WHEAT_1
            | Self::WHEAT_2
            | Self::WHEAT_3
            | Self::LILY_PAD
            | Self::FARMLAND => false,
            _ => true,
        }
    }
}

impl From<u8> for BlockId {
    #[inline]
    fn from(val: u8) -> Self {
        Self(val)
    }
}

#[derive(Debug, Clone)]
pub struct BlockDefinition {
    pub id: BlockId,
    pub name: &'static str,
    pub is_solid: bool,
    pub is_transparent: bool,
    pub luminance: u8,
    /// Texture index on the texture atlas for each face:
    /// [PosY, NegY, PosZ, NegZ, PosX, NegX]
    pub textures: [u16; 6],
    pub hardness: f32,
    pub required_tool: Option<ToolType>,
    pub min_tier: ToolTier,
    pub drop_item: Option<ItemId>,
    pub drop_count: (u16, u16),
}

pub struct BlockRegistry {
    definitions: [Option<BlockDefinition>; 256],
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockRegistry {
    pub fn new() -> Self {
        const NONE_DEF: Option<BlockDefinition> = None;
        let mut registry = Self {
            definitions: [NONE_DEF; 256],
        };

        // Texture atlas tile indices:
        // 0: Air / Empty
        // 1: Stone
        // 2: Dirt
        // 3: Grass Top
        // 4: Grass Side
        // 5: Cobblestone
        // 6: Wood Log Side
        // 7: Wood Log Top
        // 8: Leaves
        // 9: Sand
        // 10: Planks
        // 11: Glass
        // 12: Bedrock
        // 13: Torch
        // 17: Crafting Table Top
        // 128: Crafting Table Side (with tools)
        // 129: Coal Ore
        // 130: Copper Ore
        // 131: Iron Ore
        // 132: Gold Ore
        // 133: Diamond Ore

        registry.register(BlockDefinition {
            id: BlockId::AIR,
            name: "Air",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [0; 6],
            hardness: 0.0,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        registry.register(BlockDefinition {
            id: BlockId::STONE,
            name: "Stone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [1; 6],
            hardness: 1.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::COBBLESTONE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::DIRT,
            name: "Dirt",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [2; 6],
            hardness: 0.5,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::DIRT),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::GRASS,
            name: "Grass",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: Grass Top (3), NegY: Dirt (2), Sides: Grass Side (4)
            textures: [3, 2, 4, 4, 4, 4],
            hardness: 0.6,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::DIRT),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::COBBLESTONE,
            name: "Cobblestone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [5; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::COBBLESTONE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::WOOD_LOG,
            name: "Wood Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY, NegY: Wood Log Top (7), Sides: Wood Log Side (6)
            textures: [7, 7, 6, 6, 6, 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::WOOD_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::LEAVES,
            name: "Leaves",
            is_solid: true,
            is_transparent: true, // Alpha cutout
            luminance: 0,
            textures: [8; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        registry.register(BlockDefinition {
            id: BlockId::SAND,
            name: "Sand",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [9; 6],
            hardness: 0.5,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SAND),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::PLANKS,
            name: "Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [10; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::GLASS,
            name: "Glass",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [11; 6],
            hardness: 0.3,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        registry.register(BlockDefinition {
            id: BlockId::BEDROCK,
            name: "Bedrock",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [12; 6],
            hardness: -1.0, // Unbreakable
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        registry.register(BlockDefinition {
            id: BlockId::TORCH,
            name: "Torch",
            is_solid: false,
            is_transparent: true,
            luminance: 14,
            textures: [13; 6],
            hardness: 0.05,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::TORCH),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::CRAFTING_TABLE,
            name: "Crafting Table",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // Top: 17, Bottom: 10, Sides: 128 (Crafting Table side with tools)
            textures: [17, 10, 128, 128, 128, 128],
            hardness: 2.5,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::CRAFTING_TABLE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::COAL_ORE,
            name: "Coal Ore",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [129; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::COAL),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::COPPER_ORE,
            name: "Copper Ore",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [130; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Stone,
            drop_item: Some(ItemId::RAW_COPPER),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::IRON_ORE,
            name: "Iron Ore",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [131; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Stone,
            drop_item: Some(ItemId::RAW_IRON),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::GOLD_ORE,
            name: "Gold Ore",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [132; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Iron,
            drop_item: Some(ItemId::RAW_GOLD),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::DIAMOND_ORE,
            name: "Diamond Ore",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [133; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Iron,
            drop_item: Some(ItemId::DIAMOND),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::FARMLAND,
            name: "Farmland",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: Farmland Top (150), NegY: Dirt (2), Sides: Dirt (2)
            textures: [150, 2, 2, 2, 2, 2],
            hardness: 0.6,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::DIRT),
            drop_count: (1, 1),
        });

        let wheat_stages = [
            (BlockId::WHEAT_0, "Wheat Crop Stage 0", 151),
            (BlockId::WHEAT_1, "Wheat Crop Stage 1", 152),
            (BlockId::WHEAT_2, "Wheat Crop Stage 2", 153),
            (BlockId::WHEAT_3, "Wheat Crop Stage 3", 154),
        ];

        for (id, name, tex) in wheat_stages {
            registry.register(BlockDefinition {
                id,
                name,
                is_solid: false,
                is_transparent: true,
                luminance: 0,
                textures: [tex; 6],
                hardness: 0.0,
                required_tool: None,
                min_tier: ToolTier::None,
                drop_item: if id == BlockId::WHEAT_3 {
                    Some(ItemId::WHEAT)
                } else {
                    Some(ItemId::WHEAT_SEEDS)
                },
                drop_count: (1, 1),
            });
        }
        registry.register(BlockDefinition {
            id: BlockId::CHEST,
            name: "Chest",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 173, NegY: 176, PosZ (front): 174, NegZ: 175, PosX: 175, NegX: 175
            textures: [173, 176, 174, 175, 175, 175],
            hardness: 2.5,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::CHEST),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::FURNACE,
            name: "Furnace",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 178, NegY: 178, PosZ (front): 180, NegZ: 179, PosX: 179, NegX: 179
            textures: [178, 178, 180, 179, 179, 179],
            hardness: 3.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::FURNACE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::FURNACE_LIT,
            name: "Burning Furnace",
            is_solid: true,
            is_transparent: false,
            luminance: 13,
            // PosY: 178, NegY: 178, PosZ (front lit): 181, NegZ: 179, PosX: 179, NegX: 179
            textures: [178, 178, 181, 179, 179, 179],
            hardness: 3.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::FURNACE),
            drop_count: (1, 1),
        });

        // Water: non-solid, transparent liquid with translucent Faithful water texture
        registry.register(BlockDefinition {
            id: BlockId::WATER,
            name: "Water",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [191; 6],
            hardness: 100.0,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        // Tall Grass: passable transparent wild plant quad, drops wheat seeds
        registry.register(BlockDefinition {
            id: BlockId::TALL_GRASS,
            name: "Tall Grass",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [200; 6],
            hardness: 0.0,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::WHEAT_SEEDS),
            drop_count: (1, 1),
        });

        // Birch Wood Family
        registry.register(BlockDefinition {
            id: BlockId::BIRCH_LOG,
            name: "Birch Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [193, 193, 192, 192, 192, 192],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::BIRCH_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::BIRCH_PLANKS,
            name: "Birch Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [194; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::BIRCH_PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::BIRCH_LEAVES,
            name: "Birch Leaves",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [195; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::APPLE),
            drop_count: (0, 1),
        });

        // Spruce Wood Family
        registry.register(BlockDefinition {
            id: BlockId::SPRUCE_LOG,
            name: "Spruce Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [197, 197, 196, 196, 196, 196],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SPRUCE_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::SPRUCE_PLANKS,
            name: "Spruce Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [198; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SPRUCE_PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::SPRUCE_LEAVES,
            name: "Spruce Leaves",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [199; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        // Snowy Biome Blocks
        registry.register(BlockDefinition {
            id: BlockId::SNOW_BLOCK,
            name: "Snow Block",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [203; 6],
            hardness: 0.2,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SNOW_BLOCK),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::ICE,
            name: "Ice",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [204; 6],
            hardness: 0.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: None,
            drop_count: (0, 0),
        });

        // Swamp & Waterway Blocks
        registry.register(BlockDefinition {
            id: BlockId::CLAY,
            name: "Clay",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [205; 6],
            hardness: 0.6,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::CLAY),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::LILY_PAD,
            name: "Lily Pad",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [206; 6],
            hardness: 0.0,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::LILY_PAD),
            drop_count: (1, 1),
        });

        // Savanna Acacia Wood Family
        registry.register(BlockDefinition {
            id: BlockId::ACACIA_LOG,
            name: "Acacia Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 208, NegY: 208, Sides: 207
            textures: [208, 208, 207, 207, 207, 207],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::ACACIA_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::ACACIA_PLANKS,
            name: "Acacia Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [209; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::ACACIA_PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::ACACIA_LEAVES,
            name: "Acacia Leaves",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [210; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::APPLE),
            drop_count: (0, 1),
        });

        // Desert Blocks
        registry.register(BlockDefinition {
            id: BlockId::CACTUS,
            name: "Cactus",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 212, NegY: 212, Sides: 211
            textures: [212, 212, 211, 211, 211, 211],
            hardness: 0.4,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::CACTUS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::SANDSTONE,
            name: "Sandstone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 214, NegY: 215, Sides: 213
            textures: [214, 215, 213, 213, 213, 213],
            hardness: 0.8,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SANDSTONE),
            drop_count: (1, 1),
        });

        // Ocean & Subsurface Aggregate
        registry.register(BlockDefinition {
            id: BlockId::GRAVEL,
            name: "Gravel",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [216; 6],
            hardness: 0.6,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::GRAVEL),
            drop_count: (1, 1),
        });

        // Jungle Wood Family
        registry.register(BlockDefinition {
            id: BlockId::JUNGLE_LOG,
            name: "Jungle Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 218, NegY: 218, Sides: 217
            textures: [218, 218, 217, 217, 217, 217],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::JUNGLE_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::JUNGLE_PLANKS,
            name: "Jungle Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [219; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::JUNGLE_PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::JUNGLE_LEAVES,
            name: "Jungle Leaves",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [220; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::APPLE),
            drop_count: (0, 1),
        });

        // Badlands / Mesa Blocks
        registry.register(BlockDefinition {
            id: BlockId::TERRACOTTA,
            name: "Terracotta",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [221; 6],
            hardness: 1.25,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::TERRACOTTA),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::RED_SAND,
            name: "Red Sand",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [222; 6],
            hardness: 0.5,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::RED_SAND),
            drop_count: (1, 1),
        });

        // Taiga & Forest Blocks
        registry.register(BlockDefinition {
            id: BlockId::PODZOL,
            name: "Podzol",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 224, NegY: 2 (Dirt), Sides: 223
            textures: [224, 2, 223, 223, 223, 223],
            hardness: 0.5,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::PODZOL),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::MOSSY_COBBLESTONE,
            name: "Mossy Cobblestone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [225; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::MOSSY_COBBLESTONE),
            drop_count: (1, 1),
        });

        // Mushroom Fields (Island) Blocks
        registry.register(BlockDefinition {
            id: BlockId::MYCELIUM,
            name: "Mycelium",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 227, NegY: 2 (Dirt), Sides: 226
            textures: [227, 2, 226, 226, 226, 226],
            hardness: 0.6,
            required_tool: Some(ToolType::Shovel),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::MYCELIUM),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::RED_MUSHROOM_BLOCK,
            name: "Red Mushroom Block",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [228; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::RED_MUSHROOM_BLOCK),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::BROWN_MUSHROOM_BLOCK,
            name: "Brown Mushroom Block",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [229; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::BROWN_MUSHROOM_BLOCK),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::MUSHROOM_STEM,
            name: "Mushroom Stem",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [230; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::MUSHROOM_STEM),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::DARK_OAK_LOG,
            name: "Dark Oak Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [232, 232, 231, 231, 231, 231],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::DARK_OAK_LOG),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::DARK_OAK_PLANKS,
            name: "Dark Oak Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [233; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::DARK_OAK_PLANKS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::DARK_OAK_LEAVES,
            name: "Dark Oak Leaves",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [234; 6],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::APPLE),
            drop_count: (0, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::CORAL_BLOCK,
            name: "Brain Coral Block",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [235; 6],
            hardness: 1.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::CORAL_BLOCK),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::TUBE_CORAL_BLOCK,
            name: "Tube Coral Block",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [236; 6],
            hardness: 1.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::TUBE_CORAL_BLOCK),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::PRISMARINE,
            name: "Prismarine",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [237; 6],
            hardness: 1.5,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::PRISMARINE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::SEA_LANTERN,
            name: "Sea Lantern",
            is_solid: true,
            is_transparent: false,
            luminance: 15,
            textures: [238; 6],
            hardness: 0.3,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SEA_LANTERN),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::SEAGRASS,
            name: "Seagrass",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [239; 6],
            hardness: 0.0,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::SEAGRASS),
            drop_count: (1, 1),
        });

        registry
    }

    pub fn register(&mut self, def: BlockDefinition) {
        let index = def.id.0 as usize;
        self.definitions[index] = Some(def);
    }

    #[inline]
    pub fn get(&self, id: BlockId) -> Option<&BlockDefinition> {
        self.definitions[id.0 as usize].as_ref()
    }

    #[inline]
    pub fn is_transparent(&self, id: BlockId) -> bool {
        match self.definitions[id.0 as usize].as_ref() {
            Some(def) => def.is_transparent,
            None => true, // Unknown block treats as air/transparent
        }
    }

    #[inline]
    pub fn is_solid(&self, id: BlockId) -> bool {
        match self.definitions[id.0 as usize].as_ref() {
            Some(def) => def.is_solid,
            None => false,
        }
    }

    #[inline]
    pub fn get_texture(&self, id: BlockId, face: Direction) -> u16 {
        match self.definitions[id.0 as usize].as_ref() {
            Some(def) => def.textures[face.index()],
            None => 0,
        }
    }

    #[inline]
    pub fn get_hardness(&self, id: BlockId) -> f32 {
        self.get(id).map(|d| d.hardness).unwrap_or(1.0)
    }

    #[inline]
    pub fn can_harvest(
        &self,
        id: BlockId,
        held_tool_type: Option<ToolType>,
        held_tier: ToolTier,
    ) -> bool {
        if let Some(def) = self.get(id) {
            if def.hardness < 0.0 {
                return false;
            }
            if def.min_tier == ToolTier::None {
                return true;
            }
            if let Some(req_tool) = def.required_tool {
                if held_tool_type == Some(req_tool) && held_tier >= def.min_tier {
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    #[inline]
    pub fn get_break_time(
        &self,
        id: BlockId,
        held_tool_type: Option<ToolType>,
        held_tier: ToolTier,
    ) -> f32 {
        if let Some(def) = self.get(id) {
            if def.hardness < 0.0 {
                return f32::INFINITY;
            }
            let is_effective_tool =
                def.required_tool.is_some() && def.required_tool == held_tool_type;
            let speed = if is_effective_tool {
                held_tier.speed_multiplier()
            } else {
                1.0
            };
            let can_harvest = self.can_harvest(id, held_tool_type, held_tier);
            let time = if can_harvest {
                (def.hardness * 1.5) / speed
            } else {
                (def.hardness * 5.0) / 1.0
            };
            time.max(0.05)
        } else {
            1.0
        }
    }

    #[inline]
    pub fn get_drop(
        &self,
        id: BlockId,
        held_tool_type: Option<ToolType>,
        held_tier: ToolTier,
    ) -> Option<(ItemId, u16)> {
        if let Some(def) = self.get(id) {
            if self.can_harvest(id, held_tool_type, held_tier) {
                def.drop_item.map(|item_id| (item_id, def.drop_count.0))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Determines if a block of type `id` at position `pos` has sufficient attachment
/// and environmental support to survive in the world.
pub fn can_block_survive<F>(
    id: BlockId,
    pos: IVec3,
    registry: &BlockRegistry,
    mut get_block: F,
) -> bool
where
    F: FnMut(IVec3) -> BlockId,
{
    match id {
        BlockId::TALL_GRASS => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            matches!(
                below,
                BlockId::GRASS
                    | BlockId::DIRT
                    | BlockId::PODZOL
                    | BlockId::FARMLAND
                    | BlockId::MYCELIUM
            )
        }
        BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 | BlockId::WHEAT_3 => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            below == BlockId::FARMLAND
        }
        BlockId::CACTUS => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            if below != BlockId::SAND && below != BlockId::RED_SAND && below != BlockId::CACTUS {
                return false;
            }
            for offset in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let neighbor = get_block(pos + offset);
                if registry.is_solid(neighbor) {
                    return false;
                }
            }
            true
        }
        BlockId::TORCH => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            if registry.is_solid(below) {
                return true;
            }
            for offset in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                let neighbor = get_block(pos + offset);
                if registry.is_solid(neighbor) {
                    return true;
                }
            }
            false
        }
        BlockId::LILY_PAD => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            below == BlockId::WATER || below == BlockId::SEAGRASS
        }
        BlockId::SEAGRASS => {
            let below = get_block(pos + IVec3::NEG_Y);
            registry.is_solid(below)
        }
        _ => true,
    }
}

/// Convenience helper checking block survival using the `World` container.
pub fn can_block_survive_in_world(
    id: BlockId,
    pos: IVec3,
    world: &crate::world::World,
    registry: &BlockRegistry,
) -> bool {
    can_block_survive(id, pos, registry, |p| world.get_block(p))
}

/// Returns the item drops produced when an unsupported block collapses.
pub fn get_support_break_drops(
    id: BlockId,
    pos: IVec3,
    registry: &BlockRegistry,
) -> Vec<ItemSlot> {
    if id == BlockId::WHEAT_3 {
        let hash = (pos.x.wrapping_mul(1234567) ^ pos.z.wrapping_mul(7654321)) % 2;
        let seed_count = (hash.abs() + 1) as u16;
        vec![
            ItemSlot::new(ItemId::WHEAT, 1),
            ItemSlot::new(ItemId::WHEAT_SEEDS, seed_count),
        ]
    } else if let Some((drop_item, drop_count)) = registry.get_drop(id, None, ToolTier::None) {
        vec![ItemSlot::new(drop_item, drop_count)]
    } else {
        Vec::new()
    }
}

/// Checks for and breaks any unsupported attached blocks adjacent to or dependent on `trigger_pos`.
/// This performs an upward cascade for stacked blocks like cactuses, and breaks crops, tall grass,
/// and torches that have lost their substrate or anchor.
///
/// Returns:
/// - A list of dropped item slots with their block positions `(pos, item_slot)`.
/// - A list of modified block positions `Vec<IVec3>`.
pub fn break_unsupported_attached(
    trigger_pos: IVec3,
    world: &mut crate::world::World,
    registry: &BlockRegistry,
) -> (Vec<(IVec3, ItemSlot)>, Vec<IVec3>) {
    let mut extra_drops = Vec::new();
    let mut modified_positions = Vec::new();
    let mut check_queue = vec![
        trigger_pos + IVec3::Y,
        trigger_pos + IVec3::X,
        trigger_pos + IVec3::NEG_X,
        trigger_pos + IVec3::Z,
        trigger_pos + IVec3::NEG_Z,
    ];
    let mut visited = HashSet::new();

    while let Some(check_pos) = check_queue.pop() {
        if !visited.insert(check_pos) {
            continue;
        }
        let block_to_check = world.get_block(check_pos);
        if block_to_check.is_air() || !block_to_check.requires_support() {
            continue;
        }
        let can_survive = can_block_survive(
            block_to_check,
            check_pos,
            registry,
            |p| world.get_block(p),
        );
        if !can_survive {
            let repl = if block_to_check == BlockId::SEAGRASS {
                BlockId::WATER
            } else {
                BlockId::AIR
            };
            world.set_block(check_pos, repl);
            modified_positions.push(check_pos);
            let drops = get_support_break_drops(block_to_check, check_pos, registry);
            for drop in drops {
                extra_drops.push((check_pos, drop));
            }
            // Cascade upward (e.g. cactus or stacked plants) and neighbors
            check_queue.push(check_pos + IVec3::Y);
            check_queue.push(check_pos + IVec3::X);
            check_queue.push(check_pos + IVec3::NEG_X);
            check_queue.push(check_pos + IVec3::Z);
            check_queue.push(check_pos + IVec3::NEG_Z);
        }
    }

    (extra_drops, modified_positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harvesting_and_hardness_rules() {
        let reg = BlockRegistry::new();

        // 1. Bedrock is unbreakable
        assert_eq!(reg.get_hardness(BlockId::BEDROCK), -1.0);
        assert!(!reg.can_harvest(BlockId::BEDROCK, Some(ToolType::Pickaxe), ToolTier::Diamond));
        assert!(reg
            .get_break_time(BlockId::BEDROCK, Some(ToolType::Pickaxe), ToolTier::Diamond)
            .is_infinite());

        // 2. Dirt can be harvested with hand or shovel
        assert!(reg.can_harvest(BlockId::DIRT, None, ToolTier::None));
        assert_eq!(
            reg.get_drop(BlockId::DIRT, None, ToolTier::None),
            Some((ItemId::DIRT, 1))
        );

        // 3. Stone & Mossy Cobblestone require Wood Pickaxe or better
        assert!(!reg.can_harvest(BlockId::STONE, None, ToolTier::None));
        assert_eq!(reg.get_drop(BlockId::STONE, None, ToolTier::None), None);
        assert!(reg.can_harvest(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Wood));
        assert_eq!(
            reg.get_drop(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Wood),
            Some((ItemId::COBBLESTONE, 1))
        );
        assert!(!reg.can_harvest(BlockId::MOSSY_COBBLESTONE, None, ToolTier::None));
        assert_eq!(reg.get_drop(BlockId::MOSSY_COBBLESTONE, None, ToolTier::None), None);
        assert!(reg.can_harvest(BlockId::MOSSY_COBBLESTONE, Some(ToolType::Pickaxe), ToolTier::Wood));
        assert_eq!(
            reg.get_drop(BlockId::MOSSY_COBBLESTONE, Some(ToolType::Pickaxe), ToolTier::Wood),
            Some((ItemId::MOSSY_COBBLESTONE, 1))
        );

        // 4. Diamond Ore requires Iron Pickaxe or better
        assert!(!reg.can_harvest(
            BlockId::DIAMOND_ORE,
            Some(ToolType::Pickaxe),
            ToolTier::Wood
        ));
        assert!(!reg.can_harvest(
            BlockId::DIAMOND_ORE,
            Some(ToolType::Pickaxe),
            ToolTier::Stone
        ));
        assert_eq!(
            reg.get_drop(
                BlockId::DIAMOND_ORE,
                Some(ToolType::Pickaxe),
                ToolTier::Stone
            ),
            None
        );
        assert!(reg.can_harvest(
            BlockId::DIAMOND_ORE,
            Some(ToolType::Pickaxe),
            ToolTier::Iron
        ));
        assert_eq!(
            reg.get_drop(
                BlockId::DIAMOND_ORE,
                Some(ToolType::Pickaxe),
                ToolTier::Iron
            ),
            Some((ItemId::DIAMOND, 1))
        );
        assert!(reg.can_harvest(
            BlockId::DIAMOND_ORE,
            Some(ToolType::Pickaxe),
            ToolTier::Diamond
        ));
        assert_eq!(
            reg.get_drop(
                BlockId::DIAMOND_ORE,
                Some(ToolType::Pickaxe),
                ToolTier::Diamond
            ),
            Some((ItemId::DIAMOND, 1))
        );

        // 5. Tool speed reduces break time
        let wood_time = reg.get_break_time(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Wood);
        let diamond_time =
            reg.get_break_time(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Diamond);
        assert!(diamond_time < wood_time);
    }

    #[test]
    fn test_block_support_and_attachment_rules() {
        let reg = BlockRegistry::new();
        let pos = IVec3::new(10, 64, 10);

        // 1. Tall grass requires soil below and no water at pos
        assert!(can_block_survive(BlockId::TALL_GRASS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::GRASS } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::TALL_GRASS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::STONE } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::TALL_GRASS, pos, &reg, |p| {
            if p == pos { BlockId::WATER } else if p == pos + IVec3::NEG_Y { BlockId::GRASS } else { BlockId::AIR }
        }));

        // 2. Wheat requires farmland below
        assert!(can_block_survive(BlockId::WHEAT_2, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::FARMLAND } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::WHEAT_2, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::DIRT } else { BlockId::AIR }
        }));

        // 3. Cactus requires sand or cactus below, and NO solid horizontal neighbors
        assert!(can_block_survive(BlockId::CACTUS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::SAND } else { BlockId::AIR }
        }));
        assert!(can_block_survive(BlockId::CACTUS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::CACTUS } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::CACTUS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::DIRT } else { BlockId::AIR }
        }));
        // Cactus with solid neighbor fails
        assert!(!can_block_survive(BlockId::CACTUS, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y {
                BlockId::SAND
            } else if p == pos + IVec3::X {
                BlockId::COBBLESTONE
            } else {
                BlockId::AIR
            }
        }));

        // 4. Torch can stand on solid floor or cling to solid wall
        assert!(can_block_survive(BlockId::TORCH, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::COBBLESTONE } else { BlockId::AIR }
        }));
        assert!(can_block_survive(BlockId::TORCH, pos, &reg, |p| {
            if p == pos + IVec3::X { BlockId::STONE } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::TORCH, pos, &reg, |_p| {
            BlockId::AIR
        }));

        // 5. Lily pad requires water below
        assert!(can_block_survive(BlockId::LILY_PAD, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::WATER } else { BlockId::AIR }
        }));
        assert!(!can_block_survive(BlockId::LILY_PAD, pos, &reg, |p| {
            if p == pos + IVec3::NEG_Y { BlockId::GRASS } else { BlockId::AIR }
        }));
    }

    #[test]
    fn test_break_unsupported_attached_cascading() {
        let reg = BlockRegistry::new();
        let mut world = crate::world::World::new();
        let chunk = crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0));
        world.insert_chunk(chunk);

        // 1. Cactus column: Sand at (5, 1, 5), Cactus at (5, 2, 5), (5, 3, 5), (5, 4, 5)
        world.set_block(IVec3::new(5, 1, 5), BlockId::SAND);
        world.set_block(IVec3::new(5, 2, 5), BlockId::CACTUS);
        world.set_block(IVec3::new(5, 3, 5), BlockId::CACTUS);
        world.set_block(IVec3::new(5, 4, 5), BlockId::CACTUS);

        // Breaking the base sand block cascades all 3 cactus blocks!
        world.set_block(IVec3::new(5, 1, 5), BlockId::AIR);
        let (drops, modified) = break_unsupported_attached(IVec3::new(5, 1, 5), &mut world, &reg);

        assert_eq!(modified.len(), 3);
        assert_eq!(world.get_block(IVec3::new(5, 2, 5)), BlockId::AIR);
        assert_eq!(world.get_block(IVec3::new(5, 3, 5)), BlockId::AIR);
        assert_eq!(world.get_block(IVec3::new(5, 4, 5)), BlockId::AIR);
        assert_eq!(drops.len(), 3);
        for (_, drop_slot) in drops {
            assert_eq!(drop_slot.item_id, ItemId::CACTUS);
            assert_eq!(drop_slot.count, 1);
        }

        // 2. Tall grass on grass block
        world.set_block(IVec3::new(8, 1, 8), BlockId::GRASS);
        world.set_block(IVec3::new(8, 2, 8), BlockId::TALL_GRASS);

        // Breaking grass block breaks tall grass and drops wheat seeds
        world.set_block(IVec3::new(8, 1, 8), BlockId::AIR);
        let (drops_grass, modified_grass) =
            break_unsupported_attached(IVec3::new(8, 1, 8), &mut world, &reg);
        assert_eq!(modified_grass, vec![IVec3::new(8, 2, 8)]);
        assert_eq!(world.get_block(IVec3::new(8, 2, 8)), BlockId::AIR);
        assert_eq!(drops_grass.len(), 1);
        assert_eq!(drops_grass[0].1.item_id, ItemId::WHEAT_SEEDS);

        // 3. Wheat on farmland
        world.set_block(IVec3::new(3, 1, 3), BlockId::FARMLAND);
        world.set_block(IVec3::new(3, 2, 3), BlockId::WHEAT_3);

        world.set_block(IVec3::new(3, 1, 3), BlockId::AIR);
        let (drops_wheat, modified_wheat) =
            break_unsupported_attached(IVec3::new(3, 1, 3), &mut world, &reg);
        assert_eq!(modified_wheat, vec![IVec3::new(3, 2, 3)]);
        assert_eq!(world.get_block(IVec3::new(3, 2, 3)), BlockId::AIR);
        // Stage 3 drops wheat and seeds
        assert!(drops_wheat.iter().any(|d| d.1.item_id == ItemId::WHEAT));
        assert!(drops_wheat.iter().any(|d| d.1.item_id == ItemId::WHEAT_SEEDS));

        // 4. Placing solid block next to cactus breaks the cactus
        world.set_block(IVec3::new(12, 1, 12), BlockId::SAND);
        world.set_block(IVec3::new(12, 2, 12), BlockId::CACTUS);
        // Place cobblestone at (11, 2, 12) next to cactus
        world.set_block(IVec3::new(11, 2, 12), BlockId::COBBLESTONE);
        let (drops_cactus, modified_cactus) =
            break_unsupported_attached(IVec3::new(11, 2, 12), &mut world, &reg);
        assert_eq!(modified_cactus, vec![IVec3::new(12, 2, 12)]);
        assert_eq!(world.get_block(IVec3::new(12, 2, 12)), BlockId::AIR);
        assert_eq!(drops_cactus[0].1.item_id, ItemId::CACTUS);
    }
}
