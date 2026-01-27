//! Merge algorithms for joining split nodes.
//!
//! This module contains merge algorithms for both intermediary and root nodes:
//! - `NodeMergeCtx`: Merges intermediary nodes and creates split halves
//! - `RootMergeCtx`: Merges root nodes and extracts the target token
//! - `MergePartitionCtx`: Context for merging individual partitions
//! - `shared`: Common merge utilities used by both contexts

pub mod context;
pub mod partition;
mod partition_range;
mod range_map;
mod required;

pub mod node;

pub use partition::MergePartitionCtx;
pub use partition_range::PartitionRange;
pub use range_map::RangeMap;
pub use required::RequiredPartitions;
