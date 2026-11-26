//! Merge algorithms for joining split nodes.
//!
//! This module contains merge algorithms for both intermediary and root nodes:
//! - `MergeCtx`: Main context for merge operations
//! - `PartitionMergeIter`: Iterator context for partition merging
//! - `MergePartitionCtx`: Context for merging individual partitions
//! - `RangeMap`: Maps partition ranges to merged tokens

pub(crate) mod context;
mod iter;
pub(crate) mod partition;
mod partition_range;
mod range_map;
mod required;

pub(crate) mod node;

pub(crate) use iter::PartitionMergeIter;
pub(crate) use partition::MergePartitionCtx;
pub(crate) use partition_range::PartitionRange;
pub(crate) use range_map::RangeMap;
pub(crate) use required::RequiredPartitions;
