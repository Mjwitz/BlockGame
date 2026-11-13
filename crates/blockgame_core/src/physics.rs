use glam::{IVec3, Vec3};

use crate::{
    block::{BlockId, BlockRegistry},
    world::World,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn from_block(pos: IVec3) -> Self {
        let min = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        Self {
            min,
            max: min + Vec3::ONE,
        }
    }

    #[inline]
    pub fn from_player(position: Vec3, half_width: f32, height: f32) -> Self {
        Self {
            min: Vec3::new(position.x - half_width, position.y, position.z - half_width),
            max: Vec3::new(
                position.x + half_width,
                position.y + height,
                position.z + half_width,
            ),
        }
    }

    #[inline]
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    #[inline]
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Fast slab ray-AABB intersection test returning distance `t` along `ray_dir` if hit.
    pub fn intersect_ray(&self, ray_origin: Vec3, ray_dir: Vec3, max_dist: f32) -> Option<f32> {
        let mut tmin = 0.0_f32;
        let mut tmax = max_dist;

        for i in 0..3 {
            let origin = ray_origin[i];
            let dir = ray_dir[i];
            let min = self.min[i];
            let max = self.max[i];

            if dir.abs() < 1e-6 {
                if origin < min || origin > max {
                    return None;
                }
            } else {
                let ood = 1.0 / dir;
                let mut t1 = (min - origin) * ood;
                let mut t2 = (max - origin) * ood;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some(tmin)
    }
}

pub struct PlayerPhysics {
    pub position: Vec3, // Feet position
    pub velocity: Vec3,
    pub half_width: f32, // 0.3 (total width 0.6)
    pub height: f32,     // 1.8
    pub eye_offset: f32, // 1.62
    pub on_ground: bool,
    pub flying: bool,
    pub fall_distance: f32,
}

impl PlayerPhysics {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            half_width: 0.3,
            height: 1.8,
            eye_offset: 1.62,
            on_ground: false,
            flying: false,
            fall_distance: 0.0,
        }
    }

    #[inline]
    pub fn eye_position(&self) -> Vec3 {
        Vec3::new(
            self.position.x,
            self.position.y + self.eye_offset,
            self.position.z,
        )
    }

    #[inline]
    pub fn aabb(&self) -> AABB {
        AABB::from_player(self.position, self.half_width, self.height)
    }

    /// Toggles flying mode (spectator vs standard physics)
    pub fn toggle_flight(&mut self) {
        self.flying = !self.flying;
        if self.flying {
            self.velocity = Vec3::ZERO;
            self.fall_distance = 0.0;
        }
    }

    /// Returns whether the player's bounding box is submerged or touching water.
    pub fn is_in_water(&self, world: &World) -> bool {
        let player_box = self.aabb();
        let min_x = player_box.min.x.floor() as i32;
        let max_x = player_box.max.x.floor() as i32;
        let min_y = player_box.min.y.floor() as i32;
        let max_y = player_box.max.y.floor() as i32;
        let min_z = player_box.min.z.floor() as i32;
        let max_z = player_box.max.z.floor() as i32;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if world.get_block(IVec3::new(x, y, z)).is_water() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns whether the player's bounding box is touching or brushing against a cactus block.
    pub fn is_touching_cactus(&self, world: &World) -> bool {
        let box_check = AABB::new(
            self.position - Vec3::new(self.half_width + 0.05, 0.0, self.half_width + 0.05),
            self.position + Vec3::new(self.half_width + 0.05, self.height, self.half_width + 0.05),
        );
        let min_x = box_check.min.x.floor() as i32;
        let max_x = box_check.max.x.floor() as i32;
        let min_y = (box_check.min.y.floor() as i32).max(0);
        let max_y = (box_check.max.y.floor() as i32).min((crate::chunk::VERTICAL_CHUNKS * 16) - 1);
        let min_z = box_check.min.z.floor() as i32;
        let max_z = box_check.max.z.floor() as i32;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    if world.get_block(IVec3::new(x, y, z)) == BlockId::CACTUS {
                        let cactus_box = AABB::from_block(IVec3::new(x, y, z));
                        if box_check.intersects(&cactus_box) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Updates player physics over a delta time `dt` against the voxel `world`.
    /// Returns any fall damage incurred upon landing.
    pub fn update(
        &mut self,
        dt: f32,
        wish_dir: Vec3,
        sprinting: bool,
        jumping: bool,
        world: &World,
        registry: &BlockRegistry,
    ) -> f32 {
        if self.flying {
            // Free flight mode
            let speed = if sprinting { 30.0 } else { 15.0 };
            self.position += wish_dir * speed * dt;
            self.velocity = Vec3::ZERO;
            self.on_ground = false;
            self.fall_distance = 0.0;
            return 0.0;
        }

        let in_water = self.is_in_water(world);
        let prev_y = self.position.y;
        let was_on_ground = self.on_ground;

        if in_water {
            // Cancel fall damage in water
            self.fall_distance = 0.0;

            // Water gravity & buoyancy
            let water_gravity = -6.0;
            self.velocity.y += water_gravity * dt;

            // Water vertical drag
            self.velocity.y *= (1.0 - 2.5 * dt).clamp(0.0, 1.0);

            // Swimming upward
            if jumping {
                self.velocity.y = 4.0;
            }

            // Water horizontal movement
            let target_speed = if sprinting { 3.8 } else { 2.4 };
            let target_vel = wish_dir * target_speed;
            let accel = 8.0;
            self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
            self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);
        } else {
            // Apply gravity
            let gravity = -28.0;
            self.velocity.y += gravity * dt;

            // Ground jumping
            if self.on_ground && jumping {
                self.velocity.y = 8.5; // Jump velocity
                self.on_ground = false;
                self.fall_distance = 0.0;
            }

            // Horizontal movement & acceleration
            let target_speed = if sprinting { 6.0 } else { 4.3 };
            let target_vel = wish_dir * target_speed;

            let accel = if self.on_ground { 15.0 } else { 3.0 };
            self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
            self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);
        }

        // Swept axis-separated collision resolution
        self.resolve_movement(dt, world, registry);

        // Calculate fall distance and landing damage
        let mut fall_damage = 0.0;
        if in_water {
            self.fall_distance = 0.0;
        } else if self.on_ground {
            if !was_on_ground && self.fall_distance > 3.0 {
                fall_damage = self.fall_distance - 3.0;
            }
            self.fall_distance = 0.0;
        } else {
            if self.position.y < prev_y {
                self.fall_distance += prev_y - self.position.y;
            } else if self.velocity.y > 0.0 {
                self.fall_distance = 0.0;
            }
        }

        fall_damage
    }

    fn resolve_movement(&mut self, dt: f32, world: &World, registry: &BlockRegistry) {
        resolve_entity_movement(
            &mut self.position,
            &mut self.velocity,
            &mut self.on_ground,
            self.half_width,
            self.height,
            dt,
            world,
            registry,
        );
    }
}

const COLLISION_SKIN: f32 = 1e-4;
const FACE_TOLERANCE: f32 = 1e-3;
const STAIR_STEP_HEIGHT: f32 = 0.52;

/// Returns the collision AABBs for a block at `bpos` and the active box count (`0..=2`).
pub fn block_collision_boxes(
    world: &World,
    registry: &BlockRegistry,
    bpos: IVec3,
) -> ([AABB; 2], usize) {
    let block = world.get_block(bpos);
    let fallback = AABB::from_block(bpos);
    if !registry.is_solid(block) {
        return ([fallback; 2], 0);
    }

    let fx = bpos.x as f32;
    let fy = bpos.y as f32;
    let fz = bpos.z as f32;

    if block == crate::block::BlockId::OAK_FENCE {
        return (
            [
                AABB::new(
                    Vec3::new(fx, fy, fz),
                    Vec3::new(fx + 1.0, fy + 1.5, fz + 1.0),
                ),
                fallback,
            ],
            1,
        );
    }

    if block.is_stairs() {
        let bottom = AABB::new(
            Vec3::new(fx, fy, fz),
            Vec3::new(fx + 1.0, fy + 0.5, fz + 1.0),
        );
        let facing = world.get_block_facing(bpos);
        let top = match facing {
            // NegZ: upper step on +Z back half
            1 => AABB::new(
                Vec3::new(fx, fy + 0.5, fz + 0.5),
                Vec3::new(fx + 1.0, fy + 1.0, fz + 1.0),
            ),
            // PosX: upper step on -X back half
            2 => AABB::new(
                Vec3::new(fx, fy + 0.5, fz),
                Vec3::new(fx + 0.5, fy + 1.0, fz + 1.0),
            ),
            // NegX: upper step on +X back half
            3 => AABB::new(
                Vec3::new(fx + 0.5, fy + 0.5, fz),
                Vec3::new(fx + 1.0, fy + 1.0, fz + 1.0),
            ),
            // PosZ (0, default): upper step on -Z back half
            _ => AABB::new(
                Vec3::new(fx, fy + 0.5, fz),
                Vec3::new(fx + 1.0, fy + 1.0, fz + 0.5),
            ),
        };
        return ([bottom, top], 2);
    }

    if block == crate::block::BlockId::OAK_DOOR_CLOSED
        || block == crate::block::BlockId::OAK_DOOR_TOP_CLOSED
    {
        let facing = world.get_block_facing(bpos);
        let (x0, z0, x1, z1) = match facing {
            1 => (0.0, 0.0, 1.0, 0.1875),
            2 => (0.8125, 0.0, 1.0, 1.0),
            3 => (0.0, 0.0, 0.1875, 1.0),
            _ => (0.0, 0.8125, 1.0, 1.0),
        };
        return (
            [
                AABB::new(
                    Vec3::new(fx + x0, fy, fz + z0),
                    Vec3::new(fx + x1, fy + 1.0, fz + z1),
                ),
                fallback,
            ],
            1,
        );
    }

    ([fallback; 2], 1)
}

/// Returns `true` if the entity AABB (shrunk by `margin` on each face) intersects any solid block.
pub fn aabb_intersects_solid(
    position: Vec3,
    half_width: f32,
    height: f32,
    margin: f32,
    world: &World,
    registry: &BlockRegistry,
) -> bool {
    let hw = (half_width - margin).max(0.01);
    let min = Vec3::new(position.x - hw, position.y + margin, position.z - hw);
    let max = Vec3::new(
        position.x + hw,
        position.y + (height - margin).max(margin + 0.01),
        position.z + hw,
    );
    let check_box = AABB::new(min, max);

    let min_x = min.x.floor() as i32;
    let max_x = max.x.floor() as i32;
    let min_y = min.y.floor() as i32 - 1;
    let max_y = max.y.floor() as i32;
    let min_z = min.z.floor() as i32;
    let max_z = max.z.floor() as i32;

    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let (boxes, count) = block_collision_boxes(world, registry, IVec3::new(x, y, z));
                for i in 0..count {
                    if check_box.intersects(&boxes[i]) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// If the entity is currently penetrating any solid block (e.g. from turning off flight mode inside
/// terrain, a falling block landing on them, or a previously saved stuck state), pushes them out to
/// the nearest valid non-overlapping position so they never slide through solid blocks.
pub fn depenetrate_entity(
    position: &mut Vec3,
    velocity: &mut Vec3,
    half_width: f32,
    height: f32,
    world: &World,
    registry: &BlockRegistry,
) {
    if !aabb_intersects_solid(*position, half_width, height, COLLISION_SKIN, world, registry) {
        return;
    }

    // 1. Try small push-out along the shallowest overlapping block face first (up to 4 iterations)
    for _ in 0..4 {
        if !aabb_intersects_solid(*position, half_width, height, COLLISION_SKIN, world, registry) {
            return;
        }

        let box_now = AABB::from_player(*position, half_width, height);
        let min_x = box_now.min.x.floor() as i32;
        let max_x = box_now.max.x.floor() as i32;
        let min_y = box_now.min.y.floor() as i32 - 1;
        let max_y = box_now.max.y.floor() as i32;
        let min_z = box_now.min.z.floor() as i32;
        let max_z = box_now.max.z.floor() as i32;

        let mut best_candidate: Option<(f32, Vec3)> = None;

        for y in min_y..=max_y {
            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let bpos = IVec3::new(x, y, z);
                    let (boxes, count) = block_collision_boxes(world, registry, bpos);
                    for i in 0..count {
                        let bbox = boxes[i];
                        if !box_now.intersects(&bbox) {
                            continue;
                        }

                        // Candidate push positions along each face of `bbox`
                        let candidates = [
                            Vec3::new(bbox.min.x - half_width - COLLISION_SKIN, position.y, position.z),
                            Vec3::new(bbox.max.x + half_width + COLLISION_SKIN, position.y, position.z),
                            Vec3::new(position.x, position.y, bbox.min.z - half_width - COLLISION_SKIN),
                            Vec3::new(position.x, position.y, bbox.max.z + half_width + COLLISION_SKIN),
                            Vec3::new(position.x, bbox.max.y, position.z),
                            Vec3::new(position.x, bbox.min.y - height - COLLISION_SKIN, position.z),
                        ];

                        for cand in candidates {
                            let dist = (cand - *position).length();
                            if dist <= half_width + 0.25
                                && !aabb_intersects_solid(
                                    cand,
                                    half_width,
                                    height,
                                    COLLISION_SKIN,
                                    world,
                                    registry,
                                )
                            {
                                if best_candidate.map_or(true, |(best_d, _)| dist < best_d) {
                                    best_candidate = Some((dist, cand));
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some((_, resolved_pos)) = best_candidate {
            *position = resolved_pos;
            return;
        } else {
            break;
        }
    }

    // 2. Full burial recovery: if the entity's center is completely encased in solid blocks,
    // search nearby voxel cells first, then upward to the surface, for a standable space.
    let base_voxel = IVec3::new(
        position.x.floor() as i32,
        position.y.floor() as i32,
        position.z.floor() as i32,
    );

    let mut best_escape: Option<(f32, Vec3)> = None;
    for dy in -1..=3 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let candidate_pos = Vec3::new(
                    (base_voxel.x + dx) as f32 + 0.5,
                    (base_voxel.y + dy) as f32,
                    (base_voxel.z + dz) as f32 + 0.5,
                );
                if !aabb_intersects_solid(
                    candidate_pos,
                    half_width,
                    height,
                    COLLISION_SKIN,
                    world,
                    registry,
                ) {
                    let dist_sq = (candidate_pos - *position).length_squared();
                    if best_escape.map_or(true, |(best_d2, _)| dist_sq < best_d2) {
                        best_escape = Some((dist_sq, candidate_pos));
                    }
                }
            }
        }
    }

    if let Some((_, escape_pos)) = best_escape {
        *position = escape_pos;
        *velocity = Vec3::ZERO;
        return;
    }

    // Fallback: pop straight up to the first open space above
    let max_world_y = (crate::chunk::VERTICAL_CHUNKS * 16) - 2;
    for y in base_voxel.y..=max_world_y {
        let candidate_pos = Vec3::new(position.x, y as f32, position.z);
        if !aabb_intersects_solid(
            candidate_pos,
            half_width,
            height,
            COLLISION_SKIN,
            world,
            registry,
        ) {
            *position = candidate_pos;
            *velocity = Vec3::ZERO;
            return;
        }
    }
}

fn try_auto_step(
    target_xz: Vec3,
    half_width: f32,
    height: f32,
    world: &World,
    registry: &BlockRegistry,
) -> Option<f32> {
    let lifted = Vec3::new(target_xz.x, target_xz.y + STAIR_STEP_HEIGHT, target_xz.z);
    if aabb_intersects_solid(lifted, half_width, height, COLLISION_SKIN, world, registry) {
        return None;
    }

    let aabb = AABB::from_player(lifted, half_width, height);
    let min_x = (aabb.min.x + COLLISION_SKIN).floor() as i32;
    let max_x = (aabb.max.x - COLLISION_SKIN).floor() as i32;
    let min_z = (aabb.min.z + COLLISION_SKIN).floor() as i32;
    let max_z = (aabb.max.z - COLLISION_SKIN).floor() as i32;
    let min_y = (target_xz.y - 0.5).floor() as i32 - 1;
    let max_y = (lifted.y + FACE_TOLERANCE).ceil() as i32;

    let mut best_y: Option<f32> = None;
    for y in min_y..=max_y {
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let (boxes, count) = block_collision_boxes(world, registry, IVec3::new(x, y, z));
                for i in 0..count {
                    let bbox = boxes[i];
                    if aabb.max.x <= bbox.min.x + COLLISION_SKIN
                        || aabb.min.x >= bbox.max.x - COLLISION_SKIN
                        || aabb.max.z <= bbox.min.z + COLLISION_SKIN
                        || aabb.min.z >= bbox.max.z - COLLISION_SKIN
                    {
                        continue;
                    }
                    if bbox.max.y <= lifted.y + FACE_TOLERANCE
                        && bbox.max.y > target_xz.y + COLLISION_SKIN
                    {
                        if best_y.map_or(true, |by| bbox.max.y > by) {
                            best_y = Some(bbox.max.y);
                        }
                    }
                }
            }
        }
    }

    best_y.map(|y| (y * 1024.0).round() / 1024.0)
}

/// Swept axis-separated AABB collision resolution with floating-point skin protection.
/// Guarantees that clamping against a wall on X or Z never leaves a 1-ULP (`f32`) overlap
/// that could false-trigger vertical floor/ceiling snapping on the Y axis.
pub fn resolve_entity_movement(
    position: &mut Vec3,
    velocity: &mut Vec3,
    on_ground: &mut bool,
    half_width: f32,
    height: f32,
    dt: f32,
    world: &World,
    registry: &BlockRegistry,
) {
    // Ensure we never start a movement step already embedded inside a solid block
    depenetrate_entity(position, velocity, half_width, height, world, registry);

    let total_delta = *velocity * dt;
    let max_axis_disp = total_delta
        .x
        .abs()
        .max(total_delta.y.abs())
        .max(total_delta.z.abs());
    let steps = ((max_axis_disp / 0.35).ceil() as usize).clamp(1, 8);
    let step_dt = dt / (steps as f32);

    let started_on_ground = *on_ground;
    *on_ground = false;

    for step_idx in 0..steps {
        // --- 1. Sweep Y axis ---
        let orig_dy = velocity.y * step_dt;
        let mut dy = orig_dy;
        if dy.abs() > 0.0 {
            let aabb = AABB::from_player(*position, half_width, height);
            let min_x = (aabb.min.x + COLLISION_SKIN).floor() as i32;
            let max_x = (aabb.max.x - COLLISION_SKIN).floor() as i32;
            let min_z = (aabb.min.z + COLLISION_SKIN).floor() as i32;
            let max_z = (aabb.max.z - COLLISION_SKIN).floor() as i32;
            let min_y = (aabb.min.y + dy.min(0.0)).floor() as i32 - 1;
            let max_y = (aabb.max.y + dy.max(0.0)).ceil() as i32 + 1;

            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    for x in min_x..=max_x {
                        let bpos = IVec3::new(x, y, z);
                        let (boxes, count) = block_collision_boxes(world, registry, bpos);
                        for i in 0..count {
                            let bbox = boxes[i];
                            if aabb.max.x <= bbox.min.x + COLLISION_SKIN
                                || aabb.min.x >= bbox.max.x - COLLISION_SKIN
                                || aabb.max.z <= bbox.min.z + COLLISION_SKIN
                                || aabb.min.z >= bbox.max.z - COLLISION_SKIN
                            {
                                continue;
                            }

                            if dy < 0.0 && aabb.min.y >= bbox.max.y - FACE_TOLERANCE {
                                let allowed = (bbox.max.y - aabb.min.y).min(0.0);
                                if allowed > dy {
                                    dy = allowed;
                                }
                            } else if dy > 0.0 && aabb.max.y <= bbox.min.y + FACE_TOLERANCE {
                                let allowed =
                                    (bbox.min.y - COLLISION_SKIN - aabb.max.y).max(0.0);
                                if allowed < dy {
                                    dy = allowed;
                                }
                            }
                        }
                    }
                }
            }

            position.y += dy;
            if (dy - orig_dy).abs() > 1e-6 {
                if orig_dy < 0.0 {
                    // Snap cleanly to top surface of the supporting block
                    position.y = (position.y * 1024.0).round() / 1024.0;
                    *on_ground = true;
                }
                velocity.y = 0.0;
            }
        }

        let can_step = *on_ground || (step_idx == 0 && started_on_ground);

        // --- 2. Sweep X axis ---
        let orig_dx = velocity.x * step_dt;
        let mut dx = orig_dx;
        if dx.abs() > 0.0 {
            let aabb = AABB::from_player(*position, half_width, height);
            let min_y = (aabb.min.y + COLLISION_SKIN).floor() as i32 - 1;
            let max_y = (aabb.max.y - COLLISION_SKIN).floor() as i32;
            let min_z = (aabb.min.z + COLLISION_SKIN).floor() as i32;
            let max_z = (aabb.max.z - COLLISION_SKIN).floor() as i32;
            let min_x = (aabb.min.x + dx.min(0.0)).floor() as i32 - 1;
            let max_x = (aabb.max.x + dx.max(0.0)).ceil() as i32 + 1;

            let mut hit_face_x: Option<f32> = None;

            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    for x in min_x..=max_x {
                        let bpos = IVec3::new(x, y, z);
                        let (boxes, count) = block_collision_boxes(world, registry, bpos);
                        for i in 0..count {
                            let bbox = boxes[i];
                            if aabb.max.y <= bbox.min.y + COLLISION_SKIN
                                || aabb.min.y >= bbox.max.y - COLLISION_SKIN
                                || aabb.max.z <= bbox.min.z + COLLISION_SKIN
                                || aabb.min.z >= bbox.max.z - COLLISION_SKIN
                            {
                                continue;
                            }

                            if dx > 0.0 && aabb.max.x <= bbox.min.x + FACE_TOLERANCE {
                                let allowed =
                                    (bbox.min.x - COLLISION_SKIN - aabb.max.x).max(0.0);
                                if allowed < dx {
                                    dx = allowed;
                                    hit_face_x = Some(bbox.min.x - half_width - COLLISION_SKIN);
                                }
                            } else if dx < 0.0 && aabb.min.x >= bbox.max.x - FACE_TOLERANCE {
                                let allowed =
                                    (bbox.max.x + COLLISION_SKIN - aabb.min.x).min(0.0);
                                if allowed > dx {
                                    dx = allowed;
                                    hit_face_x = Some(bbox.max.x + half_width + COLLISION_SKIN);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(clamped_x) = hit_face_x {
                let stepped = can_step
                    .then(|| {
                        let target = Vec3::new(position.x + orig_dx, position.y, position.z);
                        try_auto_step(target, half_width, height, world, registry)
                            .map(|new_y| (target.x, new_y))
                    })
                    .flatten();
                if let Some((new_x, new_y)) = stepped {
                    position.x = new_x;
                    position.y = new_y;
                    *on_ground = true;
                } else {
                    position.x = clamped_x;
                    velocity.x = 0.0;
                }
            } else {
                position.x += dx;
            }
        }

        // --- 3. Sweep Z axis ---
        let orig_dz = velocity.z * step_dt;
        let mut dz = orig_dz;
        if dz.abs() > 0.0 {
            let aabb = AABB::from_player(*position, half_width, height);
            let min_x = (aabb.min.x + COLLISION_SKIN).floor() as i32;
            let max_x = (aabb.max.x - COLLISION_SKIN).floor() as i32;
            let min_y = (aabb.min.y + COLLISION_SKIN).floor() as i32 - 1;
            let max_y = (aabb.max.y - COLLISION_SKIN).floor() as i32;
            let min_z = (aabb.min.z + dz.min(0.0)).floor() as i32 - 1;
            let max_z = (aabb.max.z + dz.max(0.0)).ceil() as i32 + 1;

            let mut hit_face_z: Option<f32> = None;

            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    for x in min_x..=max_x {
                        let bpos = IVec3::new(x, y, z);
                        let (boxes, count) = block_collision_boxes(world, registry, bpos);
                        for i in 0..count {
                            let bbox = boxes[i];
                            if aabb.max.x <= bbox.min.x + COLLISION_SKIN
                                || aabb.min.x >= bbox.max.x - COLLISION_SKIN
                                || aabb.max.y <= bbox.min.y + COLLISION_SKIN
                                || aabb.min.y >= bbox.max.y - COLLISION_SKIN
                            {
                                continue;
                            }

                            if dz > 0.0 && aabb.max.z <= bbox.min.z + FACE_TOLERANCE {
                                let allowed =
                                    (bbox.min.z - COLLISION_SKIN - aabb.max.z).max(0.0);
                                if allowed < dz {
                                    dz = allowed;
                                    hit_face_z = Some(bbox.min.z - half_width - COLLISION_SKIN);
                                }
                            } else if dz < 0.0 && aabb.min.z >= bbox.max.z - FACE_TOLERANCE {
                                let allowed =
                                    (bbox.max.z + COLLISION_SKIN - aabb.min.z).min(0.0);
                                if allowed > dz {
                                    dz = allowed;
                                    hit_face_z = Some(bbox.max.z + half_width + COLLISION_SKIN);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(clamped_z) = hit_face_z {
                let stepped = can_step
                    .then(|| {
                        let target = Vec3::new(position.x, position.y, position.z + orig_dz);
                        try_auto_step(target, half_width, height, world, registry)
                            .map(|new_y| (target.z, new_y))
                    })
                    .flatten();
                if let Some((new_z, new_y)) = stepped {
                    position.z = new_z;
                    position.y = new_y;
                    *on_ground = true;
                } else {
                    position.z = clamped_z;
                    velocity.z = 0.0;
                }
            } else {
                position.z += dz;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, ChunkPos};

    #[test]
    fn test_aabb_intersection() {
        let box1 = AABB::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let box2 = AABB::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5));
        let box3 = AABB::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));

        assert!(box1.intersects(&box2));
        assert!(!box1.intersects(&box3));
    }

    #[test]
    fn test_player_landing_on_solid_ground() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Create solid stone floor at y = 0
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, crate::block::BlockId::STONE);
            }
        }
        world.insert_chunk(chunk);

        // Drop player from y = 5.0
        let mut player = PlayerPhysics::new(Vec3::new(8.0, 5.0, 8.0));
        assert!(!player.on_ground);

        // Step simulation for 1.0 second
        for _ in 0..60 {
            player.update(1.0 / 60.0, Vec3::ZERO, false, false, &world, &registry);
        }

        // Player must have landed on top of block at y = 1.0
        assert!(player.on_ground);
        assert!((player.position.y - 1.0).abs() < 0.01);
        assert_eq!(player.velocity.y, 0.0);
    }

    #[test]
    fn test_player_fall_damage() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, crate::block::BlockId::STONE);
            }
        }
        world.insert_chunk(chunk);

        // Drop player from y = 10.0 (falls 9.0 blocks down to y = 1.0)
        let mut player = PlayerPhysics::new(Vec3::new(8.0, 10.0, 8.0));
        let mut total_fall_damage = 0.0;

        for _ in 0..60 {
            let dmg = player.update(1.0 / 60.0, Vec3::ZERO, false, false, &world, &registry);
            total_fall_damage += dmg;
        }

        assert!(player.on_ground);
        // Fell 9.0 blocks, threshold is 3.0 blocks -> ~6.0 damage (3 hearts)
        assert!(
            total_fall_damage > 5.5 && total_fall_damage < 6.5,
            "Expected ~6.0 fall damage, got {}",
            total_fall_damage
        );
    }

    #[test]
    fn test_player_touching_cactus() {
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));
        chunk.set(5, 1, 5, crate::block::BlockId::CACTUS);
        world.insert_chunk(chunk);

        // Player positioned right adjacent to the cactus block (cactus is at [5..6, 1..2, 5..6])
        // Player half_width is 0.3. If player is at x = 4.75, max.x is 5.05 -> brushes against cactus!
        let player_near = PlayerPhysics::new(Vec3::new(4.75, 1.0, 5.5));
        assert!(player_near.is_touching_cactus(&world));

        // Player far away at x = 2.0 -> not touching
        let player_far = PlayerPhysics::new(Vec3::new(2.0, 1.0, 5.5));
        assert!(!player_far.is_touching_cactus(&world));
    }

    #[test]
    fn test_player_walking_into_walls_never_glitches_or_climbs_inside() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(-1, 0, -1)));
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, -1)));
        world.insert_chunk(Chunk::new(ChunkPos::new(-1, 0, 0)));
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));

        // Build solid stone floor at y = 0 across [-16..16, -16..16]
        for z in -16..16 {
            for x in -16..16 {
                world.set_block(IVec3::new(x, 0, z), BlockId::STONE);
            }
        }

        // Build walls at x = -3 (where (-3.0_f32 - 0.3_f32) + 0.3_f32 > -3.0 in IEEE-754 f32!)
        // and at x = 2 (where (3.0_f32 + 0.3_f32) - 0.3_f32 < 3.0 in IEEE-754 f32!)
        // and a corner at z = -3
        for y in 1..=5 {
            for z in -8..=8 {
                world.set_block(IVec3::new(-3, y, z), BlockId::STONE);
                world.set_block(IVec3::new(2, y, z), BlockId::STONE);
            }
            for x in -8..=8 {
                world.set_block(IVec3::new(x, y, -3), BlockId::STONE);
            }
        }

        // 1. Walk +X and diagonally into wall at x = -3 and corner at (-3, -3)
        let mut player = PlayerPhysics::new(Vec3::new(-3.8, 1.0, -1.5));
        for _ in 0..180 {
            player.update(
                1.0 / 60.0,
                Vec3::new(1.0, 0.0, -0.5).normalize(),
                true,
                false,
                &world,
                &registry,
            );
            assert!(
                (player.position.y - 1.0).abs() < 1e-3,
                "Player must stay on the floor (y=1.0) when pressing into a wall, got y={}",
                player.position.y
            );
            assert!(
                !aabb_intersects_solid(
                    player.position,
                    player.half_width,
                    player.height,
                    COLLISION_SKIN,
                    &world,
                    &registry
                ),
                "Player AABB must never penetrate the wall at {:?}",
                player.position
            );
        }

        // 2. Walk -X into wall at x = 2 (face at x = 3.0)
        let mut player2 = PlayerPhysics::new(Vec3::new(3.8, 1.0, 0.5));
        for _ in 0..120 {
            player2.update(
                1.0 / 60.0,
                Vec3::new(-1.0, 0.0, 0.0),
                true,
                false,
                &world,
                &registry,
            );
            assert!((player2.position.y - 1.0).abs() < 1e-3);
            assert!(player2.position.x - player2.half_width >= 3.0);
        }
    }

    #[test]
    fn test_player_buried_in_blocks_depenetrates_cleanly() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));

        // Create a 3x3x4 solid stone pillar with floor around it
        for z in 0..16 {
            for x in 0..16 {
                world.set_block(IVec3::new(x, 0, z), BlockId::STONE);
            }
        }
        for y in 1..=3 {
            for z in 4..=6 {
                for x in 4..=6 {
                    world.set_block(IVec3::new(x, y, z), BlockId::STONE);
                }
            }
        }

        // Place player completely stuck inside the center of the solid 3x3x3 stone block mass
        let mut player = PlayerPhysics::new(Vec3::new(5.5, 1.0, 5.5));
        assert!(aabb_intersects_solid(
            player.position,
            player.half_width,
            player.height,
            COLLISION_SKIN,
            &world,
            &registry
        ));

        // Single physics update should immediately depenetrate the player out to a valid open space
        player.update(1.0 / 60.0, Vec3::X, false, false, &world, &registry);
        assert!(
            !aabb_intersects_solid(
                player.position,
                player.half_width,
                player.height,
                COLLISION_SKIN,
                &world,
                &registry
            ),
            "Player should be pushed out of solid blocks immediately, got pos={:?}",
            player.position
        );
    }

    #[test]
    fn test_stairs_walk_up_without_jumping() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));

        for z in 0..8 {
            for x in 0..16 {
                world.set_block(IVec3::new(x, 0, z), BlockId::STONE);
            }
        }
        // Stair at (4, 1, 4) facing NegX (3): lower step on -X side (x=4..4.5, y=1.5), upper step on +X side (x=4.5..5.0, y=2.0)
        world.set_block(IVec3::new(4, 1, 4), BlockId::OAK_STAIRS);
        world.set_block_facing(IVec3::new(4, 1, 4), 3);
        for x in 5..16 {
            world.set_block(IVec3::new(x, 1, 4), BlockId::STONE);
        }

        let mut player = PlayerPhysics::new(Vec3::new(3.2, 1.0, 4.5));
        player.on_ground = true;

        let mut reached_half_step = false;
        for _ in 0..120 {
            player.update(1.0 / 60.0, Vec3::X, false, false, &world, &registry);
            if (player.position.y - 1.5).abs() < 0.05 {
                reached_half_step = true;
            }
        }

        assert!(
            reached_half_step,
            "Player should step onto 0.5-high bottom stair step without jumping"
        );
        assert!(
            (player.position.y - 2.0).abs() < 1e-2 && player.position.x > 5.0,
            "Player should walk all the way up the stairs onto y=2.0 without jumping, got pos={:?}",
            player.position
        );
    }

    #[test]
    fn test_fence_blocks_player_jump() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        world.insert_chunk(Chunk::new(ChunkPos::new(0, 0, 0)));

        for z in 0..8 {
            for x in 0..8 {
                world.set_block(IVec3::new(x, 0, z), BlockId::STONE);
            }
        }
        // Fence wall along x = 4 (hitbox extends from y=1.0 to y=2.5)
        for z in 0..8 {
            world.set_block(IVec3::new(4, 1, z), BlockId::OAK_FENCE);
        }

        let mut player = PlayerPhysics::new(Vec3::new(3.2, 1.0, 4.5));
        player.on_ground = true;

        for _ in 0..120 {
            player.update(1.0 / 60.0, Vec3::X, true, false, &world, &registry);
        }

        assert!(
            player.position.x + player.half_width <= 4.0 + 1e-3,
            "Player should not be able to jump over a 1.5-tall oak fence, got pos={:?}",
            player.position
        );
    }
}

