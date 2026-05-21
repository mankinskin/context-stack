use super::*;
use crate::svo::VoxelMaterial;

#[test]
fn default_state_values() {
    let s = DebugOverlayState::default();
    assert!(!s.enabled);
    assert_eq!(s.display_depth, 4);
    assert!(s.occupied_only);
}

#[test]
fn shared_enabled_roundtrip() {
    WIREFRAME_ENABLED.store(true, Ordering::Relaxed);
    assert!(WIREFRAME_ENABLED.load(Ordering::Relaxed));
    WIREFRAME_ENABLED.store(false, Ordering::Relaxed);
    assert!(!WIREFRAME_ENABLED.load(Ordering::Relaxed));
}

#[test]
fn shared_depth_roundtrip() {
    WIREFRAME_DEPTH.store(7, Ordering::Relaxed);
    assert_eq!(WIREFRAME_DEPTH.load(Ordering::Relaxed), 7);
    WIREFRAME_DEPTH.store(4, Ordering::Relaxed);
}

#[test]
fn wire_cube_corner_geometry() {
    let center = Vec3::new(10.0, 20.0, 30.0);
    let size = 4.0;
    let h = size * 0.5;
    assert_eq!(center - Vec3::splat(h), Vec3::new(8.0, 18.0, 28.0));
    assert_eq!(center + Vec3::splat(h), Vec3::new(12.0, 22.0, 32.0));
}

#[test]
fn empty_world_root_is_leaf() {
    let world = VoxelWorld::new(4);
    assert_eq!(world.nodes.len(), 1);
    assert!(world.nodes[0].is_leaf());
    assert_eq!(world.nodes[0].color_data, 0);
}

#[test]
fn occupied_world_has_children() {
    let mut world = VoxelWorld::new(4);
    world.set_voxel(IVec3::new(1, 2, 3), VoxelMaterial::new(255, 0, 0, 8));
    assert!(world.nodes[0].child_mask() != 0);
    assert!(!world.nodes[0].is_leaf());
}

#[test]
fn collect_finds_occupied_voxel() {
    let mut world = VoxelWorld::new(4);
    let red = VoxelMaterial::new(255, 0, 0, 8);

    world.set_voxel(IVec3::new(1, 1, 1), red);
    world.set_voxel(IVec3::new(14, 14, 14), red);

    let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
        std::collections::VecDeque::new();
    let mut cubes: Vec<(Vec3, f32)> = Vec::new();
    let root_extent = (1u32 << world.max_depth) as f32;
    queue.push_back((world.root_index as usize, 0, Vec3::ZERO, root_extent));

    while let Some((idx, depth, origin, extent)) = queue.pop_front() {
        if idx >= world.nodes.len() {
            continue;
        }
        let node = &world.nodes[idx];
        let center = origin + Vec3::splat(extent * 0.5);
        let half = extent * 0.5;

        if node.color_data == 0 && node.child_mask() == 0 {
            continue;
        }
        if depth >= 2 || node.is_leaf() {
            cubes.push((center, extent));
            continue;
        }

        let mask = node.child_mask();
        let first = node.first_child_index();
        for slot in 0..8usize {
            if mask & (1 << slot) == 0 {
                continue;
            }
            let child_origin = origin
                + Vec3::new(
                    if slot & 1 != 0 { half } else { 0.0 },
                    if slot & 2 != 0 { half } else { 0.0 },
                    if slot & 4 != 0 { half } else { 0.0 },
                );
            queue.push_back((first + slot, depth + 1, child_origin, half));
        }
    }

    assert!(!cubes.is_empty());
}

#[test]
fn collect_respects_depth_limit() {
    let mut world = VoxelWorld::new(4);
    world.set_voxel(IVec3::new(8, 8, 8), VoxelMaterial::new(255, 0, 0, 8));

    let target = 1;
    let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
        std::collections::VecDeque::new();
    let mut depths = Vec::new();
    let root_extent = (1u32 << world.max_depth) as f32;
    queue.push_back((world.root_index as usize, 0, Vec3::ZERO, root_extent));

    while let Some((idx, depth, origin, extent)) = queue.pop_front() {
        if idx >= world.nodes.len() {
            continue;
        }
        let node = &world.nodes[idx];
        let half = extent * 0.5;

        if node.color_data == 0 && node.child_mask() == 0 {
            continue;
        }
        if depth >= target || node.is_leaf() {
            depths.push(depth);
            continue;
        }

        let mask = node.child_mask();
        let first = node.first_child_index();
        for slot in 0..8usize {
            if mask & (1 << slot) == 0 {
                continue;
            }
            let child_origin = origin
                + Vec3::new(
                    if slot & 1 != 0 { half } else { 0.0 },
                    if slot & 2 != 0 { half } else { 0.0 },
                    if slot & 4 != 0 { half } else { 0.0 },
                );
            queue.push_back((first + slot, depth + 1, child_origin, half));
        }
    }

    assert!(depths.iter().all(|&depth| depth <= target));
}

#[test]
fn collect_occ_only_skips_empty() {
    let world = VoxelWorld::new(4);
    let mut queue: std::collections::VecDeque<(usize, u32, Vec3, f32)> =
        std::collections::VecDeque::new();
    let mut cubes: Vec<(Vec3, f32)> = Vec::new();
    let root_extent = (1u32 << world.max_depth) as f32;
    queue.push_back((world.root_index as usize, 0, Vec3::ZERO, root_extent));

    while let Some((idx, depth, origin, extent)) = queue.pop_front() {
        if idx >= world.nodes.len() {
            continue;
        }
        let node = &world.nodes[idx];
        let center = origin + Vec3::splat(extent * 0.5);
        let half = extent * 0.5;

        if node.color_data == 0 && node.child_mask() == 0 {
            continue;
        }
        if depth >= 4 || node.is_leaf() {
            cubes.push((center, extent));
            continue;
        }

        let mask = node.child_mask();
        let first = node.first_child_index();
        for slot in 0..8usize {
            if mask & (1 << slot) == 0 {
                continue;
            }
            let child_origin = origin
                + Vec3::new(
                    if slot & 1 != 0 { half } else { 0.0 },
                    if slot & 2 != 0 { half } else { 0.0 },
                    if slot & 4 != 0 { half } else { 0.0 },
                );
            queue.push_back((first + slot, depth + 1, child_origin, half));
        }
    }

    assert!(
        cubes.is_empty(),
        "empty world should produce no occupied cubes"
    );
}
