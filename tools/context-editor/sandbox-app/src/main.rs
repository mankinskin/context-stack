mod world;
#[cfg(target_arch = "wasm32")]
mod bootstrap;
use world::ContextWorld;

fn main() {
    context_editor_kernel::launch::<ContextWorld>();
}
