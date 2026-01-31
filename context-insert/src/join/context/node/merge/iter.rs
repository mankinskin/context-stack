//! Partition merge iteration context.
//!
//! This module provides `PartitionMergeIter`, a context for iterating over
//! and merging partitions within a `MergeCtx`. It encapsulates the state
//! needed during partition iteration, including the range map and target tracking.

use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
};

use context_trace::{
    Pattern,
    Token,
    VertexSet,
};
use tracing::debug;

use super::{
    MergePartitionCtx,
    PartitionRange,
    RangeMap,
    context::{
        MergeCtx,
        MergeMode,
        PartitionType,
    },
    partition::MergeResult,
};
use crate::{
    PatternSubDeltas,
    RootMode,
    interval::partition::info::range::role::{
        In,
        Post,
        Pre,
    },
    join::partition::Join,
};

/// Context for iterating over partitions and merging them.
///
/// This struct manages the state during partition iteration, including:
/// - The range map of already-merged partitions
/// - The target token being tracked (for root merges)
/// - The operating range being processed
pub struct PartitionMergeIter<'a, 'b> {
    /// The underlying merge context
    ctx: &'a mut MergeCtx<'b>,
    /// Map of merged partition ranges to their tokens
    range_map: RangeMap,
    /// The target range we're looking for (None for intermediary nodes)
    target_range: Option<PartitionRange>,
    /// The target token once found
    target_token: Option<Token>,
    /// The operating range for this merge
    operating_range: PartitionRange,
    /// Whether a perfect pattern replacement occurred during operating range merge.
    /// When true, `add_root_pattern` should be skipped because the pattern was
    /// already modified in place by `replace_in_pattern`.
    had_perfect_replacement: bool,
}

impl<'a, 'b> PartitionMergeIter<'a, 'b> {
    /// Create a new partition merge iterator.
    pub fn new(
        ctx: &'a mut MergeCtx<'b>,
        target_range: Option<PartitionRange>,
    ) -> Self {
        let operating_range = ctx.operating_partition_range();
        Self {
            ctx,
            range_map: RangeMap::default(),
            target_range,
            target_token: None,
            operating_range,
            had_perfect_replacement: false,
        }
    }

    /// Check if this is a root merge (has a target range).
    fn is_root_merge(&self) -> bool {
        self.target_range.is_some()
    }

    /// Get the required partitions filter (only for root merges).
    fn required(&self) -> Option<&super::RequiredPartitions> {
        if self.is_root_merge() {
            Some(&self.ctx.ctx.ctx.interval.required)
        } else {
            None
        }
    }

    /// Merge all partitions in the operating range.
    ///
    /// This is the main iteration loop that processes partitions from smallest
    /// to largest, building up the range map.
    pub fn merge_all(&mut self) {
        let op_start = *self.operating_range.start();
        let op_end = *self.operating_range.end();
        let op_len = op_end - op_start + 1;

        debug!(
            node=?self.ctx.ctx.index,
            patterns=?self.ctx.ctx.patterns(),
            offsets=?self.ctx.offsets,
            operating_range=?self.operating_range,
            mode=?self.ctx.mode,
            target_range=?self.target_range,
            is_root_merge=self.is_root_merge(),
            "PartitionMergeIter::merge_all: starting"
        );

        // For root merges, populate edge partitions (outside operating range)
        // These are needed for add_root_pattern to construct the complete pattern
        self.populate_edge_partitions();

        // Iterate over partition ranges by increasing length
        for len in 1..=op_len {
            debug!(
                "
    ==============================================================
    merging partitions of length {}
    ==============================================================",
                len
            );

            for start in op_start..=(op_start + op_len - len) {
                let end = start + len - 1;
                let partition_range = PartitionRange::new(start..=end);

                // Check if this partition should be processed
                if !self.should_process_partition(&partition_range, len) {
                    debug!(?partition_range, "Skipping non-required partition");
                    continue;
                }

                self.merge_partition(partition_range);
            }
        }
    }

    /// Check if a partition should be processed.
    fn should_process_partition(
        &self,
        range: &PartitionRange,
        len: usize,
    ) -> bool {
        // Single partitions (len == 1) are always needed as base cases
        let is_single = len == 1;
        if is_single {
            return true;
        }

        // For non-single partitions in root merge, check required set
        if let Some(req) = self.required() {
            req.is_required(range)
        } else {
            true
        }
    }

