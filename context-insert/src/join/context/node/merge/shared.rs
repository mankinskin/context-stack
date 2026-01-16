//! Shared merge utilities for both intermediary and root node merge contexts.
//!
//! This module provides common logic for merging partitions that can be reused
//! by both NodeMergeCtx (intermediary) and RootMergeCtx (root).

use std::ops::Range;

use itertools::Itertools;
use tracing_subscriber::field::debug;

use crate::{
    interval::partition::{
        Infix,
        Postfix,
        Prefix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::{
                role::{
                    In,
                    Post,
                    Pre,
                },
                splits::PostfixRangeFrom,
            },
        },
    },
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::{
                PartitionRange,
                RangeMap,
            },
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
    partition_range: Range<usize>,
) -> Vec<Token> {
    let num_offsets = offsets.len();
    let mut partitions = Vec::<Token>::with_capacity(partition_range.len());

    // Get split positions in sorted order
    let mut offset_ctxs: Vec<_> =
        offsets.iter().map(PosSplitCtx::from).collect();
    offset_ctxs.sort_by_key(|ctx| ctx.pos);

    debug!(num_offsets, ?partition_range, "Creating initial partitions");

    // Determine if we should create prefix/postfix based on partition_range
    let create_prefix = partition_range.start == 0;
    let create_postfix = partition_range.end == num_offsets + 1;

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
                ctx.trav.insert_patterns(patterns)
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
        debug!(?prefix_token, "Created prefix partition");
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
                ctx.trav.insert_patterns(patterns)
            },
            Err(existing) => existing,
        };
        partitions.push(infix_token);
        debug!(i, ?infix_token, "Created infix partition");
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
                ctx.trav.insert_patterns(patterns)
            },
            Err(existing) => existing,
        };
        partitions.push(postfix_token);
        debug!(?postfix_token, "Created postfix partition");
    }

    debug!(
        num_created = partitions.len(),
        expected = partition_range.len(),
        "Initial partitions created"
    );

    partitions
}

