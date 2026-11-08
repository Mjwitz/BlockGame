use glam::IVec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    PosY, // Top (+Y)
    NegY, // Bottom (-Y)
    PosZ, // South (+Z)
    NegZ, // North (-Z)
    PosX, // East (+X)
    NegX, // West (-X)
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::PosY,
        Direction::NegY,
        Direction::PosZ,
        Direction::NegZ,
        Direction::PosX,
        Direction::NegX,
    ];

    #[inline]
    pub const fn normal_ivec(&self) -> IVec3 {
        match self {
            Direction::PosY => IVec3::new(0, 1, 0),
            Direction::NegY => IVec3::new(0, -1, 0),
            Direction::PosZ => IVec3::new(0, 0, 1),
            Direction::NegZ => IVec3::new(0, 0, -1),
            Direction::PosX => IVec3::new(1, 0, 0),
            Direction::NegX => IVec3::new(-1, 0, 0),
        }
    }

    #[inline]
    pub const fn normal_f32(&self) -> [f32; 3] {
        match self {
            Direction::PosY => [0.0, 1.0, 0.0],
            Direction::NegY => [0.0, -1.0, 0.0],
            Direction::PosZ => [0.0, 0.0, 1.0],
            Direction::NegZ => [0.0, 0.0, -1.0],
            Direction::PosX => [1.0, 0.0, 0.0],
            Direction::NegX => [-1.0, 0.0, 0.0],
        }
    }

    #[inline]
    pub const fn opposite(&self) -> Self {
        match self {
            Direction::PosY => Direction::NegY,
            Direction::NegY => Direction::PosY,
            Direction::PosZ => Direction::NegZ,
            Direction::NegZ => Direction::PosZ,
            Direction::PosX => Direction::NegX,
            Direction::NegX => Direction::PosX,
        }
    }

    #[inline]
    pub const fn index(&self) -> usize {
        match self {
            Direction::PosY => 0,
            Direction::NegY => 1,
            Direction::PosZ => 2,
            Direction::NegZ => 3,
            Direction::PosX => 4,
            Direction::NegX => 5,
        }
    }
}
