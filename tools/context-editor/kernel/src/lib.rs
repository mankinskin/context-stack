pub mod ui;

pub struct WorldEvent {
    pub name: String,
    pub payload: String,
}

pub trait SandboxWorld: 'static + Send + Sync {
    fn name(&self) -> &str;
    fn process_event(&self, event: WorldEvent);
    fn trigger_generation(&self, prompt: String);
    
    // UI Providers
    fn sidebar_content<'a>(&'a self, cx: dioxus::core::Scope<'a>) -> dioxus::core::Element<'a>;
    fn inventory_content<'a>(&'a self, cx: dioxus::core::Scope<'a>) -> dioxus::core::Element<'a>;
}

// Pseudo launch stub to prove the entrypoint interface
pub fn launch<W: SandboxWorld + Default>() {
    // In reality this would init Dioxus web and Bevy loop side-by-side
    println!("Launching context-editor-kernel with World: {}", W::default().name());
}
