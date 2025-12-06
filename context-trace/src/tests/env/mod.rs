//! Test environment module - provides pre-configured graph states for testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod env1;
mod env2;
mod index_infix1;
mod index_infix2;
mod index_pattern1;
mod index_pattern2;
mod index_postfix1;
mod index_prefix1;

// Re-export all environments
pub use env1::Env1;
pub use env2::Env2;
pub use index_infix1::EnvIndexInfix1;
pub use index_infix2::EnvIndexInfix2;
pub use index_pattern1::EnvIndexPattern1;
pub use index_pattern2::EnvIndexPattern2;
pub use index_postfix1::EnvIndexPostfix1;
pub use index_prefix1::EnvIndexPrefix1;
