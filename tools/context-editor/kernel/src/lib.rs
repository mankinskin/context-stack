// ---------------------------------------------------------------------------
// Module tree
// ---------------------------------------------------------------------------

// Core data & rendering
pub mod gpu;
pub mod net;
pub mod physics;
pub mod render;
pub mod splat;
pub mod svo;

// Domain modules
pub mod editor;
pub mod multiplayer;
pub mod simulation;
pub mod ui;
pub mod world;

// ---------------------------------------------------------------------------
// Backward-compatible re-exports — keep all existing `crate::xxx` paths working
// ---------------------------------------------------------------------------

// world/
pub use world::{
    svo_lod,
    theme,
    world_gen,
};

// render/
pub use render::runtime_params;

// splat/
pub use splat::{
    force_compute,
    particle_splat,
};

// multiplayer/
pub use multiplayer::{
    backend as multiplayer_backend,
    chars as multiplayer_chars,
    combat,
    latency_comp,
    net as multiplayer_net,
};

// editor/ — core and ux items are re-exported via editor/mod.rs wildcard;
// the sub-modules themselves are also re-exported for direct access.
pub use editor::{
    advanced_tools,
    debug_overlay,
    sdf_cutting,
    ux as editor_ux,
};

// ui/
pub use ui::{
    bridge as ui_bridge,
    code_viewer,
    doc_editor,
    interaction,
    inventory,
    panel_interaction,
    skill,
    ticket_editor,
    world_panel,
};

// simulation/
pub use simulation::{
    character,
    context_graph,
    llm_integration,
};

use std::sync::{
    Arc,
    Mutex,
    OnceLock,
};

// ---------------------------------------------------------------------------
// World preset registry
// ---------------------------------------------------------------------------

/// A callback that paints a world preset into the Bevy ECS.
///
/// The function receives exclusive world access so it can read any resource
/// (e.g. [`ThemePalette`](crate::world::theme::ThemePalette)) while mutating
/// [`VoxelWorld`](crate::svo::VoxelWorld).
#[cfg(target_arch = "wasm32")]
pub type WorldPresetFn = Arc<dyn Fn(&mut bevy::prelude::World) + Send + Sync>;

#[cfg(target_arch = "wasm32")]
static PRESET_REGISTRY: Mutex<Vec<(String, WorldPresetFn)>> =
    Mutex::new(Vec::new());

/// Register all selectable world presets before calling [`launch`].
///
/// Each entry is `(name, fn)`. Index 0 is the default scene shown on startup.
/// Presets appear in the debug panel in registration order.
#[cfg(target_arch = "wasm32")]
pub fn register_world_presets(
    presets: impl IntoIterator<
        Item = (
            impl Into<String>,
            impl Fn(&mut bevy::prelude::World) + Send + Sync + 'static,
        ),
    >
) {
    let mut reg = PRESET_REGISTRY.lock().unwrap();
    for (name, f) in presets {
        reg.push((name.into(), Arc::new(f)));
    }
}

/// Returns the names of all registered presets (in registration order).
#[cfg(target_arch = "wasm32")]
pub fn world_preset_names() -> Vec<String> {
    PRESET_REGISTRY
        .lock()
        .unwrap()
        .iter()
        .map(|(n, _)| n.clone())
        .collect()
}

/// Resets [`VoxelWorld`](crate::svo::VoxelWorld) and applies preset `index`.
///
/// Called by the `apply_world_preset` exclusive system in `debug_overlay`.
#[cfg(target_arch = "wasm32")]
pub fn apply_registered_preset(
    index: u32,
    world: &mut bevy::prelude::World,
) {
    use crate::svo::VoxelWorld;
    let max_depth = world.resource::<VoxelWorld>().max_depth;
    *world.resource_mut::<VoxelWorld>() = VoxelWorld::new(max_depth);
    let reg = PRESET_REGISTRY.lock().unwrap();
    if let Some((_, f)) = reg.get(index as usize) {
        let f = f.clone();
        drop(reg);
        f(world);
    }
}

pub struct WorldEvent {
    pub name: String,
    pub payload: String,
}

pub trait SandboxWorld: 'static + Send + Sync {
    fn name(&self) -> &str;
    fn process_event(
        &self,
        event: WorldEvent,
    );
    fn trigger_generation(
        &self,
        prompt: String,
    );

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
    fn run_bevy_app(
        &self,
        mut app: bevy::prelude::App,
    ) {
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
