//! Shared merge utilities for both intermediary and root node merge contexts.
//!
//! This module provides common logic for merging partitions that can be reused
//! by both NodeMergeCtx (intermediary) and RootMergeCtx (root).

use itertools::Itertools;

use crate::{
    RangeRole,
    RootMode,
    interval::partition::{
        Infix,
        Postfix,
        Prefix,
        ToPartition,
        info::{
            InfoPartition,
            PartitionInfo,
            border::perfect::BorderPerfect,
            range::role::{
                In,
                Post,
                Pre,
            },
        },
    },
    join::{
        context::{
            node::{
                context::NodeJoinCtx,
                merge::{
                    PartitionRange,
                    RangeMap,
                },
            },
            pattern::borders::JoinBorders,
        },
        partition::Join,
    },
    split::cache::vertex::SplitVertexCache,
};
use context_trace::{
    tests::macros::patterns,
    *,
};
use std::borrow::Borrow;
use tracing::debug;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MergeMode {
    Full,
    Root(RootMode),
}

/// Context for partition merging that properly handles index translations.
///
/// # Index Spaces
///
/// - **Offset indices**: `0..num_offsets` - indices into the offsets array
/// - **Partition indices**: `0..=num_offsets` - indices for partitions (num_offsets + 1 total)
///
/// # Partition Types by Range
///
/// | Partition Type | Partition Range | Left Offset | Right Offset |
/// |----------------|-----------------|-------------|--------------|
/// | Prefix | `0..=p` where `p < num_offsets` | None | `p` |
/// | Postfix | `p..=num_offsets` where `p > 0` | `p - 1` | None |
/// | Infix | `a..=b` where `a > 0` and `b < num_offsets` | `a - 1` | `b` |
/// | Full | `0..=num_offsets` | None | None |
#[derive(Debug, Clone, Copy)]
pub struct MergeContext {
    pub num_offsets: usize,
    pub mode: MergeMode,
}

/// The type of a partition range based on its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionType {
    /// Partition starts at 0 and ends before the last partition
    Prefix,
    /// Partition starts after 0 and ends at the last partition  
    Postfix,
    /// Partition spans from 0 to num_offsets (entire node)
    Full,
    /// Partition is in the middle (not touching either boundary)
    Infix,
}

impl MergeContext {
    pub fn new(
        num_offsets: usize,
        mode: MergeMode,
    ) -> Self {
        Self { num_offsets, mode }
    }

    /// Total number of partitions (num_offsets + 1)
    pub fn num_partitions(&self) -> usize {
        self.num_offsets + 1
    }

    /// The target partition range based on merge mode.
    pub fn target_partition_range(&self) -> PartitionRange {
        PartitionRange::from(match self.mode {
            MergeMode::Full => 0..=self.num_offsets,
            MergeMode::Root(root_mode) => match root_mode {
                RootMode::Prefix => 0..=self.num_offsets.saturating_sub(1),
                RootMode::Postfix => 1..=self.num_offsets,
                RootMode::Infix => 1..=self.num_offsets.saturating_sub(1),
            },
        })
    }

    /// Determine the partition type for a given partition range.
    pub fn partition_type(
        &self,
        range: &PartitionRange,
    ) -> PartitionType {
        let starts_at_zero = *range.start() == 0;
        let ends_at_last = *range.end() == self.num_offsets;

        match (starts_at_zero, ends_at_last) {
            (true, true) => PartitionType::Full,
            (true, false) => PartitionType::Prefix,
            (false, true) => PartitionType::Postfix,
            (false, false) => PartitionType::Infix,
        }
    }

    /// Get the right offset index for a prefix partition.
    /// For partition `0..=p`, the right offset is at index `p`.
    pub fn prefix_right_offset(
        &self,
        partition_end: usize,
    ) -> usize {
        debug_assert!(
            partition_end < self.num_partitions(),
            "Prefix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions()
        );
        partition_end
    }

    /// Get the left offset index for a postfix partition.
    /// For partition `p..=num_offsets`, the left offset is at index `p - 1`.
    pub fn postfix_left_offset(
        &self,
        partition_start: usize,
    ) -> usize {
        debug_assert!(
            partition_start > 0,
            "Postfix partition start {} must be > 0",
            partition_start
        );
        partition_start - 1
    }

