//! Search test environment module - provides pre-configured graph states for search testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod index_postfix1;
mod index_prefix1;

// Re-export all environments
pub use index_postfix1::EnvIndexPostfix1;
pub use index_prefix1::EnvIndexPrefix1;
