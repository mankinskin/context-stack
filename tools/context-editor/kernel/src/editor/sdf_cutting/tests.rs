use super::*;

#[test]
fn csg_subtraction_basic() {
    let result = sdf_csg(CsgOp::Subtraction, -1.0, -0.5);
    assert!((result - 0.5).abs() < f32::EPSILON);
}

#[test]
fn csg_union_takes_minimum() {
    assert!((sdf_csg(CsgOp::Union, 2.0, 3.0) - 2.0).abs() < f32::EPSILON);
    assert!((sdf_csg(CsgOp::Union, -1.0, 1.0) - (-1.0)).abs() < f32::EPSILON);
}

#[test]
fn csg_intersection_takes_maximum() {
    assert!((sdf_csg(CsgOp::Intersection, 2.0, 3.0) - 3.0).abs() < f32::EPSILON);
}

#[test]
fn smooth_subtraction_matches_hard_at_zero_k() {
    let hard = sdf_csg(CsgOp::Subtraction, -1.0, -0.5);
    let smooth = sdf_smooth_subtraction(-1.0, -0.5, 0.0);
    assert!((hard - smooth).abs() < f32::EPSILON);
}

#[test]
fn sdf_sphere_center_is_negative_radius() {
    let d = sdf_sphere(Vec3::ZERO, Vec3::ZERO, 2.0);
    assert!((d - (-2.0)).abs() < f32::EPSILON);
}

#[test]
fn sdf_sphere_surface_is_zero() {
    let d = sdf_sphere(Vec3::new(3.0, 0.0, 0.0), Vec3::ZERO, 3.0);
    assert!(d.abs() < f32::EPSILON);
}

#[test]
fn sdf_box_center_is_negative() {
    let d = sdf_box(Vec3::ZERO, Vec3::ZERO, Vec3::splat(1.0));
    assert!(d < 0.0, "center of box should be inside (negative)");
}

#[test]
fn sdf_box_outside_is_positive() {
    let d = sdf_box(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(1.0));
    assert!(d > 0.0, "point far from box should be outside (positive)");
}

#[test]
fn cut_particles_expire() {
    let mut pool = CutParticles::default();
    let req = CutRequest {
        center: Vec3::ZERO,
        radius: 1.0,
        blend: 0.0,
        direction: Vec3::Y,
        impact_strength: 1.0,
    };
    emit_cut_particles(&mut pool, &req, [200, 100, 50, 255], 10);
    assert_eq!(pool.particles.len(), 10);

    tick_cut_particles(&mut pool, 2.0);
    assert!(pool.particles.is_empty(), "all particles should have expired");
}

#[test]
fn glass_impacts_expire() {
    let mut impacts = GlassImpacts::default();
    let req = CutRequest {
        center: Vec3::ZERO,
        radius: 1.0,
        blend: 0.0,
        direction: Vec3::Y,
        impact_strength: 2.0,
    };
    spawn_glass_impact(&mut impacts, &req);
    assert_eq!(impacts.impacts.len(), 1);
    assert!((impacts.impacts[0].strength - 2.0).abs() < f32::EPSILON);

    tick_glass_impacts(&mut impacts, 1.0);
    assert!(impacts.impacts.is_empty(), "impact should have expired");
}

#[test]
fn emit_respects_max_cap() {
    let mut pool = CutParticles::default();
    let req = CutRequest {
        center: Vec3::ZERO,
        radius: 1.0,
        blend: 0.0,
        direction: Vec3::Y,
        impact_strength: 1.0,
    };
    emit_cut_particles(&mut pool, &req, [255; 4], MAX_CUT_PARTICLES as u32 + 100);
    assert_eq!(pool.particles.len(), MAX_CUT_PARTICLES);
}

#[test]
fn apply_sdf_cut_removes_voxels() {
    let mut world = crate::svo::VoxelWorld::new(8);
    let material = crate::svo::VoxelMaterial::new(200, 100, 50, 128);
    world.set_voxel(IVec3::ZERO, material);

    let req = CutRequest {
        center: Vec3::ZERO,
        radius: 2.0,
        blend: 0.0,
        direction: Vec3::Y,
        impact_strength: 1.0,
    };
    let (removed, color) = apply_sdf_cut(&mut world, &req);
    assert!(removed >= 1, "should remove at least the placed voxel");
    assert!(color[0] > 0);
}

#[test]
fn smooth_union_is_symmetric() {
    let a = sdf_smooth_union(1.0, 2.0, 0.5);
    let b = sdf_smooth_union(2.0, 1.0, 0.5);
    assert!((a - b).abs() < 1e-5, "smooth union should be commutative");
}
