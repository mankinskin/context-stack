//! Voxel Splat Kernel — compute node for T6a.
//!
//! Dispatches `voxel_splat_kernel.wgsl` to convert SVO leaf nodes into
//! [`VoxelSplat`]s. The atomic `splat_count` buffer tracks how many splats
//! were emitted so downstream stages know the working set size.

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, BufferBindingType, BufferDescriptor, BufferUsages,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor,
            PipelineCache, ShaderStages, Buffer,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};

use crate::gpu::{SvoDoubleBuffer, SplatBuffers};
use crate::splat::SPLAT_PARAMS_SIZE;

// ---------------------------------------------------------------------------
// SplatParamsUniform — Bevy resource holding the GPU uniform buffer
// ---------------------------------------------------------------------------

/// GPU uniform buffer for [`SplatParams`], updated each frame from camera +
/// SVO state by [`update_splat_params_system`].
#[derive(Resource)]
pub struct SplatParamsUniform(pub Buffer);

impl SplatParamsUniform {
    pub fn new(device: &RenderDevice) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("splat_params_uniform"),
            size: SPLAT_PARAMS_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

// ---------------------------------------------------------------------------
// VoxelSplatBindGroup — the kernel's own bind group (group 0 in the shader)
// ---------------------------------------------------------------------------

/// Bind group layout descriptor matching `voxel_splat_kernel.wgsl` group(0).
///
/// ```wgsl
/// @group(0) @binding(0) var<storage, read>       octree
/// @group(0) @binding(1) var<storage, read_write>  splats
/// @group(0) @binding(2) var<storage, read_write>  splat_count
/// @group(0) @binding(3) var<uniform>              params
/// ```
pub fn splat_kernel_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_voxel_splat_kernel",
        &[
            // binding 0: octree (read-only storage)
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
            // binding 1: splats (read-write storage)
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
            // binding 2: splat_count (read-write storage, atomic)
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
            // binding 3: SplatParams uniform
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
        ],
    )
}

/// Pre-built bind group for the voxel splat kernel dispatch.
#[derive(Resource)]
pub struct VoxelSplatBindGroup(pub BindGroup);

impl VoxelSplatBindGroup {
    pub fn new(
        device: &RenderDevice,
        layout: &bevy::render::render_resource::BindGroupLayout,
        svo: &SvoDoubleBuffer,
        splat_buffers: &SplatBuffers,
        params: &SplatParamsUniform,
    ) -> Self {
        Self(device.create_bind_group(
            "bg_voxel_splat_kernel",
            layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: svo.read_source().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: splat_buffers.splats.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: splat_buffers.splat_count.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: params.0.as_entire_binding(),
                },
            ],
        ))
    }
}

// ---------------------------------------------------------------------------
// Cached compute pipeline
// ---------------------------------------------------------------------------

/// Holds the [`CachedComputePipelineId`] for the voxel splat kernel shader.
#[derive(Resource)]
pub struct VoxelSplatPipeline(pub CachedComputePipelineId);

/// System that queues the splat kernel pipeline for compilation.
pub fn queue_voxel_splat_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
) {
    let shader = asset_server.load("render/voxel_splat_kernel.wgsl");
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("voxel_splat_kernel_pipeline".into()),
        layout: vec![splat_kernel_bind_group_layout_descriptor()],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("generate_splats".into()),
        zero_initialize_workgroup_memory: true,
    });
    commands.insert_resource(VoxelSplatPipeline(id));
}

// ---------------------------------------------------------------------------
// Rebuild bind group each frame (SVO double-buffer may have swapped)
// ---------------------------------------------------------------------------

/// System that rebuilds the splat kernel bind group each frame because the
/// SVO double-buffer swap changes which buffer is the read source.
pub fn rebuild_splat_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    svo: Res<SvoDoubleBuffer>,
    splat_buffers: Res<SplatBuffers>,
    params: Res<SplatParamsUniform>,
) {
    let descriptor = splat_kernel_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);
    commands.insert_resource(VoxelSplatBindGroup::new(
        &device,
        &layout,
        &svo,
        &splat_buffers,
        &params,
    ));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that dispatches the voxel splat kernel compute shader.
///
/// 1. Clears `splat_count` to 0 via a zero-fill write.
/// 2. Dispatches `ceil(total_nodes / 256)` workgroups.
/// 3. Output: `splats[]` + `splat_count` ready for sort key build (T6b).
#[derive(Default)]
pub struct VoxelSplatKernelNode;

impl Node for VoxelSplatKernelNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // Retrieve resources; bail silently if pipeline not ready yet.
        let Some(pipeline_res) = world.get_resource::<VoxelSplatPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<VoxelSplatBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(splat_buffers) = world.get_resource::<SplatBuffers>() else {
            return Ok(());
        };
        let Some(svo) = world.get_resource::<SvoDoubleBuffer>() else {
            return Ok(());
        };

        // Wait for pipeline compilation to finish.
        let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline_res.0) else {
            return Ok(());
        };

        let encoder = render_context.command_encoder();

        // Reset splat_count atomic to 0
        let zero = [0u8; 4];
        let render_queue = world.get_resource::<RenderQueue>().unwrap();
        render_queue.write_buffer(&splat_buffers.splat_count, 0, &zero);

        // Dispatch compute
        let total_nodes = svo.capacity_nodes as u32;
        let workgroups = (total_nodes + 255) / 256;

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("voxel_splat_kernel"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        Ok(())
    }
}
