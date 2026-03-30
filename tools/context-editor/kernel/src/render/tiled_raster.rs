//! Tiled Forward+ Rasteriser — render node for T6d Phase 2.
//!
//! Draws a fullscreen triangle and evaluates per-pixel ray-box SDF against
//! the sorted, tile-binned voxel splats.  PBR lighting via Cook-Torrance/GGX,
//! front-to-back alpha compositing with early-out.
//!
//! ## Bind Group Layout (group 0)
//!
//! | Binding | Type | Content |
//! |---------|------|---------|
//! | 0 | `storage<read>` | `sorted_values: array<u32>` |
//! | 1 | `storage<read>` | `projected: array<ProjectedSplat>` |
//! | 2 | `storage<read>` | `tile_data: array<u32>` (packed: offset<<12 | count) |
//! | 3 | `uniform` | `uniforms: RasterUniforms` |
//! | 4 | `storage<read>` | `glass_panels: array<GlassPanelData>` |

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            MultisampleState, PipelineCache, PrimitiveState, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderStages, TextureFormat, VertexState,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        view::ViewTarget,
    },
};

use crate::gpu::SplatBuffers;
use super::glass::GlassPanelBuffer;

// ---------------------------------------------------------------------------
// Uniform data (matches WGSL `RasterUniforms`)
// ---------------------------------------------------------------------------

/// Raster uniforms: inv_view_proj (64), camera_pos+pad (16),
/// resolution+grid_width+max_depth (16), light_dir+pad (16),
/// light_color+pad (16) = 128 bytes.
const RASTER_UNIFORM_SIZE: u64 = 128;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RasterUniforms {
    pub inv_view_proj: [f32; 16],
    pub camera_pos: [f32; 3],
    pub _pad0: f32,
    pub resolution: [f32; 2],
    pub grid_width: u32,
    pub max_depth: f32,
    pub light_dir: [f32; 3],
    pub _pad1: f32,
    pub light_color: [f32; 3],
    pub glass_count: u32,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// GPU uniform buffer for the rasteriser's per-frame data.
#[derive(Resource, Clone)]
pub struct RasterUniformBuffer(pub Buffer);

impl ExtractResource for RasterUniformBuffer {
    type Source = RasterUniformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Cached render pipeline for the fullscreen-triangle rasteriser.
#[derive(Resource)]
pub struct RasterPipeline(pub CachedRenderPipelineId);

/// Pre-built bind group for the rasteriser.
#[derive(Resource)]
pub struct TiledRasterBindGroup(pub BindGroup);

// ---------------------------------------------------------------------------
// Resource init
// ---------------------------------------------------------------------------

/// Create the uniform buffer for the tiled rasteriser.
pub fn init_raster_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<RasterUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };

    let buf = device.create_buffer(&BufferDescriptor {
        label: Some("raster_uniform"),
        size: RASTER_UNIFORM_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(RasterUniformBuffer(buf));
}

/// Write raster uniforms each frame.
pub fn update_raster_uniforms(
    camera_query: Query<(&GlobalTransform, &Projection), With<Camera3d>>,
    windows: Query<&Window>,
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<RasterUniformBuffer>>,
    render_queue: Option<Res<RenderQueue>>,
    glass_buffer: Option<Res<GlassPanelBuffer>>,
) {
    let Some(uniform) = uniform else { return };
    let Some(render_queue) = render_queue else { return };
    let Some(splat_buffers) = splat_buffers else { return };
    let Ok((transform, projection)) = camera_query.single() else { return };
    let Ok(window) = windows.single() else { return };

    let view_mat = transform.to_matrix().inverse();
    let proj_mat = projection.get_clip_from_view();
    let view_proj = proj_mat * view_mat;
    let inv_vp = view_proj.inverse();
    let pos = transform.translation();

    let u = RasterUniforms {
        inv_view_proj: inv_vp.to_cols_array(),
        camera_pos: [pos.x, pos.y, pos.z],
        _pad0: 0.0,
        resolution: [window.physical_width() as f32, window.physical_height() as f32],
        grid_width: splat_buffers.tiles_x,
        max_depth: 1000.0,
        light_dir: [0.267, 0.802, 0.534], // normalize(0.3, 0.9, 0.6)
        _pad1: 0.0,
        light_color: [1.0, 0.98, 0.95],
        glass_count: glass_buffer.as_ref().map_or(0, |b| b.count),
    };
    render_queue.write_buffer(&uniform.0, 0, bytemuck::bytes_of(&u));
}

// ---------------------------------------------------------------------------
// Bind group layout
// ---------------------------------------------------------------------------

/// Bind group layout matching `tiled_raster.wgsl` group(0).
pub fn raster_bind_group_layout_descriptor() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_tiled_raster",
        &[
            // binding 0 — sorted_values
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 1 — projected
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 2 — tile_data
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 3 — uniforms
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 4 — glass_panels
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
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

/// Queue the render pipeline for the tiled rasteriser (once).
pub fn queue_raster_pipeline(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing: Option<Res<RasterPipeline>>,
) {
    if existing.is_some() {
        return;
    }

    let shader = asset_server.load("embedded://context_editor_kernel/render/tiled_raster.wgsl");
    let layout = vec![raster_bind_group_layout_descriptor()];

    let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("tiled_raster_pipeline".into()),
        layout,
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![], // fullscreen triangle — no VBO
        },
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState {
            count: 4, // matches Bevy default Msaa::Sample4
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        zero_initialize_workgroup_memory: false,
    });
    commands.insert_resource(RasterPipeline(id));
}

// ---------------------------------------------------------------------------
// Bind group rebuild
// ---------------------------------------------------------------------------

/// Rebuild the rasteriser bind group each frame.
pub fn rebuild_raster_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    splat_buffers: Option<Res<SplatBuffers>>,
    uniform: Option<Res<RasterUniformBuffer>>,
    glass_buffer: Option<Res<GlassPanelBuffer>>,
) {
    let Some(splat_buffers) = splat_buffers else { return };
    let Some(uniform) = uniform else { return };
    let Some(glass_buffer) = glass_buffer else { return };

    let descriptor = raster_bind_group_layout_descriptor();
    let layout = pipeline_cache.get_bind_group_layout(&descriptor);

    let bg = device.create_bind_group(
        "bg_tiled_raster",
        &layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: splat_buffers.active_list.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: splat_buffers.projected.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: splat_buffers.tile_data.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: uniform.0.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: glass_buffer.buffer.as_entire_binding(),
            },
        ],
    );
    commands.insert_resource(TiledRasterBindGroup(bg));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Fullscreen-triangle render node for the tiled forward+ rasteriser.
