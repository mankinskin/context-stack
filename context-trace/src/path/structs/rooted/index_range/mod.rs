//! Index range path - paths within a pattern starting from an index root
//!
//! Provides IndexRangePath type and associated trait implementations for
//! path traversal, position tracking, and movement operations.

mod accessors;
mod lower;
mod movement;
mod position;
mod type_def;

// Re-export the main type
pub use type_def::IndexRangePath;

// Trait implementations are available via the submodules
// (no need to re-export individual impls, they're automatically available)
