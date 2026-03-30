//! Dioxus–Taffy bridge: extracts 2D layout data from the Dioxus overlay into
//! ECS resources consumed by the GPU composite pass.
//!
//! ## Architecture
//!
//! ```text
//! [Dioxus DOM overlay]  ←── already runs via HTML/CSS
//!         ↓  (DOM query each frame)
//! [UiPanel list]        ←── ECS resource
//!         ↓
//! [UiPanelBuffer GPU]   ←── storage buffer for composite shader
//!         ↓
//! [UiComposite pass]    ←── blends UI quads over scene colour
//! ```
//!
//! The bridge reads panel rectangles from the Dioxus DOM via `web_sys` and
//! packs them into a GPU buffer. The composite shader draws filled rounded
//! rectangles with optional frosted-glass scene sampling.

use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bytemuck::{Pod, Zeroable};

/// Maximum number of UI panels the GPU buffer can hold.
pub const MAX_UI_PANELS: usize = 32;

// ---------------------------------------------------------------------------
// ECS Resources
// ---------------------------------------------------------------------------

/// A single 2D UI panel extracted from the Dioxus overlay.
#[derive(Clone, Debug)]
pub struct UiPanel {
    /// Normalised rect (0..1 in viewport space): x, y, width, height.
    pub rect: [f32; 4],
    /// Background colour (premultiplied alpha).
    pub bg_color: [f32; 4],
    /// Border colour.
    pub border_color: [f32; 4],
    /// Border width in pixels.
    pub border_width: f32,
    /// Corner radius in pixels.
    pub corner_radius: f32,
    /// Whether this panel samples the scene mipmap for a frosted-glass look.
    pub frosted_glass: bool,
    /// Mipmap LOD level for frosted blur (0 = sharp, 4 = heavy).
    pub frost_blur: f32,
}

/// Frame-level list of panels extracted from the DOM.
#[derive(Resource, Default)]
pub struct UiPanelList {
    pub panels: Vec<UiPanel>,
}

/// GPU-side representation of a single UI panel (128 bytes, 16-float aligned).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiPanelGpu {
    /// Normalised rect: x, y, w, h
    pub rect: [f32; 4],
    /// Background colour (premultiplied alpha)
    pub bg_color: [f32; 4],
    /// Border colour
    pub border_color: [f32; 4],
    /// border_width, corner_radius, frosted (0/1), frost_blur
    pub params: [f32; 4],
}

/// GPU buffer holding the packed panel array.
#[derive(Resource)]
pub struct UiPanelBuffer {
    pub buffer: wgpu::Buffer,
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct UiBridgePlugin;

impl Plugin for UiBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiPanelList>();
        app.add_systems(Update, extract_dom_panels);
        app.add_systems(
            PostUpdate,
            (init_ui_panel_buffer, update_ui_panel_buffer).chain(),
        );
    }
}

// ---------------------------------------------------------------------------
// DOM → UiPanelList extraction (WASM only)
// ---------------------------------------------------------------------------