///
/// Draws 3 vertices (no VBO) with the `tiled_raster.wgsl` shader.  Each pixel
/// loops over the splats in its tile, evaluates a ray-box SDF, and composites
/// with PBR lighting.
pub struct TiledRasterNode {
    view_query: QueryState<&'static ViewTarget>,
}

impl FromWorld for TiledRasterNode {
    fn from_world(world: &mut World) -> Self {
        Self {
            view_query: QueryState::new(world),
        }
    }
}

impl Node for TiledRasterNode {
    fn update(&mut self, world: &mut World) {
        self.view_query.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<RasterPipeline>() else {
            return Ok(());
        };
        let Some(bind_group) = world.get_resource::<TiledRasterBindGroup>() else {
            return Ok(());
        };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>() else {
            return Ok(());
        };
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.0) else {
            return Ok(());
        };

        let binding = self.view_query.query_manual(world);
        let Ok(view_target) = binding.single() else {
            return Ok(());
        };

        let color_attachment = view_target.get_color_attachment();

        {
            let mut render_pass =
                render_context
                    .command_encoder()
                    .begin_render_pass(&RenderPassDescriptor {
                        label: Some("tiled_raster_pass"),
                        color_attachments: &[Some(color_attachment)],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &bind_group.0, &[]);
            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_uniforms_size_matches_wgsl() {
        // mat4x4f(64) + vec3f+pad(16) + vec2f+u32+f32(16) + vec3f+pad(16) + vec3f+pad(16) = 128
        assert_eq!(std::mem::size_of::<RasterUniforms>(), 128);
        assert_eq!(RASTER_UNIFORM_SIZE, 128);
    }

    #[test]
    fn raster_uniforms_is_pod() {
        let bytes = [0u8; 128];
        let _: &RasterUniforms = bytemuck::from_bytes(&bytes);
    }
}
