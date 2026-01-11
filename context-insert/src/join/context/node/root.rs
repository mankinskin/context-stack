//! Root node join implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.
//!
//! The algorithm is the same as intermediary nodes, with key differences:
//! 1. Determine which initial partitions to create based on mode (protect non-participating ranges)
//! 2. Use same merge loop as intermediary: for len in 1..num_offsets { for start in 0..num_offsets-len+1 }
//! 3. Extract and return the target token instead of creating all split halves

use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::Range,
};

use itertools::Itertools;

use crate::{
    interval::partition::{
        Infix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::role::In,
        },
    },
    join::{
        context::node::context::NodeJoinCtx,
        partition::Join,
    },
    split::{
        cache::vertex::SplitVertexCache,
        vertex::{
            PosSplitCtx,
            output::RootMode,
        },
    },
};
use context_trace::*;
use tracing::{
    debug,
    info,
};

/// RangeMap for tracking merged partitions by offset index range.
/// Exactly matches `NodeMergeCtx::RangeMap` - range is offset INDEX, not atom position.
#[derive(Debug, Default)]
struct RangeMap {
    map: HashMap<Range<usize>, Token>,
}

impl RangeMap {
    /// Initialize with single-element partitions.
    /// Matches the `From<I>` impl in merge.rs - uses i..i range notation.
    fn from_partitions(partitions: &[Token]) -> Self {
        let mut map = HashMap::default();
        for (i, &part) in partitions.iter().enumerate() {
            map.insert(i..i, part);
        }
        Self { map }
    }

    /// Get all 2-way merge combinations for a range.
    /// Exactly matches `range_sub_merges` in merge.rs.
    fn range_sub_merges(
        &self,
        range: Range<usize>,
    ) -> impl IntoIterator<Item = Pattern> + '_ {
        let (start, end) = (range.start, range.end);
        // Note: merge.rs uses range.into_iter() which iterates start..end (not start+1..end)
        // This gives split points at each interior position
        range.into_iter().map(move |ri| {
            let &left = self.map.get(&(start..ri)).unwrap();
            let &right = self.map.get(&(ri..end)).unwrap();
            Pattern::from(vec![left, right])
        })
    }

    fn insert(&mut self, range: Range<usize>, token: Token) {
        self.map.insert(range, token);
    }

    fn get(&self, range: &Range<usize>) -> Option<&Token> {
        self.map.get(range)
    }
}

/// Main entry point for root node joining.
///
/// Reuses intermediary merge algorithm with protection of non-participating ranges.
pub fn join_root_partitions(ctx: &mut NodeJoinCtx) -> Token {
    let root_mode = ctx.ctx.interval.cache.root_mode;
    let offsets = ctx.vertex_cache().clone();
    let num_offsets = offsets.len();
    let root_index = ctx.index;

    info!(
        ?root_mode,
        num_offsets,
        root_index = ?root_index,
        "Starting root join (reusing intermediary algorithm)"
    );

    // Determine which partitions to create based on mode
    let (start_idx, end_idx) = match root_mode {
        RootMode::Prefix => {
            // Prefix: target is BEFORE all offsets
            // Create partitions from 0 to num_offsets (protect last postfix)
            // Target will be at partition 0
            (0, num_offsets)
        }
        RootMode::Postfix => {
            // Postfix: target is AFTER all offsets
            // Create partitions from 1 to num_offsets+1 (protect first prefix)
            // Target will be at partition num_offsets
            (1, num_offsets + 1)
        }
        RootMode::Infix => {
            // Infix: target is in the middle
            // Create all partitions 0 to num_offsets+1 (may protect some based on wrapper analysis)
            // Target will be merged from partitions containing target offsets
            (0, num_offsets + 1)
        }
    };

    debug!(
        start_idx,
        end_idx,
        "Creating initial partitions in index range"
    );

    // Get initial partitions - same as intermediary
    let mut all_partitions = get_initial_partitions(ctx, &offsets);

    // For Prefix/Postfix, we only use a subset of partitions
    let partitions: Vec<Token> = all_partitions
        .drain(start_idx..end_idx)
        .collect();

    debug!(
        num_partitions = partitions.len(),
        ?partitions,
        "Initial partitions (protected ranges excluded)"
    );

    // Run the merge algorithm - exactly like intermediary
    let range_map = merge_partitions(ctx, &offsets, &partitions, start_idx);

    // Extract target token based on mode
    let target_range = match root_mode {
        RootMode::Prefix => 0..0, // First partition at adjusted index 0
        RootMode::Postfix => {
            let adjusted_last = num_offsets - start_idx;
            adjusted_last..adjusted_last
        }
        RootMode::Infix => {
            // Target is between first two offsets
            // After merge, this is at range 0..1
            0..1
        }
    };

    let target_token = range_map.get(&target_range).copied().expect("Target token not found in range_map");

    info!(?target_token, "Root join complete - returning target token");

    target_token
}

