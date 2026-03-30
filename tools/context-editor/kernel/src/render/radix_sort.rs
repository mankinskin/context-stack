//! Radix Sort — GPU 4-bit / 8-pass radix sort for T6c.
//!
//! Sorts ~1 M voxel splats by composite key `(tile_id | depth)` entirely in
//! compute shaders — data never leaves VRAM.
//!
//! Three WGSL entry points per pass:
//! 1. **Histogram** — count digit occurrences per workgroup
//! 2. **Prefix sum** — single-workgroup exclusive scan of global histograms
//! 3. **Scatter** — stable write elements to sorted positions
//!
//! The node dispatches 3 compute passes × 8 digits = 24 compute passes,
//! ping-ponging between `sort_keys`/`sort_scratch_keys` and
//! `sort_values`/`sort_scratch_values` buffers.
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `keys_src: array<u32>` |
//! | 1 | `storage<read>` | `vals_src: array<u32>` |
//! | 2 | `storage<read_write>` | `keys_dst: array<u32>` |
//! | 3 | `storage<read_write>` | `vals_dst: array<u32>` |
//! | 4 | `storage<read_write>` | `histograms: array<u32>` |
//! | 5 | `uniform` | `uniforms: RadixUniforms` |

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

use crate::gpu::{SplatBuffers, MAX_GAUSSIANS};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of radix sort passes (32-bit key / 4 bits per pass).
const NUM_PASSES: u32 = 8;

/// Byte size of one `RadixSortUniforms` instance (4 × u32).
const UNIFORM_SIZE: u64 = 16;

/// Staging buffer: one `RadixSortUniforms` per pass.
const STAGING_SIZE: u64 = (NUM_PASSES as u64) * UNIFORM_SIZE;

// ---------------------------------------------------------------------------
// Uniform data (matches WGSL `RadixUniforms`)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RadixSortUniforms {
    bit_shift: u32,
    num_elements: u32,
    num_workgroups_sort: u32,
    _pad: u32,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// GPU uniform buffer (16 bytes) updated inline via `copy_buffer_to_buffer`
/// before each histogram dispatch.
#[derive(Resource, Clone)]
pub struct RadixSortUniformBuffer(pub Buffer);

impl ExtractResource for RadixSortUniformBuffer {
    type Source = RadixSortUniformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Pre-filled staging buffer holding all 8 parameter sets (128 bytes).
/// Copied into [`RadixSortUniformBuffer`] per-pass inside the render node.
#[derive(Resource, Clone)]
pub struct RadixSortStagingBuffer(pub Buffer);

impl ExtractResource for RadixSortStagingBuffer {
    type Source = RadixSortStagingBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Cached pipeline IDs for the three compute entry points.
#[derive(Resource)]
pub struct RadixSortPipelines {
    pub histogram: CachedComputePipelineId,
    pub prefix_sum: CachedComputePipelineId,
    pub scatter: CachedComputePipelineId,
}

/// Bind group for **even** passes: reads `sort_keys` → writes `sort_scratch_keys`.
#[derive(Resource)]
pub struct RadixSortForwardBindGroup(pub BindGroup);

/// Bind group for **odd** passes: reads `sort_scratch_keys` → writes `sort_keys`.
#[derive(Resource)]
pub struct RadixSortReverseBindGroup(pub BindGroup);

// ---------------------------------------------------------------------------
// Resource init (runs once when RenderDevice is available)
// ---------------------------------------------------------------------------

/// Create the staging and uniform buffers for the radix sort.
///
/// The staging buffer is pre-filled with all 8 parameter sets so the render
/// node only needs a 16-byte `copy_buffer_to_buffer` per pass.
pub fn init_radix_sort_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
    existing: Option<Res<RadixSortUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let Some(render_queue) = render_queue else { return };

    let max_splats = MAX_GAUSSIANS;
    let num_wg = (max_splats + 255) / 256;

    // Build all 8 parameter sets
    let staging_data: Vec<u8> = (0..NUM_PASSES)
        .flat_map(|pass| {
            let u = RadixSortUniforms {
                bit_shift: pass * 4,
                num_elements: max_splats,
                num_workgroups_sort: num_wg,
                _pad: 0,
            };
            bytemuck::bytes_of(&u).to_vec()
        })
        .collect();

    let staging = device.create_buffer(&BufferDescriptor {
        label: Some("radix_sort_staging"),
        size: STAGING_SIZE,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_queue.write_buffer(&staging, 0, &staging_data);

    let uniform = device.create_buffer(&BufferDescriptor {
        label: Some("radix_sort_uniform"),
        size: UNIFORM_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    commands.insert_resource(RadixSortUniformBuffer(uniform));
    commands.insert_resource(RadixSortStagingBuffer(staging));
}

// ---------------------------------------------------------------------------
// Bind group layout descriptor
// ---------------------------------------------------------------------------

/// Bind group layout matching `radix_sort.wgsl` group(0).
pub fn radix_sort_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_radix_sort",
        &[
            // binding 0 — keys_src (read-only storage)
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
            // binding 1 — vals_src (read-only storage)
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
            // binding 2 — keys_dst (read-write storage)
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
            // binding 3 — vals_dst (read-write storage)
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
            // binding 4 — histograms (read-write storage)
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
            // binding 5 — uniforms
            BindGroupLayoutEntry {
                binding: 5,
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

/// Queue the three radix sort compute pipelines (once).
pub fn queue_radix_sort_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<RadixSortPipelines>>,
) {
    if existing.is_some() {
        return;
    }

    let shader = asset_server.load("embedded://context_editor_kernel/render/radix_sort.wgsl");
    let layout = vec![radix_sort_bind_group_layout_descriptor()];

    let histogram = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("radix_sort_histogram".into()),
        layout: layout.clone(),
        push_constant_ranges: vec![],
        shader: shader.clone(),
        shader_defs: vec![],
        entry_point: Some("radix_histogram".into()),
        zero_initialize_workgroup_memory: true,
    });

    let prefix_sum = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("radix_sort_prefix_sum".into()),
        layout: layout.clone(),
        push_constant_ranges: vec![],
        shader: shader.clone(),
        shader_defs: vec![],
        entry_point: Some("radix_prefix_sum".into()),
        zero_initialize_workgroup_memory: true,
    });

    let scatter = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("radix_sort_scatter".into()),
        layout,
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("radix_scatter".into()),
        zero_initialize_workgroup_memory: true,
    });

