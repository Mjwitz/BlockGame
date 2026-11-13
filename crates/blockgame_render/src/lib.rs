pub mod atlas;
pub mod camera;
pub mod day_night;
pub mod font;
pub mod mesh;
pub mod pipeline;
pub mod ui;
pub mod wireframe;

pub use atlas::TextureAtlas;
pub use camera::{Camera, CameraUniform, Frustum, FrustumPlane};
pub use day_night::DayNightCycle;
pub use mesh::{
    build_animals_mesh, build_arrows_mesh, build_celestial_mesh, build_falling_blocks_mesh,
    build_item_drops_mesh, build_monsters_mesh, build_players_mesh, ChunkMesh, ChunkMesher,
    VoxelVertex,
};
pub use pipeline::{
    create_depth_texture, DynamicMeshBuffers, GpuMeshBuffers, VoxelPipeline, DEPTH_FORMAT,
};
pub use ui::{UiBuilder, UiMesh, UiPipeline, UiVertex};
pub use wireframe::{LineVertex, WireframePipeline};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
