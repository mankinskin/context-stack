//! Tests for path mutation operations
//!
//! This module contains comprehensive tests for path mutators including:
//! - move_root_index: moving the root index within a pattern
//! - move_leaf: moving the leaf (sub_index) within a child location
//! - move_path: moving through the path hierarchy
//! - move_key: moving atom positions
//! - path_append: appending child locations
//! - path_raise: raising paths to parent level
//! - path_operations: combined operations with TraceCtx
//!
//! These operations are critical for search and traversal algorithms.

pub mod move_key;
pub mod move_leaf;
pub mod move_root_index;
pub mod path_append;
pub mod path_operations;
