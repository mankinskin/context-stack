//! Wireframe overlay — renders SVO wireframe lines on top of voxel splats.
//!
//! Positions are computed by [`crate::debug_overlay::draw_svo_wireframe`] and
//! stored in [`WireframeData`]. This module uploads them to GPU vertex and
//! index buffers and draws them in a render pass after TiledRaster.

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, BlendState, Buffer, BufferBindingType, BufferDescriptor,
            BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            CompareFunction, DepthBiasState, DepthStencilState,
            FragmentState, LoadOp, MultisampleState, Operations,
            IndexFormat, PipelineCache, PrimitiveState,
            PrimitiveTopology, RenderPassDepthStencilAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderStages,
            StencilState, StoreOp, TextureFormat, VertexAttribute,
            VertexFormat, VertexState, VertexStepMode,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::ViewTarget,
    },
};

use crate::debug_overlay::DebugOverlayState;

/// Maximum octree depth used for wireframe buffer budget calculations.
/// Must match the `max_depth` passed to `VoxelWorld::new()` in `lib.rs`.
pub const MAX_OCTREE_DEPTH: u32 = 12;

/// Maximum wireframe grid side length in cells, equal to the world size at
/// [`MAX_OCTREE_DEPTH`]: `2^max_depth = 1024`.
pub const MAX_WIREFRAME_GRID_SIZE: u64 = 1 << MAX_OCTREE_DEPTH;

/// Number of corner vertices per wireframe cube.
pub const WIREFRAME_VERTS_PER_CUBE: usize = 8;

/// Number of indices per wireframe cube (12 edges × 2 endpoints).
pub const WIREFRAME_INDICES_PER_CUBE: usize = 24;

/// Maximum number of wireframe cubes renderable per frame.
///
/// The index budget is kept at `2 × MAX_WIREFRAME_GRID_SIZE²` (two full
/// face-planes at max resolution); dividing by [`WIREFRAME_INDICES_PER_CUBE`]
/// converts that to a cube count (~87 K cubes at `MAX_OCTREE_DEPTH = 10`).
pub const MAX_WIREFRAME_CUBES: u64 =
    2 * MAX_WIREFRAME_GRID_SIZE * MAX_WIREFRAME_GRID_SIZE
    / WIREFRAME_INDICES_PER_CUBE as u64;

const WIREFRAME_UNIFORM_SIZE: u64 = 80; // mat4x4f (64) + vec4f (16)
const MAT4_BYTES: usize = core::mem::size_of::<[f32; 16]>(); // 64 — byte offset of the color field

// ---------------------------------------------------------------------------
// Main-world resources
// ---------------------------------------------------------------------------

/// Corner positions and edge indices computed each frame by the debug overlay.
#[derive(Resource, Default)]
pub struct WireframeData {
    pub corners: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// GPU buffers and metadata for the wireframe overlay. Extracted to the render
/// world via [`ExtractResource`].
#[derive(Resource, Clone)]
pub struct WireframeOverlayBuffers {
    pub vertex_buf: Buffer,
    pub index_buf: Buffer,
    pub uniform_buf: Buffer,
    pub index_count: u32,
}

impl ExtractResource for WireframeOverlayBuffers {
    type Source = WireframeOverlayBuffers;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

// ---------------------------------------------------------------------------
// Render-world resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct WireframeOverlayPipeline(pub CachedRenderPipelineId);

#[derive(Resource)]
pub struct WireframeOverlayBindGroup(pub BindGroup);

// ---------------------------------------------------------------------------
// Init + upload (main world)
// ---------------------------------------------------------------------------

pub fn init_wireframe_overlay(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<WireframeOverlayBuffers>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };

