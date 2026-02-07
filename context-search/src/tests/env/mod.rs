//! Search test environment module - provides pre-configured graph states for search testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod ababab;
mod xyyxy;
mod insert_prefix1;

// Re-export all environments
pub(crate) use ababab::EnvAbabab;
pub(crate) use xyyxy::EnvXyyxy;
pub use insert_prefix1::EnvInsertPrefix1;
