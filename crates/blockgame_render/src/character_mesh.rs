use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use blockgame_core::{
    character::{
        BodyFrame, CharacterAppearance, EyeStyle, FacialHairStyle, HairStyle, PantsStyle,
        RgbColor, ShirtStyle, ShoeStyle,
    },
    item::{ItemId, ItemRegistry},
    net::PlayerModelState,
};

use crate::{
    camera::CameraUniform,
    mesh::{ChunkMesh, VoxelVertex},
};

/// Vertex format for the dedicated stylized character pipeline.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct CharacterVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub light: [f32; 2], // [sunlight, blocklight]
}

impl CharacterVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CharacterVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Light
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 3) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// CPU-side indexed mesh for stylized player characters.
#[derive(Debug, Default, Clone)]
pub struct CharacterMesh {
    pub vertices: Vec<CharacterVertex>,
    pub indices: Vec<u32>,
}

impl CharacterMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Appends a CCW quad in local space transformed by `xform`, computing its exact face normal.
    fn push_quad(
        &mut self,
        xform: Mat4,
        p0: Vec3,
        p1: Vec3,
        p2: Vec3,
        p3: Vec3,
        color: [f32; 3],
        light: [f32; 2],
    ) {
        let w0 = xform.transform_point3(p0);
        let w1 = xform.transform_point3(p1);
        let w2 = xform.transform_point3(p2);
        let w3 = xform.transform_point3(p3);

        let e1 = w1 - w0;
        let e2 = w2 - w0;
        let n = e1.cross(e2).normalize_or_zero();
        let normal = [n.x, n.y, n.z];

        let base = self.vertices.len() as u32;
        for w in [w0, w1, w2, w3] {
            self.vertices.push(CharacterVertex {
                position: [w.x, w.y, w.z],
                normal,
                color,
                light,
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Appends a tapered 4-sided frustum spanning `y0..=y1` with bottom half-extents `(bx, bz)`
    /// and top half-extents `(tx, tz)` centered around `(cx, cz)`.
    #[allow(clippy::too_many_arguments)]
    fn add_tapered_prism(
        &mut self,
        xform: Mat4,
        cx: f32,
        cz: f32,
        y0: f32,
        y1: f32,
        bx: f32,
        bz: f32,
        tx: f32,
        tz: f32,
        color: [f32; 3],
        light: [f32; 2],
    ) {
        let b0 = Vec3::new(cx - bx, y0, cz - bz); // back-left-bottom
        let b1 = Vec3::new(cx + bx, y0, cz - bz); // back-right-bottom
        let b2 = Vec3::new(cx + bx, y0, cz + bz); // front-right-bottom
        let b3 = Vec3::new(cx - bx, y0, cz + bz); // front-left-bottom

        let t0 = Vec3::new(cx - tx, y1, cz - tz); // back-left-top
        let t1 = Vec3::new(cx + tx, y1, cz - tz); // back-right-top
        let t2 = Vec3::new(cx + tx, y1, cz + tz); // front-right-top
        let t3 = Vec3::new(cx - tx, y1, cz + tz); // front-left-top

        // +Z front
        self.push_quad(xform, b3, b2, t2, t3, color, light);
        // -Z back
        self.push_quad(xform, b1, b0, t0, t1, color, light);
        // +X right
        self.push_quad(xform, b2, b1, t1, t2, color, light);
        // -X left
        self.push_quad(xform, b0, b3, t3, t0, color, light);
        // +Y top
        self.push_quad(xform, t3, t2, t1, t0, color, light);
        // -Y bottom
        self.push_quad(xform, b0, b1, b2, b3, color, light);
    }

    /// Convenience axis-aligned box in local space.
    fn add_box(
        &mut self,
        xform: Mat4,
        min: Vec3,
        max: Vec3,
        color: [f32; 3],
        light: [f32; 2],
    ) {
        let cx = 0.5 * (min.x + max.x);
        let cz = 0.5 * (min.z + max.z);
        let hx = 0.5 * (max.x - min.x);
        let hz = 0.5 * (max.z - min.z);
        self.add_tapered_prism(xform, cx, cz, min.y, max.y, hx, hz, hx, hz, color, light);
    }

    /// Appends a stylized chamfered octagonal prism (beveled corners in XZ) for a non-blocky head/crown.
    #[allow(clippy::too_many_arguments)]
    fn add_chamfered_prism(
        &mut self,
        xform: Mat4,
        y0: f32,
        y1: f32,
        rx: f32,
        rz: f32,
        chamfer: f32,
        color: [f32; 3],
        light: [f32; 2],
    ) {
        let cx = (rx - chamfer).max(0.01);
        let cz = (rz - chamfer).max(0.01);
        // 8 points in CCW order when viewed from +Y (so +Z is front)
        let ring = [
            (cx, rz),
            (rx, cz),
            (rx, -cz),
            (cx, -rz),
            (-cx, -rz),
            (-rx, -cz),
            (-rx, cz),
            (-cx, rz),
        ];

        for i in 0..8 {
            let (x_a, z_a) = ring[i];
            let (x_b, z_b) = ring[(i + 1) % 8];
            let p0 = Vec3::new(x_b, y0, z_b);
            let p1 = Vec3::new(x_a, y0, z_a);
            let p2 = Vec3::new(x_a, y1, z_a);
            let p3 = Vec3::new(x_b, y1, z_b);
            self.push_quad(xform, p0, p1, p2, p3, color, light);
        }

        // Top & bottom caps as quad fans from center
        let top_center = Vec3::new(0.0, y1, 0.0);
        let bot_center = Vec3::new(0.0, y0, 0.0);
        for i in (0..8).step_by(2) {
            let (x0, z0) = ring[i];
            let (x1, z1) = ring[(i + 1) % 8];
            let (x2, z2) = ring[(i + 2) % 8];
            // Top cap (+Y normal)
            self.push_quad(
                xform,
                top_center,
                Vec3::new(x2, y1, z2),
                Vec3::new(x1, y1, z1),
                Vec3::new(x0, y1, z0),
                color,
                light,
            );
            // Bottom cap (-Y normal)
            self.push_quad(
                xform,
                bot_center,
                Vec3::new(x0, y0, z0),
                Vec3::new(x1, y0, z1),
                Vec3::new(x2, y0, z2),
                color,
                light,
            );
        }
    }
}

/// Anatomical dimensions derived from a character's `BodyFrame`.
#[derive(Debug, Clone, Copy)]
struct FrameDimensions {
    shoulder_half_x: f32,
    chest_half_x: f32,
    chest_half_z: f32,
    waist_half_x: f32,
    waist_half_z: f32,
    hip_half_x: f32,
    hip_half_z: f32,
    arm_half: f32,
    leg_half: f32,
    leg_offset_x: f32,
}

fn frame_dimensions(frame: BodyFrame) -> FrameDimensions {
    match frame {
        BodyFrame::Masculine => FrameDimensions {
            shoulder_half_x: 0.255,
            chest_half_x: 0.215,
            chest_half_z: 0.125,
            waist_half_x: 0.165,
            waist_half_z: 0.108,
            hip_half_x: 0.175,
            hip_half_z: 0.112,
            arm_half: 0.068,
            leg_half: 0.082,
            leg_offset_x: 0.095,
        },
        BodyFrame::Feminine => FrameDimensions {
            shoulder_half_x: 0.215,
            chest_half_x: 0.178,
            chest_half_z: 0.115,
            waist_half_x: 0.132,
            waist_half_z: 0.095,
            hip_half_x: 0.185,
            hip_half_z: 0.118,
            arm_half: 0.056,
            leg_half: 0.074,
            leg_offset_x: 0.090,
        },
        BodyFrame::Neutral => FrameDimensions {
            shoulder_half_x: 0.235,
            chest_half_x: 0.195,
            chest_half_z: 0.118,
            waist_half_x: 0.152,
            waist_half_z: 0.102,
            hip_half_x: 0.175,
            hip_half_z: 0.112,
            arm_half: 0.062,
            leg_half: 0.078,
            leg_offset_x: 0.092,
        },
    }
}

/// Shared root + right-hand transform calculation so the character mesh and held 3D item align.
fn player_root_transform(player: &PlayerModelState) -> Mat4 {
    // Align local +Z with Camera::forward (cos(yaw), 0, sin(yaw))
    let angle_y = std::f32::consts::FRAC_PI_2 - player.yaw;
    Mat4::from_translation(player.position) * Mat4::from_rotation_y(angle_y)
}

/// Computes the world-space transform of the player's articulated right hand for anchoring held items.
pub fn compute_right_hand_transform(player: &PlayerModelState, time_secs: f32) -> Mat4 {
    let dims = frame_dimensions(player.appearance.body_frame);
    let root = player_root_transform(player);

    let breath = (time_secs * 2.2).sin() * 0.012;
    let crouch_y = if player.is_sneaking { -0.14 } else { 0.0 };
    let torso_lean = if player.is_sneaking { 0.28 } else { 0.0 };

    let walk = player.walk_anim;
    let swing_amp = if walk.abs() > 0.001 { 0.58 } else { 0.0 };
    let leg_swing = walk.sin() * swing_amp;

    let swing_boost = if player.swing_anim > 0.0 {
        (player.swing_anim * std::f32::consts::PI).sin() * 1.25
    } else {
        0.0
    };
    let hold_pose = if player.held_item != ItemId::EMPTY {
        0.38
    } else {
        0.0
    };

    let hip_y = 0.78 + crouch_y + breath;
    let torso_xform = root * Mat4::from_translation(Vec3::new(0.0, hip_y, 0.0)) * Mat4::from_rotation_x(torso_lean);

    // Right arm is on local -X side (matching existing camera/player convention)
    let shoulder_xform = torso_xform
        * Mat4::from_translation(Vec3::new(-dims.shoulder_half_x, 0.52, 0.0))
        * Mat4::from_rotation_x(-(leg_swing * 0.75 + hold_pose + swing_boost))
        * Mat4::from_rotation_z(-0.07);

    let elbow_bend = (0.14 + hold_pose * 0.6 + swing_boost * 0.45 + (-leg_swing).max(0.0) * 0.35).min(1.2);
    let forearm_xform = shoulder_xform
        * Mat4::from_translation(Vec3::new(0.0, -0.28, 0.0))
        * Mat4::from_rotation_x(-elbow_bend);

    forearm_xform * Mat4::from_translation(Vec3::new(0.0, -0.27, 0.04))
}

#[inline]
fn tint_hurt(c: RgbColor, hurt: bool) -> [f32; 3] {
    let rgb = if hurt {
        c.lerp(RgbColor::new(235, 55, 55), 0.55)
    } else {
        c
    };
    rgb.to_f32_array()
}

/// Builds the complete stylized articulated low-poly mesh for all visible players.
pub fn build_character_rig_mesh(players: &[PlayerModelState], time_secs: f32) -> CharacterMesh {
    let mut mesh = CharacterMesh::new();
    for player in players {
        if player.health <= 0.0 {
            continue;
        }
        append_single_player(&mut mesh, player, time_secs);
    }
    mesh
}

/// Convenience builder for the Main Menu Character Creator 3D turntable preview.
pub fn build_single_character_preview_mesh(
    appearance: &CharacterAppearance,
    yaw: f32,
    walk_anim: f32,
    time_secs: f32,
) -> CharacterMesh {
    let mut p = PlayerModelState::new(0, "", Vec3::ZERO, *appearance);
    p.yaw = yaw;
    p.walk_anim = walk_anim;
    let mut mesh = CharacterMesh::new();
    append_single_player(&mut mesh, &p, time_secs);
    mesh
}

/// Builds a studio-lit `CameraUniform` for the Main Menu Character Creator 3D turntable preview.
pub fn preview_camera_uniform(face_zoom: bool, aspect: f32) -> CameraUniform {
    let (eye, target, fovy_deg): (Vec3, Vec3, f32) = if face_zoom {
        (Vec3::new(0.0, 1.56, 1.35), Vec3::new(0.0, 1.53, 0.0), 34.0)
    } else {
        (Vec3::new(0.0, 1.12, 3.25), Vec3::new(0.0, 0.94, 0.0), 38.0)
    };
    let forward = (target - eye).normalize();
    let view = glam::camera::rh::view::look_to_mat4(eye, forward, Vec3::Y);
    let proj = glam::camera::rh::proj::directx::perspective(
        fovy_deg.to_radians(),
        aspect.max(0.1),
        0.1,
        50.0,
    );
    let view_proj = proj * view;

    CameraUniform {
        view_proj: view_proj.to_cols_array_2d(),
        camera_pos: [eye.x, eye.y, eye.z, 1.0],
        sun_dir: [0.45, 0.72, 0.65, 0.0],
        sun_color: [1.0, 0.98, 0.95, 1.0],
        // fog density = 0.0 disables atmospheric fog in character.wgsl
        sky_color: [0.12, 0.14, 0.18, 0.0],
    }
}

fn append_single_player(mesh: &mut CharacterMesh, player: &PlayerModelState, time_secs: f32) {
    let app = player.appearance.sanitized();
    let dims = frame_dimensions(app.body_frame);
    let hurt = player.hurt_timer > 0.0;
    let light = if hurt { [1.0, 0.5] } else { [1.0, 0.0] };

    let skin = tint_hurt(app.skin_tone, hurt);
    let skin_shade = tint_hurt(app.skin_tone.scale(0.88), hurt);
    let hair = tint_hurt(app.hair_color, hurt);
    let hair_dark = tint_hurt(app.hair_color.scale(0.80), hurt);
    let hair_light = tint_hurt(app.hair_color.scale(1.12), hurt);
    let shirt_p = tint_hurt(app.shirt_primary, hurt);
    let shirt_p_dark = tint_hurt(app.shirt_primary.scale(0.84), hurt);
    let shirt_a = tint_hurt(app.shirt_accent, hurt);
    let pants = tint_hurt(app.pants_color, hurt);
    let pants_dark = tint_hurt(app.pants_color.scale(0.82), hurt);
    let pants_light = tint_hurt(app.pants_color.scale(1.14), hurt);
    let shoe = tint_hurt(app.shoe_color, hurt);
    let shoe_sole = tint_hurt(
        match app.shoe_style {
            ShoeStyle::Sneakers => RgbColor::new(236, 236, 240),
            _ => app.shoe_color.scale(0.55),
        },
        hurt,
    );

    let root = player_root_transform(player);
    let breath = (time_secs * 2.2).sin() * 0.012;
    let crouch_y = if player.is_sneaking { -0.14 } else { 0.0 };
    let torso_lean = if player.is_sneaking { 0.28 } else { 0.0 };

    let walk = player.walk_anim;
    let walking = walk.abs() > 0.001;
    let leg_swing = if walking { walk.sin() * 0.58 } else { 0.0 };
    let swing_boost = if player.swing_anim > 0.0 {
        (player.swing_anim * std::f32::consts::PI).sin() * 1.25
    } else {
        0.0
    };
    let hold_pose = if player.held_item != ItemId::EMPTY {
        0.38
    } else {
        0.0
    };

    // Hip pivot at y = 0.78
    let hip_y = 0.78 + crouch_y + breath;
    let hip_xform = root * Mat4::from_translation(Vec3::new(0.0, hip_y, 0.0));
    let torso_xform = hip_xform * Mat4::from_rotation_x(torso_lean);

    // =========================================================================
    // 1. TWO-PART SHAPED TORSO (Pelvis/Waist + Upper Chest) + SHIRT STYLES
    // =========================================================================
    let lower_torso_color = match app.shirt_style {
        ShirtStyle::Jacket => shirt_a,
        _ => shirt_p,
    };
    // Pelvis / hip band (pants top)
    mesh.add_tapered_prism(
        hip_xform,
        0.0,
        0.0,
        -0.06,
        0.09,
        dims.hip_half_x,
        dims.hip_half_z,
        dims.waist_half_x,
        dims.waist_half_z,
        pants,
        light,
    );

    // Lower torso (waist -> mid-chest)
    mesh.add_tapered_prism(
        torso_xform,
        0.0,
        0.0,
        0.07,
        0.28,
        dims.waist_half_x,
        dims.waist_half_z,
        dims.chest_half_x * 0.94,
        dims.chest_half_z * 0.95,
        lower_torso_color,
        light,
    );

    // Upper torso (mid-chest -> shoulders)
    mesh.add_tapered_prism(
        torso_xform,
        0.0,
        0.0,
        0.28,
        0.56,
        dims.chest_half_x * 0.94,
        dims.chest_half_z * 0.95,
        dims.chest_half_x,
        dims.chest_half_z,
        lower_torso_color,
        light,
    );

    // Shirt-specific 3D layers
    match app.shirt_style {
        ShirtStyle::TShirt => {
            // Crew collar trim
            mesh.add_tapered_prism(
                torso_xform,
                0.0,
                0.0,
                0.54,
                0.585,
                0.095,
                0.085,
                0.085,
                0.075,
                shirt_a,
                light,
            );
        }
        ShirtStyle::LongSleeve => {
            // Subtle accent hem & collar
            mesh.add_tapered_prism(
                torso_xform,
                0.0,
                0.0,
                0.54,
                0.585,
                0.095,
                0.085,
                0.085,
                0.075,
                shirt_a,
                light,
            );
            mesh.add_tapered_prism(
                torso_xform,
                0.0,
                0.0,
                0.06,
                0.10,
                dims.waist_half_x + 0.008,
                dims.waist_half_z + 0.008,
                dims.waist_half_x + 0.006,
                dims.waist_half_z + 0.006,
                shirt_a,
                light,
            );
        }
        ShirtStyle::Tunic => {
            // Flared lower tunic skirt over hips + accent belt
            mesh.add_tapered_prism(
                hip_xform,
                0.0,
                0.0,
                -0.14,
                0.08,
                dims.hip_half_x + 0.025,
                dims.hip_half_z + 0.02,
                dims.waist_half_x + 0.012,
                dims.waist_half_z + 0.012,
                shirt_p_dark,
                light,
            );
            // Belt in accent color
            mesh.add_tapered_prism(
                torso_xform,
                0.0,
                0.0,
                0.07,
                0.125,
                dims.waist_half_x + 0.016,
                dims.waist_half_z + 0.016,
                dims.waist_half_x + 0.016,
                dims.waist_half_z + 0.016,
                shirt_a,
                light,
            );
        }
        ShirtStyle::Hoodie => {
            // Folded 3D hood resting behind neck
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.14, 0.48, -dims.chest_half_z - 0.08),
                Vec3::new(0.14, 0.62, -dims.chest_half_z + 0.02),
                shirt_p_dark,
                light,
            );
            // Front kangaroo pocket
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.11, 0.11, dims.waist_half_z - 0.01),
                Vec3::new(0.11, 0.23, dims.waist_half_z + 0.022),
                shirt_p_dark,
                light,
            );
            // Accent drawstrings
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.045, 0.36, dims.chest_half_z),
                Vec3::new(-0.028, 0.53, dims.chest_half_z + 0.015),
                shirt_a,
                light,
            );
            mesh.add_box(
                torso_xform,
                Vec3::new(0.028, 0.36, dims.chest_half_z),
                Vec3::new(0.045, 0.53, dims.chest_half_z + 0.015),
                shirt_a,
                light,
            );
        }
        ShirtStyle::Jacket => {
            // Left & right outer jacket panels over accent undershirt
            let x_out = dims.chest_half_x + 0.014;
            let z_out = dims.chest_half_z + 0.014;
            mesh.add_box(
                torso_xform,
                Vec3::new(-x_out, 0.06, -z_out),
                Vec3::new(-0.045, 0.565, z_out),
                shirt_p,
                light,
            );
            mesh.add_box(
                torso_xform,
                Vec3::new(0.045, 0.06, -z_out),
                Vec3::new(x_out, 0.565, z_out),
                shirt_p,
                light,
            );
            // Jacket back panel
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.05, 0.06, -z_out),
                Vec3::new(0.05, 0.565, -dims.chest_half_z + 0.02),
                shirt_p,
                light,
            );
            // Raised collar
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.12, 0.55, -dims.chest_half_z - 0.01),
                Vec3::new(0.12, 0.62, -0.02),
                shirt_p_dark,
                light,
            );
        }
        ShirtStyle::Vest => {
            // Trimmed vest outer layer + accent shoulder trim
            mesh.add_tapered_prism(
                torso_xform,
                0.0,
                0.0,
                0.09,
                0.55,
                dims.waist_half_x + 0.01,
                dims.waist_half_z + 0.01,
                dims.chest_half_x + 0.01,
                dims.chest_half_z + 0.01,
                shirt_p,
                light,
            );
            mesh.add_box(
                torso_xform,
                Vec3::new(-0.035, 0.12, dims.chest_half_z + 0.005),
                Vec3::new(0.035, 0.52, dims.chest_half_z + 0.018),
                shirt_a,
                light,
            );
        }
    }

    // =========================================================================
    // 2. DISTINCT NECK + CHAMFERED HEAD + FACE + 3D HAIR + FACIAL HAIR
    // =========================================================================
    // Neck cylinder/prism
    mesh.add_tapered_prism(
        torso_xform,
        0.0,
        0.0,
        0.55,
        0.65,
        0.068,
        0.068,
        0.062,
        0.062,
        skin_shade,
        light,
    );

    let pitch_clamped = player.pitch.clamp(-1.05, 1.05);
    let head_xform = torso_xform
        * Mat4::from_translation(Vec3::new(0.0, 0.64, 0.0))
        * Mat4::from_rotation_x(-pitch_clamped - torso_lean * 0.65);

    // Chamfered stylized head (height ~0.38, width 0.36, depth 0.36, beveled corners)
    mesh.add_chamfered_prism(head_xform, 0.0, 0.36, 0.185, 0.185, 0.048, skin, light);
    // Subtle chin bevel
    mesh.add_tapered_prism(
        head_xform,
        0.0,
        0.01,
        -0.025,
        0.02,
        0.11,
        0.11,
        0.145,
        0.145,
        skin_shade,
        light,
    );
    // Ears
    mesh.add_box(
        head_xform,
        Vec3::new(-0.208, 0.13, -0.03),
        Vec3::new(-0.178, 0.22, 0.03),
        skin_shade,
        light,
    );
    mesh.add_box(
        head_xform,
        Vec3::new(0.178, 0.13, -0.03),
        Vec3::new(0.208, 0.22, 0.03),
        skin_shade,
        light,
    );
    // Subtle 3D nose
    mesh.add_tapered_prism(
        head_xform,
        0.0,
        0.192,
        0.115,
        0.165,
        0.022,
        0.022,
        0.014,
        0.012,
        skin_shade,
        light,
    );

    // Layered 3D Eyes & Eyebrows
    append_eyes(mesh, head_xform, &app, hurt, light);

    // Facial Hair (inherits hair color)
    append_facial_hair(mesh, head_xform, app.facial_hair, hair, hair_dark, skin, light);

    // 3D Sculpted Hairstyles
    append_hairstyle(mesh, head_xform, app.hair_style, hair, hair_dark, hair_light, light);

    // =========================================================================
    // 3. TWO-SEGMENT ARTICULATED ARMS (Upper Arm -> Elbow -> Forearm + Hand)
    // =========================================================================
    let (upper_arm_color, forearm_color, has_cuff) = match app.shirt_style {
        ShirtStyle::TShirt => (shirt_p, skin, false),
        ShirtStyle::LongSleeve | ShirtStyle::Hoodie | ShirtStyle::Jacket => (shirt_p, shirt_p, true),
        ShirtStyle::Tunic => (shirt_p, shirt_p_dark, true),
        ShirtStyle::Vest => (skin, skin, false),
    };

    for is_right in [false, true] {
        let side = if is_right { -1.0 } else { 1.0 };
        let arm_angle = if is_right {
            leg_swing * 0.75 + hold_pose + swing_boost
        } else {
            -leg_swing * 0.75
        };
        let elbow_bend = if is_right {
            (0.14 + hold_pose * 0.6 + swing_boost * 0.45 + (-leg_swing).max(0.0) * 0.35).min(1.2)
        } else {
            (0.12 + leg_swing.max(0.0) * 0.35).min(0.8)
        };

        let shoulder_xform = torso_xform
            * Mat4::from_translation(Vec3::new(side * dims.shoulder_half_x, 0.52, 0.0))
            * Mat4::from_rotation_x(-arm_angle)
            * Mat4::from_rotation_z(side * 0.07);

        // Upper arm
        mesh.add_tapered_prism(
            shoulder_xform,
            0.0,
            0.0,
            -0.28,
            0.02,
            dims.arm_half * 0.90,
            dims.arm_half * 0.90,
            dims.arm_half,
            dims.arm_half,
            upper_arm_color,
            light,
        );

        let forearm_xform = shoulder_xform
            * Mat4::from_translation(Vec3::new(0.0, -0.28, 0.0))
            * Mat4::from_rotation_x(-elbow_bend);

        // Forearm
        mesh.add_tapered_prism(
            forearm_xform,
            0.0,
            0.0,
            -0.22,
            0.02,
            dims.arm_half * 0.78,
            dims.arm_half * 0.78,
            dims.arm_half * 0.88,
            dims.arm_half * 0.88,
            forearm_color,
            light,
        );

        // Sleeve cuff accent
        if has_cuff {
            mesh.add_tapered_prism(
                forearm_xform,
                0.0,
                0.0,
                -0.225,
                -0.185,
                dims.arm_half * 0.84,
                dims.arm_half * 0.84,
                dims.arm_half * 0.86,
                dims.arm_half * 0.86,
                shirt_a,
                light,
            );
        }

        // Stylized hand
        mesh.add_tapered_prism(
            forearm_xform,
            0.0,
            0.008,
            -0.31,
            -0.215,
            dims.arm_half * 0.68,
            dims.arm_half * 0.64,
            dims.arm_half * 0.76,
            dims.arm_half * 0.74,
            skin,
            light,
        );
    }

    // =========================================================================
    // 4. TWO-SEGMENT ARTICULATED LEGS (Thigh -> Knee -> Calf -> 3D Shoes) + PANTS
    // =========================================================================
    // Optional Skirt overlay at hips
    if app.pants_style == PantsStyle::Skirt {
        mesh.add_tapered_prism(
            hip_xform,
            0.0,
            0.0,
            -0.34,
            0.06,
            dims.hip_half_x + 0.065,
            dims.hip_half_z + 0.055,
            dims.hip_half_x + 0.01,
            dims.hip_half_z + 0.01,
            pants,
            light,
        );
    }

    let (thigh_color, calf_color) = match app.pants_style {
        PantsStyle::Trousers | PantsStyle::Cargo | PantsStyle::Cuffed => (pants, pants),
        PantsStyle::Shorts => (pants, skin),
        PantsStyle::Skirt => (pants_dark, skin),
    };

    for is_right in [false, true] {
        let side = if is_right { -1.0 } else { 1.0 };
        let hip_angle = if is_right { -leg_swing } else { leg_swing };
        let sneak_knee = if player.is_sneaking { 0.45 } else { 0.0 };
        let knee_bend = (sneak_knee + (-hip_angle).max(0.0) * 0.75).min(1.1);

        let thigh_xform = hip_xform
            * Mat4::from_translation(Vec3::new(side * dims.leg_offset_x, -0.02, 0.0))
            * Mat4::from_rotation_x(-hip_angle - sneak_knee * 0.5);

        // Upper leg (thigh)
        mesh.add_tapered_prism(
            thigh_xform,
            0.0,
            0.0,
            -0.37,
            0.02,
            dims.leg_half * 0.86,
            dims.leg_half * 0.86,
            dims.leg_half,
            dims.leg_half,
            thigh_color,
            light,
        );

        // Cargo side pockets on thighs
        if app.pants_style == PantsStyle::Cargo {
            let px0 = if side > 0.0 {
                dims.leg_half * 0.75
            } else {
                -dims.leg_half * 1.12
            };
            let px1 = if side > 0.0 {
                dims.leg_half * 1.12
            } else {
                -dims.leg_half * 0.75
            };
            mesh.add_box(
                thigh_xform,
                Vec3::new(px0, -0.28, -0.05),
                Vec3::new(px1, -0.14, 0.05),
                pants_dark,
                light,
            );
        }

        let calf_xform = thigh_xform
            * Mat4::from_translation(Vec3::new(0.0, -0.37, 0.0))
            * Mat4::from_rotation_x(knee_bend);

        // Lower leg (calf)
        let calf_bottom_y = match app.shoe_style {
            ShoeStyle::Boots => -0.22,
            _ => -0.32,
        };
        mesh.add_tapered_prism(
            calf_xform,
            0.0,
            0.0,
            calf_bottom_y,
            0.02,
            dims.leg_half * 0.72,
            dims.leg_half * 0.72,
            dims.leg_half * 0.86,
            dims.leg_half * 0.86,
            calf_color,
            light,
        );

        // Rolled ankle cuff for Cuffed pants
        if app.pants_style == PantsStyle::Cuffed && app.shoe_style != ShoeStyle::Boots {
            mesh.add_tapered_prism(
                calf_xform,
                0.0,
                0.0,
                -0.31,
                -0.25,
                dims.leg_half * 0.80,
                dims.leg_half * 0.80,
                dims.leg_half * 0.82,
                dims.leg_half * 0.82,
                pants_light,
                light,
            );
        }

        // 3D Shoes / Feet
        append_shoe(mesh, calf_xform, dims.leg_half, app.shoe_style, shoe, shoe_sole, skin, light);
    }
}

