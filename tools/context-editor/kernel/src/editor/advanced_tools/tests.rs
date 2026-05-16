use super::*;

#[test]
fn fill_stops_at_solid() {
    let mut svo = VoxelWorld::new(4);
    let mat = VoxelMaterial::new(255, 0, 0, 16);
    svo.set_voxel(IVec3::new(1, 0, 0), mat);

    let result = flood_fill(&svo, IVec3::new(1, 0, 0), mat, 10);
    assert!(result.is_empty());
}

#[test]
fn fill_respect_max_count() {
    let svo = VoxelWorld::new(4);
    let mat = VoxelMaterial::new(0, 255, 0, 8);
    let result = flood_fill(&svo, IVec3::ZERO, mat, 50);
    assert!(result.len() <= 50);
}

#[test]
fn smooth_averages_colors() {
    let mut svo = VoxelWorld::new(8);
    let pos = IVec3::new(100, 100, 100);
    let mat = VoxelMaterial::new(200, 100, 50, 16);
    svo.set_voxel(pos, mat);

    let readback = read_material(&svo, pos);
    assert!(readback.is_some(), "single voxel should be readable");
    let material = readback.unwrap();
    assert_eq!(material.r, 200);
    assert_eq!(material.g, 100);
    assert_eq!(material.b, 50);
}

#[test]
fn capture_and_clone() {
    let mut svo = VoxelWorld::new(4);
    let mat = VoxelMaterial::new(50, 100, 150, 8);
    svo.set_voxel(IVec3::ZERO, mat);

    let info = HitInfo {
        position: Vec3::ZERO,
        normal: Vec3::Y,
        cell: IVec3::ZERO,
    };
    let clip = capture_region(&svo, &info, 1);
    assert!(!clip.voxels.is_empty());
    assert!(clip.voxels.iter().any(|(offset, _)| *offset == IVec3::ZERO));
}
