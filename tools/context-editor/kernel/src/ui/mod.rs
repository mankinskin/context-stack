use dioxus::prelude::*;
use crate::SandboxWorld;

#[inline_props]
pub fn GlassScaffold<'a, W: SandboxWorld>(cx: Scope<'a>, world: &'a W) -> Element<'a> {
    render! {
        div { class: "kernel-root",
            // WebGPU Canvas Layer
            canvas { id: "gpu-canvas", class: "absolute inset-0 z-0" }

            // UI Layer
            div { class: "relative z-10 flex h-screen w-screen p-4 pointer-events-none",
                // Left: Status & Inspection
                div { class: "w-1/4 pointer-events-auto", world.sidebar_content(cx) }
                
                // Center: HUD
                div { class: "flex-1 flex justify-center items-center pointer-events-none",
                    div { class: "w-4 h-4 rounded-full border-2 border-white/50" } // Crosshair
                }

                // Right: Inventory & Generative Terminal
                div { class: "w-1/4 flex flex-col gap-4 pointer-events-auto",
                    world.inventory_content(cx),
                    LiquidTerminal { world: world }
                }
            }
        }
    }
}

#[inline_props]
pub fn GlassPanel<'a>(cx: Scope<'a>, title: &'a str, children: Element<'a>) -> Element<'a> {
    render! {
        div { 
            class: "glass-panel rounded-xl border border-white/20 bg-white/5 backdrop-blur-md p-4 shadow-2xl",
            div { class: "panel-header border-b border-white/10 pb-2 mb-4 font-bold text-white/80", "{title}" }
            div { class: "panel-content overflow-y-auto max-h-64", children }
        }
    }
}

#[inline_props]
pub fn LiquidTerminal<'a, W: SandboxWorld>(cx: Scope<'a>, world: &'a W) -> Element<'a> {
    let input_val = use_state(cx, || String::new());

    render! {
        GlassPanel { title: "Nexus Terminal",
            div { class: "space-y-4",
                p { class: "text-xs opacity-50", "Generative World Modification Active..." }
                input {
                    class: "w-full bg-black/40 border border-white/20 p-2 text-green-400 font-mono text-sm focus:outline-none focus:border-green-500",
                    placeholder: "> Enter a world prompt...",
                    value: "{input_val}",
                    oninput: move |evt| input_val.set(evt.value.clone()),
                    onkeydown: move |evt| {
                        if evt.key() == "Enter" {
                            world.trigger_generation(input_val.get().clone());
                            input_val.set(String::new());
                        }
                    }
                }
            }
        }
    }
}
