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
    core_pipeline::core_3d::graph::{Core3d, Node3d},
    render::{
        extract_resource::ExtractResourcePlugin,
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        renderer::RenderContext,
        Render, RenderApp, RenderSystems,
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
        // Embed WGSL shaders into the binary so they are available on WASM
        // without a filesystem or asset-serving HTTP path.
        bevy::asset::embedded_asset!(app, "voxel_splat_kernel.wgsl");
        bevy::asset::embedded_asset!(app, "sort_key_build.wgsl");
        bevy::asset::embedded_asset!(app, "radix_sort.wgsl");
        bevy::asset::embedded_asset!(app, "tile_binning.wgsl");
        bevy::asset::embedded_asset!(app, "tiled_raster.wgsl");

        // Main world: resource init + per-frame uniform updates.
        // Only systems that use main-world resources (RenderDevice, RenderQueue,
        // AssetServer) belong here.  Pipeline and bind-group systems need
        // PipelineCache which lives in the render sub-app — they are registered
        // below on the render sub-app.
        app.add_systems(
            PostUpdate,
            crate::gpu::init_splat_buffers,
        );
        app.add_systems(
            PostUpdate,
            (
                voxel_splat_kernel::init_splat_params,
                voxel_splat_kernel::update_splat_params,
                voxel_splat_kernel::init_node_positions,
                voxel_splat_kernel::update_node_positions,
            )
                .chain(),
        );
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

        // Extract main-world resources into the render sub-app each frame.
        // ExtractResourcePlugin must be added to the MAIN app (not render_app)
        // because its build() internally accesses app.get_sub_app_mut(RenderApp).
        app.add_plugins(ExtractResourcePlugin::<crate::gpu::SplatBuffers>::default())
            .add_plugins(ExtractResourcePlugin::<crate::gpu::SvoDoubleBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<voxel_splat_kernel::SplatParamsUniform>::default())
            .add_plugins(ExtractResourcePlugin::<voxel_splat_kernel::NodePositionBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<sort_key_build::SortKeyCameraUniform>::default())
            .add_plugins(ExtractResourcePlugin::<radix_sort::RadixSortUniformBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<radix_sort::RadixSortStagingBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<tiled_raster::RasterUniformBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<tile_binning::TileBinUniformBuffer>::default())
            .add_plugins(ExtractResourcePlugin::<glass::GlassPanelBuffer>::default());

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
                voxel_splat_kernel::queue_voxel_splat_pipeline,
                voxel_splat_kernel::rebuild_splat_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );
        render_app.add_systems(
            Render,
            (
                sort_key_build::queue_sort_key_pipeline,
                sort_key_build::rebuild_sort_key_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );
        render_app.add_systems(
            Render,
            (
                radix_sort::queue_radix_sort_pipelines,
                radix_sort::rebuild_radix_sort_bind_groups,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );
        render_app.add_systems(
            Render,
            (
                tiled_raster::queue_raster_pipeline,
                tiled_raster::rebuild_raster_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );
        render_app.add_systems(
            Render,
            (
                tile_binning::queue_tile_bin_pipeline,
                tile_binning::rebuild_tile_bin_bind_group,
            )
                .chain()
                .in_set(RenderSystems::Queue),
        );

        // Construct nodes that need FromWorld before borrowing the graph.
        let tiled_raster_node = TiledRasterNode::from_world(render_app.world_mut());

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
        core3d.add_node(ContextEditorLabel::VoxelSplatKernel, VoxelSplatKernelNode::default());
        core3d.add_node(ContextEditorLabel::SortKeyBuild, SortKeyBuildNode::default());
        core3d.add_node(ContextEditorLabel::RadixSort, RadixSortNode::default());
        core3d.add_node(ContextEditorLabel::TileBin, TileBinNode::default());
        core3d.add_node(ContextEditorLabel::TiledRaster, tiled_raster_node);
        core3d.add_node(ContextEditorLabel::UiComposite, UiCompositeNode::default());

        // Wire the sequential edge chain (internal ordering)
        core3d.add_node_edge(ContextEditorLabel::BufferSwap, ContextEditorLabel::ParticleCompute);
        core3d.add_node_edge(
            ContextEditorLabel::ParticleCompute,
            ContextEditorLabel::VoxelSplatKernel,
        );
        core3d.add_node_edge(ContextEditorLabel::VoxelSplatKernel, ContextEditorLabel::SortKeyBuild);
        core3d.add_node_edge(ContextEditorLabel::SortKeyBuild, ContextEditorLabel::RadixSort);
        core3d.add_node_edge(ContextEditorLabel::RadixSort, ContextEditorLabel::TileBin);
        core3d.add_node_edge(ContextEditorLabel::TileBin, ContextEditorLabel::TiledRaster);
        core3d.add_node_edge(ContextEditorLabel::TiledRaster, ContextEditorLabel::UiComposite);

        // Anchor into the existing Core3d sub-graph:
        //   ... → EndMainPass → [our chain] → Tonemapping → Upscaling → ...
        core3d.add_node_edge(Node3d::EndMainPass, ContextEditorLabel::BufferSwap);
        core3d.add_node_edge(ContextEditorLabel::UiComposite, Node3d::Tonemapping);
    }
}
