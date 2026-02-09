//! Type-safe partition range for indexing partitions.
//!
//! This module provides a clear distinction between partition indices and offset indices.

use derive_more::{
    Deref,
    DerefMut,
};
use std::ops::{
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
pub(crate) struct PartitionRange(pub(crate) RangeInclusive<usize>);

impl PartitionRange {
    pub(crate) fn new(range: RangeInclusive<usize>) -> Self {
        Self(range)
    }

    //pub(crate) fn len(&self) -> usize {
    //    self.end() - self.start()
    //}

    //pub(crate) fn into_range(self) -> Range<usize> {
    //    *self.start()..(self.end() + 1)
    //}

    /// Compute the overlap of two partition ranges.
    ///
    /// Returns `Some(overlap)` if the two ranges intersect and the intersection
    /// is a proper subset of at least one of the ranges. Returns `None` if:
    /// - The ranges don't intersect, or
    /// - The intersection equals one of the input ranges (not a proper subset)
    ///
    /// This is used for computing inner partitions: when target and wrapper
    /// overlap, the overlapping portion becomes a required inner partition.
    pub(crate) fn overlap(&self, other: &Self) -> Option<Self> {
        let start = (*self.start()).max(*other.start());
        let end = (*self.end()).min(*other.end());

        // Must be non-empty
        if start <= end {
            let overlap = Self::new(start..=end);
            // Only return if it's a proper subset (not equal to either input)
            if overlap != *self && overlap != *other {
                return Some(overlap);
            }
        }
        None
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