    /// Populate edge partitions that are outside the operating range.
    ///
    /// For Root modes, some partitions are excluded from the operating range:
    /// - Postfix: partition 0 (prefix) is outside
    /// - Prefix: partition num_offsets (postfix) is outside
    /// - Infix: both partition 0 and num_offsets are outside
    ///
    /// IMPORTANT: Edge partitions are merged using `merge_token_only` which
    /// does NOT modify the root pattern. This avoids corrupting the existing
    /// pattern structure. The tokens are only needed for `add_root_pattern`.
    fn populate_edge_partitions(&mut self) {
        let num_offsets = self.ctx.offsets.len();
        let op_start = *self.operating_range.start();
        let op_end = *self.operating_range.end();

        // Check if there's a prefix edge partition (partition 0 is outside operating range)
        if op_start > 0 {
            let prefix_range = PartitionRange::from(0);
            debug!(
                ?prefix_range,
                "Merging edge partition (prefix) - token only, no pattern modification"
            );
            let prefix_token = MergePartitionCtx::<Pre<Join>>::from_merge_ctx(
                self.ctx,
                &self.range_map,
                prefix_range.clone(),
            )
            .merge_token_only();
            self.range_map.insert(prefix_range, prefix_token);
        }

        // Check if there's a postfix edge partition (last partition is outside operating range)
        if op_end < num_offsets {
            let postfix_range = PartitionRange::from(num_offsets);
            debug!(
                ?postfix_range,
                "Merging edge partition (postfix) - token only, no pattern modification"
            );
            let postfix_token =
                MergePartitionCtx::<Post<Join>>::from_merge_ctx(
                    self.ctx,
                    &self.range_map,
                    postfix_range.clone(),
                )
                .merge_token_only();
            self.range_map.insert(postfix_range, postfix_token);
        }
    }

    /// Merge a single partition range.
    fn merge_partition(
        &mut self,
        partition_range: PartitionRange,
    ) {
        debug!(
            node=?self.ctx.ctx.index,
            ?partition_range,
            operating_range=?self.operating_range,
            mode=?self.ctx.mode,
            "Merging partition range"
        );

        // Determine partition type and whether this is the full operating range
        let partition_type = self.ctx.partition_type(&partition_range);
        let is_full_operating_range = partition_range == self.operating_range;

        debug!(
            ?partition_type,
            ?is_full_operating_range,
            "Detected partition type"
        );

        // Perform the merge based on partition type
        let result = self.merge_by_type(
            &partition_range,
            partition_type,
            is_full_operating_range,
        );

        // Track if a perfect replacement happened during the operating range merge
        if is_full_operating_range && result.had_pattern_replacement {
            self.had_perfect_replacement = true;
        }

        // Apply deltas to offset cache if needed
        self.apply_deltas(&partition_range, result.delta.as_ref());

        // Track target token if we've reached the target range
        self.track_target_token(&partition_range, result.token);

        // Insert into range map
        debug!(
            ?partition_range,
            merged_token=?result.token,
            "RangeMap INSERT: inserting token for range"
        );
        self.range_map.insert(partition_range.clone(), result.token);

        // Compute and store splits for merged tokens
        self.compute_splits(&partition_range, result.token);
    }

    /// Merge a partition based on its type.
    fn merge_by_type(
        &mut self,
        partition_range: &PartitionRange,
        partition_type: PartitionType,
        is_full_operating_range: bool,
    ) -> MergeResult {
        match (is_full_operating_range, partition_type) {
            // Full operating range for intermediary node - use the node itself
            (true, _) if matches!(self.ctx.mode, MergeMode::Full) =>
                self.merge_full_intermediary(partition_range),
            // All other cases: use partition_type to determine merge role
            (_, PartitionType::Full) => {
                // Full node partition - shouldn't happen in normal flow
                MergeResult {
                    token: self.ctx.ctx.index,
                    delta: None,
                    had_pattern_replacement: false,
                }
            },
            (_, PartitionType::Prefix) =>
                MergePartitionCtx::<Pre<Join>>::from_merge_ctx(
                    self.ctx,
                    &self.range_map,
                    partition_range.clone(),
                )
                .merge_with_info(),
            (_, PartitionType::Postfix) =>
                MergePartitionCtx::<Post<Join>>::from_merge_ctx(
                    self.ctx,
                    &self.range_map,
                    partition_range.clone(),
                )
                .merge_with_info(),
            (_, PartitionType::Infix) =>
                MergePartitionCtx::<In<Join>>::from_merge_ctx(
                    self.ctx,
                    &self.range_map,
                    partition_range.clone(),
                )
                .merge_with_info(),
        }
    }

