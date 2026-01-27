//! Required partition tracking for selective merge.
//!
//! This module provides the `RequiredPartitions` type which tracks which
//! partition ranges require token creation during the merge phase.
//!
//! ## Required Partition Types
//!
//! A partition is required if:
//! 1. **Target:** The partition for the token being inserted
//! 2. **Wrapper:** Extends target to aligned pattern boundary (for unperfect splits)
//! 3. **Inner:** The sequence around an unperfect split that would repeat
//! 4. **Overlap:** Intersection of two required partitions
//!
//! ## Example
//!
//! For inserting `aby` into `xxabyzw` with pattern `[x, x, a, b, yz, w]`:
//! - Target: `1..=3` (aby)
//! - Wrapper: `1..=4` (abyz) 
//! - Overlap: `1..=2` (ab) ← intersection of target and wrapper
//!
//! NOT required: `2..=3` (by), `2..=4` (byz) - just covered, not overlaps

use super::PartitionRange;
use std::collections::HashSet;
use tracing::debug;

/// Tracks which partition ranges require token creation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredPartitions {
    required: HashSet<PartitionRange>,
}

impl RequiredPartitions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a partition range as required.
    pub fn add(&mut self, range: PartitionRange) {
        debug!(?range, "RequiredPartitions: adding");
        self.required.insert(range);
    }

    /// Check if a partition range is required.
    pub fn is_required(&self, range: &PartitionRange) -> bool {
        self.required.contains(range)
    }

    /// Compute all overlaps between currently required partitions
    /// and add them to the required set. Repeats until fixed point.
    ///
    /// This handles inner partitions: when target `aby` (1..=3) overlaps with
    /// wrapper `abyz` (1..=4), the overlap `ab` (1..=2) becomes required.
    pub fn close_under_overlaps(&mut self) {
        loop {
            let current: Vec<_> = self.required.iter().cloned().collect();
            let before = self.required.len();

            for i in 0..current.len() {
                for j in (i + 1)..current.len() {
                    if let Some(overlap) = current[i].overlap(&current[j]) {
                        debug!(
                            a = ?current[i],
                            b = ?current[j],
                            ?overlap,
                            "RequiredPartitions: adding overlap"
                        );
                        self.required.insert(overlap);
                    }
                }
            }

            if self.required.len() == before {
                break;
            }
        }
    }

    /// Add all sub-ranges of currently required partitions.
    ///
    /// For each required partition like `1..=3`, adds all sub-ranges:
    /// - `1..=1`, `1..=2`, `2..=2`, `2..=3`, `3..=3`
    /// 
    /// These are needed as components for building 2-way merge patterns.
    /// For example, to build `aby` (1..=3) with pattern `[ab, y]`:
    /// - Need `ab` (1..=2) as left component
    /// - Need `y` (3..=3) as right component
    pub fn close_under_subranges(&mut self) {
        loop {
            let current: Vec<_> = self.required.iter().cloned().collect();
            let before = self.required.len();

            for range in &current {
                let start = *range.start();
                let end = *range.end();
                
                // Add all sub-ranges of length > 0 (multi-partition ranges)
                // Single partitions (start == end) don't need sub-ranges
                if end > start {
                    for sub_start in start..=end {
                        for sub_end in sub_start..=end {
                            // Skip the range itself
                            if sub_start == start && sub_end == end {
                                continue;
                            }
                            // Skip single-partition ranges (they're base cases from range_map init)
                            if sub_start == sub_end {
                                continue;
                            }
                            let sub_range = PartitionRange::new(sub_start..=sub_end);
                            if !self.required.contains(&sub_range) {
                                debug!(
                                    parent = ?range,
                                    ?sub_range,
                                    "RequiredPartitions: adding sub-range"
                                );
                                self.required.insert(sub_range);
                            }
                        }
                    }
                }
            }

            if self.required.len() == before {
                break;
            }
        }
    }

    /// Iterate over all required partition ranges.
    pub fn iter(&self) -> impl Iterator<Item = &PartitionRange> {
        self.required.iter()
    }

    /// Number of required partitions.
    pub fn len(&self) -> usize {
        self.required.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.required.is_empty()
    }
}
