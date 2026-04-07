//! Custom render-graph pipeline — SVO ray-march + depth-bridge renderer.
//!
//! # Pipeline
//!
//! ```text
//! BufferSwap → ParticleCompute → SvoRayMarch → DepthBridge
//!           → UiComposite → WireframeOverlay
//! ```
//!
//! | Stage | Purpose |
//! |---|---|
//! | `BufferSwap` | Flip the SVO double-buffer pointer |
//! | `ParticleCompute` | Simulate voxel particle dynamics |
//! | `SvoRayMarch` | Direct SVO ray march — per-pixel colour + NDC depth |
//! | `DepthBridge` | Copy NDC depth from storage buffer to hardware depth attachment |
//! | `UiComposite` | Composite 2D UI panels over scene colour |
//! | `WireframeOverlay` | Draw SVO wireframe lines with depth testing |

pub mod depth_bridge;
pub mod glass;
pub mod ui_composite;
pub mod wireframe_overlay;
pub mod particle_inject;
pub mod svo_ray_march;
pub mod runtime_params;

use bevy::{
    prelude::*,
    core_pipeline::core_3d::graph::{Core3d, Node3d},
    render::{
        extract_resource::ExtractResourcePlugin,
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
        Render, RenderApp, RenderSystems,
    },
};

use depth_bridge::DepthBridgeNode;
use ui_composite::UiCompositeNode;
use wireframe_overlay::WireframeOverlayNode;
use particle_inject::ParticleComputeNode;
use svo_ray_march::SvoRayMarchNode;

// ---------------------------------------------------------------------------
// Node labels
// ---------------------------------------------------------------------------

/// Type-safe labels for every node in the context-editor render graph.
#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub enum ContextEditorLabel {
    /// Flip the SVO double-buffer pointer so the GPU can read the newly
    /// uploaded octree data without stalling.
    BufferSwap,
    /// Compute voxel particle dynamics (spawn / destroy / move splats).
    ParticleCompute,
    /// SVO direct ray march — per-pixel colour and NDC depth output.
    SvoRayMarch,
    /// Copy NDC depth from storage buffer to hardware Depth32Float attachment.
    DepthBridge,
    /// Composite 2D UI panels over the scene colour output.
    UiComposite,
    /// Draw SVO wireframe lines on top of scene with depth testing.
    WireframeOverlay,
}

// ---------------------------------------------------------------------------
// Stub node impls
// ---------------------------------------------------------------------------

macro_rules! stub_node {
    ($name:ident) => {
        /// Stub render-graph node — runs `Ok(())` until the WGSL shader is
        /// loaded in a later ticket.
        #[derive(Default)]
        pub struct $name;

        impl Node for $name {
            fn run(
                &self,
                _graph: &mut RenderGraphContext,
                _render_context: &mut RenderContext,
                _world: &World,
            ) -> Result<(), NodeRunError> {
                Ok(())
            }
        }
    };
}

stub_node!(BufferSwapNode);
// ParticleComputeNode lives in render::particle_inject (real implementation)
// SvoRayMarchNode lives in render::svo_ray_march (Phase 1b)
// DepthBridgeNode lives in render::depth_bridge (Phase 3a)

// ---------------------------------------------------------------------------
// Mipmap helper (stub)
// ---------------------------------------------------------------------------

/// Generate mipmaps for a GPU texture by blitting each level from the one above.
///
/// **Stub** — currently a no-op. Will be implemented when the tiled rasteriser
/// writes to an intermediate RGBA16F target that requires mip-maps for TAA.
///
/// # Parameters
/// * `encoder` — active command encoder (mip blits require separate passes)
/// * `texture` — texture to generate mipmaps for
/// * `mip_levels` — number of mip levels to generate (>= 2 to do any work)
pub fn generate_mipmaps(
    _encoder: &mut bevy::render::render_resource::CommandEncoder,
    _texture: &bevy::render::render_resource::Texture,
    _mip_levels: u32,
) {
    // Mip-blit render pipeline will be filled in with the TiledRaster shader.
}

// ---------------------------------------------------------------------------
// Canvas helper
// ---------------------------------------------------------------------------

