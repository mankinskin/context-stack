use crate::{
    editor::debug_overlay::DebugPanel,
    world,
};
use dioxus::prelude::*;

pub mod bridge;
pub mod code_viewer;
pub mod doc_editor;
pub mod interaction;
pub mod inventory;
pub mod panel_interaction;
pub mod skill;
pub mod ticket_editor;
pub mod world_panel;

/// Root Dioxus component — passed by function pointer to `dioxus::launch`.
/// Renders the kernel glass scaffold, injecting world-specific UI via the
/// global [`SandboxWorld`](crate::SandboxWorld) instance.
#[component]
pub fn root_app() -> Element {
    rsx! {
        div { class: "kernel-root pointer-events-none relative h-screen w-screen",
            // Dioxus UI overlay (z-10)
            div { class: "relative z-10 flex h-screen w-screen p-4 pointer-events-none",
                // Left: world-injected status / inspection panel + debug settings
                div { class: "w-1/4 flex flex-col gap-4 pointer-events-auto",
                    {world().sidebar_content()},
                    DebugPanel {}
                }

                // Centre: minimal HUD crosshair
                div { class: "flex-1 flex justify-center items-center pointer-events-none",
                    div {
                        id: "bevy-loading-spinner",
                        class: "w-4 h-4 rounded-full border-2 border-white/50 transition-all duration-300"
                    }
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
pub fn GlassPanel(
    title: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "glass-panel rounded-xl border border-white/20 bg-white/5 backdrop-blur-md p-4 shadow-2xl",
            div { class: "panel-header border-b border-white/10 pb-2 mb-4 font-bold text-white/80", "{title}" }
            div { class: "panel-content overflow-y-auto max-h-[80vh]", {children} }
        }
    }
}

/// Collapsible tree section for the debug panel.
///
/// Renders a clickable header with a ▶/▼ indicator and conditionally shows
/// its children. `default_open` controls the initial expansion state.
#[component]
pub fn TreeSection(
    label: String,
    default_open: bool,
    children: Element,
) -> Element {
    let mut open = use_signal(move || default_open);

    rsx! {
        div { class: "tree-section",
            button {
                class: "flex items-center gap-1 w-full text-left text-white/60 hover:text-white text-[11px] uppercase tracking-wide py-1",
                onclick: move |_| open.set(!open()),
                span { class: "font-mono text-[9px]", if *open.read() { "▼" } else { "▶" } }
                span { "{label}" }
            }
            if *open.read() {
                div { class: "pl-3 border-l border-white/10 space-y-1.5 pb-1",
                    {children}
                }
            }
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
