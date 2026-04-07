mod world;
#[cfg(target_arch = "wasm32")]
mod bootstrap;
#[cfg(target_arch = "wasm32")]
mod presets;
use world::ContextWorld;

fn main() {
    context_editor_kernel::launch::<ContextWorld>();
}
