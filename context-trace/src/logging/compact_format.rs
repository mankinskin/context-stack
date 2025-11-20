//! Compact formatting for tracing logs
//!
//! Provides wrapper types that format complex data structures in a readable way for logs.
//! Use these wrappers with tracing's field formatting to get nice output without cluttering
//! the logs with full Debug output.
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_trace::logging::compact_format::Compact;
//!
//! let path = RootedRangePath { /* ... */ };
//!
//! // Use % for multi-line indented format (recommended for complex types)
//! tracing::info!(path = %Compact(&path), "Processing path");
//!
//! // Use ? for single-line compact format (good for simple types)
//! tracing::debug!(result = ?Compact(&result), "Got result");
//! ```
//!
//! # Display (%) vs Debug (?)
//!
//! - `%Compact(value)` - Multi-line indented format (uses fmt_indented)
//! - `?Compact(value)` - Single-line compact format (uses fmt_compact)
//! - `?DebugFull(value)` - Original verbose Debug output
//!
//! # Custom Formatting
//!
//! Implement `CompactFormat` trait for types that need custom compact formatting:
//!
//! ```rust,ignore
//! impl CompactFormat for MyType {
//!     fn fmt_compact(&self, f: &mut fmt::Formatter) -> fmt::Result {
//!         write!(f, "MyType(id:{})", self.id)
//!     }
//!     
//!     fn fmt_indented(&self, f: &mut fmt::Formatter, indent: usize) -> fmt::Result {
//!         write_indent(f, indent)?;
//!         writeln!(f, "MyType {{")?;
//!         write_indent(f, indent + 1)?;
//!         writeln!(f, "id: {}", self.id)?;
//!         write_indent(f, indent)?;
//!         write!(f, "}}")
//!     }
//! }
//! ```

use std::fmt;

/// Global formatting mode control
pub mod format_mode {
    use std::sync::atomic::{
        AtomicBool,
        Ordering,
    };

    static USE_COMPACT: AtomicBool = AtomicBool::new(true);

    /// Enable compact formatting as default (enabled by default)
    pub fn enable_compact() {
        USE_COMPACT.store(true, Ordering::Relaxed);
    }

    /// Disable compact formatting (use full Debug everywhere)
    pub fn disable_compact() {
        USE_COMPACT.store(false, Ordering::Relaxed);
    }

    /// Check if compact formatting is enabled
    pub fn is_compact_enabled() -> bool {
        USE_COMPACT.load(Ordering::Relaxed)
    }
}

/// Trait for types that can be formatted compactly for logs
pub trait CompactFormat {
    /// Format in a single-line compact way (used with Display)
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result;

    /// Format with indentation (used with Debug)
    ///
    /// By default, formats with a newline at the start so that tracing
    /// field formatting puts the value on its own line
    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        indent: usize,
    ) -> fmt::Result;
}

/// Wrapper that provides compact formatting for logs
///
/// Use with `%` for Display (single-line) or `?` for Debug (multi-line indented)
pub struct Compact<'a, T: ?Sized>(pub &'a T);

impl<'a, T: CompactFormat + ?Sized> fmt::Display for Compact<'a, T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        // For Display (% specifier), use indented format for multi-line output
        // This works better with our field visitor
        self.0.fmt_indented(f, 0)
    }
}

impl<'a, T: CompactFormat + ?Sized> fmt::Debug for Compact<'a, T> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        // For Debug (? specifier), use compact single-line format
        self.0.fmt_compact(f)
    }
}

/// Helper to write indentation
pub fn write_indent(
    f: &mut fmt::Formatter,
    indent: usize,
) -> fmt::Result {
    write!(f, "{}", "  ".repeat(indent))
}

/// Macro to implement Display using CompactFormat::fmt_indented
/// This allows types to automatically use compact formatting with % in tracing
#[macro_export]
macro_rules! impl_display_via_compact {
    ($ty:ty) => {
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                use $crate::logging::compact_format::CompactFormat;
                self.fmt_indented(f, 0)
            }
        }
    };
    ($ty:ty where $($bounds:tt)*) => {
        impl<$($bounds)*> std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                use $crate::logging::compact_format::CompactFormat;
                self.fmt_indented(f, 0)
            }
        }
    };
}

// Convenience macros for logging with compact formatting
#[macro_export]
macro_rules! compact {
    ($val:expr) => {
        $crate::logging::compact_format::Compact(&$val)
    };
}

#[macro_export]
macro_rules! debug_full {
    ($val:expr) => {
        $crate::logging::compact_format::DebugFull(&$val)
    };
}
