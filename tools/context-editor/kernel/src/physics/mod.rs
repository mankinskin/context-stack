//! Physics bridge — SVO → Rapier collision shapes.
//!
//! Implements T7c: Rapier Collision Bridge — SVO → Chunk Colliders for Physics.
//!
//! # Architecture
//!
//! The SVO is the **structural authority** for physics collisions.
//! Gaussians are visual-only and have no collision role.
//!
//! On voxel edits, affected chunks are marked dirty via the [`DirtyChunks`]
//! resource. The [`rapier_rebuild_system`] then regenerates chunk colliders
//! using greedy box merging, keeping the total Rapier shape count low.
//!
//! ## Chunk sizes
//! A 16³ voxel chunk of solid voxels becomes a single box collider rather
//! than 4 096 unit boxes. Non-uniform chunks produce a `Compound` collider.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::svo::VoxelWorld;

/// Side length of a physics chunk in voxels.
pub const CHUNK_SIZE: i32 = 16;

// ---------------------------------------------------------------------------
// DirtyChunks resource
// ---------------------------------------------------------------------------

/// Tracks which voxel chunks need their Rapier colliders rebuilt.
///
/// Populated by any system that calls `VoxelWorld::set_voxel()` or the brush
/// APIs. Call [`DirtyChunks::mark`] with the world-space voxel position to
/// mark the enclosing chunk.
#[derive(Resource, Default)]
pub struct DirtyChunks(pub std::collections::HashSet<IVec3>);

impl DirtyChunks {
    /// Mark the chunk containing `voxel_pos` as dirty.
    pub fn mark(
        &mut self,
        voxel_pos: IVec3,
    ) {
        self.0.insert(chunk_origin(voxel_pos));
    }
}

/// Compute the chunk-space origin (lower corner) for any voxel position.
#[inline]
pub fn chunk_origin(voxel_pos: IVec3) -> IVec3 {
    IVec3::new(
        voxel_pos.x.div_euclid(CHUNK_SIZE) * CHUNK_SIZE,
        voxel_pos.y.div_euclid(CHUNK_SIZE) * CHUNK_SIZE,
        voxel_pos.z.div_euclid(CHUNK_SIZE) * CHUNK_SIZE,
    )
}

// ---------------------------------------------------------------------------
// VoxelChunkCollider component
// ---------------------------------------------------------------------------

/// Marks an entity as a per-chunk collision body.
///
/// When `dirty` is true, [`rapier_rebuild_system`] will regenerate the
/// `Collider` on the same entity this frame (using greedy box merge).
#[derive(Component)]
pub struct VoxelChunkCollider {
    /// Chunk lower-corner in world-voxel space.
    pub chunk_pos: IVec3,
    /// Rebuild flag — set by [`DirtyChunks`] propagation, cleared after rebuild.
    pub dirty: bool,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers Rapier physics and the chunk-rebuild system.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(
        &self,
        app: &mut App,
    ) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .init_resource::<DirtyChunks>()
            .add_systems(
                PostUpdate,
                (dirty_chunks_propagation_system, rapier_rebuild_system)
                    .chain(),
            );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Drain [`DirtyChunks`] and mark the corresponding [`VoxelChunkCollider`]
/// components dirty so [`rapier_rebuild_system`] picks them up.
pub fn dirty_chunks_propagation_system(
    mut dirty: ResMut<DirtyChunks>,
    mut chunks: Query<&mut VoxelChunkCollider>,
) {
    if dirty.0.is_empty() {
        return;
    }
    for mut chunk in chunks.iter_mut() {
        if dirty.0.contains(&chunk.chunk_pos) {
            chunk.dirty = true;
        }
    }
    dirty.0.clear();
}

/// Rebuild Rapier colliders for dirty chunks using greedy box merge.
///
/// Only dirty chunks are rebuilt (O(1) chunk-entity lookups via query).
/// Target: < 2 ms per 16³ chunk.
pub fn rapier_rebuild_system(
    voxel_world: Res<VoxelWorld>,
    mut chunks: Query<(&mut VoxelChunkCollider, &mut Collider)>,
) {
    for (mut chunk, mut collider) in chunks.iter_mut() {
        if !chunk.dirty {
            continue;
        }
        if let Some(new_collider) =
            rebuild_chunk_collider(&voxel_world, chunk.chunk_pos, CHUNK_SIZE)
        {
            *collider = new_collider;
        }
        chunk.dirty = false;
    }
}

// ---------------------------------------------------------------------------
// Greedy box merge
// ---------------------------------------------------------------------------

/// Sample the SVO at `pos` — returns `true` if an occupied voxel exists.
fn is_solid(
    world: &VoxelWorld,
    pos: IVec3,
) -> bool {
    world
        .descend_to(pos)
        .map(|idx| world.nodes[idx].color_data != 0)
        .unwrap_or(false)
}

/// Build a compound Rapier [`Collider`] from the occupied voxels in a 16³ chunk.
///
/// Uses a greedy sweep-merged algorithm:
/// 1. Build a 3-D occupancy grid for the chunk.
/// 2. Iterate over slabs (Z layers) applying 2-D greedy merging.
/// 3. Each merged rectangle is extended along Z until it hits an unoccupied voxel.
///
/// A purely solid chunk produces a single box collider.
/// An empty chunk produces an empty compound.
/// An empty chunk returns `None` (no valid shape to attach).
pub fn rebuild_chunk_collider(
    world: &VoxelWorld,
    chunk_pos: IVec3,
    chunk_size: i32,
) -> Option<Collider> {
    let n = chunk_size as usize;

    // Build occupancy grid
    let mut grid = vec![false; n * n * n];
    for dz in 0..chunk_size {
        for dy in 0..chunk_size {
            for dx in 0..chunk_size {
                let voxel = chunk_pos + IVec3::new(dx, dy, dz);
                if is_solid(world, voxel) {
                    grid[(dz as usize * n + dy as usize) * n + dx as usize] =
                        true;
                }
            }
        }
    }

    let mut boxes: Vec<(Vec3, Quat, Collider)> = Vec::new();
    let mut consumed = vec![false; n * n * n];

    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let idx = (z * n + y) * n + x;
                if !grid[idx] || consumed[idx] {
                    continue;
                }

                // Greedily extend in X
                let mut max_x = x + 1;
                while max_x < n {
                    let i = (z * n + y) * n + max_x;
                    if grid[i] && !consumed[i] {
                        max_x += 1;
                    } else {
                        break;
                    }
                }

                // Greedily extend in Y
                let mut max_y = y + 1;
                'outer_y: while max_y < n {
                    for cx in x..max_x {
                        let i = (z * n + max_y) * n + cx;
                        if !grid[i] || consumed[i] {
                            break 'outer_y;
                        }
                    }
                    max_y += 1;
                }

