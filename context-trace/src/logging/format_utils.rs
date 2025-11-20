//! Formatting utilities for better log output with indentation

use std::fmt;

/// Format a value with pretty Debug output (multi-line with indentation)
///
/// # Example
/// ```
/// use context_trace::logging::format_utils::pretty;
///
/// let data = vec![1, 2, 3];
/// tracing::debug!(data = %pretty(&data), "Processing");
/// ```
pub fn pretty<T: fmt::Debug>(value: &T) -> PrettyDebug<'_, T> {
    PrettyDebug(value)
}

/// Wrapper that formats using pretty Debug (multi-line)
pub struct PrettyDebug<'a, T: ?Sized>(&'a T);

impl<T: fmt::Debug + ?Sized> fmt::Display for PrettyDebug<'_, T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        // Use Debug formatting with alternate flag for pretty-printing
        write!(f, "{:#?}", self.0)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for PrettyDebug<'_, T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        // Use Debug formatting with alternate flag for pretty-printing
        write!(f, "{:#?}", self.0)
    }
}

/// Macro to log with pretty-printed values
///
/// # Example
/// ```no_run
/// use context_trace::pretty_log;
///
/// let tokens = vec![1, 2, 3];
/// pretty_log!(debug, tokens, "Processing tokens");
/// ```
#[macro_export]
macro_rules! pretty_log {
    ($level:ident, $value:expr, $msg:literal) => {{
        use $crate::logging::format_utils::pretty;
        tracing::$level!(value = %pretty(&$value), $msg);
    }};
    ($level:ident, $($field:ident = $value:expr),+ $(,)?, $msg:literal) => {{
        use $crate::logging::format_utils::pretty;
        tracing::$level!(
            $($field = %pretty(&$value)),+,
            $msg
        );
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pretty_debug() {
        #[derive(Debug)]
        #[allow(unused)]
        struct TestStruct {
            field1: i32,
            field2: Vec<String>,
        }

        let test = TestStruct {
            field1: 42,
            field2: vec!["hello".to_string(), "world".to_string()],
        };

        let formatted = format!("{}", pretty(&test));

        // Should contain newlines and indentation
        assert!(formatted.contains('\n'));
        assert!(formatted.contains("field1"));
        assert!(formatted.contains("field2"));
    }
}
