//! Editor — voxel paint/carve tools, undo/redo, SDF cutting, and debug overlay.

pub mod advanced_tools;
pub mod core;
pub mod debug_overlay;
pub mod sdf_cutting;
pub mod ux;

// Re-export all public items from core and ux so that `crate::editor::Foo`
// continues to resolve as before.
pub use self::{
    core::*,
    ux::*,
};
