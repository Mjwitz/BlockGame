#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    None = 0,
    Wood = 1,
    Stone = 2,
    Iron = 3,
    Diamond = 4,
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
            Self::Wood => 2.0,
            Self::Stone => 4.0,
            Self::Iron => 6.0,
            Self::Diamond => 8.0,
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
