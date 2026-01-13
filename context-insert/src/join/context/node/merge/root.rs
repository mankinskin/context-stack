//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use std::{
    borrow::Borrow,
    collections::HashMap,
    ops::Range,
};

use derive_new::new;
use itertools::Itertools;

use crate::{
    interval::partition::{
        Infix,
        info::{
            InfoPartition,
            PartitionInfo,
            range::role::{In, Post, Pre},
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
    /// Matches the `From<I>` impl in intermediary.rs - uses i..i range notation.
    fn from_partitions(partitions: &[Token]) -> Self {
        let mut map = HashMap::default();
        for (i, &part) in partitions.iter().enumerate() {
            map.insert(i..i, part);
        }
        Self { map }
    }

    /// Get all 2-way merge combinations for a range.
    /// Exactly matches `range_sub_merges` in intermediary.rs.
    fn range_sub_merges(
        &self,
        range: Range<usize>,
    ) -> impl IntoIterator<Item = Pattern> + '_ {
        let (start, end) = (range.start, range.end);
        // Iterate over interior split points only (not boundaries)
        // For range 0..2, we want split point at 1, giving (0..1) + (1..2)
        (start + 1..end).map(move |ri| {
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

/// Root merge context - follows same pattern as NodeMergeCtx but extracts target token.
#[derive(Debug, new)]
pub struct RootMergeCtx<'a: 'b, 'b> {
    pub ctx: &'b mut NodeJoinCtx<'a>,
}

impl<'a: 'b, 'b> RootMergeCtx<'a, 'b> {
    /// Main entry point for root node joining.
    ///
    /// Reuses intermediary merge algorithm with protection of non-participating ranges.
    pub fn merge_root(&mut self) -> Token {
        let root_mode = self.ctx.ctx.interval.cache.root_mode;
        let offsets = self.ctx.vertex_cache().clone();
        let num_offsets = offsets.len();
        let root_index = self.ctx.index;

        info!(
            ?root_mode,
            num_offsets,
            root_index = ?root_index,
            "Starting root join (reusing intermediary algorithm)"
        );

        // Create initial partitions with protection
        // - Prefix: Don't create postfix partition (protect range after last offset)
        // - Postfix: Don't create prefix partition (protect range before first offset)
        // - Infix: Don't create either end unless needed for wrappers
        let (create_prefix, create_postfix) = match root_mode {
            RootMode::Prefix => (true, false),  // Target IS the prefix
            RootMode::Postfix => (false, true), // Target IS the postfix
            RootMode::Infix => (false, false),  // Target is between offsets, protect both ends
        };

        debug!(
            create_prefix,
            create_postfix,
            "Protection strategy for initial partitions"
        );

        // Get initial partitions with protection
        let partitions = self.get_initial_partitions(&offsets, create_prefix, create_postfix);

        debug!(
            num_partitions = partitions.len(),
            expected = if create_prefix && create_postfix { num_offsets + 1 } else { num_offsets },
            "Initial partitions created"
        );

        // Define target offset range based on mode
        // Target partition is defined by a range of offsets (in offset index space)
        let target_offset_range = match root_mode {
            RootMode::Prefix => 0..1,       // Prefix: from start (0) to first offset (1)
            RootMode::Postfix => {
                // Postfix: from last offset to end
                // Target is the entire postfix range - all partitions from first offset to end
                if num_offsets == 0 {
                    0..1
                } else {
                    0..(partitions.len() - 1)
                }
            }
            RootMode::Infix => 0..2,        // Infix: between first two offsets
        };

        debug!(?target_offset_range, num_partitions = partitions.len(), "Target partition offset range");

        // Run the merge algorithm - exactly like intermediary
        // Extract target when we complete the merge of target_offset_range
        let (_range_map, target_token) = self.merge_partitions(
            &offsets,
            &partitions,
            num_offsets,
            target_offset_range.clone(),
        );

        info!(?target_token, "Root join complete - returning target token");

        target_token
    }

    /// Core merge algorithm - exactly mirrors `NodeMergeCtx::merge_partitions` from intermediary.rs.
    ///
    /// The only difference is we extract the target token instead of creating split halves.
    fn merge_partitions(
        &mut self,
        offsets: &SplitVertexCache,
        partitions: &[Token],
        num_offsets: usize,
        target_offset_range: Range<usize>,
    ) -> (RangeMap, Token) {
        let mut range_map = RangeMap::from_partitions(partitions);
        let mut target_token = None;

        // Determine the maximum merge length based on how many partitions we have
        let max_len = partitions.len();

        debug!(
            num_partitions = partitions.len(),
            num_offsets,
            max_len,
            "Merge loop bounds"
        );

        // Same loop structure as intermediary merge in intermediary.rs
        // Merges partitions from smallest to largest, but up to max_len instead of num_offsets
        for len in 1..max_len {
            for start in 0..(max_len - len) {
                let range = start..start + len;

                // Check if this range includes prefix or postfix boundaries
                let has_prefix = start == 0 && partitions.len() > num_offsets;
                let has_postfix = start + len == partitions.len() - 1 && partitions.len() > num_offsets;

                let index = if has_prefix && start == 0 && start + len < num_offsets {
                    // Merging prefix with infix partitions: use Prefix partition type
                    let ro = offsets
                        .iter()
                        .map(PosSplitCtx::from)
                        .nth(start + len)
                        .unwrap();
                    let prefix_end = crate::interval::partition::Prefix::new(ro);
                    let res: Result<PartitionInfo<Pre<Join>>, _> = prefix_end.info_partition(self.ctx);
                    
                    match res {
                        Ok(info) => {
                            let merges = range_map.range_sub_merges(range.clone());
                            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(self.ctx, &pid).borrow()
                                        as &'_ Pattern)
                                        .iter()
                                        .cloned()
                                        .collect_vec(),
                                )
                            });
                            let patterns: Vec<_> = merges.into_iter().chain(joined).collect();
                            self.ctx.trav.insert_patterns(patterns)
                        },
                        Err(existing) => existing,
                    }
                } else if has_postfix && start + len == num_offsets {
                    // Merging infix with postfix partitions: use Postfix partition type  
                    let lo = offsets
                        .iter()
                        .map(PosSplitCtx::from)
                        .nth(start)
                        .unwrap();
                    let postfix_start = crate::interval::partition::Postfix::new(lo);
                    let res: Result<PartitionInfo<Post<Join>>, _> = postfix_start.info_partition(self.ctx);
                    
                    match res {
                        Ok(info) => {
                            let merges = range_map.range_sub_merges(range.clone());
                            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(self.ctx, &pid).borrow()
                                        as &'_ Pattern)
                                        .iter()
                                        .cloned()
                                        .collect_vec(),
                                )
                            });
                            let patterns: Vec<_> = merges.into_iter().chain(joined).collect();
                            self.ctx.trav.insert_patterns(patterns)
                        },
                        Err(existing) => existing,
                    }
                } else {
                    // Normal infix merge between two offsets
                    let lo = offsets
                        .iter()
                        .map(PosSplitCtx::from)
                        .nth(start)
                        .unwrap();
                    let ro = offsets
                        .iter()
                        .map(PosSplitCtx::from)
                        .nth(start + len)
                        .unwrap();
                    
                    let infix = Infix::new(lo, ro);
                    let res: Result<PartitionInfo<In<Join>>, _> = infix.info_partition(self.ctx);

                    match res {
                        Ok(info) => {
                            // Get 2-way merges from range_map - same as intermediary
                            let merges = range_map.range_sub_merges(range.clone());

                            // Get patterns from perfect boundaries - same as intermediary
                            let joined = info.patterns.into_iter().map(|(pid, pinfo)| {
                                Pattern::from(
                                    (pinfo.join_pattern(self.ctx, &pid).borrow()
                                        as &'_ Pattern)
                                        .iter()
                                        .cloned()
                                        .collect_vec(),
                                )
                            });

                            // Combine and insert - same as intermediary
                            let patterns = merges.into_iter().chain(joined).collect_vec();
                            self.ctx.trav.insert_patterns(patterns)
                        },
                        Err(existing) => existing,
                    }
                };

                range_map.insert(range.clone(), index);

                // Check if we just merged the target partition
                if range == target_offset_range {
                    debug!(?range, ?index, "Extracted target token from merge");
                    target_token = Some(index);
                }
            }
        }

        let target_token = target_token.expect("Target token was never extracted during merge");
        (range_map, target_token)
    }

    /// Get initial partitions with protection of non-participating ranges.
    ///
    /// Creates partitions between consecutive offsets, with optional prefix/postfix.
    fn get_initial_partitions(
        &mut self,
        offsets: &SplitVertexCache,
        create_prefix: bool,
        create_postfix: bool,
    ) -> Vec<Token> {
        let num_offsets = offsets.len();
        let mut partitions = Vec::<Token>::with_capacity(num_offsets + 1);

        // Get split positions in order
        let mut offset_ctxs: Vec<_> = offsets
            .iter()
            .map(PosSplitCtx::from)
            .collect();
        offset_ctxs.sort_by_key(|ctx| ctx.pos);

        // Create prefix partition (before first offset) if requested
        if create_prefix {
            let first_offset = offset_ctxs[0];
            let prefix = crate::interval::partition::Prefix::new(first_offset);
            let res: Result<PartitionInfo<Pre<Join>>, _> = prefix.info_partition(self.ctx);
            let prefix_token = match res {
                Ok(part_info) => {
                    let patterns: Vec<Pattern> = part_info
                        .patterns
                        .into_iter()
                        .map(|(pid, pat_info)| {
                            Pattern::from(
                                (pat_info.join_pattern(self.ctx, &pid).borrow()
                                    as &'_ Pattern)
                                    .iter()
                                    .cloned()
                                    .collect_vec(),
                            )
                        })
                        .collect();
                    self.ctx.trav.insert_patterns(patterns)
                },
                Err(existing) => existing,
            };
            partitions.push(prefix_token);
        }

        // Create infix partitions between consecutive offsets
        for i in 0..num_offsets - 1 {
            let lo = offset_ctxs[i];
            let ro = offset_ctxs[i + 1];
            let infix = Infix::new(lo, ro);
            let res: Result<PartitionInfo<In<Join>>, _> = infix.info_partition(self.ctx);
            let infix_token = match res {
                Ok(part_info) => {
                    let patterns: Vec<Pattern> = part_info
                        .patterns
                        .into_iter()
                        .map(|(pid, pat_info)| {
                            Pattern::from(
                                (pat_info.join_pattern(self.ctx, &pid).borrow()
                                    as &'_ Pattern)
                                    .iter()
                                    .cloned()
                                    .collect_vec(),
                            )
                        })
                        .collect();
                    self.ctx.trav.insert_patterns(patterns)
                },
                Err(existing) => existing,
            };
            partitions.push(infix_token);
        }

        // Create postfix partition (after last offset) if requested
        if create_postfix {
            let last_offset = offset_ctxs[num_offsets - 1];
            let postfix = crate::interval::partition::Postfix::new(last_offset);
            let res: Result<PartitionInfo<Post<Join>>, _> = postfix.info_partition(self.ctx);
            let postfix_token = match res {
                Ok(part_info) => {
                    let patterns: Vec<Pattern> = part_info
                        .patterns
                        .into_iter()
                        .map(|(pid, pat_info)| {
                            Pattern::from(
                                (pat_info.join_pattern(self.ctx, &pid).borrow()
                                    as &'_ Pattern)
                                    .iter()
                                    .cloned()
                                    .collect_vec(),
                            )
                        })
                        .collect();
                    self.ctx.trav.insert_patterns(patterns)
                },
                Err(existing) => existing,
            };
            partitions.push(postfix_token);
        }

        partitions
    }
}
