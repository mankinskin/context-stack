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
            Err(existing) => {
                // Token already exists - need to add patterns to it!
                // Re-call info_partition to try to get patterns from node structure
                // Even though it returns Err(existing), we should still try to build
                // patterns from the node's child patterns at this location
                debug!(?existing, "PREFIX partition token already exists in create_initial_partitions");
                // For now, just return existing - patterns will be added during merge
                existing
            },
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
    debug!(
        partition_range_start = partition_range.start,
        partition_range_end = partition_range.end,
        num_offsets,
        num_partitions = partitions.len(),
        has_node_index = node_index.is_some(),
        "merge_partitions_in_range: ENTERED"
    );
    
    let max_len = partition_range.len();
    
    // Merge from smallest to largest partitions
    // Start at len=2 because len=1 (single partitions) are already created in create_initial_partitions
    for len in 2..=max_len {
        debug!(len, max_len, "merge_partitions_in_range: starting merge loop iteration");
        // For each partition length, iterate over all valid starting positions
        // E.g., with max_len=2, len=2: start_offset in 0..1 gives start_offset=0, creating range 0..2
        for start_offset in 0..(max_len - len + 1) {
            let start = partition_range.start + start_offset;
            let end = start + len;
            let range = start..end;

            // Determine partition type based on boundaries
            let has_prefix = start == 0 && partitions.len() > num_offsets;
            let has_postfix = end == partitions.len() && partitions.len() > num_offsets;

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
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_tokens = vec![token];
                let pattern_loc = node_idx.to_pattern_location(pid);
                debug!(
                    ?node_idx,
                    ?pid,
                    ?pattern_loc,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "PREFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
            }
            
            token
        },
        Err(existing) => {
            debug!(
                ?existing,
                range_start = range.start,
                range_end = range.end,
                "PREFIX: Token already exists - need to merge subranges and build new token"
            );
            
            // When info_partition returns Err(existing), we need to build the merged token ourselves
            // by combining patterns from:
            // 1. range_sub_merges - patterns from previously merged subranges
            // 2. Node's child patterns - by merging the subrange tokens
            
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // Build the merged token from subranges in range_map
            // For prefix partition spanning range start..end, we need to merge the subranges:
            // e.g., for range 1..3: merge (1..2) with (2..3) to create (1..3)
            let merged_patterns = if range.end > range.start + 1 {
                // Multi-partition range - merge subranges from range_map
                let subrange_tokens: Vec<Token> = (range.start..range.end)
                    .map(|i| {
                        let subrange = i..(i + 1);
                        *range_map.get(&subrange).expect("subrange should exist in range_map")
                    })
                    .collect();
                
                debug!(
                    ?subrange_tokens,
                    num_subranges = subrange_tokens.len(),
                    "PREFIX: Merging subrange tokens to create new merged token"
                );
                
                // Create pattern from merged subrange tokens
                vec![Pattern::from(subrange_tokens)]
            } else {
                // Single-partition range - just use merges from range_sub_merges
                Vec::new()
            };
            
            // Combine all patterns: range_sub_merges + merged subranges
            let all_patterns: Vec<_> = merges.iter().cloned()
                .chain(merged_patterns.into_iter())
                .collect();
            
            if !all_patterns.is_empty() {
                debug!(
                    num_patterns = all_patterns.len(),
                    "PREFIX: Creating new merged token with combined patterns"
                );
                ctx.trav.insert_patterns(all_patterns)
            } else {
                debug!(
                    ?existing,
                    "PREFIX: No patterns to add, returning existing token"
                );
                existing
            }
        },
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
    debug!(
        range_start = range.start,
        range_end = range.end,
        start,
        has_node_idx = _node_index.is_some(),
        "merge_postfix_partition: ENTERED"
    );
    
    let lo = offsets
        .iter()
        .map(PosSplitCtx::from)
        .nth(start)
        .unwrap();
    
    let postfix_start = Postfix::new(lo);
    let res: Result<PartitionInfo<Post<Join>>, _> = postfix_start.info_partition(ctx);
    
    debug!(
        is_ok = res.is_ok(),
        " merge_postfix_partition: info_partition result"
    );
    
    match res {
        Ok(info) => {
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // For Postfix, SinglePerfect contains Option<PatternId>
            // We replace when the left boundary is perfect
            let perfect_pattern_id = info.perfect.0;
            
            debug!(
                ?perfect_pattern_id,
                has_node_idx = _node_index.is_some(),
                range_start = range.start,
                range_end = range.end,
                num_patterns = info.patterns.len(),
                num_merges = merges.len(),
                "POSTFIX merge_postfix_partition: perfect border check"
            );
            
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
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_tokens = vec![token];
                let pattern_loc = node_idx.to_pattern_location(pid);
                debug!(
                    ?node_idx,
                    ?pid,
                    ?pattern_loc,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "POSTFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
            }
            
            token
        },
        Err(existing) => {
            debug!(
                ?existing,
                range_start = range.start,
                range_end = range.end,
                "POSTFIX: Token already exists - need to merge subranges and build new token"
            );
            
            // When info_partition returns Err(existing), we need to build the merged token ourselves
            // by combining patterns from:
            // 1. range_sub_merges - patterns from previously merged subranges
            // 2. Node's child patterns - by merging the subrange tokens
            
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // Build the merged token from subranges in range_map
            // For postfix partition spanning range start..end, we need to merge the subranges:
            // e.g., for range 1..3: merge (1..2) with (2..3) to create (1..3)
            let merged_patterns = if range.end > range.start + 1 {
                // Multi-partition range - merge subranges from range_map
                let subrange_tokens: Vec<Token> = (range.start..range.end)
                    .map(|i| {
                        let subrange = i..(i + 1);
                        *range_map.get(&subrange).expect("subrange should exist in range_map")
                    })
                    .collect();
                
                debug!(
                    ?subrange_tokens,
                    num_subranges = subrange_tokens.len(),
                    "POSTFIX: Merging subrange tokens to create new merged token"
                );
                
                // Create pattern from merged subrange tokens
                vec![Pattern::from(subrange_tokens)]
            } else {
                // Single-partition range - just use merges from range_sub_merges
                Vec::new()
            };
            
            // Combine all patterns: range_sub_merges + merged subranges
            let all_patterns: Vec<_> = merges.iter().cloned()
                .chain(merged_patterns.into_iter())
                .collect();
            
            if !all_patterns.is_empty() {
                debug!(
                    num_patterns = all_patterns.len(),
                    "POSTFIX: Creating new merged token with combined patterns"
                );
                ctx.trav.insert_patterns(all_patterns)
            } else {
                debug!(
                    ?existing,
                    "POSTFIX: No patterns to add, returning existing token"
                );
                existing
            }
        },
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
                // Build pattern replacement: should be the newly merged token at this range
                let pattern_tokens = vec![token];
                let pattern_loc = node_idx.to_pattern_location(pid);
                debug!(
                    ?node_idx,
                    ?pid,
                    ?pattern_loc,
                    ?token,
                    range_start = range.start,
                    range_end = range.end,
                    "INFIX: Replacing pattern with merged token"
                );
                ctx.trav.replace_pattern(pattern_loc, pattern_tokens);
            }
            
            token
        },
        Err(existing) => {
            debug!(
                ?existing,
                range_start = range.start,
                range_end = range.end,
                "INFIX: Token already exists - need to merge subranges and build new token"
            );
            
            // When info_partition returns Err(existing), we need to build the merged token ourselves
            // by combining patterns from:
            // 1. range_sub_merges - patterns from previously merged subranges
            // 2. Node's child patterns - by merging the subrange tokens
            
            let merges: Vec<_> = range_map.range_sub_merges(range.clone()).into_iter().collect();
            
            // Build the merged token from subranges in range_map
            // For infix partition spanning range start..end, we need to merge the subranges:
            // e.g., for range 1..3: merge (1..2) with (2..3) to create (1..3)
            let merged_patterns = if range.end > range.start + 1 {
                // Multi-partition range - merge subranges from range_map
                let subrange_tokens: Vec<Token> = (range.start..range.end)
                    .map(|i| {
                        let subrange = i..(i + 1);
                        *range_map.get(&subrange).expect("subrange should exist in range_map")
                    })
                    .collect();
                
                debug!(
                    ?subrange_tokens,
                    num_subranges = subrange_tokens.len(),
                    "INFIX: Merging subrange tokens to create new merged token"
                );
                
                // Create pattern from merged subrange tokens
                vec![Pattern::from(subrange_tokens)]
            } else {
                // Single-partition range - just use merges from range_sub_merges
                Vec::new()
            };
            
            // Combine all patterns: range_sub_merges + merged subranges
            let all_patterns: Vec<_> = merges.iter().cloned()
                .chain(merged_patterns.into_iter())
                .collect();
            
            if !all_patterns.is_empty() {
                debug!(
                    num_patterns = all_patterns.len(),
                    "INFIX: Creating new merged token with combined patterns"
                );
                ctx.trav.insert_patterns(all_patterns)
            } else {
                debug!(
                    ?existing,
                    "INFIX: No patterns to add, returning existing token"
                );
                existing
            }
        },
    }
}
