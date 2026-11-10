use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::day_night::DayNightCycle;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sun_color: [f32; 4], // RGB + Intensity
    pub sky_color: [f32; 4], // RGB + Fog Density
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 4],
            sun_dir: [0.4, 0.8, 0.3, 0.0],
            sun_color: [1.0, 0.96, 0.88, 1.0],
            sky_color: [0.53, 0.81, 0.98, 1.0],
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
        let proj =
            glam::camera::rh::proj::directx::perspective(self.fovy, aspect, self.znear, self.zfar);
        proj * view
    }

    pub fn build_frustum(&self, aspect: f32) -> Frustum {
        let view_proj = self.build_view_projection_matrix(aspect);
        Frustum::from_view_projection(&view_proj)
    }

    pub fn update_uniform(&self, uniform: &mut CameraUniform, aspect: f32) {
        let default_cycle = DayNightCycle::new();
        self.update_uniform_with_atmosphere(uniform, aspect, &default_cycle);
    }

    pub fn update_uniform_with_atmosphere(
        &self,
        uniform: &mut CameraUniform,
        aspect: f32,
        cycle: &DayNightCycle,
    ) {
        let view_proj = self.build_view_projection_matrix(aspect);
        uniform.view_proj = view_proj.to_cols_array_2d();
        uniform.camera_pos = [self.position.x, self.position.y, self.position.z, 1.0];

        let sun = cycle.sun_direction();
        uniform.sun_dir = [sun.x, sun.y, sun.z, 0.0];
        uniform.sun_color = cycle.light_color_and_intensity();
        uniform.sky_color = cycle.sky_color();
    }
}

/// A single normalized 3D plane in Hessian normal form: dot(normal, p) + d = 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrustumPlane {
    pub normal: Vec3,
    pub d: f32,
}

impl FrustumPlane {
    pub fn from_vec4(v: glam::Vec4) -> Self {
        let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        if len > 0.0 {
            Self {
                normal: Vec3::new(v.x / len, v.y / len, v.z / len),
                d: v.w / len,
            }
        } else {
            Self {
                normal: Vec3::ZERO,
                d: 0.0,
            }
        }
    }

    /// Tests whether an Axis-Aligned Bounding Box (AABB) is completely outside (behind) this plane.
    #[inline(always)]
    pub fn is_box_outside(&self, min: Vec3, max: Vec3) -> bool {
        let p = Vec3::new(
            if self.normal.x > 0.0 { max.x } else { min.x },
            if self.normal.y > 0.0 { max.y } else { min.y },
            if self.normal.z > 0.0 { max.z } else { min.z },
        );
        self.normal.dot(p) + self.d < 0.0
    }
}

/// 6-plane viewing frustum extracted from the camera view-projection matrix.
/// Used for high-performance CPU chunk culling before draw call submission.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [FrustumPlane; 6],
}

impl Frustum {
    pub fn from_view_projection(vp: &Mat4) -> Self {
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);

        Self {
            planes: [
                FrustumPlane::from_vec4(r3 + r0), // Left:   w + x >= 0
                FrustumPlane::from_vec4(r3 - r0), // Right:  w - x >= 0
                FrustumPlane::from_vec4(r3 + r1), // Bottom: w + y >= 0
                FrustumPlane::from_vec4(r3 - r1), // Top:    w - y >= 0
                FrustumPlane::from_vec4(r2),      // Near:   z >= 0
                FrustumPlane::from_vec4(r3 - r2), // Far:    w - z >= 0
            ],
        }
    }

    /// Returns true if the AABB intersects or is inside the viewing frustum.
    #[inline(always)]
    pub fn is_box_visible(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            if plane.is_box_outside(min, max) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frustum_culling_box_in_front_and_behind() {
        let camera = Camera::new(Vec3::new(0.0, 10.0, 0.0), (-90.0_f32).to_radians(), 0.0);
        let frustum = camera.build_frustum(16.0 / 9.0);

        // Box directly in front of the camera (forward is -Z when yaw is -90 deg)
        let front_min = Vec3::new(-5.0, 5.0, -30.0);
        let front_max = Vec3::new(5.0, 15.0, -20.0);
        assert!(frustum.is_box_visible(front_min, front_max));

        // Box completely behind the camera (positive Z)
        let back_min = Vec3::new(-5.0, 5.0, 20.0);
        let back_max = Vec3::new(5.0, 15.0, 30.0);
        assert!(!frustum.is_box_visible(back_min, back_max));
    }
}
