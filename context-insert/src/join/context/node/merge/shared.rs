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
    split::{
        cache::vertex::SplitVertexCache,
        vertex::PosSplitCtx,
    },
};
use context_trace::*;
use std::borrow::Borrow;
use tracing::debug;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MergeMode {
    Full,
    Root(RootMode),
}
impl MergeMode {
    pub fn partition_range(
        &self,
        num_offsets: usize,
    ) -> std::ops::Range<usize> {
        match self {
            MergeMode::Full => 0..(num_offsets + 1),
            MergeMode::Root(root_mode) => match root_mode {
                RootMode::Prefix => 0..num_offsets,
                RootMode::Postfix => 1..(num_offsets + 1),
                RootMode::Infix => 1..num_offsets,
            },
        }
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

/// Create initial partitions between consecutive offsets.
///
/// This function creates the smallest partitions needed before the merge step.
/// It can create prefix/infix/postfix partitions based on the partition_range parameter.
///
/// # Parameters
///
/// - `ctx`: The node join context
/// - `offsets`: Split vertex cache containing offset positions  
/// - `partition_range`: Range of partition indices to create
///   - For intermediary: 0..num_offsets+1 (creates all: prefix + infixes + postfix)
///   - For root with Prefix mode: 0..num_offsets (creates prefix + infixes, no postfix)
///   - For root with Postfix mode: 1..num_offsets+1 (creates infixes + postfix, no prefix)
///   - For root with Infix mode: 1..num_offsets (creates infixes only, no prefix/postfix)
///
/// # Returns
///
/// A vector of tokens representing the initial partitions.
pub fn create_initial_partitions(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    merge_mode: MergeMode,
) -> Vec<Token> {
    let num_offsets = offsets.len();
    let partition_range = merge_mode.partition_range(num_offsets);
    let mut partitions = Vec::<Token>::with_capacity(partition_range.len());
    debug!(
        node = ?ctx.index,
        node_patterns = ?ctx.patterns(),
        offset_positions = ?offsets.keys().collect::<Vec<_>>(),
        ?partition_range,
        num_partitions = partition_range.len(),
        ?merge_mode,
        "create_initial_partitions: ENTERED"
    );

    // Get split positions in sorted order
    let offset_ctxs: Vec<_> = offsets
        .iter()
        .map(PosSplitCtx::from)
        .sorted_by_key(|ctx| ctx.pos)
        .collect();

    // Log offset positions
    let offset_positions: Vec<usize> =
        offset_ctxs.iter().map(|o| o.pos.get()).collect();

    // Determine if we should create prefix/postfix based on partition_range
    let create_prefix = merge_mode.has_prefix();
    let create_postfix = merge_mode.has_postfix();
    debug!(
        ?offset_positions,
        create_prefix,
        create_postfix,
        "create_initial_partitions: offset positions (sorted)"
    );

    // Create prefix partition (before first offset) if requested
    if create_prefix && num_offsets > 0 {
        let first_offset = offset_ctxs[0];
        let prefix = Prefix::new(first_offset);
        let res: Result<PartitionInfo<Pre<Join>>, _> =
            prefix.info_partition(ctx);
        let prefix_token = match res {
            Ok(part_info) => {
                let patterns: Vec<Pattern> = part_info
                    .patterns
                    .into_iter()
                    .map(|(pid, pat_info)| {
                        Pattern::from(
                            (pat_info.join_pattern(ctx, &pid).borrow()
                                as &'_ Pattern)
                                .iter()
                                .cloned()
                                .collect_vec(),
                        )
                    })
                    .collect();
                let prefix_token = ctx.trav.insert_patterns(patterns.clone());
                debug!(
                    ?prefix_token,
                    ?patterns,
                    "Created prefix partition with child patterns"
                );
                prefix_token
            },
            Err(existing) => {
                debug!(
                    existing = %pretty(&existing),
                    "PREFIX partition token already exists in create_initial_partitions"
                );
                existing
            },
        };
        partitions.push(prefix_token);
    }

    // Create infix partitions between consecutive offsets
    for i in 0..num_offsets - 1 {
        let lo = offset_ctxs[i];
        let ro = offset_ctxs[i + 1];
        let infix = Infix::new(lo, ro);
        let res: Result<PartitionInfo<In<Join>>, _> = infix.info_partition(ctx);
        let infix_token = match res {
            Ok(part_info) => {
                let patterns: Vec<Pattern> = part_info
                    .patterns
                    .into_iter()
                    .map(|(pid, pat_info)| {
                        Pattern::from(
                            (pat_info.join_pattern(ctx, &pid).borrow()
                                as &'_ Pattern)
                                .iter()
                                .cloned()
                                .collect_vec(),
                        )
                    })
                    .collect();
                let token = ctx.trav.insert_patterns(patterns.clone());
                debug!(
                    i,
                    ?token,
                    ?patterns,
                    "Created infix partition with child patterns"
                );
                token
            },
            Err(existing) => {
                debug!(
                    existing = %pretty(&existing),
                    "INFIX partition token already exists in create_initial_partitions"
                );
                existing
            },
        };
        partitions.push(infix_token);
    }

    // Create postfix partition (after last offset) if requested
    if create_postfix && num_offsets > 0 {
        let last_offset = offset_ctxs[num_offsets - 1];
        let postfix = Postfix::new(last_offset);
        let res: Result<PartitionInfo<Post<Join>>, _> =
            postfix.info_partition(ctx);
        let postfix_token = match res {
            Ok(part_info) => {
                let patterns: Vec<Pattern> = part_info
                    .patterns
                    .into_iter()
                    .map(|(pid, pat_info)| {
                        Pattern::from(
                            (pat_info.join_pattern(ctx, &pid).borrow()
                                as &'_ Pattern)
                                .iter()
                                .cloned()
                                .collect_vec(),
                        )
                    })
                    .collect();
                let token = ctx.trav.insert_patterns(patterns.clone());
                debug!(
                    ?token,
                    ?patterns,
                    "Created postfix partition with child patterns"
                );
                token
            },
            Err(existing) => {
                debug!(
                    existing = %pretty(&existing),
                    "POSTFIX partition token already exists in create_initial_partitions"
                );
                existing
            },
        };
        partitions.push(postfix_token);
    }

    debug!(num_created = partitions.len(), "Initial partitions created");

    partitions
}

/// The range map is updated in place with merged partition tokens.
pub fn merge_partitions_in_range(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    partitions: &[Token],
    range_map: &mut RangeMap,
    merge_mode: MergeMode,
) {
    let num_partitions = partitions.len();
    let is_full_cover =
        merge_mode == MergeMode::Full && num_partitions == offsets.len() + 1;
    debug!(
        num_partitions,
        ?merge_mode,
        ?is_full_cover,
        "merge_partitions_in_range: ENTERED"
    );
    let max_len = num_partitions - is_full_cover as usize;
    // Merge from smallest to largest partitions
    // Start at len=2 because len=1 (single partitions) are already in range_map
    for len in 2..=max_len {
        debug!(
            "merge_partitions_in_range: merging partitions of length {}",
            len
        );
        // For each partition length, iterate over all valid starting positions in the array
        // E.g., with max_len=3, len=2: start in 0..2 gives ranges [0..2] and [1..3]
        for start in 0..(max_len - len + 1) {
            let end = start + len;
            let range = PartitionRange::new(start..end);

            // Determine partition type based on position in array
            // is_start: this range starts at array index 0
            // is_end: this range ends at array index partitions.len()
            let is_start = start == 0;
            let is_end = end == max_len - 1;
            debug!(is_start, is_end, ?range, "Merging partition range");
            let merged_token = if merge_mode.has_prefix() && is_start {
                // Merging prefix with some infixes (range [0..k])
                // Right boundary at offset index (end - 1)
                merge_prefix_partition(ctx, offsets, range_map, &range)
            } else if merge_mode.has_postfix() && is_end {
                // Merging some infixes to the end (range [k..partitions.len()])
                // This is a postfix partition if and only if no prefix exists
                // Left boundary at offset index (start - (has_prefix ? 1 : 0))
                merge_postfix_partition(ctx, offsets, range_map, &range)
            } else {
                // Merging infixes in the middle (range [k..m] where k > 0 and m < partitions.len())
                // Use infix merge
                // either a side of an infix mode range or somewhere in the middle of any mode
                merge_infix_partition(ctx, offsets, range_map, &range)
            };

            debug!(
                range_start = range.start(),
                range_end = range.end(),
                ?merged_token,
                "RangeMap INSERT: inserting token for range"
            );
            range_map.insert(range.clone(), merged_token);
        }
    }
    if is_full_cover {
        debug!(
            "merge_partitions_in_range: full cover - skipped merging full token"
        );
    }
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
            range_start = range.start,
            range_end = range.end,
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
            "merge_prefix_partition: ENTERED"
        );
        let res: Result<PartitionInfo<_>, _> = self.info_partition(ctx);
        match res {
            Ok(info) => {
                let combined_patterns =
                    self.join_patterns(ctx, range_map, range, &info);

                let token = ctx.trav.insert_patterns(combined_patterns);

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
                    range_start = range.start,
                    range_end = range.end,
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
use crate::interval::partition::ToPartition;
/// Merge a prefix partition.
fn merge_prefix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &PartitionRange,
) -> Token {
    debug!(
        range_start = range.start(),
        range_end = range.end(),
        num_offsets = offsets.len(),
        "merge_prefix_partition: ENTERED"
    );
    let ro = offsets.pos_ctx_by_index(range.end());
    Prefix::new(ro).merge_partition(ctx, offsets, range_map, range)
}

/// Merge a postfix partition.
fn merge_postfix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &PartitionRange,
) -> Token {
    debug!(
        range_start = range.start(),
        range_end = range.end(),
        num_offsets = offsets.len(),
        "merge_postfix_partition: ENTERED"
    );

    let lo = offsets.pos_ctx_by_index(range.start());
    Postfix::new(lo).merge_partition(ctx, offsets, range_map, range)
}

/// Merge an infix partition between two offsets.
///
/// # Parameters
/// - `start_partition_idx`: Partition index (conceptual, from partition_range)
/// - `end_partition_idx`: Partition index (conceptual, from partition_range)
/// - `partitions`: The partitions array to access tokens
/// - `num_offsets`: Total number of offsets
/// - `has_prefix`: Whether partition 0 is a prefix (partition_range.start == 0)
fn merge_infix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &PartitionRange,
) -> Token {
    debug!(
        range_start = range.start(),
        range_end = range.end(),
        num_offsets = offsets.len(),
        "merge_infix_partition: ENTERED"
    );
    let lo = offsets.pos_ctx_by_index(range.start());
    let ro = offsets.pos_ctx_by_index(range.end());
    Infix::new(lo, ro).merge_partition(ctx, offsets, range_map, range)
}
