//! Declarative test case framework for end-to-end testing
//!
//! This module provides a flexible, type-safe way to define and organize test cases
//! across the entire context-engine workspace. Test cases can validate individual
//! operations (search, insert, interval) or entire operation chains.
//!
//! # Core Concepts
//!
//! - **TestEnv**: Pre-initialized graph state with all relevant tokens/patterns
//! - **TestCase**: Base trait for all test cases with metadata and organization
//! - **Operation-specific traits**: SearchTestCase, InsertTestCase, IntervalTestCase
//! - **TestRegistry**: Central registry for discovering and running tests
//!

pub mod environment;
pub mod error;

pub use environment::TestEnv;
pub use error::TestError;

/// Core trait for all test case definitions.
///
/// Provides metadata for organization, filtering, and documentation.
pub trait TestCase {
    /// The graph environment this test uses
    type Env: TestEnv;

    /// Unique identifier for this test case.
    fn name(&self) -> &'static str;
}
