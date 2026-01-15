//! Type-safe partition range for indexing partitions.
//!
//! This module provides a clear distinction between partition indices and offset indices.

use std::ops::Range;
use derive_more::{Deref, DerefMut};

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
pub struct PartitionRange(pub Range<usize>);

impl PartitionRange {
    pub fn new(range: Range<usize>) -> Self {
        Self(range)
    }

    pub fn start(&self) -> usize {
        self.0.start
    }

    pub fn end(&self) -> usize {
        self.0.end
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_range(&self) -> &Range<usize> {
        &self.0
    }

    pub fn into_range(self) -> Range<usize> {
        self.0
    }
}

impl From<Range<usize>> for PartitionRange {
    fn from(range: Range<usize>) -> Self {
        Self(range)
    }
}

impl From<PartitionRange> for Range<usize> {
    fn from(pr: PartitionRange) -> Self {
        pr.0
    }
}
