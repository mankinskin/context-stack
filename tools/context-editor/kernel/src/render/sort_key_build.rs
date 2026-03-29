//! Sort Key Build — compute node for T6b.
//!
//! Projects each [`VoxelSplat`]'s AABB to screen-space, assigns a tile ID,
//! and builds composite sort keys `(tile_id << 12) | depth` for the GPU radix
//! sort (T6c).
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `splats: array<VoxelSplat>` |
//! | 1 | `storage<read_write>` | `projected: array<ProjectedSplat>` |
//! | 2 | `storage<read_write>` | `sort_keys: array<u32>` |
//! | 3 | `storage<read_write>` | `sort_values: array<u32>` |
//! | 4 | `uniform` | `camera: CameraUniforms` |
//! | 5 | `storage<read>` | `splat_count_buf: array<u32>` |

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
use crate::splat::CAMERA_UNIFORMS_SIZE;

// ---------------------------------------------------------------------------
// Resource initialisation (runs once when RenderDevice becomes available)
// ---------------------------------------------------------------------------

/// Initialise the [`SortKeyCameraUniform`] once the `RenderDevice` becomes
/// available.
pub fn init_sort_key_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<SortKeyCameraUniform>>,
) {
    if existing.is_some() {
        return; // already initialised
    }
    let Some(device) = device else { return };
    commands.insert_resource(SortKeyCameraUniform::new(&device));
}

// ---------------------------------------------------------------------------
// SortKeyCameraUniform — GPU uniform buffer for camera data
// ---------------------------------------------------------------------------

/// GPU uniform buffer holding [`CameraUniforms`] data for the sort key shader.
#[derive(Resource)]
pub struct SortKeyCameraUniform(pub Buffer);

impl SortKeyCameraUniform {
    pub fn new(device: &RenderDevice) -> Self {
        Self(device.create_buffer(&BufferDescriptor {
            label: Some("sort_key_camera_uniform"),
            size: CAMERA_UNIFORMS_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }))
    }
}

// ---------------------------------------------------------------------------
// Bind group layout descriptor
// ---------------------------------------------------------------------------

/// Bind group layout descriptor matching `sort_key_build.wgsl` group(0).
///
/// ```wgsl
/// @group(0) @binding(0) var<storage, read>        splats
/// @group(0) @binding(1) var<storage, read_write>  projected
/// @group(0) @binding(2) var<storage, read_write>  sort_keys
/// @group(0) @binding(3) var<storage, read_write>  sort_values
/// @group(0) @binding(4) var<uniform>              camera
/// @group(0) @binding(5) var<storage, read>        splat_count_buf
/// ```
pub fn sort_key_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_sort_key_build",
        &[
            // binding 0: splats (read-only storage)
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
            // binding 1: projected (read-write storage)
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
            // binding 2: sort_keys (read-write storage)
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
            // binding 3: sort_values (read-write storage)
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
            // binding 4: camera uniform
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 5: splat_count (read-only storage)
            BindGroupLayoutEntry {
                binding: 5,
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
// SortKeyBindGroup
// ---------------------------------------------------------------------------

/// Pre-built bind group for the sort key build dispatch.
#[derive(Resource)]
pub struct SortKeyBindGroup(pub BindGroup);

impl SortKeyBindGroup {
    pub fn new(
        device: &RenderDevice,
        layout: &bevy::render::render_resource::BindGroupLayout,
        splat_buffers: &SplatBuffers,
        camera_uniform: &SortKeyCameraUniform,
    ) -> Self {
        Self(device.create_bind_group(
            "bg_sort_key_build",
            layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: splat_buffers.splats.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: splat_buffers.projected.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: splat_buffers.sort_keys.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: splat_buffers.sort_values.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: camera_uniform.0.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: splat_buffers.splat_count.as_entire_binding(),
                },
            ],
        ))
    }
}

// ---------------------------------------------------------------------------
// Cached compute pipeline
// ---------------------------------------------------------------------------

/// Holds the [`CachedComputePipelineId`] for the sort key build shader.
#[derive(Resource)]
pub struct SortKeyPipeline(pub CachedComputePipelineId);

/// System that queues the sort key pipeline for compilation at startup.
pub fn queue_sort_key_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
) {
    let shader = asset_server.load("render/sort_key_build.wgsl");
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("sort_key_build_pipeline".into()),
        layout: vec![sort_key_bind_group_layout_descriptor()],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("build_sort_keys".into()),
        zero_initialize_workgroup_memory: true,
    });
    commands.insert_resource(SortKeyPipeline(id));
}

/// System that rebuilds the sort key bind group each frame because the
/// camera uniform buffer is updated each frame.
pub fn rebuild_sort_key_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    splat_buffers: Res<SplatBuffers>,
    camera_uniform: Res<SortKeyCameraUniform>,
) {
    let descriptor = sort_key_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);
    commands.insert_resource(SortKeyBindGroup::new(
        &device,
        &layout,
        &splat_buffers,
        &camera_uniform,
    ));
}

// ---------------------------------------------------------------------------
// Camera uniform update system (runs in main world Update schedule)
// ---------------------------------------------------------------------------

/// Extracts camera matrices and viewport info into the [`SortKeyCameraUniform`]
/// buffer each frame.
pub fn update_camera_uniforms(
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    windows: Query<&Window>,
    render_queue: Option<Res<RenderQueue>>,
    camera_uniform: Option<Res<SortKeyCameraUniform>>,
) {
    let Some(render_queue) = render_queue else {
        return;
    };
    let Some(camera_uniform) = camera_uniform else {
        return;
    };
    let Ok((transform, projection)) = camera_query.single() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let view_mat = transform.to_matrix().inverse();
    let proj_mat = projection.get_clip_from_view();
    let view_proj = proj_mat * view_mat;

    let camera_pos = transform.translation();
    let resolution = [window.physical_width() as f32, window.physical_height() as f32];

    let uniforms = crate::splat::CameraUniforms {
        view_proj: view_proj.to_cols_array(),
        view_mat: view_mat.to_cols_array(),
        camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
        _pad0: 0.0,
        resolution,
        max_depth: 1000.0,
        _pad1: 0.0,
    };

    render_queue.write_buffer(
        &camera_uniform.0,
        0,
        bytemuck::bytes_of(&uniforms),
    );
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that dispatches the sort key build compute shader.
///
/// 1. Dispatches `ceil(max_splats / 256)` workgroups — the shader itself
///    reads `splat_count` to early-exit threads beyond the actual count.
/// 2. Output: `projected[]`, `sort_keys[]`, `sort_values[]` ready for
///    radix sort (T6c).
#[derive(Default)]
pub struct SortKeyBuildNode;

impl Node for SortKeyBuildNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<SortKeyPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<SortKeyBindGroup>() else {
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

        // Dispatch over max_splats — the shader reads splat_count to skip unused threads
        let workgroups = (splat_buffers.max_splats + 255) / 256;

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("sort_key_build"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        Ok(())
    }
}
