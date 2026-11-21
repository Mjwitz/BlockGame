pub mod temple_gen;
pub mod terrain;

pub use temple_gen::{biome_to_temple_kind, stamp_temple_exterior, TemplePlacement};
pub use terrain::{Biome, TerrainGenerator};
