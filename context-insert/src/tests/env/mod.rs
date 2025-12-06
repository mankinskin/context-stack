//! Insert test environment module - provides pre-configured graph states for insertion testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod index_infix1;
mod index_infix2;
mod index_pattern1;
mod index_pattern2;

// Re-export all environments
pub use index_infix1::EnvIndexInfix1;
pub use index_infix2::EnvIndexInfix2;
pub use index_pattern1::EnvIndexPattern1;
pub use index_pattern2::EnvIndexPattern2;
