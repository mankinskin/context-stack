use context_editor_kernel::{SandboxWorld, WorldEvent};
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

    fn process_event(&self, event: WorldEvent) {
        // In the full implementation this dispatches to SpacetimeDB reducers.
        log::info!("[ContextWorld] event {}: {}", event.name, event.payload);
    }

    fn trigger_generation(&self, prompt: String) {
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
    fn run_bevy_app(&self, mut app: bevy::prelude::App) {
        use context_editor_kernel::svo::VoxelWorld;
        app.insert_resource(VoxelWorld::new(12));
        app.add_plugins(super::bootstrap::BootstrapPlugin);
        app.add_systems(bevy::prelude::Startup, super::bootstrap::seed_ambient_emitter);
        app.run();
    }
}
