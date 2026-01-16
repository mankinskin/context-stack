//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use std::{
    borrow::Borrow,
    ops::Range,
};

use derive_new::new;
use tracing_subscriber::field::debug;

use crate::{
    TokenTracePositions,
    join::context::node::{
        context::NodeJoinCtx,
        merge::{
            PartitionRange,
            RangeMap,
        },
    },
    split::{
        cache::vertex::SplitVertexCache,
        vertex::output::RootMode,
    },
};
use context_trace::*;
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

        // Determine partition range based on root_mode
        // This controls which initial partitions to create (with protection)
        let partition_range = match root_mode {
            RootMode::Prefix => 0..num_offsets, // Prefix + infixes (no postfix)
            RootMode::Postfix => 1..num_offsets + 1, // Infixes + postfix (no prefix)
            RootMode::Infix => 1..num_offsets, // Infixes only (no prefix/postfix)
        };

        debug!(
            ?partition_range,
            "Protection strategy - partition range for initial partitions"
        );
        // Get initial partitions using shared function
        let partitions = super::shared::create_initial_partitions(
            self.ctx,
            &offsets,
            partition_range.clone(),
        );

        // Define target partition range based on mode
        // Target is defined by a range of partition indices in the partitions array
        // We use partition indices throughout - NOT offset indices
        //
        // The target is what we're INSERTING, not necessarily all created partitions.
        // For modes with protection (prefix/postfix), we exclude the first/last partition.
        let target_partition_range = if partitions.len() == 1 {
            PartitionRange::new(0..partitions.len()) // Edge case: only one partition
        } else {
            match root_mode {
                RootMode::Prefix => {
                    // Prefix mode: partition_range is 0..num_offsets (prefix + infixes, no postfix)
                    // Partitions created: [prefix, infix1, infix2, ...]
                    // Target: infixes only (exclude protected prefix at index 0)
                    // Example: [ab, c, d] → target is [1..3] = [c, d] = "cd"
                    PartitionRange::new(1..partitions.len())
                },
                RootMode::Postfix => {
                    // Postfix mode: skip first partition (wrapper that merges with prefix)
                    // Partitions array: [a, b, cd] at indices [0, 1, 2]
                    // Target: [1..3] extracts partitions[1..3] = [b, cd] = "bcd"
                    PartitionRange::new(1..partitions.len())
                },
                RootMode::Infix => {
                    // Infix mode: all partitions are infix, target is all of them
                    // Partitions array: [a, b, c] at indices [0, 1, 2]
                    // Target: [0..3] extracts all partitions = "abc"
                    PartitionRange::new(0..partitions.len())
                },
            }
        };

        debug!(
            ?target_partition_range,
            num_partitions = partitions.len(),
            "Target partition range (partition indices)"
        );

        // Run the merge algorithm - exactly like intermediary
        // Extract target when we complete the merge of target_partition_range
        // RangeMap uses array indices into partitions array
        let target_token = self.merge_partitions(
            &offsets,
            &partitions,
            partition_range,
            target_partition_range.clone(),
        );

        // Print actual VertexData child patterns to diagnose pattern issues
        self.print_token_vertex_patterns(target_token);
        self.print_token_vertex_patterns(root_index);

        info!(?target_token, "Root join complete - returning target token");

        target_token
    }

    /// Print actual VertexData child patterns for tokens.
    ///
    /// This shows what patterns each vertex ACTUALLY contains in its VertexData,
    /// not what we find it through (search cursor patterns).
    fn print_token_vertex_patterns(
        &mut self,
        target: Token,
    ) {
        info!("=== VERTEX DATA PATTERNS (actual token child patterns) ===");

        // Print root patterns
        let root = self.ctx.index;
        let vertex = self.ctx.trav.expect_vertex_data(root);
        let patterns = vertex.child_patterns();
        info!(
            node = ?root,
            num_patterns = patterns.len(),
            "Root has {} child pattern(s)", patterns.len()
        );
        for (i, (_pattern_id, pattern)) in patterns.iter().enumerate() {
            let tokens_str: Vec<String> =
                pattern.iter().map(|t| format!("{:?}", t)).collect();
            info!("  Child Pattern {}: [{}]", i, tokens_str.join(", "));
        }

        // Print target token patterns
        let vertex = self.ctx.trav.expect_vertex_data(target);
        let patterns = vertex.child_patterns();
        info!(
            token = ?target,
            num_patterns = patterns.len(),
            "Target token has {} child pattern(s)", patterns.len()
        );
        for (i, (_pattern_id, pattern)) in patterns.iter().enumerate() {
            let tokens_str: Vec<String> =
                pattern.iter().map(|t| format!("{:?}", t)).collect();
            info!("  Child Pattern {}: [{}]", i, tokens_str.join(", "));
        }

        info!("=== END VERTEX DATA PATTERNS ===");
    }

    /// Core merge algorithm - now uses shared `merge_partitions_in_range` utility.
    ///
    /// The only difference from intermediary is we extract the target token instead of creating split halves.
    ///
    /// # Parameters
    ///
    /// - `partition_range_for_creation`: Range of partition indices that were created (only used to determine has_prefix)
    /// - `target_partition_range`: Array indices into partitions that constitute the target token
    fn merge_partitions(
        &mut self,
        offsets: &SplitVertexCache,
        partitions: &[Token],
        partition_range_for_creation: Range<usize>,
        target_partition_range: PartitionRange,
    ) -> Token {
        // Initialize range_map with simple array indices
        // For partitions=[a, b, cd] at array indices [0, 1, 2]:
        // - partitions[0] → PartitionRange(0..1) for "a"
        // - partitions[1] → PartitionRange(1..2) for "b"
        // - partitions[2] → PartitionRange(2..3) for "cd"
        let mut range_map = RangeMap::default();
        for (i, &token) in partitions.iter().enumerate() {
            range_map.insert(PartitionRange::new(i..(i + 1)), token);
        }

        debug!(
            num_partitions = partitions.len(),
            ?partition_range_for_creation,
            ?target_partition_range,
            "Using shared merge logic with array indices"
        );

        // Determine has_prefix and has_postfix from partition_range_for_creation
        // Whether the initial partitions include a prefix/postfix partition
        let has_prefix = partition_range_for_creation.start == 0;
        let has_postfix = partition_range_for_creation.end
            == partition_range_for_creation.start + partitions.len();
        assert!(
            !(has_prefix && has_postfix),
            "Cannot have both prefix and postfix simultaneously in root merge"
        );
        debug!(
            has_prefix,
            has_postfix, "Determined protection flags for merge"
        );
        // Use shared merge logic with array indices [0..partitions.len()]
        // partition_range_for_creation only used to determine has_prefix flag
        super::shared::merge_partitions_in_range(
            self.ctx,
            offsets,
            partitions,
            &mut range_map,
            has_prefix,
            has_postfix,
        );

        // Extract target token from range_map
        let target_token = *range_map.get(&target_partition_range)
            .unwrap_or_else(|| panic!(
                "Target token not found in range_map for range {:?}. Available ranges: {:?}",
                target_partition_range,
                range_map.map.keys().collect::<Vec<_>>()
            ));

        info!(?target_token, "Target token extracted from range_map");

        target_token
    }
}
