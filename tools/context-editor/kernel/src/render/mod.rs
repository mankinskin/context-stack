//! Custom render-graph pipeline — 8-node voxel splat renderer.
//!
//! Implements T2b (skeleton) + T6a (voxel splat kernel) + T6b (sort key build)
//! + T6c (radix sort) + UI composite.
//!
//! # Pipeline
//!
//! ```text
//! BufferSwap → ParticleCompute → VoxelSplatKernel → SortKeyBuild
//!           → RadixSort → TileBin → TiledRaster → UiComposite
//! ```
//!
//! | Stage | Purpose |
//! |---|---|
//! | `BufferSwap` | Flip the SVO double-buffer pointer |
//! | `ParticleCompute` | Simulate voxel particle dynamics |
//! | `VoxelSplatKernel` | Generate voxel splats from SVO leaves (T6a) |
//! | `SortKeyBuild` | Build tile+depth sort keys (T6b) |
//! | `RadixSort` | 8-pass 4-bit GPU radix sort (T6c) |
//! | `TileBin` | Bin splat bounding rects into screen tiles |
//! | `TiledRaster` | Per-tile rasterise & alpha-composite into framebuffer |
//! | `UiComposite` | Composite 2D UI panels over scene colour |
//!
//! `VoxelSplatKernelNode` dispatches the splat generation compute shader.
//! `SortKeyBuildNode` dispatches AABB projection + sort key construction.
//! `RadixSortNode` dispatches 24 compute passes (3 × 8 passes) for sorting.
//! `TileBinNode` bins sorted splats into screen tiles (T6d Phase 1).
//! `TiledRasterNode` renders per-pixel SDF + PBR (T6d Phase 2).

pub mod voxel_splat_kernel;
pub mod sort_key_build;
pub mod radix_sort;
pub mod tile_binning;
pub mod tiled_raster;
pub mod glass;
pub mod ui_composite;

use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
        RenderApp,
    },
};

use voxel_splat_kernel::VoxelSplatKernelNode;
use sort_key_build::SortKeyBuildNode;
use radix_sort::RadixSortNode;
use tile_binning::TileBinNode;
use tiled_raster::TiledRasterNode;
use ui_composite::UiCompositeNode;

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
    /// Composite 2D UI panels over the scene colour output.
    UiComposite,
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
// SortKeyBuildNode lives in render::sort_key_build (T6b)
// RadixSortNode lives in render::radix_sort (T6c)
// TileBinNode lives in render::tile_binning (T6d)
// TiledRasterNode lives in render::tiled_raster (T6d)

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
/// Adds nodes to Bevy's top-level [`RenderGraph`] in the correct execution
/// order. Also registers init and per-frame systems for pipeline setup.
pub struct ContextEditorRenderPlugin;

impl Plugin for ContextEditorRenderPlugin {
    fn build(&self, app: &mut App) {
        // Main world: resource init + per-frame uniform updates.
        // Only systems that use main-world resources (RenderDevice, RenderQueue,
        // AssetServer) belong here.  Pipeline and bind-group systems need
        // PipelineCache which lives in the render sub-app — they are NOT
        // registered here (the render nodes bail silently when missing).
        app.add_systems(
            PostUpdate,
            (
                sort_key_build::init_sort_key_resources,
                sort_key_build::update_camera_uniforms,
            )
                .chain(),
        );
        app.add_systems(
            PostUpdate,
            radix_sort::init_radix_sort_resources,
        );
        app.add_systems(
            PostUpdate,
            (
                tile_binning::init_tile_bin_resources,
                tile_binning::update_tile_bin_uniforms,
            )
                .chain(),
        );
        app.add_systems(
            PostUpdate,
            (
                tiled_raster::init_raster_resources,
                tiled_raster::update_raster_uniforms,
            )
                .chain(),
        );
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
        graph.add_node(ContextEditorLabel::UiComposite, UiCompositeNode::default());

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
        graph.add_node_edge(ContextEditorLabel::TiledRaster, ContextEditorLabel::UiComposite);
    }
}
