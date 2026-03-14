//! JQ-style query language for filtering and transforming JSON values.
//!
//! Provides a thin wrapper around the `jaq-*` crates. Feature-gated behind
//! the `jq` feature (enabled by default).
//!
//! When the `jq` feature is disabled, all public functions return
//! `Err(JqError::NotAvailable)`.
//!
//! # Example Queries
//!
//! ```text
//! # Filter by field value
//! select(.level == "ERROR")
//!
//! # Multiple conditions
//! select(.level == "ERROR" and .span_name == "my_function")
//!
//! # Search in text field
//! select(.message | contains("panic"))
//!
//! # Combine with regex
//! select(.message | test("error|panic"; "i"))
//!
//! # Filter by field existence
//! select(.fields.some_key != null)
//!
//! # Extract specific fields
//! {level, message, timestamp}
//!
//! # Filter by array element
//! select(.tags | any(. == "testing"))
//! ```

use serde_json::Value;
use std::fmt;

/// Error type for JQ operations.
#[derive(Debug)]
pub enum JqError {
    /// The JQ expression failed to parse or compile.
    CompileError(String),
    /// The `jq` feature is not enabled.
    NotAvailable,
}

impl fmt::Display for JqError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            JqError::CompileError(msg) =>
                write!(f, "JQ compile error: {}", msg),
            JqError::NotAvailable => {
                write!(
                    f,
                    "JQ support not compiled in (enable the 'jq' feature)"
                )
            },
        }
    }
}

impl std::error::Error for JqError {}

/// A compiled JQ filter ready for execution.
#[cfg(feature = "jq")]
pub struct JqFilter {
    #[allow(dead_code)]
    ctx: jaq_interpret::ParseCtx,
    filter: jaq_interpret::Filter,
}

/// Stub when `jq` feature is disabled.
#[cfg(not(feature = "jq"))]
pub struct JqFilter {
    _private: (),
}

#[cfg(feature = "jq")]
impl JqFilter {
    /// Compile a JQ filter string.
    pub fn compile(query: &str) -> Result<Self, JqError> {
        use jaq_interpret::ParseCtx;

        let mut ctx = ParseCtx::new(Vec::new());
        ctx.insert_natives(jaq_core::core());
        ctx.insert_defs(jaq_std::std());

        let parsed = jaq_syn::parse(query, |p| p.module(|p| p.term()));
        let main = parsed.ok_or_else(|| {
            JqError::CompileError(
                "Parse error: invalid jq expression".to_string(),
            )
        })?;

        let filter = main.conv(query);
        let filter = ctx.compile(filter);

        if !ctx.errs.is_empty() {
            let msg = ctx
                .errs
                .iter()
                .map(|(e, _span)| format!("{}", e))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(JqError::CompileError(msg));
        }

        Ok(Self { ctx, filter })
    }

    /// Run the filter on a JSON value, returning all outputs.
    pub fn run(
        &self,
        input: &Value,
    ) -> Vec<Result<Value, String>> {
        use jaq_interpret::{
            Ctx,
            FilterT,
            RcIter,
            Val,
        };

        let inputs = RcIter::new(std::iter::empty());
        let val = Val::from(input.clone());
        let ctx = Ctx::new([], &inputs);

        self.filter
            .run((ctx, val))
            .map(|result: Result<jaq_interpret::Val, jaq_interpret::Error>| {
                result.map(|v| Value::from(v)).map_err(|e| format!("{}", e))
            })
            .collect()
    }

    /// Run the filter on a value, returning true if any output is truthy.
    pub fn matches(
        &self,
        input: &Value,
    ) -> bool {
        let results = self.run(input);
        results.into_iter().any(|r| match r {
            Ok(Value::Bool(true)) => true,
            Ok(Value::Null) | Ok(Value::Bool(false)) => false,
            Ok(_) => true, // Non-null, non-false values are truthy
            Err(_) => false,
        })
    }
}

#[cfg(not(feature = "jq"))]
impl JqFilter {
    /// Compile a JQ filter string (stub — always returns `NotAvailable`).
    pub fn compile(_query: &str) -> Result<Self, JqError> {
        Err(JqError::NotAvailable)
    }

    /// Run the filter (stub).
    pub fn run(
        &self,
        _input: &Value,
    ) -> Vec<Result<Value, String>> {
        vec![]
    }

