use context_editor_kernel::{SandboxWorld, WorldEvent};
use context_editor_kernel::ui::GlassPanel;
use dioxus::prelude::*;

#[derive(Default)]
pub struct CyberpunkWorld {
    // Specific domain rules, SpacetimeDB table connections, etc.
}

impl SandboxWorld for CyberpunkWorld {
    fn name(&self) -> &str {
        "Neon Abyss 2084"
    }

    fn process_event(&self, event: WorldEvent) {
        println!("[CyberpunkWorld] Processing event {}: {}", event.name, event.payload);
        // Dispatch to domain reducers
    }

    fn trigger_generation(&self, prompt: String) {
        println!("[CyberpunkWorld] Sending prompt to domain-specific LLM: {}", prompt);
        // Prefix with Cyberpunk context and send to LLM integration
    }

    fn sidebar_content(&self) -> Element {
        rsx! {
            GlassPanel { title: "Implants & Vitals".to_string(),
                div { class: "flex flex-col gap-2",
                    div { class: "flex justify-between text-sm",
                        span { "Neural Load" },
                        span { class: "text-red-400", "89%" }
                    }
                    div { class: "flex justify-between text-sm",
                        span { "Mantis Blades" },
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
                    // Pseudo inventory slots
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded flex items-center justify-center text-xs", "Gun" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded flex items-center justify-center text-xs", "Stim" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded" }
                    div { class: "w-12 h-12 bg-white/10 border border-white/20 rounded" }
                }
            }
        }
    }
}

fn main() {
    context_editor_kernel::launch::<CyberpunkWorld>();
}
