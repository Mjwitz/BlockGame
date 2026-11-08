use crate::direction::Direction;

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

        registry.register(BlockDefinition {
            id: BlockId::AIR,
            name: "Air",
            is_solid: false,
            is_transparent: true,
            luminance: 0,
            textures: [0; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::STONE,
            name: "Stone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [1; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::DIRT,
            name: "Dirt",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [2; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::GRASS,
            name: "Grass",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY: Grass Top (3), NegY: Dirt (2), Sides: Grass Side (4)
            textures: [3, 2, 4, 4, 4, 4],
        });

        registry.register(BlockDefinition {
            id: BlockId::COBBLESTONE,
            name: "Cobblestone",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [5; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::WOOD_LOG,
            name: "Wood Log",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            // PosY, NegY: Wood Log Top (7), Sides: Wood Log Side (6)
            textures: [7, 7, 6, 6, 6, 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::LEAVES,
            name: "Leaves",
            is_solid: true,
            is_transparent: true, // Alpha cutout
            luminance: 0,
            textures: [8; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::SAND,
            name: "Sand",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [9; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::PLANKS,
            name: "Planks",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [10; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::GLASS,
            name: "Glass",
            is_solid: true,
            is_transparent: true,
            luminance: 0,
            textures: [11; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::BEDROCK,
            name: "Bedrock",
            is_solid: true,
            is_transparent: false,
            luminance: 0,
            textures: [12; 6],
        });

        registry.register(BlockDefinition {
            id: BlockId::TORCH,
            name: "Torch",
            is_solid: false,
            is_transparent: true,
            luminance: 14,
            textures: [13; 6],
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
}