/// Core merge algorithm - exactly mirrors `NodeMergeCtx::merge_partitions` from merge.rs.
///
/// The only difference is we extract the target token instead of creating split halves.
fn merge_partitions(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
    partitions: &[Token],
    start_idx: usize, // Offset for partition indices (0 for Infix/Prefix, 1 for Postfix)
) -> RangeMap {
    let num_offsets = offsets.len();
    let adjusted_offsets = num_offsets - start_idx;

    let mut range_map = RangeMap::from_partitions(partitions);

    // Same loop structure as intermediary merge in merge.rs
    for len in 1..adjusted_offsets {
        for start in 0..adjusted_offsets - len + 1 {
            let range = start..start + len;

            // Get offset contexts - adjust indices back to actual offset positions
            let actual_start = start + start_idx;
            let actual_end = start + len + start_idx;

            let lo = offsets
                .iter()
                .map(PosSplitCtx::from)
                .nth(actual_start)
                .unwrap();
            let ro = offsets
                .iter()
                .map(PosSplitCtx::from)
                .nth(actual_end)
                .unwrap();

            // Use Infix::info_partition - same as intermediary
            let infix = Infix::new(lo, ro);
            let res: Result<PartitionInfo<In<Join>>, _> =
                infix.info_partition(ctx);

            let index = match res {
                Ok(info) => {
                    // Get 2-way merges from range_map - same as intermediary
                    let merges = range_map.range_sub_merges(range.clone());

                    // Get patterns from perfect boundaries - same as intermediary
                    let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                        Pattern::from(
                            (pinfo.join_pattern(ctx, &pid).borrow()
                                as &'_ Pattern)
                                .iter()
                                .cloned()
                                .collect_vec(),
                        )
                    });

                    // Combine and insert - same as intermediary
                    let patterns = merges.into_iter().chain(joined).collect_vec();
                    ctx.trav.insert_patterns(patterns)
                },
                Err(existing) => existing,
            };

            range_map.insert(range, index);
        }
    }

    range_map
}

/// Get initial partitions - same approach as intermediary nodes.
fn get_initial_partitions(
    ctx: &mut NodeJoinCtx,
    offsets: &SplitVertexCache,
) -> Vec<Token> {
    let num_offsets = offsets.len();
    let mut partitions = Vec::<Token>::with_capacity(num_offsets + 1);

    // Get split positions
    let mut positions: Vec<_> = offsets.iter().collect();
    positions.sort_by_key(|(pos, _)| *pos);

    // Helper to join a partition and return the token
    let join_part = |ctx: &mut NodeJoinCtx, lo: Option<&PosSplitCtx>, ro: Option<&PosSplitCtx>| -> Token {
        match (lo, ro) {
            (Some(l), Some(r)) => {
                // Infix
                let infix = Infix::new(l, r);
                match infix.info_partition(ctx) {
                    Ok(info) => {
                        let patterns: Vec<Pattern> = info
                            .patterns
                            .into_iter()
                            .map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(ctx, &pid).borrow()
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
                }
            },
            (None, Some(r)) => {
                // Prefix (before first offset)
                let prefix = crate::interval::partition::Prefix::new(r);
                match prefix.info_partition(ctx) {
                    Ok(info) => {
                        let patterns: Vec<Pattern> = info
                            .patterns
                            .into_iter()
                            .map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(ctx, &pid).borrow()
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
                }
            },
            (Some(l), None) => {
                // Postfix (after last offset)
                let postfix = crate::interval::partition::Postfix::new(*l);
                match postfix.info_partition(ctx) {
                    Ok(info) => {
                        let patterns: Vec<Pattern> = info
                            .patterns
                            .into_iter()
                            .map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(ctx, &pid).borrow()
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
                }
            },
            (None, None) => unreachable!("Need at least one offset"),
        }
    };

    // Prefix partition (before first offset)
    let first_split = positions.first().map(|(pos, splits)| PosSplitCtx {
        pos: **pos,
        splits: (*splits.borrow() as &crate::TokenTracePositions).clone(),
    });
    partitions.push(join_part(ctx, None, first_split.as_ref()));

    // Infix partitions (between consecutive offsets)
    for i in 0..num_offsets - 1 {
        let lo = positions.get(i).map(|(pos, splits)| PosSplitCtx {
            pos: **pos,
            splits: (*splits.borrow() as &crate::TokenTracePositions).clone(),
        });
        let ro = positions.get(i + 1).map(|(pos, splits)| PosSplitCtx {
            pos: **pos,
            splits: (*splits.borrow() as &crate::TokenTracePositions).clone(),
        });
        partitions.push(join_part(ctx, lo.as_ref(), ro.as_ref()));
    }

    // Postfix partition (after last offset)
    let last_split = positions.last().map(|(pos, splits)| PosSplitCtx {
        pos: **pos,
        splits: (*splits.borrow() as &crate::TokenTracePositions).clone(),
    });
    partitions.push(join_part(ctx, last_split.as_ref(), None));

    partitions
}

