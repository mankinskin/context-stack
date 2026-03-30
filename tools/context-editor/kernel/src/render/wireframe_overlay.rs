//! Wireframe overlay — renders SVO wireframe lines on top of voxel splats.
//!
//! Positions are computed by [`crate::debug_overlay::draw_svo_wireframe`] and
//! stored in [`WireframeVertices`]. This module uploads them to a GPU vertex
//! buffer and draws them in a render pass after TiledRaster.

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, BlendState, Buffer, BufferBindingType, BufferDescriptor,
            BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            FragmentState, MultisampleState, PipelineCache, PrimitiveState,
            PrimitiveTopology, RenderPassDescriptor, RenderPipelineDescriptor,
            ShaderStages, TextureFormat, VertexAttribute,
            VertexFormat, VertexState, VertexStepMode,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::ViewTarget,
    },
};

use crate::debug_overlay::DebugOverlayState;

const MAX_WIREFRAME_VERTICES: u64 = 65536;
const WIREFRAME_UNIFORM_SIZE: u64 = 80; // mat4x4f (64) + vec4f (16)

// ---------------------------------------------------------------------------
// Main-world resources
// ---------------------------------------------------------------------------

/// Line vertex positions computed each frame by the debug overlay system.
#[derive(Resource, Default)]
pub struct WireframeVertices {
    pub positions: Vec<[f32; 3]>,
}

/// GPU buffers and metadata for the wireframe overlay. Extracted to the render
/// world via [`ExtractResource`].
#[derive(Resource, Clone)]
pub struct WireframeOverlayBuffers {
    pub vertex_buf: Buffer,
    pub uniform_buf: Buffer,
    pub vertex_count: u32,
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
        label: Some("wireframe_vertices"),
        size: MAX_WIREFRAME_VERTICES * 12,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
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
        uniform_buf,
        vertex_count: 0,
    });
    commands.init_resource::<WireframeVertices>();
}

/// Upload wireframe vertex + uniform data to the GPU each frame.
pub fn upload_wireframe_data(
    vertices: Option<Res<WireframeVertices>>,
    mut buffers: Option<ResMut<WireframeOverlayBuffers>>,
    render_queue: Option<Res<RenderQueue>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    state: Res<DebugOverlayState>,
) {
    let Some(ref mut buffers) = buffers else { return };
    let Some(render_queue) = render_queue else { return };
    let Some(vertices) = vertices else { return };

    if !state.enabled || vertices.positions.is_empty() {
        buffers.vertex_count = 0;
        return;
    }

    let count = vertices.positions.len().min(MAX_WIREFRAME_VERTICES as usize);
    let bytes: &[u8] = bytemuck::cast_slice(&vertices.positions[..count]);
    render_queue.write_buffer(&buffers.vertex_buf, 0, bytes);
    buffers.vertex_count = count as u32;

    let Ok((transform, projection)) = camera_q.single() else { return };

    let view_mat = transform.to_matrix().inverse();
    let proj_mat = projection.get_clip_from_view();
    let view_proj = proj_mat * view_mat;

    let color: [f32; 4] = match state.wire_color {
        Color::Srgba(c) => [c.red, c.green, c.blue, c.alpha],
        _ => [0.0, 1.0, 0.0, 1.0],
    };

    let mut data = [0u8; 80];
    data[..64].copy_from_slice(bytemuck::bytes_of(&view_proj.to_cols_array()));
    data[64..80].copy_from_slice(bytemuck::bytes_of(&color));
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
        depth_stencil: None,
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
        if buffers.vertex_count == 0 {
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

        {
            let mut pass = render_context
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("wireframe_overlay_pass"),
                    color_attachments: &[Some(color_attachment)],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.set_vertex_buffer(0, *buffers.vertex_buf.slice(..));
            pass.draw(0..buffers.vertex_count, 0..1);
        }

        Ok(())
    }
}
