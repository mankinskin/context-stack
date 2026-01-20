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
impl MergeMode {
    pub fn partition_range(
        &self,
        num_offsets: usize,
    ) -> PartitionRange {
        PartitionRange::from(match self {
            MergeMode::Full => 0..(num_offsets + 1),
            MergeMode::Root(root_mode) => match root_mode {
                RootMode::Prefix => 0..num_offsets,
                RootMode::Postfix => 1..(num_offsets + 1),
                RootMode::Infix => 1..num_offsets,
            },
        })
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
    let mut range_map = RangeMap::default();
    let create_prefix = merge_mode.has_prefix();
    let create_postfix = merge_mode.has_postfix();
    let num_offsets = offsets.len();
    let is_full_cover =
        merge_mode == MergeMode::Full && create_prefix && create_postfix;
    let num_partitions = if is_full_cover {
        num_offsets + 1
    } else if create_prefix || create_postfix {
        num_offsets
    } else {
        num_offsets - 1
    };
    debug!(
        node=?ctx.index,
        patterns=?ctx.patterns(),
        ?offsets,
        num_partitions,
        ?merge_mode,
        ?is_full_cover,
        "merge_partitions_in_range: ENTERED"
    );
    let max_len = num_partitions;
    for len in 1..=max_len {
        debug!(
            "
    ==============================================================
    merging partitions of length {}
    ==============================================================",
            len
        );
        for start in 0..(max_len - len + 1) {
            let end = start + len;
            let first = start; // inclusive
            let last = end - 1; // exclusive
            let partition_range = PartitionRange::new(start..end);
            debug!(
                node=?ctx.index,
                patterns=?ctx.patterns(),
                ?offsets,
                num_partitions,
                ?merge_mode,
                %start,
                %end, %first, %last,
                "Merging partition range"
            );

            let is_start = start == 0;
            let is_end = end == max_len;
            debug!(?merge_mode, is_start, is_end, "Detecting partition type");
            let merged_token = match (is_start, is_end, merge_mode) {
                (true, false, MergeMode::Full)
                | (true, false, MergeMode::Root(RootMode::Prefix)) => {
                    debug!("Merge Prefix partition: ENTERED");
                    let ro = offsets.pos_ctx_by_index(last);
                    Prefix::new(ro).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
                (false, true, MergeMode::Full)
                | (false, true, MergeMode::Root(RootMode::Postfix)) => {
                    debug!("Merge Postfix partition: ENTERED");

                    let lo = offsets.pos_ctx_by_index(start - 1);
                    Postfix::new(lo).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
                (true, true, MergeMode::Full) => {
                    debug!("Merging full existing token - skipping");
                    ctx.index
                },
                _ => {
                    debug!("Merge Infix partition: ENTERED");
                    let lo = offsets.pos_ctx_by_index(first);
                    let ro = offsets.pos_ctx_by_index(end);
                    Infix::new(lo, ro).merge_partition(
                        ctx,
                        offsets,
                        &range_map,
                        &partition_range,
                    )
                },
            };

            debug!(
                ?start,
                ?end,
                ?merged_token,
                "RangeMap INSERT: inserting token for range"
            );
            range_map.insert(partition_range.clone(), merged_token);
        }
    }
    if is_full_cover {
        debug!(
            "merge_partitions_in_range: full cover - skipped merging full token"
        );
    }
    let target_partition_range = merge_mode.partition_range(num_offsets);
    // Extract target token from range_map
    let target_token = *range_map.get(&target_partition_range)
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
