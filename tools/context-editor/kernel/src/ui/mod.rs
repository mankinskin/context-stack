use dioxus::prelude::*;
use crate::world;

/// Root Dioxus component — passed by function pointer to `dioxus::launch`.
/// Renders the kernel glass scaffold, injecting world-specific UI via the
/// global [`SandboxWorld`](crate::SandboxWorld) instance.
#[component]
pub fn root_app() -> Element {
    rsx! {
        div { class: "kernel-root",
            // WebGPU canvas — GPU rendering layer (z-0)
            canvas { id: "gpu-canvas", class: "absolute inset-0 z-0" }

            // Dioxus UI overlay (z-10)
            div { class: "relative z-10 flex h-screen w-screen p-4 pointer-events-none",
                // Left: world-injected status / inspection panel
                div { class: "w-1/4 pointer-events-auto", {world().sidebar_content()} }

                // Centre: minimal HUD crosshair
                div { class: "flex-1 flex justify-center items-center pointer-events-none",
                    div { class: "w-4 h-4 rounded-full border-2 border-white/50" }
                }

                // Right: world-injected inventory + generative terminal
                div { class: "w-1/4 flex flex-col gap-4 pointer-events-auto",
                    {world().inventory_content()},
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

/// Generative terminal panel.  
/// On Enter, the typed prompt is routed to [`SandboxWorld::trigger_generation`].
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
                    onkeydown: move |evt| {
                        // Route Enter to the active SandboxWorld's trigger_generation
                        if evt.key().to_string() == "Enter" {
                            let prompt = input_val.peek().clone();
                            if !prompt.is_empty() {
                                world().trigger_generation(prompt);
                                input_val.set(String::new());
                            }
                        }
                    }
                }
            }
        }
    }
}
