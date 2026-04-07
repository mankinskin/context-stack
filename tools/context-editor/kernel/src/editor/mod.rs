//! Editor — voxel paint/carve tools, undo/redo, SDF cutting, and debug overlay.

pub mod core;
pub mod ux;
pub mod advanced_tools;
pub mod sdf_cutting;
pub mod debug_overlay;

// Re-export all public items from core and ux so that `crate::editor::Foo`
// continues to resolve as before.
pub use self::core::*;
pub use self::ux::*;
