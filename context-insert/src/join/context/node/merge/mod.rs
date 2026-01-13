//! Merge algorithms for joining split nodes.
//!
//! This module contains merge algorithms for both intermediary and root nodes:
//! - `NodeMergeCtx`: Merges intermediary nodes and creates split halves
//! - `RootMergeCtx`: Merges root nodes and extracts the target token

mod range_map;

pub mod intermediary;
pub mod root;

pub use intermediary::NodeMergeCtx;
pub use range_map::RangeMap;
pub use root::RootMergeCtx;