    /// Merge a full intermediary node partition.
    ///
    /// For intermediary nodes at their full operating range, we use the node
    /// itself and add any new sub-merge patterns.
    fn merge_full_intermediary(
        &mut self,
        partition_range: &PartitionRange,
    ) -> MergeResult {
        debug!(
            "Merging full partition - adding sub-merge patterns (intermediary node)"
        );

        let token = self.ctx.ctx.index;
        let existing_patterns = self
            .ctx
            .ctx
            .trav
            .expect_vertex_data(token)
            .child_pattern_set();

        let sub_merges: Vec<_> = self
            .range_map
            .range_sub_merges(partition_range)
            .into_iter()
            .filter(|p| !existing_patterns.contains(p))
            .collect();

        if !sub_merges.is_empty() {
            debug!(
                num_sub_merges = sub_merges.len(),
                ?sub_merges,
                "Adding sub-merge patterns to full token"
            );
            for merge_pattern in sub_merges {
                self.ctx
                    .ctx
                    .trav
                    .add_pattern_with_update(token, merge_pattern);
            }
        }

        MergeResult {
            token,
            delta: None,
            had_pattern_replacement: false,
        }
    }

    /// Apply deltas to offset cache after a partition merge.
    fn apply_deltas(
        &mut self,
        partition_range: &PartitionRange,
        delta: Option<&PatternSubDeltas>,
    ) {
        let Some(deltas) = delta else { return };
        if !deltas.iter().any(|(_, &d)| d > 0) {
            return;
        }

        let start = *partition_range.start();
        let end = *partition_range.end();

        debug!(
            ?deltas,
            partition_start = start,
            partition_end = end,
            "Applying deltas to offset cache"
        );

        // Compute inner_offsets for positions inside the merged region
        let inner_offsets: BTreeMap<usize, NonZeroUsize> = (start..end)
            .filter_map(|partition_idx| {
                let mut cumulative_width = 0usize;
                for p in (*partition_range.start())..=partition_idx {
                    if let Some(token) =
                        self.range_map.get(&PartitionRange::from(p))
                    {
                        cumulative_width += *token.width;
                    }
                }
                NonZeroUsize::new(cumulative_width).map(|o| (partition_idx, o))
            })
            .collect();

        self.ctx.offsets.apply_deltas_with_inner_offsets(
            deltas,
            start,
            end,
            &inner_offsets,
        );
    }

    /// Track the target token when we reach the target range.
    fn track_target_token(
        &mut self,
        partition_range: &PartitionRange,
        token: Token,
    ) {
        if let Some(ref target_range) = self.target_range {
            if partition_range == target_range {
                debug!(
                    ?partition_range,
                    "merge_partitions_in_range: reached target partition range"
                );
                assert!(
                    self.target_token.is_none(),
                    "Target token already set"
                );
                self.target_token = Some(token);
            }
        }
    }

    /// Compute and store splits for a merged token.
    fn compute_splits(
        &mut self,
        partition_range: &PartitionRange,
        merged_token: Token,
    ) {
        if partition_range.is_empty() {
            return;
        }

        let computed_splits = self.range_map.compute_splits_for_merged_token(
            merged_token,
            partition_range,
            self.ctx.ctx.splits,
        );

        debug!(
            ?merged_token,
            ?partition_range,
            num_splits = computed_splits.len(),
            "Computed splits for merged token"
        );

        for (key, split) in computed_splits {
            self.ctx.add_split(key, split);
        }
    }

    /// Finalize the merge iteration and return results.
    ///
    /// Returns the target token, the completed range map, and whether
    /// a perfect pattern replacement occurred.
    pub fn finalize(self) -> MergeIterResult {
        let target_token = match self.target_range {
            Some(ref target_range) => self.target_token.unwrap_or_else(|| {
                panic!(
                    "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                    target_range,
                    self.range_map.map.keys().collect::<Vec<_>>()
                )
            }),
            None => {
                // For intermediary nodes, the "target" is the full node
                self.ctx.ctx.index
            }
        };

        MergeIterResult {
            target_token,
            range_map: self.range_map,
            had_perfect_replacement: self.had_perfect_replacement,
        }
    }
}

