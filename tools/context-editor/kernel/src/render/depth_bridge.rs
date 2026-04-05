//! Depth Bridge — Phase 3a render node.
//!
//! Converts the per-pixel NDC depth values written by the SVO ray march compute
//! shader (stored in `SvoRayMarchBuffers::depth` as a flat `array<f32>`) into a
//! hardware `Depth32Float` texture attachment usable by downstream passes
//! (wireframe overlay, particle system, UI compositing).
//!
//! ## How it works
//!
//! 1. `SvoRayMarchBuffers::depth` is filled by the ray march compute shader with
//!    per-pixel NDC depth values (Bevy's infinite reverse-Z convention:
//!    near = 1.0, far = 0.0).
//! 2. `DepthBridgeNode` runs a fullscreen triangle render pass that reads the
//!    buffer via a storage binding and writes `@builtin(frag_depth)` into the
//!    hardware depth attachment (`SvoDepthTexture`).
//! 3. `SvoDepthTexture` is then available to downstream render nodes (e.g.
//!    `WireframeOverlayNode`) as a depth attachment for correct depth testing.

use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_graph::{Node, NodeRunError, RenderGraphContext},
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
            CachedRenderPipelineId, CompareFunction, DepthBiasState, DepthStencilState,
            Extent3d, FragmentState, LoadOp, MultisampleState, Operations, PipelineCache,
            PrimitiveState, RenderPassDepthStencilAttachment, RenderPassDescriptor,
            RenderPipelineDescriptor, ShaderStages, StencilState, StoreOp, Texture,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
            TextureViewDescriptor, VertexState,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};
use bytemuck::{Pod, Zeroable};

use crate::render::svo_ray_march::SvoRayMarchBuffers;

// ---------------------------------------------------------------------------
// DepthBridgeUniforms — tiny uniform containing screen_width
// ---------------------------------------------------------------------------

const DEPTH_BRIDGE_UNIFORM_SIZE: u64 = 32;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub struct DepthBridgeUniformData {
    pub screen_width: u32,
    pub _pad: [u32; 7],  // WGSL: vec3u at offset 16 due to align(16) → struct = 32 bytes
}

const _: () = assert!(
    std::mem::size_of::<DepthBridgeUniformData>() == 32,
    "DepthBridgeUniformData must be 32 bytes"
);

/// GPU uniform buffer for `depth_bridge.wgsl`.
#[derive(Resource, Clone)]
pub struct DepthBridgeUniformBuffer(pub Buffer);

