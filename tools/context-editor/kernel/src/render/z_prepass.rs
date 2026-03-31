//! Z-Prepass — compute node for Phase 5B (ticket 283c2bc7).
//!
//! Runs two compute dispatches per frame before `TiledRasterNode`:
//!
//! 1. **Clear** (`clear_depth` entry point, `@workgroup_size(256)`) — fills
//!    the `depth_prepass` storage buffer with `f32::INFINITY`.
//! 2. **Prepass** (`z_prepass_main`, `@workgroup_size(8,8)`) — for each pixel
//!    scans the first ≤8 front-to-back splats in its tile and writes the
//!    view-space depth of the first opaque box hit.
//!
//! The fragment shader in `tiled_raster.wgsl` then reads `depth_prepass` to
//! skip any splat whose depth exceeds the prepass result, eliminating
//! z-fighting between adjacent terrain voxels.
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `active_list: array<u32>` |
//! | 1 | `storage<read>` | `projected: array<ProjectedSplat>` |
//! | 2 | `storage<read>` | `tile_data: array<u32>` (offset/count pairs) |
//! | 3 | `uniform` | `uniforms: RasterUniforms` |
//! | 4 | `storage<read_write>` | `depth_prepass: array<f32>` |

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, BufferBindingType, CachedComputePipelineId, ComputePassDescriptor,
            ComputePipelineDescriptor, PipelineCache, ShaderStages,
        },
        renderer::{RenderContext, RenderDevice},
    },
};

use crate::gpu::{SplatBuffers, TILE_SIZE};
use super::tiled_raster::RasterUniformBuffer;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Cached IDs for the two compute pipelines (clear + prepass).
#[derive(Resource)]
pub struct ZPrepassPipelines {
    pub clear:   CachedComputePipelineId,
    pub prepass: CachedComputePipelineId,
}

/// Combined bind group used by both clear and prepass dispatches.
#[derive(Resource)]
pub struct ZPrepassBindGroup(pub BindGroup);

// ---------------------------------------------------------------------------
// Bind group layout
// ---------------------------------------------------------------------------

/// Bind group layout matching `z_prepass.wgsl` group(0).
pub fn z_prepass_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_z_prepass",
        &[
            // 0 — active_list
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 1 — projected
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 2 — tile_data
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 3 — uniforms (shared RasterUniforms)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 4 — depth_prepass (read_write — cleared by clear pass, written by prepass)
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    )
}

// ---------------------------------------------------------------------------
// Pipeline queueing
// ---------------------------------------------------------------------------

/// Queue both compute pipelines from `z_prepass.wgsl` (runs once).
pub fn queue_z_prepass_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<ZPrepassPipelines>>,
) {
    if existing.is_some() {
        return;
    }

    let shader = asset_server.load("embedded://context_editor_kernel/render/z_prepass.wgsl");
    let layout = vec![z_prepass_bind_group_layout_descriptor()];

    let clear = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("z_prepass_clear".into()),
        layout: layout.clone(),
        push_constant_ranges: vec![],
        shader: shader.clone(),
        shader_defs: vec![],
        entry_point: Some("clear_depth".into()),
        zero_initialize_workgroup_memory: false,
    });

    let prepass = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("z_prepass_main".into()),
        layout,
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("z_prepass_main".into()),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(ZPrepassPipelines { clear, prepass });
}

// ---------------------------------------------------------------------------
// Bind group rebuild
// ---------------------------------------------------------------------------

/// Rebuild the z-prepass bind group each frame (runs in `RenderSystems::Queue`).
pub fn rebuild_z_prepass_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<RasterUniformBuffer>>,
) {
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(uniform) = uniform else { return };

    let descriptor = z_prepass_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);

    let bg = device.create_bind_group(
        "bg_z_prepass",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: splat_buffers.active_list.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: splat_buffers.projected.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: splat_buffers.tile_data.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: uniform.0.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: splat_buffers.depth_prepass.as_entire_binding(),
            },
        ],
    );
    commands.insert_resource(ZPrepassBindGroup(bg));
}

// ---------------------------------------------------------------------------
// Render graph node
// ---------------------------------------------------------------------------

/// Compute node: clear `depth_prepass`, then populate it with per-pixel depths.
///
/// Must run **after** `TileBinNode` (tile_data available) and **before**
/// `TiledRasterNode` (fragment shader reads depth_prepass).
#[derive(Default)]
pub struct ZPrepassNode;

impl Node for ZPrepassNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipelines) = world.get_resource::<ZPrepassPipelines>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<ZPrepassBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(splat_buffers) = world.get_resource::<SplatBuffers>() else {
            return Ok(());
        };
        let Some(clear_pipeline) = pipeline_cache.get_compute_pipeline(pipelines.clear) else {
            return Ok(());
        };
        let Some(prepass_pipeline) = pipeline_cache.get_compute_pipeline(pipelines.prepass) else {
            return Ok(());
        };

        // Tile-rounded viewport dimensions (the shader bounds-checks internally).
        let width  = splat_buffers.tiles_x * TILE_SIZE;
        let height = splat_buffers.tiles_y * TILE_SIZE;

        // --- Clear pass: fill depth_prepass with f32 +infinity ---
        let total_pixels    = width * height;
        let clear_dispatch  = total_pixels.div_ceil(256);
        {
            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("z_prepass_clear"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(clear_pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(clear_dispatch, 1, 1);
        }

        // --- Prepass: one thread per pixel, 8×8 workgroup ---
        let wg_x = width.div_ceil(8);
        let wg_y = height.div_ceil(8);
        {
            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("z_prepass_main"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(prepass_pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        Ok(())
    }
}