    /// Get both offset indices for an infix partition.
    /// For partition `a..=b`, left offset is at `a - 1`, right offset is at `b`.
    pub fn infix_offsets(
        &self,
        partition_start: usize,
        partition_end: usize,
    ) -> (usize, usize) {
        debug_assert!(
            partition_start > 0,
            "Infix partition start {} must be > 0",
            partition_start
        );
        debug_assert!(
            partition_end < self.num_partitions(),
            "Infix partition end {} must be < num_partitions {}",
            partition_end,
            self.num_partitions()
        );
        (partition_start - 1, partition_end)
    }
}

impl MergeMode {
    pub fn partition_range(
        &self,
        num_offsets: usize,
    ) -> PartitionRange {
        MergeContext::new(num_offsets, *self).target_partition_range()
    }
    #[allow(dead_code)]
    pub fn is_prefix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Prefix))
    }
    #[allow(dead_code)]
    pub fn is_postfix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Postfix))
    }
    #[allow(dead_code)]
    pub fn is_full(&self) -> bool {
        matches!(self, MergeMode::Full)
    }
    #[allow(dead_code)]
    pub fn is_infix(&self) -> bool {
        matches!(self, MergeMode::Root(RootMode::Infix))
    }
    pub fn has_prefix(&self) -> bool {
        matches!(self, MergeMode::Full | MergeMode::Root(RootMode::Prefix))
    }
    pub fn has_postfix(&self) -> bool {
        matches!(self, MergeMode::Full | MergeMode::Root(RootMode::Postfix))
    }
}

