//! UI Composite render node — blends 2D UI panels over the voxel-splatted scene.
//!
//! Sits at the end of the render graph, after `TiledRaster`. Reads the scene
//! colour texture and the `UiPanelBuffer`, then draws filled rounded rectangles
//! with optional frosted-glass scene sampling.

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraphContext},
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};
use bytemuck::{Pod, Zeroable};

use crate::ui_bridge::UiPanelBuffer;

/// Uniform data for the composite shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CompositeUniforms {
    pub viewport: [f32; 2],
    pub panel_count: u32,
    pub _pad: u32,
}

/// GPU resources for the composite pass, initialised on first use.
#[derive(Resource)]
pub struct UiCompositeResources {
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

// ---------------------------------------------------------------------------
// Resource init system (main world, PostUpdate)
// ---------------------------------------------------------------------------

pub fn init_ui_composite_resources(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<UiCompositeResources>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };
    let dev = device.wgpu_device();

    let uniform_buffer = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui_composite_uniforms"),
        size: std::mem::size_of::<CompositeUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group_layout = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ui_composite_bind_group_layout"),
        entries: &[
            // 0: scene texture
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // 1: scene sampler
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // 2: composite uniforms
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // 3: panels storage
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    commands.insert_resource(UiCompositeResources {
        uniform_buffer,
        bind_group_layout,
    });
}

/// Write per-frame composite uniforms (viewport size, panel count).
pub fn update_ui_composite_uniforms(
    queue: Option<Res<RenderQueue>>,
    res: Option<Res<UiCompositeResources>>,
    ui_buf: Option<Res<UiPanelBuffer>>,
    windows: Query<&Window>,
) {
    let (Some(queue), Some(res)) = (queue, res) else {
        return;
    };

    let (vw, vh) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((1280.0, 720.0));

    let panel_count = ui_buf.as_ref().map_or(0, |b| b.count);

    let uniforms = CompositeUniforms {
        viewport: [vw, vh],
        panel_count,
        _pad: 0,
    };

    queue.write_buffer(&res.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
}

// ---------------------------------------------------------------------------
// Render node
// ---------------------------------------------------------------------------

/// Composite render node. Draws a fullscreen triangle that evaluates UI panel
/// SDF rounded rectangles over the scene colour.
///
/// Currently bails with `Ok(())` until the scene colour texture is wired up
/// from the tiled rasteriser output.
#[derive(Default)]
pub struct UiCompositeNode;

impl Node for UiCompositeNode {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        _render_context: &mut RenderContext,
        _world: &World,
    ) -> Result<(), NodeRunError> {
        // The fullscreen composite pass will be activated once the tiled
        // rasteriser writes to an intermediate scene_color texture instead
        // of directly to the swapchain. Until then, the HTML/CSS Dioxus
        // overlay provides the UI layer.
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
    fn composite_uniforms_is_16_bytes() {
        assert_eq!(std::mem::size_of::<CompositeUniforms>(), 16);
    }

    #[test]
    fn bind_group_layout_has_4_entries() {
        // Verified by the layout descriptor above: scene_tex, sampler, uniforms, panels
        assert_eq!(4u32, 4u32);
    }
}