/// Result of a partition merge iteration.
pub struct MergeIterResult {
    /// The target token that was merged
    pub target_token: Token,
    /// Map of all merged partition ranges to their tokens
    pub range_map: RangeMap,
    /// Whether a perfect pattern replacement occurred during the operating range merge
    pub had_perfect_replacement: bool,
}

/// Extension methods for MergeCtx to add root patterns.
impl<'a> MergeCtx<'a> {
    /// Add a root pattern that includes the merged operating range token.
    ///
    /// For Root merge modes, this adds a new pattern to the root node that
    /// decomposes into [prefix..., operating_token, ...postfix] based on the mode:
    /// - Prefix: [operating_token, postfix]
    /// - Postfix: [prefix, operating_token]  
    /// - Infix: [prefix, operating_token, postfix]
    ///
    /// Note: The operating_token is the result of merging the full operating range,
    /// NOT just the target_range. For example, for Postfix mode:
    /// - target_range might be 2..=2 (just the new token "bcd")
    /// - operating_range is 1..=2 (includes wrapper, producing "abcd")
    /// - The root pattern uses the operating_range result: [prefix, abcd]
    pub fn add_root_pattern(
        &mut self,
        range_map: &RangeMap,
        _target_token: Token,
    ) {
        let root = self.ctx.index;
        let num_offsets = self.offsets.len();

        // Get the operating range - this is what we merged and what goes in the pattern
        let operating_range = self.operating_partition_range();
        let operating_token = range_map.get(&operating_range).unwrap_or_else(|| {
            panic!(
                "Operating range {:?} not found in range_map. Available: {:?}",
                operating_range,
                range_map.map.keys().collect::<Vec<_>>()
            )
        });

        // Build the pattern based on root mode
        let pattern = match self.mode {
            MergeMode::Root(RootMode::Prefix) => {
                // Pattern: [operating_token, postfix]
                // Postfix is the last partition (after all offsets)
                let postfix_range = PartitionRange::from(num_offsets);
                let postfix =
                    range_map.get(&postfix_range).unwrap_or_else(|| {
                        panic!(
                            "Postfix partition {:?} not found. Available: {:?}",
                            postfix_range,
                            range_map.map.keys().collect::<Vec<_>>()
                        )
                    });
                Pattern::from(vec![*operating_token, *postfix])
            },
            MergeMode::Root(RootMode::Postfix) => {
                // Pattern: [prefix, operating_token]
                // Prefix is the first partition (before first offset)
                let prefix_range = PartitionRange::from(0);
                let prefix =
                    range_map.get(&prefix_range).unwrap_or_else(|| {
                        panic!(
                            "Prefix partition {:?} not found. Available: {:?}",
                            prefix_range,
                            range_map.map.keys().collect::<Vec<_>>()
                        )
                    });
                Pattern::from(vec![*prefix, *operating_token])
            },
            MergeMode::Root(RootMode::Infix) => {
                // Pattern: [prefix, operating_token, postfix]
                let prefix_range = PartitionRange::from(0);
                let postfix_range = PartitionRange::from(num_offsets);

                let prefix =
                    range_map.get(&prefix_range).unwrap_or_else(|| {
                        panic!(
                            "Prefix partition {:?} not found. Available: {:?}",
                            prefix_range,
                            range_map.map.keys().collect::<Vec<_>>()
                        )
                    });
                let postfix =
                    range_map.get(&postfix_range).unwrap_or_else(|| {
                        panic!(
                            "Postfix partition {:?} not found. Available: {:?}",
                            postfix_range,
                            range_map.map.keys().collect::<Vec<_>>()
                        )
                    });

                Pattern::from(vec![*prefix, *operating_token, *postfix])
            },
            MergeMode::Full => {
                // No root pattern needed for intermediary nodes
                return;
            },
        };

        // Check if pattern already exists
        let existing_patterns =
            self.ctx.trav.expect_vertex_data(root).child_pattern_set();
        if existing_patterns.contains(&pattern) {
            debug!(?pattern, "Root pattern already exists, skipping");
            return;
        }

        debug!(?root, ?pattern, "Adding root pattern");
        self.ctx.trav.add_pattern_with_update(root, pattern);
    }
}
