//! Wireframe overlay — renders SVO wireframe lines on top of voxel splats.
//!
//! Positions are computed by [`crate::debug_overlay::draw_svo_wireframe`] and
//! stored in [`WireframeData`]. This module uploads them to GPU vertex and
//! index buffers and draws them in a render pass after TiledRaster.

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{
            Node,
            NodeRunError,
            RenderGraphContext,
        },
        render_resource::{
            BindGroup,
            BindGroupEntry,
            BindGroupLayoutDescriptor,
            BindGroupLayoutEntry,
            BindingType,
            BlendState,
            Buffer,
            BufferBindingType,
            BufferDescriptor,
            BufferUsages,
            CachedRenderPipelineId,
            ColorTargetState,
            ColorWrites,
            CompareFunction,
            DepthBiasState,
            DepthStencilState,
            FragmentState,
            IndexFormat,
            LoadOp,
            MultisampleState,
            Operations,
            PipelineCache,
            PrimitiveState,
            PrimitiveTopology,
            RenderPassDepthStencilAttachment,
            RenderPassDescriptor,
            RenderPipelineDescriptor,
            ShaderStages,
            StencilState,
            StoreOp,
            TextureFormat,
            VertexAttribute,
            VertexFormat,
            VertexState,
            VertexStepMode,
        },
        renderer::{
            RenderContext,
            RenderDevice,
            RenderQueue,
        },
        view::ViewTarget,
    },
};

use crate::debug_overlay::DebugOverlayState;

mod node;

pub use node::WireframeOverlayNode;

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

const WIREFRAME_UNIFORM_SIZE: u64 = 80;
const MAT4_BYTES: usize = core::mem::size_of::<[f32; 16]>();

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

#[derive(Resource)]
pub struct WireframeOverlayPipeline(pub CachedRenderPipelineId);

#[derive(Resource)]
pub struct WireframeOverlayBindGroup(pub BindGroup);

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
        label: Some("wireframe_overlay_vertices"),
        size: MAX_WIREFRAME_CUBES
            * WIREFRAME_VERTS_PER_CUBE as u64
            * core::mem::size_of::<[f32; 3]>() as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let index_buf = device.create_buffer(&BufferDescriptor {
        label: Some("wireframe_overlay_indices"),
        size: MAX_WIREFRAME_CUBES
            * WIREFRAME_INDICES_PER_CUBE as u64
            * core::mem::size_of::<u32>() as u64,
        usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_buf = device.create_buffer(&BufferDescriptor {
        label: Some("wireframe_overlay_uniforms"),
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
}

pub fn upload_wireframe_data(
    wire_data: Option<Res<WireframeData>>,
    buffers: Option<ResMut<WireframeOverlayBuffers>>,
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    render_queue: Option<Res<RenderQueue>>,
    state: Option<Res<DebugOverlayState>>,
) {
    let Some(render_queue) = render_queue else {
        return;
    };
    let Some(mut buffers) = buffers else {
        return;
    };

    let Some(wire_data) = wire_data else {
        buffers.index_count = 0;
        return;
    };

    let max_vertices =
        (MAX_WIREFRAME_CUBES * WIREFRAME_VERTS_PER_CUBE as u64) as usize;
    let max_indices =
        (MAX_WIREFRAME_CUBES * WIREFRAME_INDICES_PER_CUBE as u64) as usize;
    let corners =
        &wire_data.corners[..wire_data.corners.len().min(max_vertices)];
    let indices =
        &wire_data.indices[..wire_data.indices.len().min(max_indices)];

    buffers.index_count = indices.len() as u32;

    if !corners.is_empty() {
        render_queue.write_buffer(
            &buffers.vertex_buf,
            0,
            bytemuck::cast_slice(corners),
        );
    }
    if !indices.is_empty() {
        render_queue.write_buffer(
            &buffers.index_buf,
            0,
            bytemuck::cast_slice(indices),
        );
    }

    let Ok((transform, projection)) = camera_query.single() else {
        return;
    };

    let view_mat = transform.to_matrix().inverse();
    let proj_mat = projection.get_clip_from_view();
    let view_proj = proj_mat * view_mat;

    let color: [f32; 4] = match state {
        Some(state) => match state.wire_color {
            Color::Srgba(c) => [c.red, c.green, c.blue, c.alpha],
            _ => [0.0, 1.0, 0.0, 1.0],
        },
        None => [0.0, 1.0, 0.0, 1.0],
    };

    let mut data = [0u8; WIREFRAME_UNIFORM_SIZE as usize];
    data[..MAT4_BYTES]
        .copy_from_slice(bytemuck::bytes_of(&view_proj.to_cols_array()));
    data[MAT4_BYTES..].copy_from_slice(bytemuck::bytes_of(&color));
    render_queue.write_buffer(&buffers.uniform_buf, 0, &data);
}

// ---------------------------------------------------------------------------
// Pipeline + bind group (render world)
// ---------------------------------------------------------------------------

fn wireframe_bind_group_layout_desc() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
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
    )
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
    let shader = asset_server
        .load("embedded://context_editor_kernel/render/wireframe_overlay.wgsl");
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
            depth_write_enabled: false,
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
    let Some(buffers) = buffers else {
        return;
    };
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