/// Return the [`WindowPlugin`] configuration needed to attach Bevy to the
/// `#bevy-canvas` HTML element on WASM targets.
///
/// On native targets, returns the default `WindowPlugin` (windowed 1280×720).
///
/// Intended to be called inside the `sandbox-app` as:
/// ```rust,ignore
/// app.add_plugins(MinimalPlugins.set(context_editor_kernel::render::canvas_window_plugin()));
/// ```
pub fn canvas_window_plugin() -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: "Context Editor".into(),
            #[cfg(target_arch = "wasm32")]
            canvas: Some("#bevy-canvas".to_string()),
            #[cfg(target_arch = "wasm32")]
            fit_canvas_to_parent: true,
            #[cfg(target_arch = "wasm32")]
            prevent_default_event_handling: true,
            ..default()
        }),
        ..default()
    }
}

/// On WASM, force-sync the Bevy [`Window`] resolution from the actual canvas
/// element dimensions each frame. This works around Bevy's `fit_canvas_to_parent`
/// ResizeObserver not firing when the parent is already the correct size at
/// creation time.
#[cfg(target_arch = "wasm32")]
pub fn sync_canvas_resolution(mut windows: Query<&mut Window>) {
    let Some(canvas) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("bevy-canvas"))
    else {
        return;
    };
    let w = canvas.client_width().max(1) as f32;
    let h = canvas.client_height().max(1) as f32;

    for mut window in &mut windows {
        let cur_w = window.resolution.physical_width() as f32;
        let cur_h = window.resolution.physical_height() as f32;
        if (cur_w - w).abs() > 1.0 || (cur_h - h).abs() > 1.0 {
            window
                .resolution
                .set_physical_resolution(w as u32, h as u32);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers the 7-node context-editor render graph.
///
/// Adds nodes to Bevy's top-level [`RenderGraph`] in the correct execution
/// order. Also registers init and per-frame systems for pipeline setup.
pub struct ContextEditorRenderPlugin;

impl Plugin for ContextEditorRenderPlugin {
    fn build(&self, app: &mut App) {
        // Embed WGSL shaders into the binary so they are available on WASM
        // without a filesystem or asset-serving HTTP path.
        bevy::asset::embedded_asset!(app, "depth_bridge.wgsl");
        bevy::asset::embedded_asset!(app, "wireframe_overlay.wgsl");
        bevy::asset::embedded_asset!(app, "particle_inject.wgsl");
        bevy::asset::embedded_asset!(app, "svo_common.wgsl");
        bevy::asset::embedded_asset!(app, "svo_ray_march.wgsl");

        // On WASM, force-sync the Window resolution from the actual canvas
        // size before any render-related systems read it.
        #[cfg(target_arch = "wasm32")]
        app.add_systems(PreUpdate, sync_canvas_resolution);

        // Main world: resource init + per-frame uniform updates.
        // Only systems that use main-world resources (RenderDevice, RenderQueue,
        // AssetServer) belong here.  Pipeline and bind-group systems need
        // PipelineCache which lives in the render sub-app — they are registered
        // below on the render sub-app.
        app.add_systems(
            PostUpdate,
            (
                glass::init_glass_resources,
                glass::update_glass_panel_buffer,
            )
                .chain(),
        );
        app.add_systems(
            PostUpdate,
            (
                ui_composite::init_ui_composite_resources,
                ui_composite::update_ui_composite_uniforms,
            )
                .chain(),
        );
        app.add_systems(
            PostUpdate,
            (
                wireframe_overlay::init_wireframe_overlay,
                wireframe_overlay::upload_wireframe_data,
            )
                .chain(),
        );

        // Phase 1a: SVO transform uniform (world-to-SVO coordinate mapping).
        app.add_systems(
            PostUpdate,
            (
                crate::gpu::svo_transform::init_svo_transform,
                crate::gpu::svo_transform::update_svo_transform,
            )
                .chain(),
        );

        // Phase 1b: SVO ray march — output buffers and per-frame uniforms.
        app.add_systems(
            PostUpdate,
            (
                svo_ray_march::init_ray_march_buffers,
                svo_ray_march::init_ray_march_uniforms,
                svo_ray_march::update_ray_march_uniforms,
            )
                .chain(),
        );

        // Phase 3a: Depth bridge — depth texture and bridge uniforms.
        app.add_systems(
            PostUpdate,
            (
                depth_bridge::init_svo_depth_texture,
                depth_bridge::init_depth_bridge_uniforms,
                depth_bridge::update_depth_bridge_uniforms,
            )
                .chain(),
        );

        // Extract main-world resources into the render sub-app each frame.
        // ExtractResourcePlugin must be added to the MAIN app (not render_app)
        // because its build() internally accesses app.get_sub_app_mut(RenderApp).
        app.add_plugins(ExtractResourcePlugin::<crate::gpu::SvoDoubleBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<crate::particle_splat::ParticleSplatBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<glass::GlassPanelBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<wireframe_overlay::WireframeOverlayBuffers>::default())
            // Phase 1a + 1b extractions
            .add_plugins(ExtractResourcePlugin::<crate::gpu::SvoTransformBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<svo_ray_march::SvoRayMarchBuffers>::default())
            .add_plugins(ExtractResourcePlugin::<svo_ray_march::SvoRayMarchUniformBuffer>::default())
            // Phase 3a: depth bridge extractions
            .add_plugins(ExtractResourcePlugin::<depth_bridge::SvoDepthTexture>::default())
            .add_plugins(ExtractResourcePlugin::<depth_bridge::DepthBridgeUniformBuffer>::default());

        // Guard: RenderApp is absent in headless / test contexts.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // Render-world systems: pipeline queueing and bind group rebuild.
        // These need PipelineCache / AssetServer which live in the render
        // sub-app, so they must run here.
        render_app.add_systems(
            Render,
            (
                particle_inject::queue_particle_inject_pipeline,
                particle_inject::rebuild_particle_inject_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );

        render_app.add_systems(
            Render,
            (
                wireframe_overlay::queue_wireframe_pipeline,
                wireframe_overlay::rebuild_wireframe_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );

        // Phase 1b: SVO ray march pipeline and bind group setup
        render_app.add_systems(
            Render,
            (
                svo_ray_march::queue_ray_march_pipelines,
                svo_ray_march::rebuild_ray_march_bind_groups,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );

        // Phase 3a: Depth bridge pipeline and bind group setup
        render_app.add_systems(
            Render,
            (
                depth_bridge::queue_depth_bridge_pipeline,
                depth_bridge::rebuild_depth_bridge_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );

        // Construct nodes that need FromWorld before borrowing the graph.
        let wireframe_node = WireframeOverlayNode::from_world(render_app.world_mut());
        let svo_ray_march_node = SvoRayMarchNode::from_world(render_app.world_mut());

        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();

        // Register nodes in the Core3d camera sub-graph so they execute
        // WITHIN the camera pipeline — after 3D rendering, before tonemapping
        // and upscaling. Placing nodes in the main graph caused their output
        // to be overwritten by Bevy's built-in upscaling pass.
        let core3d = graph.sub_graph_mut(Core3d);

        core3d.add_node(ContextEditorLabel::BufferSwap, BufferSwapNode::default());
        core3d.add_node(
            ContextEditorLabel::ParticleCompute,
            ParticleComputeNode::default(),
        );
        core3d.add_node(ContextEditorLabel::SvoRayMarch, svo_ray_march_node);
        core3d.add_node(ContextEditorLabel::DepthBridge, DepthBridgeNode::default());
        core3d.add_node(ContextEditorLabel::UiComposite, UiCompositeNode::default());
        core3d.add_node(ContextEditorLabel::WireframeOverlay, wireframe_node);

        // Wire the sequential edge chain:
        // BufferSwap → ParticleCompute → SvoRayMarch → DepthBridge → UiComposite → WireframeOverlay
        core3d.add_node_edge(ContextEditorLabel::BufferSwap, ContextEditorLabel::ParticleCompute);
        core3d.add_node_edge(ContextEditorLabel::ParticleCompute, ContextEditorLabel::SvoRayMarch);
        core3d.add_node_edge(ContextEditorLabel::SvoRayMarch, ContextEditorLabel::DepthBridge);
        core3d.add_node_edge(ContextEditorLabel::DepthBridge, ContextEditorLabel::UiComposite);
        core3d.add_node_edge(ContextEditorLabel::UiComposite, ContextEditorLabel::WireframeOverlay);

        // Anchor into the existing Core3d sub-graph:
        //   ... → EndMainPass → [our chain] → Tonemapping → Upscaling → ...
        core3d.add_node_edge(Node3d::EndMainPass, ContextEditorLabel::BufferSwap);
        core3d.add_node_edge(ContextEditorLabel::WireframeOverlay, Node3d::Tonemapping);
    }
}
