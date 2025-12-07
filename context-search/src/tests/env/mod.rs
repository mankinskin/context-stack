//! Search test environment module - provides pre-configured graph states for search testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod insert_postfix1;
mod insert_prefix1;

// Re-export all environments
pub use insert_postfix1::EnvInsertPostfix1;
pub use insert_prefix1::EnvInsertPrefix1;
