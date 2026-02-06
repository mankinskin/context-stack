//! Insert test environment module - provides pre-configured graph states for insertion testing
//!
//! Each environment is defined in its own module for clarity and maintainability.
//!
//! ## Environment Categories
//!
//! ### Insertion Pattern Environments
//! - `EnvInsertInfix1`, `EnvInsertInfix2` - Infix/range matching
//! - `EnvInsertPattern1`, `EnvInsertPattern2` - General pattern matching
//!
//! ### Validation/Edge Case Environments
//! - `EnvAbcd` - InitInterval validation (4-atom pattern)
//! - `EnvAb` - Empty pattern rejection (2-atom)
//! - `EnvAbc` - Mismatch testing (3-atom with extra)
//!
//! ### Repeat Pattern Environments
//! - `EnvTripleRepeat` - ababab scenario
//! - `EnvSingleAtom` - Single atom for [a,a] patterns
//!
//! ### Context-Read Scenario Environments
//! - `EnvHypergra` - Partial match scenarios

// Core insertion test environments
mod insert_infix1;
mod insert_infix2;
mod insert_pattern1;
mod insert_pattern2;

// Validation/edge case environments
mod validation;
mod triple_repeat;
mod single_atom;
mod hypergra;

// Re-export core insertion environments
pub use insert_infix1::EnvInsertInfix1;
pub use insert_infix2::EnvInsertInfix2;
pub use insert_pattern1::EnvInsertPattern1;
pub use insert_pattern2::EnvInsertPattern2;

// Re-export validation environments
pub use validation::{EnvAbcd, EnvAb, EnvAbc};
pub use triple_repeat::EnvTripleRepeat;
pub use single_atom::EnvSingleAtom;
pub use hypergra::EnvHypergra;