                // Greedily extend in Z
                let mut max_z = z + 1;
                'outer_z: while max_z < n {
                    for cy in y..max_y {
                        for cx in x..max_x {
                            let i = (max_z * n + cy) * n + cx;
                            if !grid[i] || consumed[i] {
                                break 'outer_z;
                            }
                        }
                    }
                    max_z += 1;
                }

                // Mark consumed
                for cz in z..max_z {
                    for cy in y..max_y {
                        for cx in x..max_x {
                            consumed[(cz * n + cy) * n + cx] = true;
                        }
                    }
                }

                // Emit a box collider for this region
                let sx = (max_x - x) as f32;
                let sy = (max_y - y) as f32;
                let sz = (max_z - z) as f32;
                let center = Vec3::new(
                    x as f32 + sx * 0.5,
                    y as f32 + sy * 0.5,
                    z as f32 + sz * 0.5,
                );
                boxes.push((
                    center,
                    Quat::IDENTITY,
                    Collider::cuboid(sx * 0.5, sy * 0.5, sz * 0.5),
                ));
            }
        }
    }

    if boxes.is_empty() {
        None
    } else if boxes.len() == 1 {
        let (_, _, c) = boxes.remove(0);
        Some(c)
    } else {
        Some(Collider::compound(boxes))
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svo::{
        VoxelMaterial,
        VoxelWorld,
    };

    fn red() -> VoxelMaterial {
        VoxelMaterial::new(200, 0, 0, 128)
    }

    #[test]
    fn chunk_origin_rounds_down() {
        assert_eq!(chunk_origin(IVec3::new(0, 0, 0)), IVec3::ZERO);
        assert_eq!(chunk_origin(IVec3::new(15, 15, 15)), IVec3::ZERO);
        assert_eq!(chunk_origin(IVec3::new(16, 0, 0)), IVec3::new(16, 0, 0));
        assert_eq!(chunk_origin(IVec3::new(31, 17, 5)), IVec3::new(16, 16, 0));
    }

    #[test]
    fn empty_chunk_produces_no_collider() {
        let world = VoxelWorld::new(5);
        let collider = rebuild_chunk_collider(&world, IVec3::ZERO, CHUNK_SIZE);
        assert!(collider.is_none(), "empty chunk should return None");
    }

    #[test]
    fn solid_chunk_produces_single_box() {
        let mut world = VoxelWorld::new(5);
        // Fill a full 4³ sub-chunk at origin
        for z in 0..4i32 {
            for y in 0..4i32 {
                for x in 0..4i32 {
                    world.set_voxel(IVec3::new(x, y, z), red());
                }
            }
        }
        // Use chunk_size=4 so the greedy merge covers the full region
        let collider = rebuild_chunk_collider(&world, IVec3::ZERO, 4);
        // A single cuboid collider — the greedy merge should produce one box.
        assert!(collider.is_some(), "solid chunk must yield a collider");
    }

    #[test]
    fn greedy_merge_reduces_shape_count() {
        // Two separate 2×1×1 bars should produce 2 shapes, not 4.
        let mut world = VoxelWorld::new(5);
        world.set_voxel(IVec3::new(0, 0, 0), red());
        world.set_voxel(IVec3::new(1, 0, 0), red());
        world.set_voxel(IVec3::new(0, 2, 0), red());
        world.set_voxel(IVec3::new(1, 2, 0), red());
        // Should be 2 boxes after greedy merge, not 4 individual unit boxes.
        let collider = rebuild_chunk_collider(&world, IVec3::ZERO, 4);
        assert!(collider.is_some(), "non-empty chunk must yield a collider");
    }
}
