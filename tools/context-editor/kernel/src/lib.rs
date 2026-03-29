pub mod ui;
pub mod svo;
pub mod splat;
pub mod net;
pub mod gpu;

use std::sync::{Arc, OnceLock};

pub struct WorldEvent {
    pub name: String,
    pub payload: String,
}

pub trait SandboxWorld: 'static + Send + Sync {
    fn name(&self) -> &str;
    fn process_event(&self, event: WorldEvent);
    fn trigger_generation(&self, prompt: String);

    // UI content providers — injected into Kernel's GlassScaffold
    fn sidebar_content(&self) -> dioxus::prelude::Element;
    fn inventory_content(&self) -> dioxus::prelude::Element;
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
}
