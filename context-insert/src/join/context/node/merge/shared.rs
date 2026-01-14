//! Shared merge utilities for both intermediary and root node merge contexts.
//!
//! This module provides common logic for merging partitions that can be reused
//! by both NodeMergeCtx (intermediary) and RootMergeCtx (root).

use std::ops::Range;

use itertools::Itertools;

use crate::{
    interval::partition::{
        Infix,
        Prefix,
        Postfix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::role::{In, Post, Pre},
        },
    },
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::RangeMap,
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
    let mut offset_ctxs: Vec<_> = offsets
        .iter()
        .map(PosSplitCtx::from)
        .collect();
    offset_ctxs.sort_by_key(|ctx| ctx.pos);

    debug!(
        num_offsets,
        ?partition_range,
        "Creating initial partitions"
    );

    // Determine if we should create prefix/postfix based on partition_range
    let create_prefix = partition_range.start == 0;
    let create_postfix = partition_range.end == num_offsets + 1;

    // Create prefix partition (before first offset) if requested
    if create_prefix && num_offsets > 0 {
        let first_offset = offset_ctxs[0];
        let prefix = Prefix::new(first_offset);
        let res: Result<PartitionInfo<Pre<Join>>, _> = prefix.info_partition(ctx);
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
            Err(existing) => existing,
        };
        partitions.push(prefix_token);
        debug!(?prefix_token, "Created prefix partition");
    }

    // Create infix partitions between consecutive offsets
    let infix_start = if create_prefix { 0 } else { partition_range.start - 1 };
    let infix_end = if create_postfix { num_offsets - 1 } else { partition_range.end - 1 };
    
    for i in infix_start..infix_end {
        if i + 1 >= num_offsets {
            break;
        }
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
        let res: Result<PartitionInfo<Post<Join>>, _> = postfix.info_partition(ctx);
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
/// - `partition_range`: Range of partition indices to merge (e.g., 0..num_offsets+1)
/// - `num_offsets`: Total number of offsets (for boundary detection)
/// - `range_map`: Mutable range map to store merged tokens
/// - `node_index`: The node being merged (for pattern updates), or None to skip updates
///
/// # Returns
///
/// The range map is updated in place with merged partition tokens.
pub fn merge_partitions_in_range(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    partitions: &[Token],
    partition_range: Range<usize>,
    num_offsets: usize,
    range_map: &mut RangeMap,
    node_index: Option<Token>,
) {
    let max_len = partition_range.len();
    
    // Merge from smallest to largest partitions
    for len in 1..max_len {
        for start_offset in 0..(max_len - len) {
            let start = partition_range.start + start_offset;
            let end = start + len;
            let range = start..end;

            // Determine partition type based on boundaries
            let has_prefix = start == 0 && partitions.len() > num_offsets;
            let has_postfix = end == partitions.len() - 1 && partitions.len() > num_offsets;

            let index = if has_prefix && start == 0 && end < num_offsets {
                // Merging prefix with infix partitions
                merge_prefix_partition(ctx, offsets, range_map, &range, end, node_index)
            } else if has_postfix && end == num_offsets {
                // Merging infix with postfix partitions
                merge_postfix_partition(ctx, offsets, range_map, &range, start, node_index)
            } else {
                // Normal infix merge between two offsets
                merge_infix_partition(ctx, offsets, range_map, &range, start, end, node_index)
            };

            range_map.insert(range.clone(), index);
            
            // Update node patterns incrementally so subsequent info_partition calls can find them
            if let Some(node_idx) = node_index {
                // Check if this merge creates a partition at a perfect boundary in the node's child patterns
                // If so, update the node's patterns to include this newly merged token
                update_node_patterns_if_perfect(ctx, node_idx, &range, index, range_map);
            }
        }
    }
}

/// Update node patterns if the merged partition is at a perfect boundary.
fn update_node_patterns_if_perfect(
    ctx: &mut NodeJoinCtx,
    node_index: Token,
    range: &Range<usize>,
    _merged_token: Token,
    range_map: &RangeMap,
) {
    // Get the tokens in this range to build the pattern
    let pattern_tokens: Vec<Token> = (range.start..range.end)
        .filter_map(|i| range_map.get(&(i..i+1)).copied())
        .collect();
    
    if pattern_tokens.len() > 1 {
        // This is a merged partition - add or replace pattern in the node
        ctx.trav.add_pattern_with_update(
            node_index,
            Pattern::from(pattern_tokens),
        );
    }
}

/// Merge a prefix partition.
fn merge_prefix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &Range<usize>,
    end: usize,
    _node_index: Option<Token>,
) -> Token {
    let ro = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(end)
        .unwrap();
    
    let prefix_end = Prefix::new(ro);
    let res: Result<PartitionInfo<Pre<Join>>, _> = prefix_end.info_partition(ctx);
    
    match res {
        Ok(info) => {
            let merges = range_map.range_sub_merges(range.clone());
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns: Vec<_> = merges.into_iter().chain(joined).collect();
            ctx.trav.insert_patterns(patterns)
        },
        Err(existing) => existing,
    }
}

/// Merge a postfix partition.
fn merge_postfix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &Range<usize>,
    start: usize,
    _node_index: Option<Token>,
) -> Token {
    let lo = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(start)
        .unwrap();
    
    let postfix_start = Postfix::new(lo);
    let res: Result<PartitionInfo<Post<Join>>, _> = postfix_start.info_partition(ctx);
    
    match res {
        Ok(info) => {
            let merges = range_map.range_sub_merges(range.clone());
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns: Vec<_> = merges.into_iter().chain(joined).collect();
            ctx.trav.insert_patterns(patterns)
        },
        Err(existing) => existing,
    }
}

/// Merge an infix partition between two offsets.
fn merge_infix_partition(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    range_map: &RangeMap,
    range: &Range<usize>,
    start: usize,
    end: usize,
    _node_index: Option<Token>,
) -> Token {
    let lo = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(start)
        .unwrap();
    let ro = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(end)
        .unwrap();
    
    let infix = Infix::new(lo, ro);
    let res: Result<PartitionInfo<In<Join>>, _> = infix.info_partition(ctx);

    match res {
        Ok(info) => {
            let merges = range_map.range_sub_merges(range.clone());
            let num_info_patterns = info.patterns.len();
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns = merges.clone().into_iter().chain(joined).collect_vec();
            debug!(
                num_merges = merges.len(),
                num_info_patterns,
                total_patterns = patterns.len(),
                "Merging infix partition - pattern counts"
            );
            ctx.trav.insert_patterns(patterns)
        },
        Err(existing) => existing,
    }
}
