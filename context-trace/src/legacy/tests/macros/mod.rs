//! Test macro collection for context-trace.
//!
//! This module provides a comprehensive set of macros for writing tests,
//! including pattern insertion, atom management, path construction, and
//! trace cache building.

// Re-export all macros from submodules
pub mod atoms;
pub mod paths;
pub mod patterns;
pub mod string_repr;
pub mod test_utils;
pub mod trace_cache;

// The macros are already exported from their respective files,
// so they're available when this module is imported.
