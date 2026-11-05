mod bootstrap;
mod presets;
mod world;
use world::ContextWorld;

fn main() {
    context_editor_kernel::launch::<ContextWorld>();
}