/// Query the live DOM for `.glass-panel` elements and convert their
/// bounding rects into normalised viewport coordinates.
fn extract_dom_panels(mut panel_list: ResMut<UiPanelList>) {
    panel_list.panels.clear();

    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::wasm_bindgen::JsCast;

        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };

        let vw = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let vh = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        if vw < 1.0 || vh < 1.0 {
            return;
        }

        let Ok(node_list) = document.query_selector_all(".glass-panel") else {
            return;
        };

        let count = node_list.length().min(MAX_UI_PANELS as u32);
        for i in 0..count {
            let Some(node) = node_list.item(i) else { continue };
            let Ok(el) = node.dyn_into::<web_sys::Element>() else {
                continue;
            };
            let rect = el.get_bounding_client_rect();

            let x = rect.x() as f32 / vw;
            let y = rect.y() as f32 / vh;
            let w = rect.width() as f32 / vw;
            let h = rect.height() as f32 / vh;

            // Check for data-frost attribute for frosted glass panels
            let frosted = el.get_attribute("data-frost").is_some();
            let frost_blur: f32 = el
                .get_attribute("data-frost-blur")
                .and_then(|v| v.parse().ok())
                .unwrap_or(3.0);

            panel_list.panels.push(UiPanel {
                rect: [x, y, w, h],
                bg_color: [0.05, 0.05, 0.08, 0.65],
                border_color: [1.0, 1.0, 1.0, 0.15],
                border_width: 1.0,
                corner_radius: 12.0,
                frosted_glass: frosted,
                frost_blur,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// GPU buffer management
// ---------------------------------------------------------------------------

fn init_ui_panel_buffer(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    existing: Option<Res<UiPanelBuffer>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(device) = device else { return };

    let size = (MAX_UI_PANELS * std::mem::size_of::<UiPanelGpu>()) as u64;
    let buffer = device.wgpu_device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui_panel_buffer"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    commands.insert_resource(UiPanelBuffer { buffer, count: 0 });
}

fn update_ui_panel_buffer(
    panel_list: Res<UiPanelList>,
    queue: Option<Res<RenderQueue>>,
    mut ui_buf: Option<ResMut<UiPanelBuffer>>,
) {
    let (Some(queue), Some(ref mut buf)) = (queue, ui_buf.as_mut()) else {
        return;
    };

    let count = panel_list.panels.len().min(MAX_UI_PANELS);
    buf.count = count as u32;

    if count == 0 {
        return;
    }

    let gpu_panels: Vec<UiPanelGpu> = panel_list
        .panels
        .iter()
        .take(count)
        .map(|p| UiPanelGpu {
            rect: p.rect,
            bg_color: p.bg_color,
            border_color: p.border_color,
            params: [
                p.border_width,
                p.corner_radius,
                if p.frosted_glass { 1.0 } else { 0.0 },
                p.frost_blur,
            ],
        })
        .collect();

    queue.write_buffer(&buf.buffer, 0, bytemuck::cast_slice(&gpu_panels));
}

// ---------------------------------------------------------------------------
// Hit testing
// ---------------------------------------------------------------------------

/// Returns `true` if the given viewport-normalised point (0..1) hits any panel.
pub fn hit_test_panels(panel_list: &UiPanelList, ndc_x: f32, ndc_y: f32) -> bool {
    panel_list.panels.iter().any(|p| {
        ndc_x >= p.rect[0]
            && ndc_x <= p.rect[0] + p.rect[2]
            && ndc_y >= p.rect[1]
            && ndc_y <= p.rect[1] + p.rect[3]
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_panel_gpu_is_64_bytes() {
        assert_eq!(std::mem::size_of::<UiPanelGpu>(), 64);
    }

    #[test]
    fn hit_test_inside() {
        let list = UiPanelList {
            panels: vec![UiPanel {
                rect: [0.1, 0.1, 0.3, 0.3],
                bg_color: [0.0; 4],
                border_color: [0.0; 4],
                border_width: 0.0,
                corner_radius: 0.0,
                frosted_glass: false,
                frost_blur: 0.0,
            }],
        };
        assert!(hit_test_panels(&list, 0.2, 0.2));
        assert!(!hit_test_panels(&list, 0.5, 0.5));
    }

    #[test]
    fn gpu_pack_frosted_flag() {
        let panel = UiPanel {
            rect: [0.0, 0.0, 1.0, 1.0],
            bg_color: [1.0; 4],
            border_color: [0.0; 4],
            border_width: 2.0,
            corner_radius: 8.0,
            frosted_glass: true,
            frost_blur: 3.5,
        };
        let gpu = UiPanelGpu {
            rect: panel.rect,
            bg_color: panel.bg_color,
            border_color: panel.border_color,
            params: [
                panel.border_width,
                panel.corner_radius,
                1.0,
                panel.frost_blur,
            ],
        };
        assert_eq!(gpu.params[2], 1.0);
        assert_eq!(gpu.params[3], 3.5);
    }
}
