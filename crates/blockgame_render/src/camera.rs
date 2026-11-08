use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub sun_dir: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 4],
            sun_dir: [0.4, 0.8, 0.3, 0.0],
        }
    }
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // in radians
    pub pitch: f32, // in radians
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            fovy: 60.0_f32.to_radians(),
            znear: 0.1,
            zfar: 500.0,
        }
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn build_view_projection_matrix(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_to_mat4(self.position, self.forward(), Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(self.fovy, aspect, self.znear, self.zfar);
        proj * view
    }

    pub fn update_uniform(&self, uniform: &mut CameraUniform, aspect: f32) {
        let view_proj = self.build_view_projection_matrix(aspect);
        uniform.view_proj = view_proj.to_cols_array_2d();
        uniform.camera_pos = [self.position.x, self.position.y, self.position.z, 1.0];
        // Normalized sunlight direction vector
        let sun = Vec3::new(0.5, 0.8, 0.3).normalize();
        uniform.sun_dir = [sun.x, sun.y, sun.z, 0.0];
    }
}
