use context_editor_kernel::{SandboxWorld, WorldEvent};
use context_editor_kernel::ui::GlassPanel;
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
        rsx! {
            GlassPanel { title: "Implants & Vitals".to_string(),
                div { class: "flex flex-col gap-2",
                    div { class: "flex justify-between text-sm",
                        span { "Neural Load" }
                        span { class: "text-red-400", "89%" }
                    }
                    div { class: "flex justify-between text-sm",
                        span { "Mantis Blades" }
                        span { class: "text-green-400", "Online" }
                    }
                }
            }
        }
    }

    fn inventory_content(&self) -> Element {
        rsx! {
            GlassPanel { title: "Black Market Cache".to_string(),
                div { class: "grid grid-cols-4 gap-2",
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded flex items-center justify-center text-xs", "Gun" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded flex items-center justify-center text-xs", "Stim" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded" }
                }
            }
        }
    }
}