    /// Check match (stub).
    pub fn matches(
        &self,
        _input: &Value,
    ) -> bool {
        false
    }
}

/// Filter a collection of JSON values using a JQ query.
///
/// Returns only values for which the query produces a truthy result.
pub fn filter_values<'a>(
    values: impl IntoIterator<Item = &'a Value>,
    query: &str,
) -> Result<Vec<Value>, JqError> {
    let filter = JqFilter::compile(query)?;

    let mut results = Vec::new();
    for value in values {
        if filter.matches(value) {
            results.push(value.clone());
        }
    }

    Ok(results)
}

/// Transform values using a JQ query (can produce multiple/different outputs per input).
pub fn transform_values<'a>(
    values: impl IntoIterator<Item = &'a Value>,
    query: &str,
) -> Result<Vec<Value>, JqError> {
    let filter = JqFilter::compile(query)?;

    let mut results = Vec::new();
    for value in values {
        for result in filter.run(value) {
            if let Ok(v) = result {
                results.push(v);
            }
        }
    }

    Ok(results)
}

#[cfg(all(test, feature = "jq"))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compile_identity() {
        let filter = JqFilter::compile(".");
        assert!(filter.is_ok());
    }

    #[test]
    fn test_compile_invalid() {
        let filter = JqFilter::compile("[invalid");
        assert!(filter.is_err());
    }

    #[test]
    fn test_filter_select() {
        let filter = JqFilter::compile("select(.level == \"ERROR\")").unwrap();
        let error_entry = json!({"level": "ERROR", "message": "fail"});
        let info_entry = json!({"level": "INFO", "message": "ok"});

        assert!(filter.matches(&error_entry));
        assert!(!filter.matches(&info_entry));
    }

    #[test]
    fn test_filter_contains() {
        let filter =
            JqFilter::compile("select(.message | contains(\"panic\"))")
                .unwrap();

        let match_entry = json!({"message": "thread panic detected"});
        let no_match = json!({"message": "all good"});

        assert!(filter.matches(&match_entry));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_filter_case_insensitive() {
        let filter =
            JqFilter::compile("select(.message | test(\"error\"; \"i\"))")
                .unwrap();

        let upper = json!({"message": "Error occurred"});
        let lower = json!({"message": "error occurred"});
        let no_match = json!({"message": "all good"});

        assert!(filter.matches(&upper));
        assert!(filter.matches(&lower));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_filter_values_fn() {
        let entries = vec![
            json!({"level": "ERROR", "message": "fail"}),
            json!({"level": "INFO", "message": "ok"}),
            json!({"level": "ERROR", "message": "crash"}),
        ];

        let results =
            filter_values(entries.iter(), "select(.level == \"ERROR\")")
                .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["message"], "fail");
        assert_eq!(results[1]["message"], "crash");
    }

    #[test]
    fn test_transform_values_fn() {
        let entries = vec![
            json!({"level": "ERROR", "message": "fail", "ts": 1}),
            json!({"level": "INFO", "message": "ok", "ts": 2}),
        ];

        let results =
            transform_values(entries.iter(), "{level, message}").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], json!({"level": "ERROR", "message": "fail"}));
        assert_eq!(results[1], json!({"level": "INFO", "message": "ok"}));
    }

    #[test]
    fn test_filter_array_any() {
        let filter =
            JqFilter::compile("select(.tags | any(. == \"testing\"))").unwrap();

        let match_entry = json!({"tags": ["testing", "debug"]});
        let no_match = json!({"tags": ["production"]});

        assert!(filter.matches(&match_entry));
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn test_run_identity() {
        let filter = JqFilter::compile(".").unwrap();
        let input = json!({"a": 1, "b": 2});
        let results = filter.run(&input);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap(), &input);
    }
}

#[cfg(all(test, not(feature = "jq")))]
mod tests_no_jq {
    use super::*;

    #[test]
    fn test_compile_returns_not_available() {
        let result = JqFilter::compile(".");
        assert!(result.is_err());
        match result.unwrap_err() {
            JqError::NotAvailable => {},
            other => panic!("Expected NotAvailable, got: {}", other),
        }
    }

    #[test]
    fn test_filter_values_returns_not_available() {
        let values = vec![serde_json::json!({"a": 1})];
        let result = filter_values(values.iter(), ".");
        assert!(result.is_err());
    }
}
