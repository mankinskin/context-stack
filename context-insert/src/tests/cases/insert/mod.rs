//! Insert test cases module
//!
//! ## Test Organization
//!
//! ### Core Insertion Tests
//! - `infix` - Infix/range matching behavior
//! - `pattern` - General pattern insertion
//! - `prefix` - Prefix matching
//! - `postfix` - Postfix matching
//!
//! ### Validation & Error Handling
//! - `validation` - Input validation (empty patterns, invalid InitInterval)
//!
//! ### Scenario Tests  
//! - `context_read_scenarios` - Tests simulating context-read failure cases
//! - `repeat_patterns` - Tests for repeated token patterns

// Core insertion tests
pub mod infix;
pub mod pattern;
pub mod postfix;
pub mod prefix;

// Validation & edge cases
pub mod validation;

// Scenario tests
pub mod context_read_scenarios;
pub mod repeat_patterns;
