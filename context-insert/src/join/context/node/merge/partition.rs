use std::borrow::Borrow;

use crate::{
    PatternSubDeltas,
    RangeRole,
    interval::partition::{
        ToPartition,
        info::{
            InfoPartition,
            PartitionInfo,
            border::perfect::BorderPerfect as _,
        },
    },
    join::{
        context::{
            node::{
                context::NodeJoinCtx,
                merge::{
                    PartitionRange,
                    RangeMap,
                    context::MergeCtx,
                },
            },
            pattern::borders::JoinBorders,
        },
        joined::partition::JoinedPartition,
        partition::{
            Join,
            info::JoinPartitionInfo,
        },
    },
};
use context_trace::{
    Pattern,
    PatternId,
    Token,
    VertexSet,
};
use itertools::Itertools;
use tracing::debug;

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

    /// Merge a partition and return (token, delta).
    ///
    /// Uses `JoinPartitionInfo` infrastructure which computes deltas automatically.
    /// The delta represents how many indices were removed from each pattern
    /// when the partition was replaced with a single merged token.
    fn merge_partition<'a, 'b>(
        &mut self,
        ctx: &mut MergeCtx<'a>,
        range_map: &RangeMap,
        range: &PartitionRange,
    ) -> (Token, Option<PatternSubDeltas>)
    where
        R: 'a,
    {
        debug!(
            range_start = range.start(),
            range_end = range.end(),
            num_offsets = ctx.offsets.len(),
            "Merge Partition with Delta: ENTERED"
        );

        // Use info_partition to get partition info, then JoinedPartition to handle
        // pattern joining, replacement, and delta computation
        // Note: info_partition expects &NodeJoinCtx, so we dereference through ctx.ctx
        match self.info_partition(&ctx.ctx) {
            Ok(info) => {
                // Convert to JoinPartitionInfo and then to JoinedPartition
                // Note: from_partition_info expects &mut NodeJoinCtx
                let joined = JoinedPartition::from_partition_info(
                    JoinPartitionInfo::new(info),
                    &mut ctx.ctx,
                );

                debug!(
                    token = %joined.index,
                    ?joined.delta,
                    "JoinPartitionInfo succeeded with delta"
                );

                // Add sub-merge patterns if any (alternative decompositions)
                // First, get existing patterns to avoid duplicates
                let existing_patterns = ctx
                    .ctx
                    .trav
                    .expect_vertex_data(joined.index)
                    .child_pattern_set();

                let sub_merges: Vec<_> = range_map
                    .range_sub_merges(range)
                    .into_iter()
                    .filter(|p| !existing_patterns.contains(p))
                    .collect();

                if !sub_merges.is_empty() {
                    debug!(
                        num_sub_merges = sub_merges.len(),
                        "Adding sub-merge patterns"
                    );
                    for merge_pattern in sub_merges {
                        ctx.ctx.trav.add_pattern_with_update(
                            joined.index,
                            merge_pattern,
                        );
                    }
                }

                let delta = if joined.delta.is_empty() {
                    None
                } else {
                    Some(joined.delta)
                };
                (joined.index, delta)
            },
            Err(existing) => {
                debug!(
                    ?existing,
                    range_start = range.start(),
                    range_end = range.end(),
                    "{}: Token already exists - using without modification",
                    R::ROLE_STR
                );
                (existing, None)
            },
        }
    }
}

impl<R: RangeRole<Mode = Join>, P: ToPartition<R>> MergePartition<R> for P where
    R::Borders: JoinBorders<R>
{
}
