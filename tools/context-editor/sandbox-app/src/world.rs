use context_editor_kernel::{SandboxWorld, WorldEvent};
use dioxus::prelude::*;

/// The Cyberpunk/neon-noir world implementation.
/// Injects domain-specific sidebar, inventory, and LLM prompt context into
/// the generic kernel scaffold.
#[derive(Default)]
pub struct CyberpunkWorld;

impl SandboxWorld for CyberpunkWorld {
    fn name(&self) -> &str {
        "Neon Abyss 2084"
    }

    fn process_event(&self, event: WorldEvent) {
        // In the full implementation this dispatches to SpacetimeDB reducers.
        log::info!("[CyberpunkWorld] event {}: {}", event.name, event.payload);
    }

    fn trigger_generation(&self, prompt: String) {
        // Prefix with Cyberpunk context and forward to the domain LLM integration.
        log::info!("[CyberpunkWorld] generate → {}", prompt);
    }

    fn sidebar_content(&self) -> Element {
        rsx! {}
    }

    fn inventory_content(&self) -> Element {
        rsx! {}
    }
}
