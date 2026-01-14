//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use std::ops::Range;

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
        context::node::{
            context::NodeJoinCtx,
            merge::RangeMap,
        },
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
use std::borrow::Borrow;
use tracing::{
    debug,
    info,
};

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

    /// Core merge algorithm - now uses shared `merge_partitions_in_range` utility.
    ///
    /// The only difference from intermediary is we extract the target token instead of creating split halves.
    fn merge_partitions(
        &mut self,
        offsets: &SplitVertexCache,
        partitions: &[Token],
        num_offsets: usize,
        target_offset_range: Range<usize>,
    ) -> (RangeMap, Token) {
        let mut range_map = RangeMap::from(partitions);

        // Determine the range of partitions to merge
        let partition_range = 0..partitions.len();

        debug!(
            num_partitions = partitions.len(),
            num_offsets,
            ?partition_range,
            "Using shared merge logic"
        );

        // Use shared merge logic - exactly the same as intermediary!
        super::shared::merge_partitions_in_range(
            self.ctx,
            offsets,
            partitions,
            partition_range,
            num_offsets,
            &mut range_map,
        );

        // Extract target token from range_map
        let target_token = *range_map.get(&target_offset_range)
            .unwrap_or_else(|| panic!(
                "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                target_offset_range,
                range_map.map.keys().collect::<Vec<_>>()
            ));

        info!(?target_token, "Target token extracted from range_map");

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
