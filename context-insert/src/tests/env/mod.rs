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
//!
//! ### Expanded Overlap Environments
//! - `EnvExpandedOverlap` - Postfix overlap scenarios (abc, insert bc)
//! - `EnvMultiOverlap` - Multiple overlapping patterns

// Core insertion test environments
mod insert_infix1;
mod insert_infix2;
mod insert_pattern1;
mod insert_pattern2;
mod insert_postfix1;

// Validation/edge case environments
mod validation;
mod triple_repeat;
mod single_atom;
mod hypergra;

// Expanded overlap environments
mod expanded_overlap;

// Re-export core insertion environments
pub(crate) use insert_infix1::EnvInsertInfix1;
pub(crate) use insert_infix2::EnvInsertInfix2;
pub(crate) use insert_pattern1::EnvInsertPattern1;
pub(crate) use insert_pattern2::EnvInsertPattern2;
pub use insert_postfix1::EnvInsertPostfix1;

// Re-export validation environments
pub(crate) use validation::{EnvAbcd, EnvAb, EnvAbc};
pub(crate) use triple_repeat::EnvTripleRepeat;
pub(crate) use single_atom::EnvSingleAtom;
pub(crate) use hypergra::EnvHypergra;

// Re-export expanded overlap environments
pub(crate) use expanded_overlap::{EnvExpandedOverlap, EnvMultiOverlap};
