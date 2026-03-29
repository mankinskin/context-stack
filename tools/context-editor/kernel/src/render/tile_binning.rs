//! Tile Binning — compute node for T6d Phase 1.
//!
//! Scans sorted keys to find per-tile `(offset, count)` boundaries in the
//! sorted splat array.  Each tile's data is written via atomic operations so
//! the pass is a single compute dispatch.
//!
//! The [`TileBinNode`] clears `tile_data` to zero before dispatching.
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `sorted_keys: array<u32>` |
//! | 1 | `storage<read_write>` | `tile_data: array<atomic<u32>>` |
//! | 2 | `uniform` | `uniforms: TileBinUniforms` |

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor,
            PipelineCache, ShaderStages,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};

use crate::gpu::SplatBuffers;

// ---------------------------------------------------------------------------
// Uniform data (matches WGSL `TileBinUniforms`)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TileBinUniforms {
    num_elements: u32,
    num_tiles: u32,
    _pad0: u32,
    _pad1: u32,
}

/// Byte size of [`TileBinUniforms`].
const UNIFORM_SIZE: u64 = 16;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// GPU uniform buffer for the tile binning shader.
#[derive(Resource)]
pub struct TileBinUniformBuffer(pub Buffer);

/// Cached pipeline ID for `tile_binning.wgsl`.
#[derive(Resource)]
pub struct TileBinPipeline(pub CachedComputePipelineId);

/// Pre-built bind group for the tile binning dispatch.
#[derive(Resource)]
pub struct TileBinBindGroup(pub BindGroup);

// ---------------------------------------------------------------------------
// Resource init
// ---------------------------------------------------------------------------

/// Create the uniform buffer for the tile binning shader.
pub fn init_tile_bin_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<TileBinUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };

    let buf = device.create_buffer(&BufferDescriptor {
        label: Some("tile_bin_uniform"),
        size: UNIFORM_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(TileBinUniformBuffer(buf));
}

/// Write tile-bin uniforms every frame (tiles_x/tiles_y may change on resize).
pub fn update_tile_bin_uniforms(
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<TileBinUniformBuffer>>,
    render_queue: Option<Res<RenderQueue>>,
) {
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(uniform) = uniform else { return };
    let Some(render_queue) = render_queue else { return };

    let u = TileBinUniforms {
        num_elements: splat_buffers.max_splats,
        num_tiles: splat_buffers.tiles_x * splat_buffers.tiles_y,
        _pad0: 0,
        _pad1: 0,
    };
    render_queue.write_buffer(&uniform.0, 0, bytemuck::bytes_of(&u));
}

// ---------------------------------------------------------------------------
// Bind group layout
// ---------------------------------------------------------------------------

/// Bind group layout matching `tile_binning.wgsl` group(0).
pub fn tile_bin_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_tile_binning",
        &[
            // binding 0 — sorted_keys (read-only storage)
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
            // binding 1 — tile_data (read-write storage, atomic)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 2 — uniforms
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
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

/// Queue the tile binning compute pipeline (once).
pub fn queue_tile_bin_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<TileBinPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    let shader = asset_server.load("render/tile_binning.wgsl");
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("tile_binning_pipeline".into()),
        layout: vec![tile_bin_bind_group_layout_descriptor()],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("build_tiles".into()),
        zero_initialize_workgroup_memory: true,
    });
    commands.insert_resource(TileBinPipeline(id));
}

// ---------------------------------------------------------------------------
// Bind group rebuild
// ---------------------------------------------------------------------------

/// Rebuild the tile binning bind group each frame.
pub fn rebuild_tile_bin_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<TileBinUniformBuffer>>,
) {
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(uniform) = uniform else { return };

    let descriptor = tile_bin_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);

    let bg = device.create_bind_group(
        "bg_tile_binning",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: splat_buffers.sort_keys.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: splat_buffers.tile_data.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: uniform.0.as_entire_binding(),
            },
        ],
    );
    commands.insert_resource(TileBinBindGroup(bg));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Compute node that bins sorted splats into per-tile (offset, count) pairs.
///
/// 1. Clears `tile_data` buffer to zero.
/// 2. Dispatches `ceil(max_splats / 256)` workgroups.
/// 3. Output: `tile_data[]` ready for the tiled rasteriser.
#[derive(Default)]
pub struct TileBinNode;

impl Node for TileBinNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<TileBinPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<TileBinBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(splat_buffers) = world.get_resource::<SplatBuffers>() else {
            return Ok(());
        };

        let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline_res.0) else {
            return Ok(());
        };

        let encoder = render_context.command_encoder();

        // Clear tile_data to zero before binning (counts are accumulated atomically)
        encoder.clear_buffer(
            &splat_buffers.tile_data,
            0,
            None, // clear entire buffer
        );

        // Dispatch
        let workgroups = (splat_buffers.max_splats + 255) / 256;
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("tile_binning"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        Ok(())
    }
}
