//! Insert test cases module
//!
//! ## Test Organization
//!
//! ### Core Insertion Tests
//! - `infix` - Infix/range matching behavior
//! - `pattern` - General pattern insertion
//! - `prefix` - Prefix matching
//! - `postfix` - Postfix matching
//! - `scoped` - Scope-restricted complement insertion
//!
//! ### Validation & Error Handling
//! - `validation` - Input validation (empty patterns, invalid InitInterval)
//!
//! ### Scenario Tests
//! - `context_read_scenarios` - Tests simulating context-read failure cases
//! - `repeat_patterns` - Tests for repeated token patterns
//! - `expanded_overlap` - Tests for postfix overlap scenarios

// Core insertion tests
pub(crate) mod infix;
pub(crate) mod pattern;
pub(crate) mod postfix;
pub(crate) mod prefix;
pub(crate) mod scoped;

// Validation & edge cases
pub(crate) mod validation;

// Scenario tests
pub(crate) mod context_read_scenarios;
pub(crate) mod expanded_overlap;
pub(crate) mod overlap_bundle;
