//! Voxel Splat Kernel — compute node for T6a.
//!
//! Dispatches `voxel_splat_kernel.wgsl` to convert SVO leaf nodes into
//! [`VoxelSplat`]s. The atomic `splat_count` buffer tracks how many splats
//! were emitted so downstream stages know the working set size.

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
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

use crate::gpu::{SvoDoubleBuffer, SplatBuffers, SVO_CAPACITY_NODES};
use crate::splat::{SplatParams, SPLAT_PARAMS_SIZE};
use crate::particle_splat::ParticleSplatBuffer;

// ---------------------------------------------------------------------------
// NodePositionBuffer — precomputed world-space positions for each SVO node
// ---------------------------------------------------------------------------

/// GPU storage buffer holding `vec4<f32>` per node (xyz = center, w = half_extent).
///
/// Positions are computed on the CPU by [`crate::svo::VoxelWorld::compute_node_positions`]
/// because WGSL does not support the recursive tree traversal needed to derive
/// them on the GPU.
#[derive(Resource, Clone)]
pub struct NodePositionBuffer(pub Buffer);

impl ExtractResource for NodePositionBuffer {
    type Source = NodePositionBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl NodePositionBuffer {
    pub fn new(device: &RenderDevice, capacity_nodes: usize) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("node_positions"),
            size: (capacity_nodes as u64) * 16, // vec4<f32> = 16 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

/// One-shot system: create [`NodePositionBuffer`] once `RenderDevice` is ready.
pub fn init_node_positions(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<NodePositionBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    commands.insert_resource(NodePositionBuffer::new(&device, SVO_CAPACITY_NODES));
}

/// Per-frame system: recompute positions from VoxelWorld and upload to GPU.
pub fn update_node_positions(
    render_queue: Option<Res<RenderQueue>>,
    pos_buf: Option<Res<NodePositionBuffer>>,
    voxel_world: Option<Res<crate::svo::VoxelWorld>>,
) {
    let Some(render_queue) = render_queue else { return };
    let Some(pos_buf) = pos_buf else { return };
    let Some(voxel_world) = voxel_world else { return };

    let positions = voxel_world.compute_node_positions();
    let bytes: &[u8] = bytemuck::cast_slice(&positions);
    render_queue.write_buffer(&pos_buf.0, 0, bytes);
}

// ---------------------------------------------------------------------------
// SplatParamsUniform — Bevy resource holding the GPU uniform buffer
// ---------------------------------------------------------------------------

/// GPU uniform buffer for [`SplatParams`], updated each frame from camera +
/// SVO state by [`update_splat_params_system`].
#[derive(Resource, Clone)]
pub struct SplatParamsUniform(pub Buffer);

impl ExtractResource for SplatParamsUniform {
    type Source = SplatParamsUniform;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

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
// Init / update systems for SplatParamsUniform
// ---------------------------------------------------------------------------

/// One-shot system: create `SplatParamsUniform` once `RenderDevice` is ready.
pub fn init_splat_params(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SplatParamsUniform>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    commands.insert_resource(SplatParamsUniform::new(&device));
}

/// Per-frame system: write camera + SVO state into the uniform buffer.
pub fn update_splat_params(
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    render_queue: Option<Res<RenderQueue>>,
    params_buf: Option<Res<SplatParamsUniform>>,
    svo: Option<Res<SvoDoubleBuffer>>,
    voxel_world: Option<Res<crate::svo::VoxelWorld>>,
) {
    let Some(render_queue) = render_queue else { return };
    let Some(params_buf) = params_buf else { return };
    let Some(svo) = svo else { return };
    let Some(voxel_world) = voxel_world else { return };
    let Ok(cam_tf) = camera_query.single() else { return };

    let cam_pos = cam_tf.translation();
    let params = SplatParams {
        camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z],
        total_nodes: svo.capacity_nodes as u32,
        lod_scale: 0.0002, // cull distance ≈ half_extent/0.0002 = 2500 for depth-10 leaves
        max_depth: voxel_world.max_depth,
        world_size: (1u32 << voxel_world.max_depth) as f32,
        _pad: 0.0,
    };
    render_queue.write_buffer(&params_buf.0, 0, bytemuck::bytes_of(&params));
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
            // binding 4: node_positions (read-only storage, vec4<f32> per node)
            BindGroupLayoutEntry {
                binding: 4,
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
        positions: &NodePositionBuffer,
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
                BindGroupEntry {
                    binding: 4,
                    resource: positions.0.as_entire_binding(),
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
    existing: Option<Res<VoxelSplatPipeline>>,
) {
    if existing.is_some() {
        return;
    }
    let shader = asset_server.load("embedded://context_editor_kernel/render/voxel_splat_kernel.wgsl");
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
    svo: Option<Res<SvoDoubleBuffer>>,
    splat_buffers: Option<Res<SplatBuffers>>,
    params: Option<Res<SplatParamsUniform>>,
    positions: Option<Res<NodePositionBuffer>>,
) {
    let Some(svo) = svo else { return };
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(params) = params else { return };
    let Some(positions) = positions else { return };

    let descriptor = splat_kernel_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);
    commands.insert_resource(VoxelSplatBindGroup::new(
        &device,
        &layout,
        &svo,
        &splat_buffers,
        &params,
        &positions,
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

        // Seed splat_count = particle_count so the SVO kernel's atomicAdd calls
        // append voxel splats AFTER the particle region splats[0..particle_count).
        // When there are no particles this is equivalent to resetting to 0.
        let particle_count = world
            .get_resource::<ParticleSplatBuffer>()
            .map(|p| p.count)
            .unwrap_or(0);
        let init_bytes = particle_count.to_le_bytes();
        let render_queue = world.get_resource::<RenderQueue>().unwrap();
        render_queue.write_buffer(&splat_buffers.splat_count, 0, &init_bytes);

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
