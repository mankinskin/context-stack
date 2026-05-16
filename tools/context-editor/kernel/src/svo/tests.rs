use super::*;

fn red() -> VoxelMaterial {
    VoxelMaterial::new(255, 0, 0, 16)
}

#[test]
fn material_pack_roundtrip() {
    let material = VoxelMaterial::new(10, 20, 30, 25);
    let unpacked = VoxelMaterial::unpack(material.pack());
    assert_eq!(unpacked.r, material.r);
    assert_eq!(unpacked.g, material.g);
    assert_eq!(unpacked.b, material.b);
    assert_eq!(unpacked.roughness, material.roughness);
    assert!(!unpacked.metallic);
}

#[test]
fn material_pack_roundtrip_metallic() {
    let material = VoxelMaterial::new_metallic(200, 150, 100, 31, true);
    let unpacked = VoxelMaterial::unpack(material.pack());
    assert_eq!(unpacked.r, 200);
    assert_eq!(unpacked.g, 150);
    assert_eq!(unpacked.b, 100);
    assert_eq!(unpacked.roughness, 31);
    assert!(unpacked.metallic);
}

#[test]
fn material_roughness_clamped_to_5_bits() {
    let material = VoxelMaterial::new(0, 0, 0, 255);
    let unpacked = VoxelMaterial::unpack(material.pack());
    assert_eq!(unpacked.roughness, 31);
}

#[test]
fn set_voxel_marks_dirty() {
    let mut world = VoxelWorld::new(4);
    world.set_voxel(IVec3::ZERO, red());
    let ranges = world.take_dirty_ranges();
    assert!(!ranges.is_empty(), "set_voxel must mark dirty ranges");
    assert!(
        world.take_dirty_ranges().is_empty(),
        "take_dirty_ranges must clear state"
    );
}

#[test]
fn set_then_descend() {
    let mut world = VoxelWorld::new(4);
    world.set_voxel(IVec3::new(3, 2, 1), red());
    let idx = world.descend_to(IVec3::new(3, 2, 1));
    assert!(idx.is_some(), "descend_to must find voxel after set_voxel");
    assert_ne!(
        world.nodes[idx.unwrap()].color_data,
        0,
        "leaf must have non-zero color_data"
    );
}

#[test]
fn apply_sdf_brush_sphere() {
    let mut world = VoxelWorld::new(5);
    let painted = world.apply_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 3.0, red());
    assert!(painted > 0, "apply_sdf_brush must paint at least one voxel");
}

#[test]
fn carve_sdf_brush() {
    let mut world = VoxelWorld::new(5);
    world.apply_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 4.0, red());
    world.take_dirty_ranges();
    let carved = world.carve_sdf_brush(Vec3::new(8.0, 8.0, 8.0), 2.0);
    assert!(carved > 0, "carve_sdf_brush must carve at least one voxel");
    let ranges = world.take_dirty_ranges();
    assert!(!ranges.is_empty(), "carve must mark dirty ranges");
}

#[test]
fn merge_ranges_basic() {
    let input = vec![(0, 8), (8, 16), (32, 40)];
    let merged = merge_ranges(&input);
    assert_eq!(merged, vec![(0, 16), (32, 40)]);
}

#[test]
fn child_index_all_combinations() {
    let mut seen = [false; 8];
    for cz in [false, true] {
        for cy in [false, true] {
            for cx in [false, true] {
                let idx = child_index(cx, cy, cz);
                assert!(!seen[idx], "duplicate child index");
                seen[idx] = true;
            }
        }
    }
    assert!(seen.iter().all(|&present| present), "all 8 slots must be covered");
}