/// Merge partitions within a specified range of offsets.
///
/// This function implements the core merge algorithm shared by both intermediary
/// and root node merge contexts. It merges partitions from smallest to largest,
/// using the same pattern matching and insertion logic.
///
/// # Parameters
///
/// - `ctx`: The node join context
/// - `offsets`: Split vertex cache containing offset positions
/// - `partitions`: Initial partitions to merge
/// - `range_map`: Mutable range map to store merged tokens (uses array indices)
/// - `has_prefix`: Whether partition 0 is a prefix (needs Prefix merge logic)
/// - `has_postfix`: Whether the last partition is a postfix (needs Postfix merge logic)
/// - `update_parent_patterns`: Whether to update ctx.index patterns when splits complete
///
/// # Returns
///
/// The range map is updated in place with merged partition tokens.
pub fn merge_partitions_in_range(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    partitions: &[Token],
    range_map: &mut RangeMap,
    has_prefix: bool,
    has_postfix: bool,
) {
    let max_len = partitions.len();
    debug!(
        max_len,
        has_prefix, has_postfix, "merge_partitions_in_range: ENTERED"
    );

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
            let is_end = end == partitions.len();
            debug!(
                range_start = range.start(),
                range_end = range.end(),
                is_start,
                is_end,
                ?range,
                "Merging partition range"
            );
            let merged_token = if is_start && is_end {
                // Merging the entire array - just use range_sub_merges patterns
                // No need for info_partition since we're combining all existing partitions
                let merges: Vec<_> =
                    range_map.range_sub_merges(&range).into_iter().collect();
                let token = ctx.trav.insert_patterns(merges);
                
                // Replace the root node pattern with the merged token
                // This is the entire pattern, so we replace all of it
                debug!(
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "FULL RANGE: Replacing entire root pattern with merged token"
                );
                // TODO(#3760834806): Replace root node pattern with merged token
                // Need to determine which pattern index to use for the root node
                // For now, just return the token without replacement
                // ctx.trav.replace_in_pattern(root_pattern_loc, 0..pattern_len, vec![token]);
                
                token
            } else if has_prefix && is_start {
                // Merging prefix with some infixes (range [0..k])
                // Right boundary at offset index (end - 1)
                merge_prefix_partition(ctx, offsets, range_map, &range)
            } else if has_postfix && is_end {
                // Merging some infixes to the end (range [k..partitions.len()])
                // This is a postfix partition if and only if no prefix exists
                // Left boundary at offset index (start - (has_prefix ? 1 : 0))
                merge_postfix_partition(ctx, offsets, range_map, &range)
            } else {
                // Merging infixes in the middle (range [k..m] where k > 0 and m < partitions.len())
                // Use infix merge
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
}

/// Merge a prefix partition.
fn merge_prefix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &PartitionRange,
) -> Token {
    let node_index = ctx.index;

    debug!(
        range_start = range.start(),
        range_end = range.end(),
        num_offsets = offsets.len(),
        "merge_prefix_partition: ENTERED"
    );
    let ro = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(range.end())
        .unwrap();

    let prefix_partition = Prefix::new(ro);
    let res: Result<PartitionInfo<Pre<Join>>, _> =
        prefix_partition.info_partition(ctx);

    match res {
        Ok(info) => {
            let merges: Vec<_> =
                range_map.range_sub_merges(range).into_iter().collect();

            // For Prefix, SinglePerfect contains Option<PatternId>
            // We replace when the right boundary is perfect
            let perfect_pattern_id = info.perfect.0;

            // Use only the merge patterns from range_sub_merges.
            let token = ctx.trav.insert_patterns(merges.clone());

            // Replace pattern if right boundary is perfect in a pattern
            if let Some(pid) = perfect_pattern_id {
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_loc = node_index.to_pattern_location(pid);
                let pattern_end_index =
                    ro.split.pattern_splits.get(&pid).unwrap().sub_index;
                debug!(
                    ?node_index,
                    ?pid,
                    ?pattern_loc,
                    ?pattern_end_index,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "PREFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_in_pattern(
                    pattern_loc,
                    0..pattern_end_index,
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
                "PREFIX: Token already exists - checking if pattern replacement needed"
            );

            // Even though token exists, we still need to replace patterns that have
            // perfect boundaries at this offset.
            // For prefix, the right boundary (ro) determines which patterns to update.
            // If a pattern has this offset in its pattern_splits, it has a boundary here.
            for (pid, trace_pos) in &ro.split.pattern_splits {
                let pattern_loc = node_index.to_pattern_location(*pid);
                let pattern_end_index = trace_pos.sub_index;
                debug!(
                    ?node_index,
                    ?pid,
                    ?pattern_loc,
                    ?pattern_end_index,
                    ?existing,
                    range_start = range.start,
                    range_end = range.end,
                    "PREFIX (existing): Replacing pattern with existing token"
                );
                ctx.trav.replace_in_pattern(
                    pattern_loc,
                    0..pattern_end_index,
                    vec![existing],
                );
            }

            existing
        },
    }
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

    let lo = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(range.start())
        .unwrap();

    let node_index = ctx.index;
    let postfix_partition = Postfix::new(lo);
    let res: Result<PartitionInfo<Post<Join>>, _> =
        postfix_partition.info_partition(ctx);

    debug!(
        is_ok = res.is_ok(),
        " merge_postfix_partition: info_partition result"
    );

    match res {
        Ok(info) => {
            let merges: Vec<_> =
                range_map.range_sub_merges(range).into_iter().collect();

            // For Postfix, SinglePerfect contains Option<PatternId>
            // We replace when the left boundary is perfect
            let perfect_pattern_id = info.perfect.0;

            debug!(
                ?perfect_pattern_id,
                range_start = range.start,
                range_end = range.end,
                num_patterns = info.patterns.len(),
                num_merges = merges.len(),
                "POSTFIX merge_postfix_partition: perfect border check"
            );

            // Use only the merge patterns from range_sub_merges.
            // These are patterns composed of tokens that already exist in the RangeMap,
            // representing all valid binary splits of this range.
            //
            // We do NOT call join_pattern here because:
            // 1. join_pattern creates new tokens as side effects, causing duplicates
            // 2. range_sub_merges already provides all necessary merge patterns using
            //    tokens that were created during initial partition creation
            // 3. The patterns from join_pattern would be equivalent to what's in range_sub_merges
            //    but with potentially different (duplicate) token indices
            let token = ctx.trav.insert_patterns(merges.clone());

            // Replace pattern if left boundary is perfect in a pattern
            if let Some(pid) = perfect_pattern_id {
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_loc = node_index.to_pattern_location(pid);
                let pattern_start_index =
                    lo.split.pattern_splits.get(&pid).unwrap().sub_index;
                debug!(
                    ?node_index,
                    ?pid,
                    ?pattern_loc,
                    ?pattern_start_index,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "POSTFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_in_pattern(
                    pattern_loc,
                    PostfixRangeFrom::new(
                        pattern_start_index,
                        ctx.patterns().get(&pid).unwrap().len(),
                    ),
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
                "POSTFIX: Token already exists - checking if pattern replacement needed"
            );

            // Even though token exists, we still need to replace patterns that have
            // perfect boundaries at this offset.
            // For postfix, the left boundary (lo) determines which patterns to update.
            for (pid, trace_pos) in &lo.split.pattern_splits {
                let pattern_loc = node_index.to_pattern_location(*pid);
                let pattern_start_index = trace_pos.sub_index;
                debug!(
                    ?node_index,
                    ?pid,
                    ?pattern_loc,
                    ?pattern_start_index,
                    ?existing,
                    range_start = range.start,
                    range_end = range.end,
                    "POSTFIX (existing): Replacing pattern with existing token"
                );
                ctx.trav.replace_in_pattern(
                    pattern_loc,
                    PostfixRangeFrom::new(
                        pattern_start_index,
                        ctx.patterns().get(pid).unwrap().len(),
                    ),
                    vec![existing],
                );
            }
            existing
        },
    }
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
    let node_index = ctx.index;
    debug!(
        range_start = range.start(),
        range_end = range.end(),
        num_offsets = offsets.len(),
        "merge_infix_partition: ENTERED"
    );
    let lo = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(range.start())
        .unwrap();
    let ro = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(range.end())
        .unwrap();

    let infix_partition = Infix::new(lo, ro);
    let res: Result<PartitionInfo<In<Join>>, _> =
        infix_partition.info_partition(ctx);

    match res {
        Ok(info) => {
            let merges: Vec<_> =
                range_map.range_sub_merges(range).into_iter().collect();

            // Check if we have BOTH perfect borders in the SAME pattern
            // For infix partitions, DoublePerfect contains (Option<PatternId>, Option<PatternId>)
            // We can only replace when both are Some AND equal (same pattern)
            let perfect_pattern_id = match (info.perfect.0, info.perfect.1) {
                (Some(left_pid), Some(right_pid)) if left_pid == right_pid =>
                    Some(left_pid),
                _ => None,
            };

            // Use only the merge patterns from range_sub_merges.
            // See merge_postfix_partition for detailed explanation.
            debug!(
                num_merges = merges.len(),
                has_perfect = perfect_pattern_id.is_some(),
                "Merging infix partition - pattern counts"
            );

            let token = ctx.trav.insert_patterns(merges.clone());

            // Only replace pattern if BOTH offsets are perfect in the SAME pattern
            if let Some(pid) = perfect_pattern_id {
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_loc = node_index.to_pattern_location(pid);
                let pattern_start_index =
                    lo.split.pattern_splits.get(&pid).unwrap().sub_index;
                let pattern_end_index =
                    ro.split.pattern_splits.get(&pid).unwrap().sub_index;
                debug!(
                    ?node_index,
                    ?pid,
                    ?pattern_loc,
                    ?pattern_start_index,
                    ?pattern_end_index,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "INFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_in_pattern(
                    pattern_loc,
                    pattern_start_index..pattern_end_index,
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
                "INFIX: Token already exists - checking if pattern replacement needed"
            );

            // Even though token exists, we still need to replace patterns that have
            // perfect boundaries at BOTH offsets in the SAME pattern.
            // Find patterns that appear in both lo and ro pattern_splits
            for (pid, lo_trace_pos) in &lo.split.pattern_splits {
                if let Some(ro_trace_pos) = ro.split.pattern_splits.get(pid) {
                    // Both boundaries exist in this pattern - replace it
                    let pattern_loc = node_index.to_pattern_location(*pid);
                    let pattern_start_index = lo_trace_pos.sub_index;
                    let pattern_end_index = ro_trace_pos.sub_index;
                    debug!(
                        ?node_index,
                        ?pid,
                        ?pattern_loc,
                        ?pattern_start_index,
                        ?pattern_end_index,
                        ?existing,
                        range_start = range.start,
                        range_end = range.end,
                        "INFIX (existing): Replacing pattern with existing token"
                    );
                    ctx.trav.replace_in_pattern(
                        pattern_loc,
                        pattern_start_index..pattern_end_index,
                        vec![existing],
                    );
                }
            }

            existing
        },
    }
}
