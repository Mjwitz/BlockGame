use crate::direction::Direction;
use crate::item::ItemId;
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

    #[inline]
    pub const fn is_air(&self) -> bool {
        self.0 == 0
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

        // 3. Stone requires Wood Pickaxe or better
        assert!(!reg.can_harvest(BlockId::STONE, None, ToolTier::None));
        assert_eq!(reg.get_drop(BlockId::STONE, None, ToolTier::None), None);
        assert!(reg.can_harvest(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Wood));
        assert_eq!(
            reg.get_drop(BlockId::STONE, Some(ToolType::Pickaxe), ToolTier::Wood),
            Some((ItemId::COBBLESTONE, 1))
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
}