    commands.insert_resource(RadixSortPipelines {
        histogram,
        prefix_sum,
        scatter,
    });
}

// ---------------------------------------------------------------------------
// Bind group rebuild (per-frame — SplatBuffers may change)
// ---------------------------------------------------------------------------

/// Rebuild the forward and reverse bind groups.
pub fn rebuild_radix_sort_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<RadixSortUniformBuffer>>,
) {
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(uniform) = uniform else { return };

    let descriptor = radix_sort_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);

    // Forward (even passes): sort_keys → sort_scratch_keys
    let forward = device.create_bind_group(
        "bg_radix_sort_forward",
        &layout,
        &[
            BindGroupEntry { binding: 0, resource: splat_buffers.sort_keys.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: splat_buffers.sort_values.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: splat_buffers.sort_scratch_keys.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: splat_buffers.sort_scratch_values.as_entire_binding() },
            BindGroupEntry { binding: 4, resource: splat_buffers.histograms.as_entire_binding() },
            BindGroupEntry { binding: 5, resource: uniform.0.as_entire_binding() },
        ],
    );

    // Reverse (odd passes): sort_scratch_keys → sort_keys
    let reverse = device.create_bind_group(
        "bg_radix_sort_reverse",
        &layout,
        &[
            BindGroupEntry { binding: 0, resource: splat_buffers.sort_scratch_keys.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: splat_buffers.sort_scratch_values.as_entire_binding() },
            BindGroupEntry { binding: 2, resource: splat_buffers.sort_keys.as_entire_binding() },
            BindGroupEntry { binding: 3, resource: splat_buffers.sort_values.as_entire_binding() },
            BindGroupEntry { binding: 4, resource: splat_buffers.histograms.as_entire_binding() },
            BindGroupEntry { binding: 5, resource: uniform.0.as_entire_binding() },
        ],
    );

    commands.insert_resource(RadixSortForwardBindGroup(forward));
    commands.insert_resource(RadixSortReverseBindGroup(reverse));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that dispatches the 8-pass, 4-bit GPU radix sort.
///
/// For each pass the node issues three compute dispatches (in separate passes
/// for implicit storage-buffer barriers):
/// 1. **Histogram** — `ceil(max_splats / 256)` workgroups
/// 2. **Prefix sum** — 1 workgroup
/// 3. **Scatter** — `ceil(max_splats / 256)` workgroups
///
/// Even passes use the forward bind group (keys → scratch), odd passes use
/// reverse (scratch → keys). After all 8 passes the final sorted data
/// resides in `sort_keys` / `sort_values`.
#[derive(Default)]
pub struct RadixSortNode;

impl Node for RadixSortNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Bail silently if any resource or pipeline is not ready.
        let Some(pipelines) = world.get_resource::<RadixSortPipelines>() else {
            return Ok(());
        };
        let Some(forward_bg) = world.get_resource::<RadixSortForwardBindGroup>() else {
            return Ok(());
        };
        let Some(reverse_bg) = world.get_resource::<RadixSortReverseBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(staging) = world.get_resource::<RadixSortStagingBuffer>() else {
            return Ok(());
        };
        let Some(uniform) = world.get_resource::<RadixSortUniformBuffer>() else {
            return Ok(());
        };
        let Some(splat_buffers) = world.get_resource::<SplatBuffers>() else {
            return Ok(());
        };

        let Some(histogram_pipeline) =
            pipeline_cache.get_compute_pipeline(pipelines.histogram)
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

        let num_wg = (splat_buffers.max_splats + 255) / 256;
        let encoder = render_context.command_encoder();

        for pass in 0..NUM_PASSES {
            let bind_group = if pass % 2 == 0 {
                &forward_bg.0
            } else {
                &reverse_bg.0
            };

            // Copy this pass's uniforms from the pre-filled staging buffer
            encoder.copy_buffer_to_buffer(
                &staging.0,
                (pass as u64) * UNIFORM_SIZE,
                &uniform.0,
                0,
                UNIFORM_SIZE,
            );

            // --- Histogram ---
            {
                let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("radix_histogram"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(histogram_pipeline);
                cpass.set_bind_group(0, bind_group, &[]);
                cpass.dispatch_workgroups(num_wg, 1, 1);
            }

            // --- Prefix sum ---
            {
                let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("radix_prefix_sum"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(prefix_sum_pipeline);
                cpass.set_bind_group(0, bind_group, &[]);
                cpass.dispatch_workgroups(1, 1, 1);
            }

            // --- Scatter ---
            {
                let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("radix_scatter"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(scatter_pipeline);
                cpass.set_bind_group(0, bind_group, &[]);
                cpass.dispatch_workgroups(num_wg, 1, 1);
            }
        }

        Ok(())
    }
}