fn append_eyes(
    mesh: &mut CharacterMesh,
    head_xform: Mat4,
    app: &CharacterAppearance,
    hurt: bool,
    light: [f32; 2],
) {
    let sclera = [0.95, 0.95, 0.97];
    let pupil = [0.08, 0.08, 0.10];
    let iris = tint_hurt(app.eye_color, hurt);
    let brow = tint_hurt(app.hair_color.scale(0.72), hurt);

    let (eye_w, eye_h, iris_w, brow_y, brow_tilt) = match app.eye_style {
        EyeStyle::Standard => (0.044, 0.036, 0.025, 0.242, 0.0),
        EyeStyle::Almond => (0.048, 0.028, 0.024, 0.238, 0.06),
        EyeStyle::Wide => (0.046, 0.044, 0.029, 0.250, 0.0),
        EyeStyle::Sharp => (0.048, 0.026, 0.022, 0.234, -0.10),
    };

    let eye_y = 0.188;
    let face_z = 0.185;

    for side in [-1.0_f32, 1.0_f32] {
        let ex = side * 0.066;
        // Sclera
        mesh.add_box(
            head_xform,
            Vec3::new(ex - eye_w * 0.5, eye_y - eye_h * 0.5, face_z - 0.002),
            Vec3::new(ex + eye_w * 0.5, eye_y + eye_h * 0.5, face_z + 0.005),
            sclera,
            light,
        );
        // Colored iris
        mesh.add_box(
            head_xform,
            Vec3::new(ex - iris_w * 0.5, eye_y - eye_h * 0.45, face_z + 0.002),
            Vec3::new(ex + iris_w * 0.5, eye_y + eye_h * 0.45, face_z + 0.009),
            iris,
            light,
        );
        // Pupil
        mesh.add_box(
            head_xform,
            Vec3::new(ex - 0.009, eye_y - 0.011, face_z + 0.006),
            Vec3::new(ex + 0.009, eye_y + 0.011, face_z + 0.012),
            pupil,
            light,
        );
        // Eyebrow
        let brow_xform = head_xform
            * Mat4::from_translation(Vec3::new(ex, brow_y, face_z + 0.008))
            * Mat4::from_rotation_z(side * brow_tilt);
        mesh.add_box(
            brow_xform,
            Vec3::new(-0.032, -0.008, -0.005),
            Vec3::new(0.032, 0.008, 0.008),
            brow,
            light,
        );
    }
}

