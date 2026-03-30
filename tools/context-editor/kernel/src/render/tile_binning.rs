//! Active-List Tile Binning — compute node for T6d Phase 2.
//!
//! Three-pass atomics + prefix-sum tile binning that replaces the linear
//! boundary scan.  Every entry in a tile's active list genuinely overlaps
//! that tile's screen region (no per-pixel AABB rejection needed at the
//! tile level).
//!
//! ## Passes
//!
//! 1. **count_tile_overlaps** — count how many sorted splats overlap each tile
//! 2. **prefix_sum_and_pack** — exclusive scan → offsets, pack tile_data, init write heads
//! 3. **scatter_to_tiles** — write projected-buffer indices into active_list
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `sorted_values: array<u32>` |
//! | 1 | `storage<read>` | `projected: array<ProjectedSplat>` |
//! | 2 | `storage<read_write>` | `tile_counts: array<atomic<u32>>` |
//! | 3 | `storage<read_write>` | `tile_write_heads: array<atomic<u32>>` |
//! | 4 | `storage<read_write>` | `tile_data: array<u32>` |
//! | 5 | `storage<read_write>` | `active_list: array<u32>` |
//! | 6 | `uniform` | `uniforms: TileBinUniforms` |
//! | 7 | `storage<read>` | `splat_count_buf: array<u32>` |

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
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
    grid_width: u32,
    max_active: u32,
}

/// Byte size of [`TileBinUniforms`].
const UNIFORM_SIZE: u64 = 16;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// GPU uniform buffer for the tile binning shader.
#[derive(Resource, Clone)]
pub struct TileBinUniformBuffer(pub Buffer);

impl ExtractResource for TileBinUniformBuffer {
    type Source = TileBinUniformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Cached pipeline IDs for the three-pass tile binning shader.
#[derive(Resource)]
pub struct TileBinPipelines {
    pub count: CachedComputePipelineId,
    pub prefix_sum: CachedComputePipelineId,
    pub scatter: CachedComputePipelineId,
}

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
        grid_width: splat_buffers.tiles_x,
        max_active: crate::gpu::MAX_ACTIVE_ENTRIES,
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
            // binding 0 — sorted_values (read-only storage)
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
            // binding 1 — projected (read-only storage)
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
            // binding 2 — tile_counts (read-write storage, atomic)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 3 — tile_write_heads (read-write storage, atomic)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 4 — tile_data (read-write storage)
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
            // binding 5 — active_list (read-write storage)
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 6 — uniforms
            BindGroupLayoutEntry {
                binding: 6,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 7 — splat_count_buf (read-only storage)
            BindGroupLayoutEntry {
                binding: 7,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
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

/// Queue the three tile binning compute pipelines (once).
pub fn queue_tile_bin_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<TileBinPipelines>>,
) {
    if existing.is_some() {
        return;
    }

    let shader = asset_server.load("embedded://context_editor_kernel/render/tile_binning.wgsl");
    let layout = vec![tile_bin_bind_group_layout_descriptor()];

    let count = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("tile_bin_count_pipeline".into()),
        layout: layout.clone(),
        push_constant_ranges: vec![],
        shader: shader.clone(),
        shader_defs: vec![],
        entry_point: Some("count_tile_overlaps".into()),
        zero_initialize_workgroup_memory: false,
    });
    let prefix_sum = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("tile_bin_prefix_sum_pipeline".into()),
        layout: layout.clone(),
        push_constant_ranges: vec![],
        shader: shader.clone(),
        shader_defs: vec![],
        entry_point: Some("prefix_sum_and_pack".into()),
        zero_initialize_workgroup_memory: false,
    });
    let scatter = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("tile_bin_scatter_pipeline".into()),
        layout,
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("scatter_to_tiles".into()),
        zero_initialize_workgroup_memory: false,
    });
    commands.insert_resource(TileBinPipelines { count, prefix_sum, scatter });
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
                resource: splat_buffers.sort_values.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: splat_buffers.projected.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: splat_buffers.tile_counts.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: splat_buffers.tile_write_heads.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: splat_buffers.tile_data.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 5,
                resource: splat_buffers.active_list.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 6,
                resource: uniform.0.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 7,
                resource: splat_buffers.splat_count.as_entire_binding(),
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
        let Some(pipelines) = world.get_resource::<TileBinPipelines>() else {
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

        let Some(count_pipeline) =
            pipeline_cache.get_compute_pipeline(pipelines.count)
        else {
            return Ok(());
        };
        let Some(prefix_sum_pipeline) =
            pipeline_cache.get_compute_pipeline(pipelines.prefix_sum)
        else {
            return Ok(());
        };
        let Some(scatter_pipeline) =
            pipeline_cache.get_compute_pipeline(pipelines.scatter)
        else {
            return Ok(());
        };

        let encoder = render_context.command_encoder();

        // Clear tile_counts to zero before the counting pass.
        encoder.clear_buffer(&splat_buffers.tile_counts, 0, None);

        let splat_workgroups = (splat_buffers.max_splats + 255) / 256;

        // Pass 1 – count per-tile overlaps
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("tile_bin_count"),
                timestamp_writes: None,
            });
            pass.set_pipeline(count_pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(splat_workgroups, 1, 1);
        }

        // Pass 2 – prefix sum, pack tile_data, init write heads (single workgroup)
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("tile_bin_prefix_sum"),
                timestamp_writes: None,
            });
            pass.set_pipeline(prefix_sum_pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        // Pass 3 – scatter splat indices into active_list
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("tile_bin_scatter"),
                timestamp_writes: None,
            });
            pass.set_pipeline(scatter_pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(splat_workgroups, 1, 1);
        }

        Ok(())
    }
}
