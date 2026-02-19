/// Utilities for test logging and tracing with automatic cleanup
pub mod tracing_utils;

/// Formatting utilities for pretty-printing debug output in logs
pub mod format_utils;

/// Compact formatting for tracing logs
pub mod compact_format;

/// Compact formatting implementations for path types
pub mod path_format;

/// Typed debug wrapper that includes full type paths
pub mod typed_debug;

// Re-export commonly used items for convenience
pub use compact_format::{
    Compact,
    CompactFormat,
    format_mode,
    write_indent,
};
pub use format_utils::{
    PrettyDebug,
    pretty,
};
pub use tracing_utils::{
    TestTracing,
    TracingConfig,
};
pub use typed_debug::TypedDebug;