fn append_facial_hair(
    mesh: &mut CharacterMesh,
    head_xform: Mat4,
    style: FacialHairStyle,
    hair: [f32; 3],
    hair_dark: [f32; 3],
    skin: [f32; 3],
    light: [f32; 2],
) {
    let z = 0.185;
    match style {
        FacialHairStyle::None => {}
        FacialHairStyle::Stubble => {
            let stubble = [
                skin[0] * 0.65 + hair[0] * 0.35,
                skin[1] * 0.65 + hair[1] * 0.35,
                skin[2] * 0.65 + hair[2] * 0.35,
            ];
            mesh.add_box(
                head_xform,
                Vec3::new(-0.125, -0.005, z - 0.005),
                Vec3::new(0.125, 0.095, z + 0.006),
                stubble,
                light,
            );
        }
        FacialHairStyle::Mustache => {
            mesh.add_box(
                head_xform,
                Vec3::new(-0.072, 0.078, z - 0.002),
                Vec3::new(0.072, 0.106, z + 0.018),
                hair,
                light,
            );
        }
        FacialHairStyle::ShortBeard => {
            mesh.add_box(
                head_xform,
                Vec3::new(-0.132, -0.018, z - 0.01),
                Vec3::new(0.132, 0.095, z + 0.016),
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(-0.07, 0.082, z),
                Vec3::new(0.07, 0.108, z + 0.018),
                hair_dark,
                light,
            );
        }
        FacialHairStyle::FullBeard => {
            mesh.add_tapered_prism(
                head_xform,
                0.0,
                z - 0.01,
                -0.12,
                0.105,
                0.085,
                0.032,
                0.145,
                0.042,
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(-0.078, 0.078, z),
                Vec3::new(0.078, 0.11, z + 0.028),
                hair_dark,
                light,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn append_hairstyle(
    mesh: &mut CharacterMesh,
    head_xform: Mat4,
    style: HairStyle,
    hair: [f32; 3],
    hair_dark: [f32; 3],
    hair_light: [f32; 3],
    light: [f32; 2],
) {
    if style == HairStyle::Bald {
        return;
    }

    // Base hair cap over top & back of chamfered head
    mesh.add_chamfered_prism(head_xform, 0.285, 0.395, 0.198, 0.198, 0.052, hair, light);
    // Back hair coverage
    mesh.add_box(
        head_xform,
        Vec3::new(-0.185, 0.12, -0.205),
        Vec3::new(0.185, 0.34, -0.145),
        hair_dark,
        light,
    );

    match style {
        HairStyle::ShortCrop => {
            // Clean tapered sides & short front fringe
            mesh.add_box(
                head_xform,
                Vec3::new(-0.198, 0.21, -0.14),
                Vec3::new(-0.168, 0.33, 0.12),
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(0.168, 0.21, -0.14),
                Vec3::new(0.198, 0.33, 0.12),
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(-0.14, 0.29, 0.14),
                Vec3::new(0.14, 0.365, 0.202),
                hair_light,
                light,
            );
        }
        HairStyle::Spiky => {
            // Angled sculpted spikes across crown
            for (sx, sz, tilt_x, tilt_z, h) in [
                (-0.09, 0.08, 0.30, 0.25, 0.13),
                (0.0, 0.11, 0.38, 0.0, 0.15),
                (0.09, 0.07, 0.28, -0.25, 0.13),
                (-0.07, -0.04, -0.22, 0.28, 0.13),
                (0.06, -0.05, -0.24, -0.22, 0.14),
                (0.0, 0.0, 0.05, 0.05, 0.16),
            ] {
                let sp = head_xform
                    * Mat4::from_translation(Vec3::new(sx, 0.36, sz))
                    * Mat4::from_rotation_x(tilt_x)
                    * Mat4::from_rotation_z(tilt_z);
                mesh.add_tapered_prism(sp, 0.0, 0.0, 0.0, h, 0.048, 0.048, 0.010, 0.010, hair_light, light);
            }
        }
        HairStyle::SweptBangs => {
            // Swept diagonal bangs across forehead + medium sides
            let bangs = head_xform
                * Mat4::from_translation(Vec3::new(0.02, 0.305, 0.185))
                * Mat4::from_rotation_z(-0.22);
            mesh.add_tapered_prism(bangs, 0.0, 0.0, -0.055, 0.06, 0.145, 0.025, 0.155, 0.032, hair_light, light);
            mesh.add_box(
                head_xform,
                Vec3::new(-0.202, 0.14, -0.15),
                Vec3::new(-0.165, 0.33, 0.13),
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(0.165, 0.14, -0.15),
                Vec3::new(0.202, 0.33, 0.13),
                hair,
                light,
            );
        }
        HairStyle::SidePart => {
            // Neat side-parted top volume
            mesh.add_box(
                head_xform,
                Vec3::new(-0.18, 0.34, -0.12),
                Vec3::new(0.05, 0.425, 0.195),
                hair_light,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(0.07, 0.33, -0.12),
                Vec3::new(0.185, 0.40, 0.175),
                hair,
                light,
            );
        }
        HairStyle::Bob => {
            // Chin-length rounded bob framing both sides and back
            mesh.add_tapered_prism(
                head_xform,
                -0.19,
                -0.02,
                0.02,
                0.34,
                0.036,
                0.15,
                0.026,
                0.14,
                hair,
                light,
            );
            mesh.add_tapered_prism(
                head_xform,
                0.19,
                -0.02,
                0.02,
                0.34,
                0.036,
                0.15,
                0.026,
                0.14,
                hair,
                light,
            );
            mesh.add_tapered_prism(
                head_xform,
                0.0,
                -0.19,
                0.01,
                0.34,
                0.195,
                0.042,
                0.185,
                0.034,
                hair_dark,
                light,
            );
            // Straight fringe
            mesh.add_box(
                head_xform,
                Vec3::new(-0.145, 0.265, 0.15),
                Vec3::new(0.145, 0.35, 0.204),
                hair_light,
                light,
            );
        }
        HairStyle::LongFlowing => {
            // Long layered drape down the back + front framing locks
            mesh.add_tapered_prism(
                head_xform,
                0.0,
                -0.20,
                -0.30,
                0.34,
                0.205,
                0.052,
                0.185,
                0.040,
                hair_dark,
                light,
            );
            mesh.add_tapered_prism(
                head_xform,
                -0.19,
                0.02,
                -0.14,
                0.34,
                0.032,
                0.11,
                0.028,
                0.12,
                hair,
                light,
            );
            mesh.add_tapered_prism(
                head_xform,
                0.19,
                0.02,
                -0.14,
                0.34,
                0.032,
                0.11,
                0.028,
                0.12,
                hair,
                light,
            );
            mesh.add_box(
                head_xform,
                Vec3::new(-0.15, 0.27, 0.145),
                Vec3::new(0.15, 0.36, 0.204),
                hair_light,
                light,
            );
        }
        HairStyle::Ponytail => {
            // Sleek sides + high sculpted ponytail hanging behind head
            let tail = head_xform
                * Mat4::from_translation(Vec3::new(0.0, 0.26, -0.21))
                * Mat4::from_rotation_x(-0.28);
            mesh.add_tapered_prism(tail, 0.0, 0.0, -0.34, 0.04, 0.075, 0.065, 0.048, 0.048, hair_light, light);
            // Hair tie band
            mesh.add_box(
                head_xform,
                Vec3::new(-0.058, 0.23, -0.23),
                Vec3::new(0.058, 0.28, -0.18),
                hair_dark,
                light,
            );
            // Soft bangs
            mesh.add_box(
                head_xform,
                Vec3::new(-0.135, 0.28, 0.15),
                Vec3::new(0.135, 0.355, 0.202),
                hair_light,
                light,
            );
        }
        HairStyle::TwinBuns => {
            // Two high sculpted buns on left & right crown
            for side in [-1.0_f32, 1.0_f32] {
                let bun = head_xform * Mat4::from_translation(Vec3::new(side * 0.20, 0.34, -0.04));
                mesh.add_chamfered_prism(bun, -0.06, 0.09, 0.085, 0.085, 0.028, hair_light, light);
            }
            mesh.add_box(
                head_xform,
                Vec3::new(-0.14, 0.275, 0.15),
                Vec3::new(0.14, 0.35, 0.202),
                hair,
                light,
            );
        }
        HairStyle::Curly => {
            // Voluminous stylized afro / curly crown
            mesh.add_chamfered_prism(head_xform, 0.22, 0.455, 0.245, 0.245, 0.075, hair, light);
            mesh.add_chamfered_prism(head_xform, 0.36, 0.485, 0.19, 0.19, 0.06, hair_light, light);
        }
        HairStyle::Bald => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn append_shoe(
    mesh: &mut CharacterMesh,
    calf_xform: Mat4,
    leg_half: f32,
    style: ShoeStyle,
    shoe: [f32; 3],
    sole: [f32; 3],
    skin: [f32; 3],
    light: [f32; 2],
) {
    let w = leg_half * 0.88;
    match style {
        ShoeStyle::Boots => {
            // Tall boot shaft
            mesh.add_tapered_prism(
                calf_xform,
                0.0,
                0.0,
                -0.34,
                -0.19,
                w * 0.96,
                w * 0.96,
                w * 1.06,
                w * 1.06,
                shoe,
                light,
            );
            // Foot & toe box
            mesh.add_box(
                calf_xform,
                Vec3::new(-w, -0.39, -w),
                Vec3::new(w, -0.32, w + 0.075),
                shoe,
                light,
            );
            // Rugged sole
            mesh.add_box(
                calf_xform,
                Vec3::new(-w - 0.006, -0.42, -w - 0.006),
                Vec3::new(w + 0.006, -0.388, w + 0.082),
                sole,
                light,
            );
        }
        ShoeStyle::Sneakers => {
            // Upper sneaker
            mesh.add_box(
                calf_xform,
                Vec3::new(-w, -0.385, -w),
                Vec3::new(w, -0.31, w + 0.075),
                shoe,
                light,
            );
            // White midsole + toe cap
            mesh.add_box(
                calf_xform,
                Vec3::new(-w - 0.005, -0.42, -w - 0.005),
                Vec3::new(w + 0.005, -0.382, w + 0.082),
                sole,
                light,
            );
        }
        ShoeStyle::Sandals => {
            // Bare foot
            mesh.add_box(
                calf_xform,
                Vec3::new(-w * 0.92, -0.395, -w * 0.92),
                Vec3::new(w * 0.92, -0.32, w + 0.065),
                skin,
                light,
            );
            // Sandal strap
            mesh.add_box(
                calf_xform,
                Vec3::new(-w, -0.38, 0.0),
                Vec3::new(w, -0.335, w + 0.025),
                shoe,
                light,
            );
            // Flat sole
            mesh.add_box(
                calf_xform,
                Vec3::new(-w - 0.004, -0.42, -w - 0.004),
                Vec3::new(w + 0.004, -0.392, w + 0.075),
                sole,
                light,
            );
        }
        ShoeStyle::Loafers => {
            // Sleek low loafer
            mesh.add_tapered_prism(
                calf_xform,
                0.0,
                0.032,
                -0.395,
                -0.315,
                w * 0.95,
                w + 0.042,
                w * 0.88,
                w + 0.025,
                shoe,
                light,
            );
            // Thin sole
            mesh.add_box(
                calf_xform,
                Vec3::new(-w, -0.42, -w),
                Vec3::new(w, -0.392, w + 0.075),
                sole,
                light,
            );
        }
    }
}

/// Builds the textured 3D held items for all players, anchored to each player's articulated right hand.
pub fn build_player_held_items_mesh(
    players: &[PlayerModelState],
    item_registry: &ItemRegistry,
    time_secs: f32,
) -> ChunkMesh {
    let mut mesh = ChunkMesh::new();

    for player in players {
        if player.health <= 0.0 || player.held_item == ItemId::EMPTY {
            continue;
        }
        let Some(def) = item_registry.get(player.held_item) else {
            continue;
        };

        let hand_xform = compute_right_hand_transform(player, time_secs);
        let item_light = if player.held_item == ItemId::TORCH {
            [1.0, 1.0]
        } else if player.hurt_timer > 0.0 {
            [1.0, 0.5]
        } else {
            [1.0, 0.0]
        };

        let tile = def.icon_tile as u16;
        let col = (tile % 16) as f32;
        let row = (tile / 16) as f32;
        let step = 1.0 / 16.0;
        let eps = 0.001;
        let u0 = col * step + eps;
        let v0 = row * step + eps;
        let u1 = (col + 1.0) * step - eps;
        let v1 = (row + 1.0) * step - eps;

        let min = Vec3::new(-0.10, -0.10, -0.02);
        let max = Vec3::new(0.10, 0.10, 0.22);

        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];

        let faces = [
            ([4, 5, 6, 7], Vec3::Z),
            ([1, 0, 3, 2], -Vec3::Z),
            ([5, 1, 2, 6], Vec3::X),
            ([0, 4, 7, 3], -Vec3::X),
            ([7, 6, 2, 3], Vec3::Y),
            ([0, 1, 5, 4], -Vec3::Y),
        ];
        let uvs = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];

        for (idx, local_n) in faces {
            let n = hand_xform.transform_vector3(local_n).normalize_or_zero();
            let base = mesh.vertices.len() as u32;
            for (k, &ci) in idx.iter().enumerate() {
                let wp = hand_xform.transform_point3(corners[ci]);
                mesh.vertices.push(VoxelVertex {
                    position: [wp.x, wp.y, wp.z],
                    uv: uvs[k],
                    normal: [n.x, n.y, n.z],
                    ao: 1.0,
                    light: item_light,
                });
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_character_rig_all_body_frames_and_styles() {
        for frame in BodyFrame::ALL {
            for hair in HairStyle::ALL {
                let mut app = CharacterAppearance::default();
                app.body_frame = frame;
                app.hair_style = hair;
                let mesh = build_single_character_preview_mesh(&app, 0.0, 0.5, 1.0);
                assert!(!mesh.is_empty());
                assert_eq!(mesh.indices.len() % 3, 0);
            }
        }
    }

    #[test]
    fn test_build_player_held_items_mesh() {
        let reg = ItemRegistry::new();
        let mut p = PlayerModelState::new(1, "Tester", Vec3::new(0.0, 10.0, 0.0), CharacterAppearance::default());
        p.held_item = ItemId::DIAMOND_SWORD;
        let mesh = build_player_held_items_mesh(&[p], &reg, 0.0);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }
}
