pub mod atlas;
pub mod camera;
pub mod character_mesh;
pub mod character_pipeline;
pub mod day_night;
pub mod font;
pub mod mesh;
pub mod pipeline;
pub mod ui;
pub mod wireframe;

pub use atlas::TextureAtlas;
pub use camera::{Camera, CameraUniform, Frustum, FrustumPlane};
pub use character_mesh::{
    build_character_rig_mesh, build_player_held_items_mesh, build_single_character_preview_mesh,
    compute_right_hand_transform, preview_camera_uniform, CharacterMesh, CharacterVertex,
};
pub use character_pipeline::CharacterPipeline;
pub use day_night::DayNightCycle;
pub use mesh::{
    build_animals_mesh, build_arrows_mesh, build_celestial_mesh, build_falling_blocks_mesh,
    build_fishing_bobber_mesh, build_item_drops_mesh, build_monsters_mesh,
    build_monsters_mesh_animated, build_wind_turbine_rotors_mesh, ChunkMesh, ChunkMesher,
    VoxelVertex,
};
pub use pipeline::{
    create_depth_texture, DynamicMeshBuffers, GpuMeshBuffers, VoxelPipeline, DEPTH_FORMAT,
};
pub use ui::{
    CharacterCreatorLayout, CharacterCreatorTab, CharacterCreatorUiState, FishingHudPhase,
    FishingHudState, ShopUiAction, ShopUiTab, UiBuilder, UiMesh, UiPipeline, UiVertex,
};
pub use wireframe::{LineVertex, WireframePipeline};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
