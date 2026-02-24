//! Typed debug wrapper that includes full type paths in Debug output
//!
//! When using tracing, the standard Debug trait outputs type names without
//! their full module paths (e.g., `MatchResult { ... }` instead of
//! `context_search::response::MatchResult { ... }`).
//!
//! This module provides a `typed!` macro that wraps values to include
//! their full type path from `std::any::type_name`.
//!
//! # Example
//!
//! ```ignore
//! use context_trace::logging::typed;
//!
//! let result = compute_match();
//! // Without typed: "MatchResult { path: ... }"
//! // With typed:    "context_search::response::MatchResult { path: ... }"
//! tracing::info!(end = ?typed!(result), "search complete");
//! ```

use std::fmt;

/// Wrapper that outputs the full type path before Debug output
pub struct TypedDebug<'a, T: fmt::Debug + ?Sized>(pub &'a T);

impl<T: fmt::Debug + ?Sized> fmt::Debug for TypedDebug<'_, T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let type_name = std::any::type_name::<T>();
        write!(f, "{} ", type_name)?;
        self.0.fmt(f)
    }
}

/// Wrap a value to include its full type path in Debug output
///
/// # Example
///
/// ```ignore
/// use context_trace::typed;
///
/// struct MyStruct { value: i32 }
///
/// let s = MyStruct { value: 42 };
/// // Outputs: "my_crate::MyStruct { value: 42 }"
/// println!("{:?}", typed!(s));
/// ```
#[macro_export]
macro_rules! typed {
    ($val:expr) => {
        $crate::logging::typed_debug::TypedDebug(&$val)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TypedDebug as TypedDebugDerive;

    #[derive(Debug)]
    struct TestStruct {
        value: i32,
    }

    #[test]
    fn test_typed_debug_includes_path() {
        let s = TestStruct { value: 42 };
        let output = format!("{:?}", TypedDebug(&s));

        // Should contain the full module path
        assert!(output.contains(
            "context_trace::logging::typed_debug::tests::TestStruct"
        ));
        // Should also contain the struct contents
        assert!(output.contains("value: 42"));
    }

    #[test]
    fn test_typed_macro() {
        let s = TestStruct { value: 123 };
        let output = format!("{:?}", typed!(s));

        assert!(output.contains("TestStruct"));
        assert!(output.contains("value: 123"));
    }

    // Tests for #[derive(TypedDebug)] macro

    #[derive(TypedDebugDerive)]
    struct DerivedStruct {
        name: String,
        count: usize,
    }

    #[test]
    fn test_derive_typed_debug_struct() {
        let s = DerivedStruct {
            name: "test".to_string(),
            count: 5,
        };
        let output = format!("{:?}", s);

        // Should contain full module path
        assert!(
            output.contains(
                "context_trace::logging::typed_debug::tests::DerivedStruct"
            ),
            "Expected full path in output: {}",
            output
        );
        // Should contain field values
        assert!(output.contains("name"));
        assert!(output.contains("test"));
        assert!(output.contains("count"));
        assert!(output.contains("5"));
    }

    #[derive(TypedDebugDerive)]
    struct DerivedTuple(i32, String);

    #[test]
    fn test_derive_typed_debug_tuple_struct() {
        let t = DerivedTuple(42, "hello".to_string());
        let output = format!("{:?}", t);

        assert!(
            output.contains(
                "context_trace::logging::typed_debug::tests::DerivedTuple"
            ),
            "Expected full path in output: {}",
            output
        );
        assert!(output.contains("42"));
        assert!(output.contains("hello"));
    }

    #[derive(TypedDebugDerive)]
    struct DerivedUnit;

    #[test]
    fn test_derive_typed_debug_unit_struct() {
        let u = DerivedUnit;
        let output = format!("{:?}", u);

        assert!(
            output.contains(
                "context_trace::logging::typed_debug::tests::DerivedUnit"
            ),
            "Expected full path in output: {}",
            output
        );
    }

    #[derive(TypedDebugDerive)]
    enum DerivedEnum {
        Unit,
        Tuple(i32),
        Struct { value: String },
    }

    #[test]
    fn test_derive_typed_debug_enum_unit() {
        let e = DerivedEnum::Unit;
        let output = format!("{:?}", e);

        assert!(
            output.contains(
                "context_trace::logging::typed_debug::tests::DerivedEnum::Unit"
            ),
            "Expected full path in output: {}",
            output
        );
    }

    #[test]
    fn test_derive_typed_debug_enum_tuple() {
        let e = DerivedEnum::Tuple(99);
        let output = format!("{:?}", e);

        assert!(output.contains("DerivedEnum"));
        assert!(output.contains("Tuple"));
        assert!(output.contains("99"));
    }

    #[test]
    fn test_derive_typed_debug_enum_struct() {
        let e = DerivedEnum::Struct {
            value: "data".to_string(),
        };
        let output = format!("{:?}", e);

        assert!(output.contains("DerivedEnum"));
        assert!(output.contains("Struct"));
        assert!(output.contains("value"));
        assert!(output.contains("data"));
    }
}
