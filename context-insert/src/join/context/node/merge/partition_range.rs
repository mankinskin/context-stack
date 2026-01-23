//! Type-safe partition range for indexing partitions.
//!
//! This module provides a clear distinction between partition indices and offset indices.

use derive_more::{
    Deref,
    DerefMut,
};
use std::ops::{
    Range,
    RangeInclusive,
};

/// A range of partition indices.
///
/// This is used to index into the partitions array, NOT the offsets array.
/// For example, `PartitionRange(1..3)` refers to partitions at indices 1 and 2 in the partitions array.
///
/// ## Example
///
/// With 3 offsets creating 4 potential partitions:
/// - Partition 0: before offset 0 (prefix)
/// - Partition 1: between offset 0 and 1 (infix)
/// - Partition 2: between offset 1 and 2 (infix)
/// - Partition 3: after offset 2 (postfix)
///
/// `PartitionRange(1..3)` refers to partitions 1 and 2, which span tokens between offsets 0 and 2.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct PartitionRange(pub RangeInclusive<usize>);

impl PartitionRange {
    pub fn new(range: RangeInclusive<usize>) -> Self {
        Self(range)
    }

    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    pub fn into_range(self) -> Range<usize> {
        *self.start()..(self.end() + 1)
    }
}

impl From<usize> for PartitionRange {
    fn from(i: usize) -> Self {
        Self(i..=i)
    }
}
impl From<RangeInclusive<usize>> for PartitionRange {
    fn from(range: RangeInclusive<usize>) -> Self {
        Self(range)
    }
}

impl From<PartitionRange> for RangeInclusive<usize> {
    fn from(pr: PartitionRange) -> Self {
        pr.0
    }
}
