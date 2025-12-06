//! Test environment module - provides pre-configured graph states for testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod env1;
mod env2;

// Re-export all environments
pub use env1::Env1;
pub use env2::Env2;
