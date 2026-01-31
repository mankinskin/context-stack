//! Merge algorithms for joining split nodes.
//!
//! This module contains merge algorithms for both intermediary and root nodes:
//! - `MergeCtx`: Main context for merge operations
//! - `PartitionMergeIter`: Iterator context for partition merging
//! - `MergePartitionCtx`: Context for merging individual partitions
//! - `RangeMap`: Maps partition ranges to merged tokens

pub mod context;
mod iter;
pub mod partition;
mod partition_range;
mod range_map;
mod required;

pub mod node;

pub use iter::PartitionMergeIter;
pub use partition::MergePartitionCtx;
pub use partition_range::PartitionRange;
pub use range_map::RangeMap;
pub use required::RequiredPartitions;