/// The range map is updated in place with merged partition tokens.
pub fn merge_partitions_in_range(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    merge_mode: MergeMode,
) -> (Token, RangeMap) {
    let num_offsets = offsets.len();
    let merge_ctx = MergeContext::new(num_offsets, merge_mode);
    let num_partitions = merge_ctx.num_partitions();
    let target_partition_range = merge_ctx.target_partition_range();

    debug!(
        node=?ctx.index,
        patterns=?ctx.patterns(),
        ?offsets,
        num_partitions,
        ?merge_mode,
        ?target_partition_range,
        "merge_partitions_in_range: ENTERED"
    );

    let mut target_token: Option<Token> = None;
    let mut range_map = RangeMap::default();

    // Iterate over ALL partition ranges by length, then by starting position
    for len in 1..=num_partitions {
        debug!(
            "
    ==============================================================
    merging partitions of length {}
    ==============================================================",
            len
        );
        for start in 0..=(num_partitions - len) {
            let end = start + len - 1; // end is inclusive (partition index)
            let partition_range = PartitionRange::new(start..=end);

            debug!(
                node=?ctx.index,
                ?partition_range,
                num_partitions,
                ?merge_mode,
                "Merging partition range"
            );

            let partition_type = merge_ctx.partition_type(&partition_range);
            debug!(?partition_type, "Detected partition type");

            let merged_token = match partition_type {
                PartitionType::Full => {
                    debug!("Merging full existing token - skipping");
                    ctx.index
                },
                PartitionType::Prefix => {
                    debug!("Merge Prefix partition: ENTERED");
                    let ro_idx = merge_ctx.prefix_right_offset(end);
                    let ro = offsets.pos_ctx_by_index(ro_idx);
                    Prefix::new(ro).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
                PartitionType::Postfix => {
                    debug!("Merge Postfix partition: ENTERED");
                    let lo_idx = merge_ctx.postfix_left_offset(start);
                    let lo = offsets.pos_ctx_by_index(lo_idx);
                    Postfix::new(lo).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
                PartitionType::Infix => {
                    debug!("Merge Infix partition: ENTERED");
                    let (lo_idx, ro_idx) = merge_ctx.infix_offsets(start, end);
                    let lo = offsets.pos_ctx_by_index(lo_idx);
                    let ro = offsets.pos_ctx_by_index(ro_idx);
                    Infix::new(lo, ro).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
            };

            if partition_range == target_partition_range {
                debug!(
                    ?partition_range,
                    "merge_partitions_in_range: reached target partition range"
                );
                assert_eq!(target_token, None, "Target token already set");
                target_token = Some(merged_token);
            }

            debug!(
                ?partition_range,
                ?merged_token,
                "RangeMap INSERT: inserting token for range"
            );
            range_map.insert(partition_range.clone(), merged_token);
        }
    }

    // Extract target token from range_map
    let target_token = target_token
            .unwrap_or_else(|| panic!(
                "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                target_partition_range,
                range_map.map.keys().collect::<Vec<_>>()
            ));
    (target_token, range_map)
}

pub trait MergePartition<R: RangeRole<Mode = Join>>:
    InfoPartition<R> + ToPartition<R>
where
    R::Borders: JoinBorders<R>,
{
    fn join_patterns<'a>(
        &mut self,
        ctx: &mut NodeJoinCtx<'a>,
        range_map: &RangeMap,
        range: &PartitionRange,
        info: &PartitionInfo<R>,
    ) -> Vec<Pattern>
    where
        R: 'a,
    {
        let sub_merges: Vec<_> =
            range_map.range_sub_merges(range).into_iter().collect();

        // Extract joined patterns from partition info
        let joined_patterns: Vec<Pattern> = info
            .patterns
            .iter()
            .map(|(pid, pat_info)| {
                Pattern::from(
                    (pat_info.clone().join_pattern(ctx, pid).borrow()
                        as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            })
            .collect();

        // Combine joined patterns with sub-merges, removing duplicates
        let mut combined_patterns: Vec<Pattern> = joined_patterns.clone();
        for merge_pattern in sub_merges.iter() {
            if !combined_patterns.contains(merge_pattern) {
                combined_patterns.push(merge_pattern.clone());
            }
        }

        debug!(
            range_start = range.start(),
            range_end = range.end(),
            num_partition_patterns = info.patterns.len(),
            num_sub_merges = sub_merges.len(),
            num_combined = combined_patterns.len(),
            ?joined_patterns,
            ?sub_merges,
            ?combined_patterns,
            "POSTFIX merge_postfix_partition: combining patterns"
        );
        combined_patterns
    }
    fn perfect_replace_range(
        &self,
        info: &PartitionInfo<R>,
    ) -> Option<(PatternId, <R as RangeRole>::PatternRange)> {
        info.perfect
            .clone()
            .all_perfect_pattern()
            .map(|pid| (pid, info.patterns.get(&pid).unwrap().range.clone()))
    }
    fn merge_partition<'a>(
        &mut self,
        ctx: &mut NodeJoinCtx<'a>,
        offsets: &SplitVertexCache,
        range_map: &RangeMap,
        range: &PartitionRange,
    ) -> Token
    where
        R: 'a,
    {
        let node_index = ctx.token();
        debug!(
            range_start = range.start(),
            range_end = range.end(),
            num_offsets = offsets.len(),
            "Merge Partition: ENTERED"
        );
        debug!("Creating partition info",);
        let res: Result<PartitionInfo<_>, _> = self.info_partition(ctx);
        match res {
            Ok(info) => {
                debug!("Merge needed, join patterns",);
                let combined_patterns =
                    self.join_patterns(ctx, range_map, range, &info);

                let token = ctx.trav.insert_patterns(combined_patterns);

                debug!(
                    %token,
                    "Merged patterns to node",
                );
                // Replace pattern if range is perfect in a pattern
                if let Some((pid, replace_range)) =
                    self.perfect_replace_range(&info)
                {
                    let pattern_loc = node_index.to_pattern_location(pid);
                    debug!(
                        ?node_index,
                        ?pid,
                        ?pattern_loc,
                        ?token,
                        ?range,
                        ?replace_range,
                        "{}: Replacing pattern with merged token",
                        R::ROLE_STR
                    );
                    ctx.trav.replace_in_pattern(
                        pattern_loc,
                        replace_range,
                        vec![token],
                    );
                }

                token
            },
            Err(existing) => {
                debug!(
                    ?existing,
                    range_start = range.start(),
                    range_end = range.end(),
                    "{}: Token already exists - using without modification",
                    R::ROLE_STR
                );

                existing
            },
        }
    }
}

impl<P: ToPartition<Pre<Join>>> MergePartition<Pre<Join>> for P {}
impl<P: ToPartition<Post<Join>>> MergePartition<Post<Join>> for P {}
impl<P: ToPartition<In<Join>>> MergePartition<In<Join>> for P {}