impl ExtractResource for DepthBridgeUniformBuffer {
    type Source = DepthBridgeUniformBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Create the uniform buffer once.
pub fn init_depth_bridge_uniforms(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<DepthBridgeUniformBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let buf = device.create_buffer(&BufferDescriptor {
        label: Some("depth_bridge_uniforms"),
        size:  DEPTH_BRIDGE_UNIFORM_SIZE,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    commands.insert_resource(DepthBridgeUniformBuffer(buf));
}

/// Write screen_width from the active window each frame.
pub fn update_depth_bridge_uniforms(
    windows:      Query<&Window>,
    render_queue: Option<Res<RenderQueue>>,
    uniform_buf:  Option<Res<DepthBridgeUniformBuffer>>,
) {
    let Some(render_queue) = render_queue else { return };
    let Some(uniform_buf)  = uniform_buf  else { return };
    let Ok(window) = windows.single() else { return };
    let w = window.physical_width().max(1);
    let data = DepthBridgeUniformData { screen_width: w, _pad: [0; 7] };
    render_queue.write_buffer(&uniform_buf.0, 0, bytemuck::bytes_of(&data));
}

// ---------------------------------------------------------------------------
// SvoDepthTexture — hardware Depth32Float attachment
// ---------------------------------------------------------------------------

/// Hardware `Depth32Float` texture written by [`DepthBridgeNode`] each frame.
///
/// Downstream render nodes (e.g. `WireframeOverlayNode`) bind this texture as
/// their depth attachment for correct hardware depth testing against ray-marched
/// voxel surfaces.
#[derive(Resource, Clone)]
pub struct SvoDepthTexture {
    pub texture: Texture,
    pub view:    TextureView,
    pub width:   u32,
    pub height:  u32,
}

impl ExtractResource for SvoDepthTexture {
    type Source = SvoDepthTexture;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Create (or recreate on resize) the hardware depth texture.
pub fn init_svo_depth_texture(
    mut commands: Commands,
    device:       Option<Res<RenderDevice>>,
    existing:     Option<Res<SvoDepthTexture>>,
    windows:      Query<&Window>,
) {
    let Some(device) = device else { return };
    let Ok(window) = windows.single() else { return };
    let w = window.physical_width().max(1);
    let h = window.physical_height().max(1);

    if let Some(tex) = &existing {
        if tex.width == w && tex.height == h {
            return;
        }
    }

    let texture = device.create_texture(&TextureDescriptor {
        label: Some("svo_depth_texture"),
        size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Depth32Float,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    commands.insert_resource(SvoDepthTexture { texture, view, width: w, height: h });
}

// ---------------------------------------------------------------------------
// Pipeline + bind group
// ---------------------------------------------------------------------------

fn depth_bridge_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "bgl_depth_bridge",
        &[
            // 0: NDC depth buffer (from ray march compute)
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
            // 1: DepthBridgeUniforms (screen_width)
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
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

/// Cached render pipeline for the depth bridge pass.
#[derive(Resource)]
pub struct DepthBridgePipeline(pub CachedRenderPipelineId);

/// Per-frame bind group for the depth bridge pass.
#[derive(Resource)]
pub struct DepthBridgeBindGroup(pub BindGroup);

/// Queue the depth bridge render pipeline for compilation (runs once at startup).
pub fn queue_depth_bridge_pipeline(
    mut commands:   Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server:   Res<AssetServer>,
    existing:       Option<Res<DepthBridgePipeline>>,
) {
    if existing.is_some() {
        return;
    }
    let shader = asset_server.load(
        "embedded://context_editor_kernel/render/depth_bridge.wgsl",
    );
    let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("depth_bridge_pipeline".into()),
        layout: vec![depth_bridge_bind_group_layout()],
        push_constant_ranges: vec![],
        vertex: VertexState {
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Some("vs_main".into()),
            buffers: vec![],
        },
        fragment: Some(FragmentState {
            shader,
            shader_defs: vec![],
            entry_point: Some("fs_main".into()),
            // No color outputs: this is a depth-only pass.
            targets: vec![],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
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
    commands.insert_resource(DepthBridgePipeline(id));
}

/// Rebuild the depth bridge bind group each frame (buffers may be recreated on
/// resize, so the binding must always point to the current buffer handle).
pub fn rebuild_depth_bridge_bind_group(
    mut commands:   Commands,
    device:         Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    rm_buffers:     Option<Res<SvoRayMarchBuffers>>,
    db_uniforms:    Option<Res<DepthBridgeUniformBuffer>>,
) {
    let Some(rm_buffers)  = rm_buffers  else { return };
    let Some(db_uniforms) = db_uniforms else { return };

    let layout = pipeline_cache.get_bind_group_layout(&depth_bridge_bind_group_layout());
    let bg = device.create_bind_group(
        "bg_depth_bridge",
        &layout,
        &[
            BindGroupEntry { binding: 0, resource: rm_buffers.depth.as_entire_binding() },
            BindGroupEntry { binding: 1, resource: db_uniforms.0.as_entire_binding() },
        ],
    );
    commands.insert_resource(DepthBridgeBindGroup(bg));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Render graph node that reads NDC depth from `SvoRayMarchBuffers::depth` and
/// writes it into the `SvoDepthTexture` hardware depth attachment via a
/// fullscreen fragment pass.
#[derive(Default)]
pub struct DepthBridgeNode;

impl Node for DepthBridgeNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let Some(pipeline_res)   = world.get_resource::<DepthBridgePipeline>()  else { return Ok(()); };
        let Some(bind_group)     = world.get_resource::<DepthBridgeBindGroup>() else { return Ok(()); };
        let Some(depth_tex)      = world.get_resource::<SvoDepthTexture>()      else { return Ok(()); };
        let Some(pipeline_cache) = world.get_resource::<PipelineCache>()        else { return Ok(()); };

        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.0) else {
            return Ok(());
        };

        let encoder = render_context.command_encoder();
        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("depth_bridge"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &depth_tex.view,
                depth_ops: Some(Operations {
                    load:  LoadOp::Clear(0.0),  // clear to 0 = far in infinite reverse-Z
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &bind_group.0, &[]);
        rpass.draw(0..3, 0..1);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_bridge_uniform_size() {
        assert_eq!(std::mem::size_of::<DepthBridgeUniformData>(), 32);
    }
}
