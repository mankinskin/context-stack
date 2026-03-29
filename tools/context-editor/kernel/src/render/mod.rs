//! Custom render-graph pipeline — 7-node voxel splat renderer.
//!
//! Implements T2b (skeleton) + T6a (voxel splat kernel).
//!
//! # Pipeline
//!
//! ```text
//! BufferSwap → ParticleCompute → VoxelSplatKernel → SortKeyBuild
//!           → RadixSort → TileBin → TiledRaster
//! ```
//!
//! | Stage | Purpose |
//! |---|---|
//! | `BufferSwap` | Flip the SVO double-buffer pointer |
//! | `ParticleCompute` | Simulate voxel particle dynamics |
//! | `VoxelSplatKernel` | Generate voxel splats from SVO leaves (T6a) |
//! | `SortKeyBuild` | Build tile+depth sort keys (T6b) |
//! | `RadixSort` | Sort splats by depth (front-to-back) |
//! | `TileBin` | Bin splat bounding rects into screen tiles |
//! | `TiledRaster` | Per-tile rasterise & alpha-composite into framebuffer |
//!
//! `VoxelSplatKernelNode` dispatches the compute shader; remaining nodes are
//! stubs pending T6b–T6d.

pub mod voxel_splat_kernel;

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
        RenderApp,
    },
};

use voxel_splat_kernel::VoxelSplatKernelNode;

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
    /// Generate voxel splats from SVO leaf nodes (T6a compute shader).
    VoxelSplatKernel,
    /// Build tile+depth sort keys from projected splats (T6b).
    SortKeyBuild,
    /// Sort splats by linear depth for correct alpha compositing.
    RadixSort,
    /// Bin splat bounding rects into a screen-tile grid.
    TileBin,
    /// Per-tile forward rasterise with alpha-composite into the output target.
    TiledRaster,
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
stub_node!(ParticleComputeNode);
// VoxelSplatKernelNode lives in render::voxel_splat_kernel (T6a)
stub_node!(SortKeyBuildNode);
stub_node!(RadixSortNode);
stub_node!(TileBinNode);
stub_node!(TiledRasterNode);

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

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers the 7-node context-editor render graph.
///
/// Adds stub nodes to Bevy's top-level [`RenderGraph`] in the correct
/// execution order. Shader pipelines are attached in subsequent tickets.
pub struct ContextEditorRenderPlugin;

impl Plugin for ContextEditorRenderPlugin {
    fn build(&self, app: &mut App) {
        // Guard: RenderApp is absent in headless / test contexts.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();

        // Register nodes
        graph.add_node(ContextEditorLabel::BufferSwap, BufferSwapNode::default());
        graph.add_node(
            ContextEditorLabel::ParticleCompute,
            ParticleComputeNode::default(),
        );
        graph.add_node(ContextEditorLabel::VoxelSplatKernel, VoxelSplatKernelNode::default());
        graph.add_node(ContextEditorLabel::SortKeyBuild, SortKeyBuildNode::default());
        graph.add_node(ContextEditorLabel::RadixSort, RadixSortNode::default());
        graph.add_node(ContextEditorLabel::TileBin, TileBinNode::default());
        graph.add_node(ContextEditorLabel::TiledRaster, TiledRasterNode::default());

        // Wire the sequential edge chain
        graph.add_node_edge(ContextEditorLabel::BufferSwap, ContextEditorLabel::ParticleCompute);
        graph.add_node_edge(
            ContextEditorLabel::ParticleCompute,
            ContextEditorLabel::VoxelSplatKernel,
        );
        graph.add_node_edge(ContextEditorLabel::VoxelSplatKernel, ContextEditorLabel::SortKeyBuild);
        graph.add_node_edge(ContextEditorLabel::SortKeyBuild, ContextEditorLabel::RadixSort);
        graph.add_node_edge(ContextEditorLabel::RadixSort, ContextEditorLabel::TileBin);
        graph.add_node_edge(ContextEditorLabel::TileBin, ContextEditorLabel::TiledRaster);
    }
}
