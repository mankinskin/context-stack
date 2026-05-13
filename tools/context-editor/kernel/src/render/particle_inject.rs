//! Particle injection render node — dispatches `particle_inject.wgsl` to
//! emit dynamic particles as [`VoxelSplat`] entries into the shared splat buffer.
//!
//! # Slot allocation
//!
//! Particles occupy `splats[0..particle_count)` directly (per-thread index).
//! After the dispatch, [`crate::render::voxel_splat_kernel::VoxelSplatKernelNode`]
//! seeds `splat_count = particle_count` so its `atomicAdd` calls append voxel
//! splats immediately after the particle region.

use bevy::{
    prelude::*,
    render::{
        render_graph::{
            Node,
            NodeRunError,
            RenderGraphContext,
        },
        render_resource::{
            BindGroup,
            BindGroupLayoutDescriptor,
            BindGroupLayoutEntry,
            BindingType,
            BufferBindingType,
            CachedComputePipelineId,
            ComputePassDescriptor,
            ComputePipelineDescriptor,
            PipelineCache,
            ShaderStages,
        },
        renderer::{
            RenderContext,
            RenderDevice,
        },
    },
};

use crate::particle_splat::ParticleSplatBuffer;

// ---------------------------------------------------------------------------
// Bind group layout
// ---------------------------------------------------------------------------

/// Layout descriptor matching `particle_inject.wgsl` group(0):
/// ```wgsl
/// @binding(0) var<storage, read>       particles
/// @binding(1) var<storage, read_write> splats
/// @binding(2) var<uniform>             uniforms
/// ```
pub fn particle_inject_bind_group_layout_descriptor(
) -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_particle_inject",
        &[
            // 0: particles (read-only storage)
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
            // 1: splats (read-write storage)
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
            // 2: particle uniforms (uniform)
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
// Pipeline
// ---------------------------------------------------------------------------

/// Holds the [`CachedComputePipelineId`] for the particle injection shader.
#[derive(Resource)]
pub struct ParticleInjectPipeline(pub CachedComputePipelineId);

/// System that queues the particle inject pipeline for compilation (once).
pub fn queue_particle_inject_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<ParticleInjectPipeline>>,
) {
    if existing.is_some() {
        return;
    }
    let shader = asset_server
        .load("embedded://context_editor_kernel/render/particle_inject.wgsl");
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("particle_inject_pipeline".into()),
        layout: vec![particle_inject_bind_group_layout_descriptor()],
        push_constant_ranges: vec![],
        shader,
        shader_defs: vec![],
        entry_point: Some("main".into()),
        zero_initialize_workgroup_memory: false,
    });
    commands.insert_resource(ParticleInjectPipeline(id));
}

// ---------------------------------------------------------------------------
// Bind group
// ---------------------------------------------------------------------------

/// Pre-built bind group for the particle inject dispatch.
#[derive(Resource)]
pub struct ParticleInjectBindGroup(pub BindGroup);

/// System that rebuilds the particle inject bind group each frame.
///
/// Particle and splat buffers don't swap, but rebuilding each frame keeps the
/// pattern consistent with other render nodes and handles late-init.
///
/// # Note
/// The tiled forward+ splat pipeline has been removed (Phase 3b). This system
/// is a no-op until particle injection is reworked for the ray-march pipeline.
pub fn rebuild_particle_inject_bind_group(
    _commands: Commands,
    _device: Res<RenderDevice>,
    _pipeline_cache: Res<PipelineCache>,
    _particles: Option<Res<ParticleSplatBuffer>>,
) {
    // No-op: SplatBuffers (tiled pipeline output) has been removed. The
    // ParticleComputeNode is kept in the graph but does nothing until
    // particle injection is re-wired for the ray-march pipeline.
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that dispatches `particle_inject.wgsl`.
///
/// Writes `particle_count` [`VoxelSplat`] entries into `splats[0..particle_count)`.
/// Skips the dispatch entirely when there are no live particles.
#[derive(Default)]
pub struct ParticleComputeNode;

impl Node for ParticleComputeNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<ParticleInjectPipeline>()
        else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<ParticleInjectBindGroup>()
        else {
            return Ok(());
        };
        let Some(particles) = world.get_resource::<ParticleSplatBuffer>()
        else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };

        let particle_count = particles.count;
        if particle_count == 0 {
            return Ok(());
        }

        let Some(pipeline) =
            pipeline_cache.get_compute_pipeline(pipeline_res.0)
        else {
            return Ok(());
        };

        let workgroups = (particle_count + 255) / 256;
        let encoder = render_context.command_encoder();
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("particle_inject"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        Ok(())
    }
}
