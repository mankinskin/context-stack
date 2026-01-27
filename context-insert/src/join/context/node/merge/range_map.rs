//! Shared RangeMap implementation for partition merging.
//!
//! Used by both intermediary and root merge contexts to track merged partitions
//! by partition index range.

use std::{
    borrow::Borrow,
    collections::HashMap,
    num::NonZeroUsize,
};

use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};

use super::partition_range::PartitionRange;
use crate::split::{
    Split,
    cache::position::PosKey,
};

/// RangeMap for tracking merged partitions by partition index range.
///
/// **Important:** The range key represents **partition indices**, NOT offset indices.
/// - Partition indices refer to positions in the partitions array
/// - Each partition represents the tokens between consecutive offsets
///
/// ## Example
///
/// With 3 offsets creating 3 partitions [a, b, cd]:
/// - `PartitionRange(0..1)` → partition 0 → "a"
/// - `PartitionRange(1..2)` → partition 1 → "b"
/// - `PartitionRange(0..2)` → partitions 0+1 → "ab"
/// - `PartitionRange(1..3)` → partitions 1+2 → merged token from "b" and "cd"
#[derive(Debug, Default, Deref, DerefMut)]
pub struct RangeMap {
    #[deref]
    #[deref_mut]
    pub map: HashMap<PartitionRange, Token>,
}

impl<C: Borrow<Token>, I: IntoIterator<Item = C>> From<I> for RangeMap {
    fn from(iter: I) -> Self {
        let mut map = HashMap::default();
        for (i, part) in iter.into_iter().enumerate() {
            // Each initial partition occupies partition range i..(i+1)
            // This represents a single partition at index i in the partitions array
            map.insert(i.into(), *part.borrow());
        }
        Self { map }
    }
}

impl RangeMap {
    /// Get all 2-way merge combinations for a partition range.
    ///
    /// Iterates over interior split points to generate all possible binary splits
    /// where BOTH components exist in the range_map.
    /// 
    /// For example, PartitionRange(0..=2) produces splits at points 1 and 2:
    /// - `(0..=0) + (1..=2)` - only if both exist in range_map
    /// - `(0..=1) + (2..=2)` - only if both exist in range_map
    ///
    /// With selective partition merge, some sub-ranges may not exist in the
    /// range_map because they were skipped as non-required. This method
    /// gracefully handles missing sub-ranges by filtering them out.
    pub fn range_sub_merges(
        &self,
        range: &PartitionRange,
    ) -> impl IntoIterator<Item = Pattern> + '_ {
        let (start, end) = (*range.start(), *range.end());
        // Iterate interior split points (start+1..=end)
        // For range 0..=2, this gives [1, 2] producing splits:
        // - (0..=0) + (1..=2)
        // - (0..=1) + (2..=2)
        // For range 0..=1, this gives [1] producing:
        // - (0..=0) + (1..=1)
        // For single-partition ranges like 0..=0, this gives [] (empty)
        (start + 1..=end).filter_map(move |ri| {
            let left_range = PartitionRange::new(start..=(ri - 1));
            let right_range = PartitionRange::new(ri..=end);
            
            // Only yield pattern if BOTH components exist in range_map
            let left = self.map.get(&left_range)?;
            let right = self.map.get(&right_range)?;
            
            Some(Pattern::from(vec![*left, *right]))
        })
    }

    /// Compute splits for a newly merged token based on the partition widths.
    ///
    /// For a merged token covering `range`, this computes the internal split positions
    /// by accumulating the widths of the sub-partitions. Each split position corresponds
    /// to a boundary between partitions.
    ///
    /// Additionally, for each constituent token, inherits its internal splits translated
    /// to the merged token's coordinate space.
    ///
    /// Returns a vector of (PosKey, Split) pairs that should be added to the SplitMap.
    pub fn compute_splits_for_merged_token(
        &self,
        merged_token: Token,
        range: &PartitionRange,
        existing_splits: &crate::split::SplitMap,
    ) -> Vec<(PosKey, Split)> {
        let (start, end) = (*range.start(), *range.end());
        
        // Single-partition ranges have no internal splits at partition boundaries
        // but may have inherited splits from constituent tokens
        
        let mut splits = Vec::new();
        let mut accumulated_width = 0usize;

        // For each partition in the range, collect its contribution
        for partition_idx in start..=end {
            let partition_range = PartitionRange::new(partition_idx..=partition_idx);
            let partition_token = *self.map.get(&partition_range).unwrap_or_else(|| {
                panic!(
                    "Partition {:?} not found in range_map. Available: {:?}",
                    partition_range,
                    self.map.keys().collect::<Vec<_>>()
                )
            });

            // Inherit splits from this constituent token, translating positions
            let partition_width = *partition_token.width();
            for (key, split) in existing_splits.iter() {
                if key.index == partition_token {
                    // Translate position: add accumulated width to get position in merged token
                    let translated_pos = accumulated_width + key.pos.get();
                    if let Some(pos) = NonZeroUsize::new(translated_pos) {
                        let new_key = PosKey::new(merged_token, pos);
                        splits.push((new_key, split.clone()));
                    }
                }
            }

            // After processing this partition, if there's a next partition,
            // add a boundary split (only if both left and right sub-ranges exist)
            if partition_idx < end {
                accumulated_width += partition_width;
                
                // Get left and right tokens for this boundary
                // If either sub-range was skipped (not required), we skip this split
                let left_range = PartitionRange::new(start..=partition_idx);
                let right_range = PartitionRange::new((partition_idx + 1)..=end);
                
                if let (Some(&left_token), Some(&right_token)) = 
                    (self.map.get(&left_range), self.map.get(&right_range)) 
                {
                    if let Some(pos) = NonZeroUsize::new(accumulated_width) {
                        let key = PosKey::new(merged_token, pos);
                        let split = Split::new(left_token, right_token);
                        splits.push((key, split));
                    }
                }
            }
        }

        splits
    }
}
