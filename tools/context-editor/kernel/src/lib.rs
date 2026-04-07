pub mod ui;
pub mod ui_bridge;
pub mod svo;
pub mod splat;
pub mod net;
pub mod gpu;
pub mod physics;
pub mod render;
pub mod theme;
pub mod character;
pub mod editor;
pub mod force_compute;
pub mod world_panel;
pub mod particle_splat;
pub mod panel_interaction;
pub mod advanced_tools;
pub mod interaction;
pub mod runtime_params;
pub mod sdf_cutting;
pub mod latency_comp;
pub mod editor_ux;
pub mod svo_lod;
pub mod multiplayer_backend;
pub mod multiplayer_net;
pub mod multiplayer_chars;
pub mod world_gen;
pub mod combat;
pub mod inventory;
pub mod skill;
pub mod context_graph;
pub mod llm_integration;
pub mod ticket_editor;
pub mod doc_editor;
pub mod code_viewer;
pub mod debug_overlay;

use std::sync::{Arc, OnceLock};

pub struct WorldEvent {
    pub name: String,
    pub payload: String,
}

pub trait SandboxWorld: 'static + Send + Sync {
    fn name(&self) -> &str;
    fn process_event(&self, event: WorldEvent);
    fn trigger_generation(&self, prompt: String);

    // UI content providers — injected into Kernel's GlassScaffold
    fn sidebar_content(&self) -> dioxus::prelude::Element;
    fn inventory_content(&self) -> dioxus::prelude::Element;

    /// Receives a fully-configured kernel [`App`](bevy::prelude::App) and is
    /// responsible for adding any world-specific resources/plugins and calling
    /// `app.run()`. The default implementation just calls `app.run()`.
    ///
    /// The app already contains all kernel plugins. Typical overrides add the
    /// world's `VoxelWorld` resource, a scene bootstrap plugin, and any
    /// startup systems before forwarding to `app.run()`.
    #[cfg(target_arch = "wasm32")]
    fn run_bevy_app(&self, mut app: bevy::prelude::App) {
        app.run();
    }
}

static WORLD: OnceLock<Arc<dyn SandboxWorld>> = OnceLock::new();

/// Returns a reference to the global [`SandboxWorld`] instance.
///
/// # Panics
/// Panics if [`launch`] has not been called yet.
pub fn world() -> &'static Arc<dyn SandboxWorld> {
    WORLD.get().expect("context_editor_kernel::launch() must be called before accessing the world")
}

/// Initialise the kernel with the given world type and start the Dioxus web app.
///
/// # Panics
/// Panics if called more than once.
pub fn launch<W: SandboxWorld + Default>() {
    WORLD
        .set(Arc::new(W::default()))
        .map_err(|_| "launch() called twice")
        .expect("context_editor_kernel::launch() must only be called once");

    dioxus::launch(ui::root_app);

    #[cfg(target_arch = "wasm32")]
    {
        // Start Bevy (this will "unwind" using an exception in Winit's run, so it
        // won't return, which means we must call Dioxus launch first).
        // build_kernel_app() contains all kernel plugins; the sandbox-app's
        // run_bevy_app() adds world-specific resources and calls app.run().
        crate::world().run_bevy_app(build_kernel_app());
    }
}

/// Build a Bevy [`App`](bevy::prelude::App) pre-loaded with all kernel plugins.
///
/// The returned app has no world-specific resources; the sandbox-app is
/// responsible for calling `app.insert_resource(VoxelWorld::new(...))`,
/// adding scene plugins, and invoking `app.run()`.
#[cfg(target_arch = "wasm32")]
pub fn build_kernel_app() -> bevy::prelude::App {
    use bevy::prelude::*;

    let mut app = App::new();

    // Standard Bevy plugins (injecting the canvas config)
    app.add_plugins(DefaultPlugins.set(render::canvas_window_plugin()));

    // Kernel plugins
    app.add_plugins(crate::render::ContextEditorRenderPlugin);
    app.add_plugins(crate::physics::PhysicsPlugin);
    app.add_plugins(crate::svo::upload::SvoUploadPlugin);
    app.add_plugins(crate::ui_bridge::UiBridgePlugin);
    app.add_plugins(crate::editor::EditorPlugin);
    app.add_plugins(crate::force_compute::ForceComputePlugin);
    app.add_plugins(crate::world_panel::WorldPanelPlugin);
    app.add_plugins(crate::particle_splat::ParticleSplatPlugin);
    app.add_plugins(crate::panel_interaction::PanelInteractionPlugin);
    app.add_plugins(crate::advanced_tools::AdvancedToolsPlugin);
    app.add_plugins(crate::interaction::InteractionBridgePlugin);
    app.add_plugins(crate::runtime_params::RuntimeParamsPlugin);
    app.add_plugins(crate::sdf_cutting::SdfCuttingPlugin);
    app.add_plugins(crate::latency_comp::LatencyCompPlugin);
    app.add_plugins(crate::editor_ux::EditorUxPlugin);
    app.add_plugins(crate::svo_lod::SvoLodPlugin);
    app.add_plugins(crate::multiplayer_backend::MultiplayerBackendPlugin);
    app.add_plugins(crate::multiplayer_net::MultiplayerNetPlugin);
    app.add_plugins(crate::multiplayer_chars::MultiplayerCharactersPlugin);
    app.add_plugins(crate::world_gen::WorldGenPlugin);
    app.add_plugins(crate::combat::CombatPlugin);
    app.add_plugins(crate::inventory::InventoryPlugin);
    app.add_plugins(crate::skill::SkillPlugin);
    app.add_plugins(crate::context_graph::ContextGraph3DPlugin);
    app.add_plugins(crate::llm_integration::LlmIntegrationPlugin);
    app.add_plugins(crate::ticket_editor::TicketEditorPlugin);
    app.add_plugins(crate::doc_editor::DocEditorPlugin);
    app.add_plugins(crate::code_viewer::CodeViewerPlugin);
    app.add_plugins(crate::debug_overlay::DebugOverlayPlugin);

    app
}
