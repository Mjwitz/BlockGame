use std::collections::HashSet;

use glam::IVec3;

use crate::direction::Direction;
use crate::item::{ItemId, ItemSlot};
use crate::tool::{ToolTier, ToolType};
use crate::world::World;

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
    pub const FARMLAND_MOIST: Self = Self(64);
    pub const BRICKS: Self = Self(65);
    pub const WHITE_WOOL: Self = Self(66);
    pub const RED_WOOL: Self = Self(67);
    pub const ORANGE_WOOL: Self = Self(68);
    pub const YELLOW_WOOL: Self = Self(69);
    pub const LIME_WOOL: Self = Self(70);
    pub const GREEN_WOOL: Self = Self(71);
    pub const CYAN_WOOL: Self = Self(72);
    pub const BLUE_WOOL: Self = Self(73);
    pub const PURPLE_WOOL: Self = Self(74);
    pub const PINK_WOOL: Self = Self(75);
    pub const BROWN_WOOL: Self = Self(76);
    pub const GRAY_WOOL: Self = Self(77);
    pub const BLACK_WOOL: Self = Self(78);
    pub const OAK_STAIRS: Self = Self(79);
    pub const COBBLESTONE_STAIRS: Self = Self(80);
    pub const BRICK_STAIRS: Self = Self(81);
    pub const OAK_FENCE: Self = Self(82);
    pub const OAK_DOOR_CLOSED: Self = Self(83);
    pub const OAK_DOOR_OPEN: Self = Self(84);
    pub const BED: Self = Self(85);
    pub const POPPY: Self = Self(86);
    pub const DANDELION: Self = Self(87);
    pub const BLUE_ORCHID: Self = Self(88);
    pub const ALLIUM: Self = Self(89);
    pub const ORANGE_TULIP: Self = Self(90);
    pub const PINK_TULIP: Self = Self(91);
    pub const OXEYE_DAISY: Self = Self(92);
    pub const PUMPKIN: Self = Self(93);
    pub const JACK_O_LANTERN: Self = Self(94);
    pub const MELON: Self = Self(95);

    #[inline]
    pub const fn is_air(&self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_water(&self) -> bool {
        self.0 == Self::WATER.0 || self.0 == Self::SEAGRASS.0
    }

    #[inline]
    pub const fn is_farmland(&self) -> bool {
        self.0 == Self::FARMLAND.0 || self.0 == Self::FARMLAND_MOIST.0
    }

    #[inline]
    pub const fn is_flower(&self) -> bool {
        matches!(
            *self,
            Self::POPPY
                | Self::DANDELION
                | Self::BLUE_ORCHID
                | Self::ALLIUM
                | Self::ORANGE_TULIP
                | Self::PINK_TULIP
                | Self::OXEYE_DAISY
        )
    }

    #[inline]
    pub const fn is_stairs(&self) -> bool {
        matches!(
            *self,
            Self::OAK_STAIRS | Self::COBBLESTONE_STAIRS | Self::BRICK_STAIRS
        )
    }

    #[inline]
    pub const fn luminance(&self) -> u8 {
        match self.0 {
            13 => 14, // TORCH
            27 => 13, // FURNACE_LIT
            62 => 15, // SEA_LANTERN
            94 => 15, // JACK_O_LANTERN
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
                | Self::POPPY
                | Self::DANDELION
                | Self::BLUE_ORCHID
                | Self::ALLIUM
                | Self::ORANGE_TULIP
                | Self::PINK_TULIP
                | Self::OXEYE_DAISY
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
            | Self::FARMLAND
            | Self::FARMLAND_MOIST
            | Self::POPPY
            | Self::DANDELION
            | Self::BLUE_ORCHID
            | Self::ALLIUM
            | Self::ORANGE_TULIP
            | Self::PINK_TULIP
            | Self::OXEYE_DAISY => false,
            _ => true,
        }
    }

    /// Whether this block is affected by gravity (falls when unsupported).
    #[inline]
    pub const fn has_gravity(&self) -> bool {
        matches!(*self, Self::SAND | Self::RED_SAND | Self::GRAVEL)
    }

    /// Whether this block is fragile and gets crushed/dropped when a falling block lands on it.
    #[inline]
    pub const fn is_fragile(&self) -> bool {
        matches!(
            *self,
            Self::TORCH
                | Self::TALL_GRASS
                | Self::WHEAT_0
                | Self::WHEAT_1
                | Self::WHEAT_2
                | Self::WHEAT_3
                | Self::LILY_PAD
                | Self::POPPY
                | Self::DANDELION
                | Self::BLUE_ORCHID
                | Self::ALLIUM
                | Self::ORANGE_TULIP
                | Self::PINK_TULIP
                | Self::OXEYE_DAISY
        )
    }

    /// Whether this block is a leaf block subject to decay when logs are absent.
    #[inline]
    pub const fn is_leaves(&self) -> bool {
        matches!(
            *self,
            Self::LEAVES
                | Self::BIRCH_LEAVES
                | Self::SPRUCE_LEAVES
                | Self::ACACIA_LEAVES
                | Self::JUNGLE_LEAVES
                | Self::DARK_OAK_LEAVES
        )
    }

    /// Whether this block is a tree log block capable of supporting leaves.
    #[inline]
    pub const fn is_log(&self) -> bool {
        matches!(
            *self,
            Self::WOOD_LOG
                | Self::BIRCH_LOG
                | Self::SPRUCE_LOG
                | Self::ACACIA_LOG
                | Self::JUNGLE_LOG
                | Self::DARK_OAK_LOG
        )
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

        registry.register(BlockDefinition {
            id: BlockId::FARMLAND_MOIST,
            name: "Moist Farmland",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: Moist Farmland Top (244), NegY: Dirt (2), Sides: Dirt (2)
            textures: [244, 2, 2, 2, 2, 2],
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

        registry.register(BlockDefinition {
            id: BlockId::BRICKS,
            name: "Bricks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [288; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::BRICKS),
            drop_count: (1, 1),
        });

        let wool_defs = [
            (BlockId::WHITE_WOOL, "White Wool", 289, ItemId::WHITE_WOOL),
            (BlockId::RED_WOOL, "Red Wool", 290, ItemId::RED_WOOL),
            (BlockId::ORANGE_WOOL, "Orange Wool", 291, ItemId::ORANGE_WOOL),
            (BlockId::YELLOW_WOOL, "Yellow Wool", 292, ItemId::YELLOW_WOOL),
            (BlockId::LIME_WOOL, "Lime Wool", 293, ItemId::LIME_WOOL),
            (BlockId::GREEN_WOOL, "Green Wool", 294, ItemId::GREEN_WOOL),
            (BlockId::CYAN_WOOL, "Cyan Wool", 295, ItemId::CYAN_WOOL),
            (BlockId::BLUE_WOOL, "Blue Wool", 296, ItemId::BLUE_WOOL),
            (BlockId::PURPLE_WOOL, "Purple Wool", 297, ItemId::PURPLE_WOOL),
            (BlockId::PINK_WOOL, "Pink Wool", 298, ItemId::PINK_WOOL),
            (BlockId::BROWN_WOOL, "Brown Wool", 299, ItemId::BROWN_WOOL),
            (BlockId::GRAY_WOOL, "Gray Wool", 300, ItemId::GRAY_WOOL),
            (BlockId::BLACK_WOOL, "Black Wool", 301, ItemId::BLACK_WOOL),
        ];
        for (id, name, tile, drop) in wool_defs {
            registry.register(BlockDefinition {
                id,
                name,
                is_solid: true,
                is_transparent: false,
                luminance: 0,
                textures: [tile; 6],
                hardness: 0.8,
                required_tool: None,
                min_tier: ToolTier::None,
                drop_item: Some(drop),
                drop_count: (1, 1),
            });
        }

        registry.register(BlockDefinition {
            id: BlockId::OAK_STAIRS,
            name: "Oak Stairs",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [10; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::OAK_STAIRS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::COBBLESTONE_STAIRS,
            name: "Cobblestone Stairs",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [5; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::COBBLESTONE_STAIRS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::BRICK_STAIRS,
            name: "Brick Stairs",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [288; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Pickaxe),
            min_tier: ToolTier::Wood,
            drop_item: Some(ItemId::BRICK_STAIRS),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::OAK_FENCE,
            name: "Oak Fence",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [10; 6],
            hardness: 2.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::OAK_FENCE),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::OAK_DOOR_CLOSED,
            name: "Oak Door",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [316; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::OAK_DOOR),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::OAK_DOOR_OPEN,
            name: "Oak Door (Open)",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [316; 6],
            hardness: 3.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::OAK_DOOR),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::BED,
            name: "Bed",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            // PosY: Bed Top (317), NegY: Planks (10), Sides: Bed Side (318)
            textures: [317, 10, 318, 318, 318, 318],
            hardness: 0.2,
            required_tool: None,
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::BED),
            drop_count: (1, 1),
        });

        let flower_defs = [
            (BlockId::POPPY, "Poppy", 319, ItemId::POPPY),
            (BlockId::DANDELION, "Dandelion", 320, ItemId::DANDELION),
            (BlockId::BLUE_ORCHID, "Blue Orchid", 321, ItemId::BLUE_ORCHID),
            (BlockId::ALLIUM, "Allium", 322, ItemId::ALLIUM),
            (BlockId::ORANGE_TULIP, "Orange Tulip", 323, ItemId::ORANGE_TULIP),
            (BlockId::PINK_TULIP, "Pink Tulip", 324, ItemId::PINK_TULIP),
            (BlockId::OXEYE_DAISY, "Oxeye Daisy", 325, ItemId::OXEYE_DAISY),
        ];
        for (id, name, tile, drop) in flower_defs {
            registry.register(BlockDefinition {
                id,
                name,
                is_solid: false,
                is_transparent: true,
                luminance: 0,
                textures: [tile; 6],
                hardness: 0.0,
                required_tool: None,
                min_tier: ToolTier::None,
                drop_item: Some(drop),
                drop_count: (1, 1),
            });
        }

        registry.register(BlockDefinition {
            id: BlockId::PUMPKIN,
            name: "Pumpkin",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 327, NegY: 327, Sides: 326
            textures: [327, 327, 326, 326, 326, 326],
            hardness: 1.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::PUMPKIN),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::JACK_O_LANTERN,
            name: "Jack o'Lantern",
            is_solid: true,
            is_transparent: false,
            luminance: 15,
            // PosY: 327, NegY: 327, PosZ: 328 (lit face), other sides: 326
            textures: [327, 327, 328, 326, 326, 326],
            hardness: 1.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::JACK_O_LANTERN),
            drop_count: (1, 1),
        });

        registry.register(BlockDefinition {
            id: BlockId::MELON,
            name: "Melon",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: 330, NegY: 330, Sides: 329
            textures: [330, 330, 329, 329, 329, 329],
            hardness: 1.0,
            required_tool: Some(ToolType::Axe),
            min_tier: ToolTier::None,
            drop_item: Some(ItemId::MELON_SLICE),
            drop_count: (3, 5),
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
                if held_tool_type == Some(req_tool) {
                    let effective_tier = match held_tier {
                        ToolTier::Gold => ToolTier::Wood,
                        other => other,
                    };
                    if effective_tier >= def.min_tier {
                        return true;
                    }
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
        b if b == BlockId::TALL_GRASS || b.is_flower() => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            below.is_farmland()
                || matches!(
                    below,
                    BlockId::GRASS
                        | BlockId::DIRT
                        | BlockId::PODZOL
                        | BlockId::MYCELIUM
                )
        }
        BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 | BlockId::WHEAT_3 => {
            let current = get_block(pos);
            if current == BlockId::WATER {
                return false;
            }
            let below = get_block(pos + IVec3::NEG_Y);
            below.is_farmland()
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

/// Checks for and breaks any unsupported attached blocks adjacent to or dependent on `trigger_pos`,
/// and detects any gravity-affected blocks (Sand, Gravel, Red Sand) that lose support and must fall.
///
/// Returns:
/// - `extra_drops`: List of dropped item slots with their block positions `(pos, item_slot)`.
/// - `modified_positions`: List of modified block positions `Vec<IVec3>`.
/// - `falling_blocks`: List of gravity blocks that lost support `(BlockId, IVec3)`.
pub fn break_unsupported_attached(
    trigger_pos: IVec3,
    world: &mut crate::world::World,
    registry: &BlockRegistry,
) -> (Vec<(IVec3, ItemSlot)>, Vec<IVec3>, Vec<(BlockId, IVec3)>) {
    let mut extra_drops = Vec::new();
    let mut modified_positions = Vec::new();
    let mut falling_blocks = Vec::new();

    // Check if any gravity blocks above trigger_pos should fall
    let mut grav_y = trigger_pos.y + 1;
    while world.get_block(IVec3::new(trigger_pos.x, grav_y, trigger_pos.z)).has_gravity() {
        let grav_pos = IVec3::new(trigger_pos.x, grav_y, trigger_pos.z);
        let grav_id = world.get_block(grav_pos);
        world.set_block(grav_pos, BlockId::AIR);
        modified_positions.push(grav_pos);
        falling_blocks.push((grav_id, grav_pos));
        grav_y += 1;
    }

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

            // Check if any gravity blocks above check_pos should fall now that check_pos is broken
            let mut gy = check_pos.y + 1;
            while world.get_block(IVec3::new(check_pos.x, gy, check_pos.z)).has_gravity() {
                let gp = IVec3::new(check_pos.x, gy, check_pos.z);
                let gid = world.get_block(gp);
                world.set_block(gp, BlockId::AIR);
                modified_positions.push(gp);
                falling_blocks.push((gid, gp));
                gy += 1;
            }

            // Cascade upward (e.g. cactus or stacked plants) and neighbors
            check_queue.push(check_pos + IVec3::Y);
            check_queue.push(check_pos + IVec3::X);
            check_queue.push(check_pos + IVec3::NEG_X);
            check_queue.push(check_pos + IVec3::Z);
            check_queue.push(check_pos + IVec3::NEG_Z);
        }
    }

    (extra_drops, modified_positions, falling_blocks)
}

/// Checks if `pos` (which is AIR or fragile) is adjacent to WATER, and if so,
/// Places a water source at `pos` (from a Water Bucket), replacing open air or fragile blocks
/// and cascading downward.
pub fn place_water_source(
    pos: IVec3,
    world: &mut World,
) -> (Vec<IVec3>, Vec<(IVec3, ItemSlot)>) {
    let current = world.get_block(pos);
    if !current.is_air() && !current.is_fragile() {
        return (Vec::new(), Vec::new());
    }
    cascade_water_downward(pos, world)
}

/// Applies water inflow when an adjacent block is broken: checks if `pos` has a water neighbor,
/// fills `pos` with WATER and cascades downward into open air or fragile blocks.
pub fn apply_water_fill(
    pos: IVec3,
    world: &mut World,
    _registry: &BlockRegistry,
) -> (Vec<IVec3>, Vec<(IVec3, ItemSlot)>) {
    let current = world.get_block(pos);
    if !current.is_air() && !current.is_fragile() {
        return (Vec::new(), Vec::new());
    }

    // Check if above is water, or any horizontal neighbor is water
    let above = pos + IVec3::Y;
    let horizontal = [
        pos + IVec3::X,
        pos - IVec3::X,
        pos + IVec3::Z,
        pos - IVec3::Z,
    ];

    let has_water_neighbor = world.get_block(above).is_water()
        || horizontal.iter().any(|&p| world.get_block(p).is_water());

    if !has_water_neighbor {
        return (Vec::new(), Vec::new());
    }

    cascade_water_downward(pos, world)
}

fn cascade_water_downward(
    pos: IVec3,
    world: &mut World,
) -> (Vec<IVec3>, Vec<(IVec3, ItemSlot)>) {
    let mut modified_positions = Vec::new();
    let mut extra_drops = Vec::new();

    let current = world.get_block(pos);
    if current.is_fragile() {
        let drop_item = match current {
            BlockId::TORCH => ItemSlot::new(crate::item::ItemId::TORCH, 1),
            BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 => {
                ItemSlot::new(crate::item::ItemId::WHEAT_SEEDS, 1)
            }
            BlockId::WHEAT_3 => ItemSlot::new(crate::item::ItemId::WHEAT, 1),
            BlockId::TALL_GRASS => ItemSlot::new(crate::item::ItemId::WHEAT_SEEDS, 1),
            BlockId::LILY_PAD => ItemSlot::new(crate::item::ItemId::LILY_PAD, 1),
            _ => ItemSlot::EMPTY,
        };
        if !drop_item.is_empty() {
            extra_drops.push((pos, drop_item));
        }
    }

    // Fill pos with water
    world.set_block(pos, BlockId::WATER);
    modified_positions.push(pos);

    // Cascade downward into open air or wash away fragile blocks
    let mut check_pos = pos - IVec3::Y;
    while check_pos.y >= 0 {
        let b = world.get_block(check_pos);
        if b.is_air() {
            world.set_block(check_pos, BlockId::WATER);
            modified_positions.push(check_pos);
            check_pos -= IVec3::Y;
        } else if b.is_fragile() {
            let drop_item = match b {
                BlockId::TORCH => ItemSlot::new(crate::item::ItemId::TORCH, 1),
                BlockId::WHEAT_0 | BlockId::WHEAT_1 | BlockId::WHEAT_2 => {
                    ItemSlot::new(crate::item::ItemId::WHEAT_SEEDS, 1)
                }
                BlockId::WHEAT_3 => ItemSlot::new(crate::item::ItemId::WHEAT, 1),
                BlockId::TALL_GRASS => ItemSlot::new(crate::item::ItemId::WHEAT_SEEDS, 1),
                BlockId::LILY_PAD => ItemSlot::new(crate::item::ItemId::LILY_PAD, 1),
                _ => ItemSlot::EMPTY,
            };
            if !drop_item.is_empty() {
                extra_drops.push((check_pos, drop_item));
            }
            world.set_block(check_pos, BlockId::WATER);
            modified_positions.push(check_pos);
            check_pos -= IVec3::Y;
        } else {
            // Hit solid block below
            break;
        }
    }

    (modified_positions, extra_drops)
}

/// Checks if a Farmland block at `farmland_pos` has a water source within horizontal radius 4
/// (and vertical offset -1..=1).
pub fn is_farmland_hydrated<F>(farmland_pos: IVec3, mut get_block: F) -> bool
where
    F: FnMut(IVec3) -> BlockId,
{
    for dy in -1..=1 {
        for dz in -4..=4 {
            for dx in -4..=4 {
                let check_pos = farmland_pos + IVec3::new(dx, dy, dz);
                let block = get_block(check_pos);
                if block.is_water() {
                    return true;
                }
            }
        }
    }
    false
}

/// Determines whether a cactus column starting at or containing `pos` can grow, and if so,
/// returns the voxel position where the new cactus block should be placed.
/// Max height is strictly 3 blocks.
pub fn try_grow_cactus<F>(
    pos: IVec3,
    registry: &BlockRegistry,
    mut get_block: F,
) -> Option<IVec3>
where
    F: FnMut(IVec3) -> BlockId,
{
    if get_block(pos) != BlockId::CACTUS {
        return None;
    }

    // 1. Trace down to the bottom of the cactus column
    let mut base = pos;
    while get_block(base + IVec3::NEG_Y) == BlockId::CACTUS {
        base += IVec3::NEG_Y;
    }

    // 2. Measure total column height and find top
    let mut height = 1;
    let mut top = base;
    while get_block(top + IVec3::Y) == BlockId::CACTUS {
        top += IVec3::Y;
        height += 1;
    }

    // Max height is 3 blocks
    if height >= 3 {
        return None;
    }

    let grow_pos = top + IVec3::Y;
    if !get_block(grow_pos).is_air() {
        return None;
    }

    // Check if the new cactus block can survive at grow_pos
    // (must have no adjacent solid blocks on north, south, east, west)
    if can_block_survive(BlockId::CACTUS, grow_pos, registry, &mut get_block) {
        Some(grow_pos)
    } else {
        None
    }
}

/// Checks if a leaf block at `leaf_pos` has a path of connected leaves of length <= max_dist (default 4)
/// reaching any tree log.
pub fn is_leaf_supported<F>(leaf_pos: IVec3, max_dist: usize, mut get_block: F) -> bool
where
    F: FnMut(IVec3) -> BlockId,
{
    let target = get_block(leaf_pos);
    if target.is_log() {
        return true;
    }
    if !target.is_leaves() {
        return false;
    }

    let mut queue = std::collections::VecDeque::new();
    let mut visited = HashSet::new();

    queue.push_back((leaf_pos, 0usize));
    visited.insert(leaf_pos);

    while let Some((p, dist)) = queue.pop_front() {
        if dist > 0 && get_block(p).is_log() {
            return true;
        }

        if dist < max_dist {
            for offset in [
                IVec3::X,
                IVec3::NEG_X,
                IVec3::Y,
                IVec3::NEG_Y,
                IVec3::Z,
                IVec3::NEG_Z,
            ] {
                let neighbor = p + offset;
                if visited.insert(neighbor) {
                    let b = get_block(neighbor);
                    if b.is_log() {
                        return true;
                    }
                    if b.is_leaves() {
                        queue.push_back((neighbor, dist + 1));
                    }
                }
            }
        }
    }

    false
}

/// Computes item drops when a leaf block decays.
pub fn get_leaf_decay_drops(leaf: BlockId, pos: IVec3) -> Vec<ItemSlot> {
    let mut drops = Vec::new();
    let hash = (pos.x.wrapping_mul(31) ^ pos.y.wrapping_mul(17) ^ pos.z.wrapping_mul(13)).abs();

    if leaf == BlockId::LEAVES {
        // Oak leaves: ~10% chance to drop an Apple
        if hash % 10 == 0 {
            drops.push(ItemSlot::new(ItemId::APPLE, 1));
        } else if hash % 5 == 0 {
            // ~20% chance to drop a Stick
            drops.push(ItemSlot::new(ItemId::STICK, 1));
        }
    } else if hash % 6 == 0 {
        // Other leaves: chance to drop sticks
        drops.push(ItemSlot::new(ItemId::STICK, 1));
    }

    drops
}

/// Finds all leaf blocks within a radius around `broken_pos` that are no longer supported by any logs.
pub fn find_decaying_leaves_around_pos<F>(
    broken_pos: IVec3,
    radius: i32,
    mut get_block: F,
) -> Vec<IVec3>
where
    F: FnMut(IVec3) -> BlockId + Copy,
{
    let mut decaying = Vec::new();
    for dy in -radius..=radius {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let p = broken_pos + IVec3::new(dx, dy, dz);
                let b = get_block(p);
                if b.is_leaves() && !is_leaf_supported(p, radius as usize, get_block) {
                    decaying.push(p);
                }
            }
        }
    }
    decaying
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
        let (drops, modified, _) = break_unsupported_attached(IVec3::new(5, 1, 5), &mut world, &reg);

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
        let (drops_grass, modified_grass, _) =
            break_unsupported_attached(IVec3::new(8, 1, 8), &mut world, &reg);
        assert_eq!(modified_grass, vec![IVec3::new(8, 2, 8)]);
        assert_eq!(world.get_block(IVec3::new(8, 2, 8)), BlockId::AIR);
        assert_eq!(drops_grass.len(), 1);
        assert_eq!(drops_grass[0].1.item_id, ItemId::WHEAT_SEEDS);

        // 3. Wheat on farmland
        world.set_block(IVec3::new(3, 1, 3), BlockId::FARMLAND);
        world.set_block(IVec3::new(3, 2, 3), BlockId::WHEAT_3);

        world.set_block(IVec3::new(3, 1, 3), BlockId::AIR);
        let (drops_wheat, modified_wheat, _) =
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
        let (drops_cactus, modified_cactus, _) =
            break_unsupported_attached(IVec3::new(11, 2, 12), &mut world, &reg);
        assert_eq!(modified_cactus, vec![IVec3::new(12, 2, 12)]);
        assert_eq!(world.get_block(IVec3::new(12, 2, 12)), BlockId::AIR);
        assert_eq!(drops_cactus[0].1.item_id, ItemId::CACTUS);

        // 5. Breaking block beneath sand column triggers falling blocks
        world.set_block(IVec3::new(2, 1, 2), BlockId::STONE);
        world.set_block(IVec3::new(2, 2, 2), BlockId::SAND);
        world.set_block(IVec3::new(2, 3, 2), BlockId::GRAVEL);
        world.set_block(IVec3::new(2, 1, 2), BlockId::AIR);
        let (_, modified_sand, falling) =
            break_unsupported_attached(IVec3::new(2, 1, 2), &mut world, &reg);
        assert_eq!(falling.len(), 2);
        assert_eq!(falling[0], (BlockId::SAND, IVec3::new(2, 2, 2)));
        assert_eq!(falling[1], (BlockId::GRAVEL, IVec3::new(2, 3, 2)));
        assert_eq!(world.get_block(IVec3::new(2, 2, 2)), BlockId::AIR);
        assert_eq!(world.get_block(IVec3::new(2, 3, 2)), BlockId::AIR);
        assert!(modified_sand.contains(&IVec3::new(2, 2, 2)));
        assert!(modified_sand.contains(&IVec3::new(2, 3, 2)));
    }

    #[test]
    fn test_water_fill_into_exposed_space() {
        let mut world = World::new();
        let reg = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // 1. Water at (6, 5, 5), wall at (5, 5, 5), floor at (5, 4, 5)
        world.set_block(IVec3::new(6, 5, 5), BlockId::WATER);
        world.set_block(IVec3::new(5, 5, 5), BlockId::STONE);
        world.set_block(IVec3::new(5, 4, 5), BlockId::STONE);

        // Break wall -> becomes AIR
        world.set_block(IVec3::new(5, 5, 5), BlockId::AIR);
        let (modified, drops) = apply_water_fill(IVec3::new(5, 5, 5), &mut world, &reg);

        assert!(drops.is_empty());
        assert_eq!(world.get_block(IVec3::new(5, 5, 5)), BlockId::WATER);
        assert_eq!(modified, vec![IVec3::new(5, 5, 5)]);
    }

    #[test]
    fn test_water_fill_cascades_downward_and_washes_fragile_blocks() {
        let mut world = World::new();
        let reg = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Water at (4, 10, 4)
        world.set_block(IVec3::new(4, 10, 4), BlockId::WATER);
        // Wall at (3, 10, 4)
        world.set_block(IVec3::new(3, 10, 4), BlockId::STONE);
        // Air at (3, 9, 4)
        world.set_block(IVec3::new(3, 9, 4), BlockId::AIR);
        // Torch at (3, 8, 4)
        world.set_block(IVec3::new(3, 8, 4), BlockId::TORCH);
        // Solid ground at (3, 7, 4)
        world.set_block(IVec3::new(3, 7, 4), BlockId::STONE);

        // Break wall at (3, 10, 4)
        world.set_block(IVec3::new(3, 10, 4), BlockId::AIR);
        let (modified, drops) = apply_water_fill(IVec3::new(3, 10, 4), &mut world, &reg);

        // Water fills (3, 10, 4), cascades down to (3, 9, 4), and washes away torch at (3, 8, 4)
        assert_eq!(world.get_block(IVec3::new(3, 10, 4)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(3, 9, 4)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(3, 8, 4)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(3, 7, 4)), BlockId::STONE);

        assert_eq!(modified.len(), 3);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].0, IVec3::new(3, 8, 4));
        assert_eq!(drops[0].1.item_id, ItemId::TORCH);
    }

    #[test]
    fn test_block_placement_in_water_displaces_water() {
        let mut world = World::new();
        let reg = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Seabed at (5, 1, 5), water at (5, 2, 5)
        world.set_block(IVec3::new(5, 1, 5), BlockId::STONE);
        world.set_block(IVec3::new(5, 2, 5), BlockId::WATER);

        assert_eq!(world.get_block(IVec3::new(5, 2, 5)), BlockId::WATER);

        // Place cobblestone at (5, 2, 5), displacing water
        world.set_block(IVec3::new(5, 2, 5), BlockId::COBBLESTONE);
        assert_eq!(world.get_block(IVec3::new(5, 2, 5)), BlockId::COBBLESTONE);
        assert!(reg.is_solid(world.get_block(IVec3::new(5, 2, 5))));
    }

    #[test]
    fn test_bucket_scoop_and_water_place_mechanics() {
        let mut world = World::new();
        let reg = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Water at (2, 5, 2), lily pad on top at (2, 6, 2)
        world.set_block(IVec3::new(2, 5, 2), BlockId::WATER);
        world.set_block(IVec3::new(2, 6, 2), BlockId::LILY_PAD);

        // 1. Scoop water at (2, 5, 2)
        world.set_block(IVec3::new(2, 5, 2), BlockId::AIR);
        let (unsupported_drops, modified, _) = break_unsupported_attached(IVec3::new(2, 5, 2), &mut world, &reg);

        // Lily pad above has lost its water support and breaks into item drop
        assert_eq!(world.get_block(IVec3::new(2, 5, 2)), BlockId::AIR);
        assert_eq!(world.get_block(IVec3::new(2, 6, 2)), BlockId::AIR);
        assert!(unsupported_drops.iter().any(|(p, slot)| *p == IVec3::new(2, 6, 2) && slot.item_id == ItemId::LILY_PAD));
        assert!(modified.contains(&IVec3::new(2, 6, 2)));

        // 2. Place water at (2, 5, 2) with air below at (2, 4, 2) and torch at (2, 3, 2)
        world.set_block(IVec3::new(2, 4, 2), BlockId::AIR);
        world.set_block(IVec3::new(2, 3, 2), BlockId::TORCH);
        world.set_block(IVec3::new(2, 2, 2), BlockId::STONE);

        let (water_filled, drops) = place_water_source(IVec3::new(2, 5, 2), &mut world);
        assert_eq!(water_filled.len(), 3);
        assert_eq!(world.get_block(IVec3::new(2, 5, 2)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(2, 4, 2)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(2, 3, 2)), BlockId::WATER);
        assert_eq!(world.get_block(IVec3::new(2, 2, 2)), BlockId::STONE);
        assert!(drops.iter().any(|(p, slot)| *p == IVec3::new(2, 3, 2) && slot.item_id == ItemId::TORCH));
    }

    #[test]
    fn test_farmland_hydration_water_range() {
        let mut world = World::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        let farmland_pos = IVec3::new(5, 5, 5);
        world.set_block(farmland_pos, BlockId::FARMLAND);

        // No water nearby initially
        assert!(!is_farmland_hydrated(farmland_pos, |p| world.get_block(p)));

        // Place water 5 blocks away (out of range)
        world.set_block(farmland_pos + IVec3::new(5, 0, 0), BlockId::WATER);
        assert!(!is_farmland_hydrated(farmland_pos, |p| world.get_block(p)));

        // Place water 4 blocks away (in range!)
        world.set_block(farmland_pos + IVec3::new(4, 0, 0), BlockId::WATER);
        assert!(is_farmland_hydrated(farmland_pos, |p| world.get_block(p)));

        // Water diagonally (4, 0, 4) is in range
        world.set_block(farmland_pos + IVec3::new(4, 0, 0), BlockId::AIR);
        world.set_block(farmland_pos + IVec3::new(4, 1, 4), BlockId::WATER);
        assert!(is_farmland_hydrated(farmland_pos, |p| world.get_block(p)));
    }

    #[test]
    fn test_cactus_growth_max_height_3() {
        let mut world = World::new();
        let reg = BlockRegistry::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Sand base at (5, 1, 5)
        world.set_block(IVec3::new(5, 1, 5), BlockId::SAND);
        // 1st cactus at (5, 2, 5)
        world.set_block(IVec3::new(5, 2, 5), BlockId::CACTUS);

        // Cactus at height 1 -> grows to (5, 3, 5)
        let grow1 = try_grow_cactus(IVec3::new(5, 2, 5), &reg, |p| world.get_block(p));
        assert_eq!(grow1, Some(IVec3::new(5, 3, 5)));
        world.set_block(IVec3::new(5, 3, 5), BlockId::CACTUS);

        // Cactus at height 2 -> grows to (5, 4, 5)
        let grow2 = try_grow_cactus(IVec3::new(5, 2, 5), &reg, |p| world.get_block(p));
        assert_eq!(grow2, Some(IVec3::new(5, 4, 5)));
        world.set_block(IVec3::new(5, 4, 5), BlockId::CACTUS);

        // Cactus at height 3 -> cannot grow further!
        let grow3 = try_grow_cactus(IVec3::new(5, 2, 5), &reg, |p| world.get_block(p));
        assert_eq!(grow3, None);

        // Also if queried from top block (5, 4, 5), still returns None
        let grow_top = try_grow_cactus(IVec3::new(5, 4, 5), &reg, |p| world.get_block(p));
        assert_eq!(grow_top, None);

        // Cactus cannot grow if adjacent block at grow position is solid
        world.set_block(IVec3::new(8, 1, 8), BlockId::SAND);
        world.set_block(IVec3::new(8, 2, 8), BlockId::CACTUS);
        // Place solid stone next to (8, 3, 8)
        world.set_block(IVec3::new(9, 3, 8), BlockId::STONE);
        let blocked = try_grow_cactus(IVec3::new(8, 2, 8), &reg, |p| world.get_block(p));
        assert_eq!(blocked, None);
    }

    #[test]
    fn test_leaf_decay_when_logs_removed() {
        let mut world = World::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        // Tree log at (5, 5, 5)
        world.set_block(IVec3::new(5, 5, 5), BlockId::WOOD_LOG);
        // Oak leaves at (5, 6, 5) and (5, 7, 5)
        world.set_block(IVec3::new(5, 6, 5), BlockId::LEAVES);
        world.set_block(IVec3::new(5, 7, 5), BlockId::LEAVES);

        // Leaves are supported initially
        assert!(is_leaf_supported(IVec3::new(5, 6, 5), 4, |p| world.get_block(p)));
        assert!(is_leaf_supported(IVec3::new(5, 7, 5), 4, |p| world.get_block(p)));

        // Remove the wood log -> leaves are no longer supported!
        world.set_block(IVec3::new(5, 5, 5), BlockId::AIR);
        assert!(!is_leaf_supported(IVec3::new(5, 6, 5), 4, |p| world.get_block(p)));
        assert!(!is_leaf_supported(IVec3::new(5, 7, 5), 4, |p| world.get_block(p)));

        // find_decaying_leaves_around_pos discovers both leaves
        let decaying = find_decaying_leaves_around_pos(IVec3::new(5, 5, 5), 4, |p| world.get_block(p));
        assert!(decaying.contains(&IVec3::new(5, 6, 5)));
        assert!(decaying.contains(&IVec3::new(5, 7, 5)));

        // Test decay drops for oak leaves
        let drops = get_leaf_decay_drops(BlockId::LEAVES, IVec3::new(5, 6, 5));
        // Drops should be valid item slots (or empty if RNG didn't roll a drop)
        for drop in drops {
            assert!(drop.item_id == ItemId::APPLE || drop.item_id == ItemId::STICK);
        }
    }

    #[test]
    fn test_farmland_moisture_and_crop_survival() {
        let reg = BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(crate::chunk::Chunk::new(crate::chunk::ChunkPos::new(0, 0, 0)));

        assert!(BlockId::FARMLAND.is_farmland());
        assert!(BlockId::FARMLAND_MOIST.is_farmland());
        assert!(!BlockId::DIRT.is_farmland());

        // Farmland moist definition
        let moist_def = reg.get(BlockId::FARMLAND_MOIST).expect("Moist farmland registered");
        assert_eq!(moist_def.textures[Direction::PosY.index()], 244);
        assert_eq!(moist_def.textures[Direction::NegY.index()], 2);

        // Dry farmland definition
        let dry_def = reg.get(BlockId::FARMLAND).expect("Dry farmland registered");
        assert_eq!(dry_def.textures[Direction::PosY.index()], 150);

        // Plant wheat on dry farmland
        world.set_block(IVec3::new(10, 1, 10), BlockId::FARMLAND);
        world.set_block(IVec3::new(10, 2, 10), BlockId::WHEAT_0);
        assert!(can_block_survive(BlockId::WHEAT_0, IVec3::new(10, 2, 10), &reg, |p| world.get_block(p)));

        // Change to moist farmland -> wheat still survives!
        world.set_block(IVec3::new(10, 1, 10), BlockId::FARMLAND_MOIST);
        assert!(can_block_survive(BlockId::WHEAT_0, IVec3::new(10, 2, 10), &reg, |p| world.get_block(p)));

        // Turn to dirt -> wheat can no longer survive!
        world.set_block(IVec3::new(10, 1, 10), BlockId::DIRT);
        assert!(!can_block_survive(BlockId::WHEAT_0, IVec3::new(10, 2, 10), &reg, |p| world.get_block(p)));

        // Hydration check: water at (12, 1, 10) hydrates farmland at (10, 1, 10) (distance 2 <= 4)
        world.set_block(IVec3::new(10, 1, 10), BlockId::FARMLAND);
        assert!(!is_farmland_hydrated(IVec3::new(10, 1, 10), |p| world.get_block(p)));

        world.set_block(IVec3::new(12, 1, 10), BlockId::WATER);
        assert!(is_farmland_hydrated(IVec3::new(10, 1, 10), |p| world.get_block(p)));
    }
}

