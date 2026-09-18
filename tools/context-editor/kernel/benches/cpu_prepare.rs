//! Criterion benchmarks for the kernel's deterministic CPU-side preparation
//! paths that feed GPU upload, paged/splat upload, and ray-march uniforms.
//!
//! # Scope and limitation (documented, not acted on)
//!
//! wgpu compute/render dispatch (the actual SVO ray-march compute pass,
//! splat sort/rasterise passes, and force-compute step) requires a live
//! `RenderDevice`/`RenderQueue` wired through Bevy's render sub-app and a
//! window/surface. That is not portable to a headless Criterion harness, so
//! these benchmarks measure only the deterministic CPU boundary that
//! prepares data for those GPU stages:
//!
//! - `svo_dirty_range_processing` — [`VoxelWorld::take_dirty_ranges`], the
//!   per-frame CPU step gating `svo_paged_upload_system` (see
//!   `svo::upload`).
//! - `svo_paged_upload_prepare` — [`PageManager::build`], which packs the
//!   CPU SVO into the GPU node buffer + page table consumed by both the
//!   ray-march compute pass and (indirectly, via the shared front buffer)
//!   splat generation. No pure CPU-only splat-generation function exists
//!   independent of Bevy ECS systems (`simulate_particles` takes
//!   `Res<Time>`/`ResMut<ParticleSystem>` and is not `pub`), so this
//!   benchmark stands in for "splat preparation" per the waypoint's
//!   documented-limitation allowance.
//! - `ray_march_uniform_prepare` — [`GpuRenderUniforms::from_params`], the
//!   per-frame CPU uniform pack consumed by the ray-march compute shader.
//!
//! No GPU timing is measured or invented anywhere in this file.

use bevy::math::{
    IVec3,
    Mat4,
    Vec2,
    Vec3,
};
use context_editor_kernel::render::runtime_params::{
    GpuRenderUniforms,
    RenderParams,
};
use context_editor_kernel::svo::paging::PageManager;
use context_editor_kernel::svo::{
    VoxelMaterial,
    VoxelWorld,
};
use criterion::{
    black_box,
    criterion_group,
    criterion_main,
    BatchSize,
    Criterion,
};

/// Octree depth used by all scenarios: small enough to keep iterations fast,
/// deep enough to exercise multi-level descend/allocate.
const MAX_DEPTH: u32 = 7;

/// Cube edge length (voxels) filled per scenario: 12^3 = 1,728 voxels.
const FILL_EDGE: i32 = 12;

fn build_filled_world(edge: i32) -> VoxelWorld {
    let mut world = VoxelWorld::new(MAX_DEPTH);
    let material = VoxelMaterial::new(120, 90, 60, 16);
    for x in 0..edge {
        for y in 0..edge {
            for z in 0..edge {
                world.set_voxel(IVec3::new(x, y, z), material);
            }
        }
    }
    world
}

fn bench_svo_dirty_range_processing(c: &mut Criterion) {
    c.bench_function("svo_dirty_range_processing", |b| {
        b.iter_batched(
            || build_filled_world(FILL_EDGE),
            |mut world| {
                black_box(world.take_dirty_ranges());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_svo_paged_upload_prepare(c: &mut Criterion) {
    let world = build_filled_world(FILL_EDGE);
    let mut manager = PageManager::new(4);
    c.bench_function("svo_paged_upload_prepare", |b| {
        b.iter(|| {
            let (nodes, table) = manager.build(&world, None);
            black_box((nodes, table));
        });
    });
}

fn bench_ray_march_uniform_prepare(c: &mut Criterion) {
    let params = RenderParams::default();
    let view = Mat4::IDENTITY;
    let proj = Mat4::IDENTITY;
    c.bench_function("ray_march_uniform_prepare", |b| {
        b.iter(|| {
            black_box(GpuRenderUniforms::from_params(
                &params,
                &view,
                &proj,
                Vec3::new(0.0, 4.0, 0.0),
                Vec2::new(1280.0, 720.0),
                0,
            ));
        });
    });
}

criterion_group!(
    benches,
    bench_svo_dirty_range_processing,
    bench_svo_paged_upload_prepare,
    bench_ray_march_uniform_prepare
);
criterion_main!(benches);
