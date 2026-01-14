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
///
/// This function checks if a merged partition has perfect borders in the node's child patterns.
/// We only replace patterns when ALL required offsets for this partition are perfect in the SAME pattern.
/// This ensures we don't prematurely replace patterns when waiting for larger wrapper partitions to form.
fn update_node_patterns_if_perfect(
    _ctx: &mut NodeJoinCtx,
    node_index: Token,
    range: &Range<usize>,
    merged_token: Token,
    range_map: &RangeMap,
) {
    // For now, we defer pattern updates - let the caller (intermediary/root) handle this
    // based on their specific logic for detecting perfect boundaries
    // 
    // The intermediary checks offsets.iter() for inner_offset.is_none() to detect perfect borders
    // The root needs similar logic but with wrapper partition awareness
    //
    // TODO: Implement perfect boundary detection here once we understand the full algorithm
    _ = (node_index, range, merged_token, range_map);
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
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // For Prefix, SinglePerfect contains Option<PatternId>
            // We replace when the right boundary is perfect
            let perfect_pattern_id = info.perfect.0;
            
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns: Vec<_> = merges.iter().cloned().chain(joined).collect();
            let token = ctx.trav.insert_patterns(patterns);
            
            // Replace pattern if right boundary is perfect in a pattern
            if let (Some(pid), Some(node_idx)) = (perfect_pattern_id, _node_index) {
                let pattern_tokens: Vec<Token> = (range.start..range.end)
                    .filter_map(|i| range_map.get(&(i..i+1)).copied())
                    .collect();
                
                if !pattern_tokens.is_empty() {
                    let pattern_loc = node_idx.to_pattern_location(pid);
                    ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
                }
            }
            
            token
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
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // For Postfix, SinglePerfect contains Option<PatternId>
            // We replace when the left boundary is perfect
            let perfect_pattern_id = info.perfect.0;
            
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns: Vec<_> = merges.iter().cloned().chain(joined).collect();
            let token = ctx.trav.insert_patterns(patterns);
            
            // Replace pattern if left boundary is perfect in a pattern
            if let (Some(pid), Some(node_idx)) = (perfect_pattern_id, _node_index) {
                let pattern_tokens: Vec<Token> = (range.start..range.end)
                    .filter_map(|i| range_map.get(&(i..i+1)).copied())
                    .collect();
                
                if !pattern_tokens.is_empty() {
                    let pattern_loc = node_idx.to_pattern_location(pid);
                    ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
                }
            }
            
            token
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
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            let num_info_patterns = info.patterns.len();
            
            // Check if we have BOTH perfect borders in the SAME pattern
            // For infix partitions, DoublePerfect contains (Option<PatternId>, Option<PatternId>)
            // We can only replace when both are Some AND equal (same pattern)
            let perfect_pattern_id = match (info.perfect.0, info.perfect.1) {
                (Some(left_pid), Some(right_pid)) if left_pid == right_pid => Some(left_pid),
                _ => None,
            };
            
            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                Pattern::from(
                    (pinfo.join_pattern(ctx, &pid).borrow() as &'_ Pattern)
                        .iter()
                        .cloned()
                        .collect_vec(),
                )
            });
            let patterns = merges.iter().cloned().chain(joined).collect_vec();
            debug!(
                num_merges = merges.len(),
                num_info_patterns,
                total_patterns = patterns.len(),
                has_perfect = perfect_pattern_id.is_some(),
                "Merging infix partition - pattern counts"
            );
            
            let token = ctx.trav.insert_patterns(patterns);
            
            // Only replace pattern if BOTH offsets are perfect in the SAME pattern
            if let (Some(pid), Some(node_idx)) = (perfect_pattern_id, _node_index) {
                // Build the pattern from individual partitions in range_map
                let pattern_tokens: Vec<Token> = (range.start..range.end)
                    .filter_map(|i| range_map.get(&(i..i+1)).copied())
                    .collect();
                
                if !pattern_tokens.is_empty() {
                    let pattern_loc = node_idx.to_pattern_location(pid);
                    ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
                }
            }
            
            token
        },
        Err(existing) => existing,
    }
}
