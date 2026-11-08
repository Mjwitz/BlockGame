use glam::{IVec3, Vec3};

use crate::{block::BlockRegistry, direction::Direction, world::World};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    /// Coordinate of the solid voxel that was struck by the ray
    pub hit_voxel: IVec3,
    /// Normal face of the voxel that was intersected (where the ray entered)
    pub hit_face: Direction,
    /// Adjacent coordinate where a new block should be placed (hit_voxel + normal)
    pub place_voxel: IVec3,
    /// Distance from ray origin to the intersection point
    pub distance: f32,
}

/// Casts a ray through the voxel world using the Amanatides-Woo Fast Voxel Traversal algorithm.
pub fn raycast_voxel(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    world: &World,
    registry: &BlockRegistry,
) -> Option<RaycastHit> {
    let dir = direction.normalize();

    let mut current = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    // If starting inside a solid block
    let start_block = world.get_block(current);
    if registry.is_solid(start_block) {
        return Some(RaycastHit {
            hit_voxel: current,
            hit_face: Direction::PosY,
            place_voxel: current + Direction::PosY.normal_ivec(),
            distance: 0.0,
        });
    }

    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    let delta_x = if dir.x.abs() > 1e-6 { (1.0 / dir.x).abs() } else { f32::INFINITY };
    let delta_y = if dir.y.abs() > 1e-6 { (1.0 / dir.y).abs() } else { f32::INFINITY };
    let delta_z = if dir.z.abs() > 1e-6 { (1.0 / dir.z).abs() } else { f32::INFINITY };

    let mut t_max_x = if step_x > 0 {
        ((current.x + 1) as f32 - origin.x) * delta_x
    } else {
        (origin.x - current.x as f32) * delta_x
    };

    let mut t_max_y = if step_y > 0 {
        ((current.y + 1) as f32 - origin.y) * delta_y
    } else {
        (origin.y - current.y as f32) * delta_y
    };

    let mut t_max_z = if step_z > 0 {
        ((current.z + 1) as f32 - origin.z) * delta_z
    } else {
        (origin.z - current.z as f32) * delta_z
    };

    let mut hit_face;
    let mut distance = 0.0;

    while distance <= max_distance {
        if t_max_x < t_max_y && t_max_x < t_max_z {
            distance = t_max_x;
            t_max_x += delta_x;
            current.x += step_x;
            hit_face = if step_x > 0 { Direction::NegX } else { Direction::PosX };
        } else if t_max_y < t_max_z {
            distance = t_max_y;
            t_max_y += delta_y;
            current.y += step_y;
            hit_face = if step_y > 0 { Direction::NegY } else { Direction::PosY };
        } else {
            distance = t_max_z;
            t_max_z += delta_z;
            current.z += step_z;
            hit_face = if step_z > 0 { Direction::NegZ } else { Direction::PosZ };
        }

        if distance > max_distance {
            break;
        }

        let block = world.get_block(current);
        if registry.is_solid(block) {
            let place_voxel = current + hit_face.normal_ivec();
            return Some(RaycastHit {
                hit_voxel: current,
                hit_face,
                place_voxel,
                distance,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{Chunk, ChunkPos};

    #[test]
    fn test_raycast_hitting_top_face() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        // Place a stone block at (5, 5, 5)
        chunk.set(5, 5, 5, crate::block::BlockId::STONE);
        world.insert_chunk(chunk);

        // Raycast straight down from (5.5, 8.0, 5.5) toward -Y
        let hit = raycast_voxel(
            Vec3::new(5.5, 8.0, 5.5),
            Vec3::new(0.0, -1.0, 0.0),
            5.0,
            &world,
            &registry,
        );

        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.hit_voxel, IVec3::new(5, 5, 5));
        assert_eq!(hit.hit_face, Direction::PosY);
        assert_eq!(hit.place_voxel, IVec3::new(5, 6, 5));
        assert!((hit.distance - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_raycast_hitting_side_face() {
        let registry = BlockRegistry::new();
        let mut world = World::new();
        let mut chunk = Chunk::new(ChunkPos::new(0, 0, 0));

        chunk.set(10, 5, 5, crate::block::BlockId::STONE);
        world.insert_chunk(chunk);

        // Raycast toward +X from (7.0, 5.5, 5.5)
        let hit = raycast_voxel(
            Vec3::new(7.0, 5.5, 5.5),
            Vec3::new(1.0, 0.0, 0.0),
            5.0,
            &world,
            &registry,
        );

        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.hit_voxel, IVec3::new(10, 5, 5));
        // Hit the West (-X) face
        assert_eq!(hit.hit_face, Direction::NegX);
        assert_eq!(hit.place_voxel, IVec3::new(9, 5, 5));
    }
}
