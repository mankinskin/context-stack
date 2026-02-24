//! Tracing setup utilities for tests
//!
//! Provides per-test logging to files with automatic cleanup on success.
//! Logs are written to `<target-dir>/test-logs/<test_name>.log` and deleted if the test passes.
//! The target directory is automatically detected from the Cargo build environment.
//!

// Internal modules
mod config;
mod debug_to_json;
mod field_visitor;
mod formatter;
mod graph_ref;
mod panic;
mod path;
mod span_fields;
mod special_fields;
mod syntax;
mod test_tracing;
mod timer;
mod writers;

// Re-export public API
pub use config::TracingConfig;
pub use span_fields::SpanFieldFormatter;
pub use test_tracing::TestTracing;

/// Convenience macro to initialize tracing for a test
///
/// # Examples
///
/// Basic usage:
/// ```no_run
/// use context_trace::init_test_tracing;
///
/// #[test]
/// fn my_test() {
///     let _tracing = init_test_tracing!();
///     // Your test code
/// }
/// ```
///
/// With a test graph (automatically registers and clears on drop):
/// ```no_run
/// use context_trace::{init_test_tracing, Hypergraph};
///
/// #[test]
/// fn my_test() {
///     let graph = Hypergraph::default();
///     // ... build graph ...
///     let _tracing = init_test_tracing!(graph);
///     // Tokens will now show string representations
/// }
/// ```
#[macro_export]
macro_rules! init_test_tracing {
    // No arguments - basic initialization
    () => {{
        // Extract test name from function
        let test_name = {
            fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            // Extract function name from the path
            name.strip_suffix("::f")
                .and_then(|s| s.split("::").last())
                .unwrap_or("unknown")
        };
        $crate::logging::tracing_utils::TestTracing::init(test_name)
    }};

    // With graph only
    ($graph:expr) => {{
        let test_name = {
            fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            name.strip_suffix("::f")
                .and_then(|s| s.split("::").last())
                .unwrap_or("unknown")
        };
        // Try to detect if this is a graph or config
        // If it has a .graph() method or is &Hypergraph, treat as graph
        $crate::init_test_tracing!(@detect test_name, $graph)
    }};

    // With graph and config
    ($graph:expr, $config:expr) => {{
        let test_name = {
            fn f() {}
            fn type_name_of<T>(_: T) -> &'static str {
                std::any::type_name::<T>()
            }
            let name = type_name_of(f);
            name.strip_suffix("::f")
                .and_then(|s| s.split("::").last())
                .unwrap_or("unknown")
        };
        #[cfg(any(test, feature = "test-api"))]
        {
            $crate::logging::tracing_utils::TestTracing::init_with_config_and_graph(
                test_name, $config, $graph
            )
        }
        #[cfg(not(any(test, feature = "test-api")))]
        {
            // Fallback - just use config if test_graph is not available
            $crate::logging::tracing_utils::TestTracing::init_with_config(
                test_name, $config
            )
        }
    }};

    // Internal rule to detect graph vs config
    (@detect $test_name:expr, $arg:expr) => {{
        // This is a bit of a hack, but we try to call init_with_graph
        // and if that fails to compile, the user should use the two-arg form
        #[cfg(any(test, feature = "test-api"))]
        {
            $crate::logging::tracing_utils::TestTracing::init_with_graph(
                $test_name, $arg
            )
        }
        #[cfg(not(any(test, feature = "test-api")))]
        {
            // Fallback to config-based init if test_graph is not available
            $crate::logging::tracing_utils::TestTracing::init_with_config(
                $test_name, $arg
            )
        }
    }};
}

/// Macro to create a tracing span with the function signature captured at compile time.
///
/// This macro wraps the standard `tracing::span!` macro and adds a `fn_sig` field
/// containing the function signature string.
///
/// # Usage
///
/// ```rust,ignore
/// use context_trace::span_with_sig;
///
/// fn my_function(x: i32, y: &str) -> Result<bool, Error> {
///     let _span = span_with_sig!(tracing::Level::INFO, "my_function");
///     // function body
/// }
/// ```
///
/// This will create a span with a field `fn_sig` containing the function signature.
#[macro_export]
macro_rules! span_with_sig {
    ($level:expr, $name:expr, $($fields:tt)*) => {{
        // Capture the function signature by stringifying the surrounding context
        // This is a compile-time operation
        let fn_sig = concat!(
            module_path!(),
            "::",
            $name,
            " @ ",
            file!(),
            ":",
            line!()
        );
        tracing::span!($level, $name, fn_sig = %fn_sig, $($fields)*)
    }};
    ($level:expr, $name:expr) => {{
        let fn_sig = concat!(
            module_path!(),
            "::",
            $name,
            " @ ",
            file!(),
            ":",
            line!()
        );
        tracing::span!($level, $name, fn_sig = %fn_sig)
    }};
}

/// Macro to wrap `#[instrument]` and automatically add function signature.
///
/// This macro generates the function signature string and adds it to the span fields.
///
/// # Usage
///
/// Instead of:
/// ```rust,ignore
/// #[instrument]
/// fn my_function(x: i32, y: &str) -> bool { }
/// ```
///
/// Use:
/// ```rust,ignore
/// #[instrument(fields(fn_sig = %fn_signature!("fn my_function(x: i32, y: &str) -> bool")))]
/// fn my_function(x: i32, y: &str) -> bool { }
/// ```
///
/// Or more simply, provide it as a string:
/// ```rust,ignore
/// #[instrument(fields(fn_sig = "fn my_function(x: i32, y: &str) -> bool"))]
/// fn my_function(x: i32, y: &str) -> bool { }
/// ```
#[macro_export]
macro_rules! fn_signature {
    ($sig:expr) => {
        $sig
    };
}
