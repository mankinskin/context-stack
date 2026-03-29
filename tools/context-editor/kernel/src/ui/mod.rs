use dioxus::prelude::*;
use crate::SandboxWorld;

pub fn render_scaffold<W: SandboxWorld>(world: &W) -> Element {
    // Basic root level render function for the PoC
    rsx! {
        div { class: "kernel-root",
            // WebGPU Canvas Layer
            canvas { id: "gpu-canvas", class: "absolute inset-0 z-0" }

            // UI Layer
            div { class: "relative z-10 flex h-screen w-screen p-4 pointer-events-none",
                // Left: Status & Inspection
                div { class: "w-1/4 pointer-events-auto", {world.sidebar_content()} }
                
                // Center: HUD
                div { class: "flex-1 flex justify-center items-center pointer-events-none",
                    div { class: "w-4 h-4 rounded-full border-2 border-white/50" } // Crosshair
                }

                // Right: Inventory & Generative Terminal
                div { class: "w-1/4 flex flex-col gap-4 pointer-events-auto",
                    {world.inventory_content()},
                    LiquidTerminal {}
                }
            }
        }
    }
}

#[component]
pub fn GlassPanel(title: String, children: Element) -> Element {
    rsx! {
        div { 
            class: "glass-panel rounded-xl border border-white/20 bg-white/5 backdrop-blur-md p-4 shadow-2xl",
            div { class: "panel-header border-b border-white/10 pb-2 mb-4 font-bold text-white/80", "{title}" }
            div { class: "panel-content overflow-y-auto max-h-64", {children} }
        }
    }
}

#[component]
pub fn LiquidTerminal() -> Element {
    let mut input_val = use_signal(|| String::new());

    rsx! {
        GlassPanel { title: "Nexus Terminal".to_string(),
            div { class: "space-y-4",
                p { class: "text-xs opacity-50", "Generative World Modification Active..." }
                input {
                    class: "w-full bg-black/40 border border-white/20 p-2 text-green-400 font-mono text-sm focus:outline-none focus:border-green-500",
                    placeholder: "> Enter a world prompt...",
                    value: "{input_val}",
                    oninput: move |evt| input_val.set(evt.value().clone()),
                }
            }
        }
    }
}
