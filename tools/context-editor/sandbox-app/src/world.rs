use context_editor_kernel::{
    SandboxWorld,
    WorldEvent,
};
use dioxus::prelude::*;

/// World implementation.
/// Injects domain-specific sidebar, inventory, and LLM prompt context into
/// the generic kernel scaffold.
#[derive(Default)]
pub struct ContextWorld;

impl SandboxWorld for ContextWorld {
    fn name(&self) -> &str {
        "Neon Abyss 2084"
    }

    fn process_event(
        &self,
        event: WorldEvent,
    ) {
        // In the full implementation this dispatches to SpacetimeDB reducers.
        log::info!("[ContextWorld] event {}: {}", event.name, event.payload);
    }

    fn trigger_generation(
        &self,
        prompt: String,
    ) {
        // Prefix with Cyberpunk context and forward to the domain LLM integration.
        log::info!("[ContextWorld] generate → {}", prompt);
    }

    fn sidebar_content(&self) -> Element {
        rsx! {}
    }

    fn inventory_content(&self) -> Element {
        rsx! {}
    }

    #[cfg(target_arch = "wasm32")]
    fn run_bevy_app(
        &self,
        mut app: bevy::prelude::App,
    ) {
        use context_editor_kernel::svo::VoxelWorld;

        // Register all world presets. Preset 0 is the default scene shown on
        // startup and when the user selects it from the debug panel.
        context_editor_kernel::register_world_presets([
            (
                "Default Scene",
                super::bootstrap::paint_default_scene
                    as fn(&mut bevy::prelude::World),
            ),
            (
                "Terrain",
                super::presets::paint_terrain as fn(&mut bevy::prelude::World),
            ),
            (
                "Flat",
                super::presets::paint_flat as fn(&mut bevy::prelude::World),
            ),
            (
                "Caves",
                super::presets::paint_caves as fn(&mut bevy::prelude::World),
            ),
            (
                "Empty",
                super::presets::paint_empty as fn(&mut bevy::prelude::World),
            ),
        ]);

        app.insert_resource(VoxelWorld::new(12));
        app.add_plugins(super::bootstrap::BootstrapPlugin);
        app.add_systems(
            bevy::prelude::Startup,
            super::bootstrap::seed_ambient_emitter,
        );
        app.run();
    }
}
