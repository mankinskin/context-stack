//! Error types for test validation

use std::fmt;

/// Errors that can occur during test validation.
#[derive(Debug)]
pub enum TestError {
    /// Search operation failed
    SearchFailed(String),

    /// Insert operation failed
    InsertFailed(String),

    /// Interval operation failed
    IntervalFailed(String),

    /// Response did not match expected value
    ResponseMismatch {
        test_case: &'static str,
        expected: String,
        actual: String,
    },

    /// Token did not match expected value
    TokenMismatch {
        test_case: &'static str,
        expected: String,
        actual: String,
    },

    /// Pattern structure did not match expected
    PatternMismatch {
        test_case: &'static str,
        details: String,
    },

    /// Trace cache did not match expected
    TraceCacheMismatch {
        test_case: &'static str,
        details: String,
    },

    /// Custom validation error
    ValidationFailed {
        test_case: &'static str,
        message: String,
    },
}

impl fmt::Display for TestError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            TestError::SearchFailed(msg) =>
                write!(f, "Search operation failed: {}", msg),
            TestError::InsertFailed(msg) =>
                write!(f, "Insert operation failed: {}", msg),
            TestError::IntervalFailed(msg) =>
                write!(f, "Interval operation failed: {}", msg),
            TestError::ResponseMismatch {
                test_case,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Response mismatch in test '{}'\nExpected:\n{}\n\nActual:\n{}",
                    test_case, expected, actual
                )
            },
            TestError::TokenMismatch {
                test_case,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Token mismatch in test '{}'\nExpected: {}\nActual: {}",
                    test_case, expected, actual
                )
            },
            TestError::PatternMismatch { test_case, details } => {
                write!(
                    f,
                    "Pattern mismatch in test '{}': {}",
                    test_case, details
                )
            },
            TestError::TraceCacheMismatch { test_case, details } => {
                write!(
                    f,
                    "Trace cache mismatch in test '{}': {}",
                    test_case, details
                )
            },
            TestError::ValidationFailed { test_case, message } => {
                write!(
                    f,
                    "Validation failed in test '{}': {}",
                    test_case, message
                )
            },
        }
    }
}

impl std::error::Error for TestError {}
