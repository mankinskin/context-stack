//! Compact event formatter for tracing output
//!
//! Provides a custom formatter that displays tracing events and spans with:
//! - Indentation showing span hierarchy
//! - Visual markers (gutters) for span boundaries
//! - Configurable trait context display
//! - Function signature highlighting
//! - Field filtering and formatting

mod core;
mod event;

// Re-export the main formatter struct
pub use core::CompactFieldsFormatter;

// Internal modules used by event.rs
use super::{
    config,
    field_visitor,
    special_fields,
    syntax,
    timer,
};
