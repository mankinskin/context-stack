//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use std::ops::Range;

use derive_new::new;

use crate::{
    join::{
        context::node::{
            context::NodeJoinCtx,
            merge::RangeMap,
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
            RootMode::Prefix => 0..num_offsets,     // Prefix + infixes (no postfix)
            RootMode::Postfix => 1..num_offsets + 1, // Infixes + postfix (no prefix)
            RootMode::Infix => 1..num_offsets,       // Infixes only (no prefix/postfix)
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

        // VERIFICATION: Print actual pattern structures after merge
        self.print_merge_verification(&target_token);

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
            Some(self.ctx.index),  // Pass node index for pattern updates
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

    /// Print verification of merge results - check actual token patterns
    fn print_merge_verification(&self, target_token: &Token) {
        info!("=== MERGE VERIFICATION ===");
        
        // Get root node token
        let root_token = self.ctx.ctx.node_token;
        info!(?root_token, "Root token (should be ababcd)");
        
        // Print root patterns
        if let Some(vertex) = self.ctx.trav.graph().get_index_ref(*root_token) {
            info!("Root vertex patterns:");
            for (pid, pattern) in vertex.iter_patterns_index_ref() {
                let pattern_vec: Vec<Token> = pattern.iter().copied().collect();
                info!(?pid, ?pattern_vec, width=pattern.width(), "  Pattern");
            }
        }
        
        // Try to find cd, bcd, abcd tokens by searching graph
        info!("Searching for expected intermediate tokens:");
        
        // Find all width-2 tokens (should include cd)
        for (token, vertex) in self.ctx.trav.graph().iter() {
            if vertex.width() == 2 {
                info!(?token, width=2, "Found width-2 token (cd candidate)");
                for (pid, pattern) in vertex.iter_patterns_index_ref() {
                    let pattern_vec: Vec<Token> = pattern.iter().copied().collect();
                    info!(?pid, ?pattern_vec, "    Pattern");
                }
            }
        }
        
        // Find all width-3 tokens (should include bcd)  
        for (token, vertex) in self.ctx.trav.graph().iter() {
            if vertex.width() == 3 {
                info!(?token, width=3, "Found width-3 token (bcd candidate)");
                for (pid, pattern) in vertex.iter_patterns_index_ref() {
                    let pattern_vec: Vec<Token> = pattern.iter().copied().collect();
                    info!(?pid, ?pattern_vec, "    Pattern");
                }
            }
        }
        
        // Find all width-4 tokens (should include abcd)
        for (token, vertex) in self.ctx.trav.graph().iter() {
            if vertex.width() == 4 {
                info!(?token, width=4, "Found width-4 token (abcd candidate)");
                for (pid, pattern) in vertex.iter_patterns_index_ref() {
                    let pattern_vec: Vec<Token> = pattern.iter().copied().collect();
                    info!(?pid, ?pattern_vec, "    Pattern");
                }
            }
        }
        
        info!(?target_token, "Target token returned from merge");
        if let Some(vertex) = self.ctx.trav.graph().get_index_ref(*target_token) {
            info!(width=vertex.width(), "Target token width");
            for (pid, pattern) in vertex.iter_patterns_index_ref() {
                let pattern_vec: Vec<Token> = pattern.iter().copied().collect();
                info!(?pid, ?pattern_vec, "  Target pattern");
            }
        }
        
        info!("=== END VERIFICATION ===");
    }
}
