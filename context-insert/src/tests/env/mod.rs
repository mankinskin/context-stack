//! Insert test environment module - provides pre-configured graph states for insertion testing
//!
//! Each environment is defined in its own module for clarity and maintainability.

mod insert_infix1;
mod insert_infix2;
mod insert_pattern1;
mod insert_pattern2;

// Re-export all environments
pub use insert_infix1::EnvInsertInfix1;
pub use insert_infix2::EnvInsertInfix2;
pub use insert_pattern1::EnvInsertPattern1;
pub use insert_pattern2::EnvInsertPattern2;
