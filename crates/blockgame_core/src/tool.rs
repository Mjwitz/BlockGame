#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    None = 0,
    Gold = 1,
    Wood = 2,
    Stone = 3,
    Copper = 4,
    Iron = 5,
    Diamond = 6,
}

impl Default for ToolTier {
    fn default() -> Self {
        Self::None
    }
}

impl ToolTier {
    #[inline]
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Self::None => 1.0,
            Self::Gold => 12.0,
            Self::Wood => 2.0,
            Self::Stone => 4.0,
            Self::Copper => 5.0,
            Self::Iron => 6.0,
            Self::Diamond => 8.0,
        }
    }

    #[inline]
    pub fn max_durability(&self) -> u16 {
        match self {
            Self::None => 0,
            Self::Gold => 32,
            Self::Wood => 59,
            Self::Stone => 131,
            Self::Copper => 190,
            Self::Iron => 250,
            Self::Diamond => 1561,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    Hoe,
}
