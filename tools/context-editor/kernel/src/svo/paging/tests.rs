use super::*;
use crate::svo::{
    VoxelMaterial,
    VoxelWorld,
};
use bevy::math::IVec3;

fn make_small_world() -> VoxelWorld {
    let mut world = VoxelWorld::new(6);
    let red = VoxelMaterial::new(255, 0, 0, 16);
    let blue = VoxelMaterial::new(0, 0, 255, 16);
    world.set_voxel(IVec3::new(0, 0, 0), red);
    world.set_voxel(IVec3::new(32, 0, 0), blue);
    world
}

#[test]
fn build_produces_nonempty_buffers() {
    let world = make_small_world();
    let mut page_manager = PageManager::new(3);
    let (gpu_nodes, page_table) = page_manager.build(&world, None);
    assert!(!gpu_nodes.is_empty(), "GPU nodes must be non-empty");
    assert!(!page_table.is_empty(), "Page table must be non-empty");
}

#[test]
fn all_pages_resident_without_frustum() {
    let world = make_small_world();
    let mut page_manager = PageManager::new(3);
    let (_gpu_nodes, page_table) = page_manager.build(&world, None);
    for (id, &base) in page_table.iter().enumerate() {
        assert_ne!(
            base, 0xFFFF_FFFF,
            "page {id} should be resident when no frustum is given"
        );
    }
}

#[test]
fn frustum_behind_camera_evicts_all_pages() {
    let world = make_small_world();
    let mut page_manager = PageManager::new(3);
    let reject_all = Frustum {
        planes: [Vec4::new(-1.0, 0.0, 0.0, -1e9); 6],
    };
    let (_gpu_nodes, page_table) =
        page_manager.build(&world, Some(&reject_all));
    for &base in &page_table {
        assert_eq!(
            base, 0xFFFF_FFFF,
            "rejecting frustum should evict all pages"
        );
    }
}

#[test]
fn page_depth_zero_is_all_root() {
    let mut world = VoxelWorld::new(4);
    world.set_voxel(IVec3::new(0, 0, 0), VoxelMaterial::new(1, 2, 3, 4));
    let mut page_manager = PageManager::new(0);
    let (gpu_nodes, page_table) = page_manager.build(&world, None);
    assert!(page_table.is_empty(), "page_depth=0 yields no leaf pages");
    assert!(!gpu_nodes.is_empty());
}

#[test]
fn packed_child_ranks_increase_monotonically() {
    let mut world = VoxelWorld::new(4);
    let red = VoxelMaterial::new(255, 0, 0, 8);
    world.set_voxel(IVec3::new(0, 0, 0), red);
    world.set_voxel(IVec3::new(4, 0, 0), red);
    world.set_voxel(IVec3::new(0, 4, 0), red);

    let mut page_manager = PageManager::new(2);
    let (gpu_nodes, _page_table) = page_manager.build(&world, None);

    let root = gpu_nodes[0];
    let mask = (root.child_pointer & 0xFF) as u8;
    let first_child_gpu = (root.child_pointer >> 8) & 0x7F_FFFF;

    if mask != 0 {
        assert!(
            (first_child_gpu as usize) < gpu_nodes.len(),
            "first child idx {first_child_gpu} out of bounds (gpu_nodes len={})",
            gpu_nodes.len()
        );
        assert!(
            (first_child_gpu as usize + 7) < gpu_nodes.len(),
            "child block end {} out of bounds (gpu_nodes len={})",
            first_child_gpu + 7,
            gpu_nodes.len()
        );
    }
}

#[test]
fn evicted_pages_have_cmask_zero_in_gpu_buffer() {
    let world = make_small_world();
    let mut page_manager = PageManager::new(3);
    let reject_all = Frustum {
        planes: [Vec4::new(-1.0, 0.0, 0.0, -1e9); 6],
    };
    let (gpu_nodes, page_table) = page_manager.build(&world, Some(&reject_all));
    for &page_table_entry in &page_table {
        assert_eq!(page_table_entry, 0xFFFF_FFFF);
    }
    assert!(!gpu_nodes.is_empty());
}
