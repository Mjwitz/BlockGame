pub mod atlas;
pub mod camera;
pub mod mesh;
pub mod pipeline;
pub mod wireframe;

pub use atlas::TextureAtlas;
pub use camera::{Camera, CameraUniform};
pub use mesh::{ChunkMesh, ChunkMesher, VoxelVertex};
pub use pipeline::{create_depth_texture, GpuMeshBuffers, VoxelPipeline, DEPTH_FORMAT};
pub use wireframe::{LineVertex, WireframePipeline};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
