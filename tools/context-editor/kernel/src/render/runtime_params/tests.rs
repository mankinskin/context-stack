use super::*;

#[test]
fn default_params_are_sane() {
    let params = RenderParams::default();
    assert_eq!(params.svo.max_depth, 10);
    assert_eq!(params.splat.max_splats, 2_000_000);
    assert!(params.splat.generation_enabled);
    assert!(params.double_buffer.enabled);
    assert_eq!(params.double_buffer.upload_budget_bytes, 4 * 1024 * 1024);
    assert_eq!(params.tile.tile_size, 16);
    assert_eq!(params.sort.num_passes, 8);
    assert!((params.glass.ior - 1.5).abs() < f32::EPSILON);
}

#[test]
fn gpu_uniforms_size_is_pod() {
    let size = std::mem::size_of::<GpuRenderUniforms>();
    assert_eq!(
        size % 16,
        0,
        "GpuRenderUniforms must be 16-byte aligned, got {size}"
    );
}

#[test]
fn from_params_computes_grid_dimensions() {
    let params = RenderParams::default();
    let uniforms = GpuRenderUniforms::from_params(
        &params,
        &Mat4::IDENTITY,
        &Mat4::IDENTITY,
        Vec3::ZERO,
        Vec2::new(800.0, 600.0),
        42,
    );
    assert_eq!(uniforms.grid_width, 50);
    assert_eq!(uniforms.grid_height, 38);
    assert_eq!(uniforms.frame_index, 42);
}

#[test]
fn glass_chromatic_scales_packed() {
    let params = RenderParams::default();
    let uniforms = GpuRenderUniforms::from_params(
        &params,
        &Mat4::IDENTITY,
        &Mat4::IDENTITY,
        Vec3::ZERO,
        Vec2::new(1920.0, 1080.0),
        0,
    );
    assert!((uniforms.chromatic_scales[0] - 1.0).abs() < f32::EPSILON);
    assert!((uniforms.chromatic_scales[1] - 1.1).abs() < f32::EPSILON);
    assert!((uniforms.chromatic_scales[2] - 1.2).abs() < f32::EPSILON);
}

#[test]
fn disabled_generation_preserves_max_splats() {
    let mut params = RenderParams::default();
    params.splat.generation_enabled = false;
    let uniforms = GpuRenderUniforms::from_params(
        &params,
        &Mat4::IDENTITY,
        &Mat4::IDENTITY,
        Vec3::ZERO,
        Vec2::new(640.0, 480.0),
        0,
    );
    assert_eq!(uniforms.max_splats, 2_000_000);
}