    let vertex_buf = device.create_buffer(&BufferDescriptor {
        label: Some("wireframe_corners"),
        size: MAX_WIREFRAME_CUBES * WIREFRAME_VERTS_PER_CUBE as u64 * 12,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let index_buf = device.create_buffer(&BufferDescriptor {
        label: Some("wireframe_indices"),
        size: MAX_WIREFRAME_CUBES * WIREFRAME_INDICES_PER_CUBE as u64 * 4,
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&BufferDescriptor {
        label: Some("wireframe_uniform"),
        size: WIREFRAME_UNIFORM_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    commands.insert_resource(WireframeOverlayBuffers {
        vertex_buf,
        index_buf,
        uniform_buf,
        index_count: 0,
    });
    commands.init_resource::<WireframeData>();
}

/// Upload wireframe corner + index + uniform data to the GPU each frame.
pub fn upload_wireframe_data(
    data: Option<Res<WireframeData>>,
    mut buffers: Option<ResMut<WireframeOverlayBuffers>>,
    render_queue: Option<Res<RenderQueue>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    state: Res<DebugOverlayState>,
) {
    let Some(ref mut buffers) = buffers else { return };
    let Some(render_queue) = render_queue else { return };
    let Some(data) = data else { return };

    if !state.enabled || data.indices.is_empty() {
        buffers.index_count = 0;
        return;
    }

    let corner_count = data.corners.len()
        .min(MAX_WIREFRAME_CUBES as usize * WIREFRAME_VERTS_PER_CUBE);
    let index_count = data.indices.len()
        .min(MAX_WIREFRAME_CUBES as usize * WIREFRAME_INDICES_PER_CUBE);
    render_queue.write_buffer(
        &buffers.vertex_buf, 0, bytemuck::cast_slice(&data.corners[..corner_count]),
    );
    render_queue.write_buffer(
        &buffers.index_buf, 0, bytemuck::cast_slice(&data.indices[..index_count]),
    );
    buffers.index_count = index_count as u32;

    let Ok((transform, projection)) = camera_q.single() else { return };

    let view_mat = transform.to_matrix().inverse();
    let proj_mat = projection.get_clip_from_view();
    let view_proj = proj_mat * view_mat;

    let color: [f32; 4] = match state.wire_color {
        Color::Srgba(c) => [c.red, c.green, c.blue, c.alpha],
        _ => [0.0, 1.0, 0.0, 1.0],
    };

    let mut data = [0u8; WIREFRAME_UNIFORM_SIZE as usize];
    data[..MAT4_BYTES].copy_from_slice(bytemuck::bytes_of(&view_proj.to_cols_array()));
    data[MAT4_BYTES..].copy_from_slice(bytemuck::bytes_of(&color));
    render_queue.write_buffer(&buffers.uniform_buf, 0, &data);
}

// ---------------------------------------------------------------------------
// Pipeline + bind group (render world)
// ---------------------------------------------------------------------------

fn wireframe_bind_group_layout_desc() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new("bgl_wireframe_overlay", &[
        BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ])
}

pub fn queue_wireframe_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<WireframeOverlayPipeline>>,
) {
    if existing.is_some() {
        return;
    }
    let shader = asset_server.load(
        "embedded://context_editor_kernel/render/wireframe_overlay.wgsl",
    );
    let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("wireframe_overlay_pipeline".into()),
        layout: vec![wireframe_bind_group_layout_desc()],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![bevy::mesh::VertexBufferLayout {
                array_stride: 12,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
        },
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::LineList,
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            // depth_write_enabled=false: wireframe is a debug overlay, never
            // clobber the depth buffer written by the depth-bridge pass.
            depth_write_enabled: false,
            // Always: wireframe lines draw on top of ray-marched voxel surfaces.
            depth_compare: CompareFunction::Always,
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }),
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        zero_initialize_workgroup_memory: false,
    });
    commands.insert_resource(WireframeOverlayPipeline(id));
}

pub fn rebuild_wireframe_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    buffers: Option<Res<WireframeOverlayBuffers>>,
) {
    let Some(buffers) = buffers else { return };
    let layout = device.create_bind_group_layout(
        "bgl_wireframe_overlay",
        &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    );
    let bind_group = device.create_bind_group(
        "bg_wireframe_overlay",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: buffers.uniform_buf.as_entire_binding(),
        }],
    );
    commands.insert_resource(WireframeOverlayBindGroup(bind_group));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

pub struct WireframeOverlayNode {
    view_query: QueryState<&'static ViewTarget>,
}

impl FromWorld for WireframeOverlayNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_query: world.query(),
        }
    }
}

impl Node for WireframeOverlayNode {
    fn update(&mut self, world: &mut World) {
        self.view_query.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<WireframeOverlayPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<WireframeOverlayBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(buffers) = world.get_resource::<WireframeOverlayBuffers>() else {
            return Ok(());
        };
        if buffers.index_count == 0 {
            return Ok(());
        }
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.0) else {
            return Ok(());
        };
        let binding = self.view_query.query_manual(world);
        let Ok(view_target) = binding.single() else {
            return Ok(());
        };

        let color_attachment = view_target.get_unsampled_color_attachment();

        // The pipeline has depth_stencil: Some(...), so the render pass MUST
        // provide a depth attachment.  Skip the draw when SvoDepthTexture is
        // not yet available (first frame, or headless mode) to avoid a WebGPU
        // validation error.
        let Some(depth_tex) = world
            .get_resource::<crate::render::depth_bridge::SvoDepthTexture>()
        else {
            return Ok(());
        };

        let depth_attachment = RenderPassDepthStencilAttachment {
            view: &depth_tex.view,
            depth_ops: Some(Operations {
                load: LoadOp::Load,
                store: StoreOp::Discard,
            }),
            stencil_ops: None,
        };

        {
            let mut pass = render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("wireframe_overlay_pass"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: Some(depth_attachment),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.set_vertex_buffer(0, *buffers.vertex_buf.slice(..));
            pass.set_index_buffer(*buffers.index_buf.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(0..buffers.index_count, 0, 0..1);
        }

        Ok(())
    }
}
