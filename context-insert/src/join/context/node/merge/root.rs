//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use std::ops::Range;
use std::borrow::Borrow;

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
    TokenTracePositions,
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
        // Target partition is defined by a range of partition indices in the partitions array
        let target_offset_range = match root_mode {
            RootMode::Prefix => 0..1,       // Prefix: just the prefix partition (index 0)
            RootMode::Postfix => {
                // Postfix: ALL partitions (entire postfix range - from first partition to last)
                // For postfix mode, partitions includes all infixes + postfix
                // We want to merge ALL of them into the target token
                0..partitions.len()
            }
            RootMode::Infix => 0..2,        // Infix: between first two offsets  
        };

        debug!(?target_offset_range, num_partitions = partitions.len(), "Target partition offset range");

        // Run the merge algorithm - exactly like intermediary
        // Extract target when we complete the merge of target_offset_range
        let (range_map, target_token) = self.merge_partitions(
            &offsets,
            &partitions,
            num_offsets,
            target_offset_range.clone(),
        );

        // Update root node patterns after merge (like intermediary does)
        // This replaces sequences of merged tokens in the root's child patterns
        self.update_root_patterns_after_merge(&offsets, &range_map);

        // Print actual VertexData child patterns to diagnose pattern issues
        self.print_token_vertex_patterns(target_token);

        info!(?target_token, "Root join complete - returning target token");

        target_token
    }

    /// Update root node patterns after merge completes.
    ///
    /// This checks each offset to see if it aligns perfectly with a pattern boundary,
    /// and if so, replaces that pattern with the merged left+right tokens from range_map.
    fn update_root_patterns_after_merge(
        &mut self,
        offsets: &SplitVertexCache,
        range_map: &RangeMap,
    ) {
        let len = offsets.len();
        let root_index = self.ctx.index;
        
        debug!(num_offsets = len, ?root_index, num_ranges_in_map = range_map.map.len(), "Updating root patterns after merge");

        for (i, (_, v)) in offsets.iter().enumerate() {
            let lr = 0..i;
            let rr = i + 1..len + 1;
            
            debug!(offset_index = i, ?lr, ?rr, "Checking offset for pattern update");
            
            // Get merged tokens from range_map
            if let (Some(&left), Some(&right)) = (range_map.get(&lr), range_map.get(&rr)) {
                debug!(?left, ?right, "Found left and right tokens in range_map");
                
                // Check if this offset is perfect (at pattern boundary)
                if let Some((&pid, _)) = (v.borrow() as &TokenTracePositions)
                    .iter()
                    .find(|(_, s)| s.inner_offset.is_none())
                {
                    debug!(
                        ?pid,
                        ?left,
                        ?right,
                        offset_index = i,
                        "Found perfect border - replacing pattern in root"
                    );
                    self.ctx.trav.replace_pattern(
                        root_index.to_pattern_location(pid),
                        vec![left, right],
                    );
                } else {
                    debug!(
                        ?left,
                        ?right,
                        offset_index = i,
                        "Offset not perfect - adding new pattern to root"
                    );
                    self.ctx.trav.add_pattern_with_update(
                        root_index,
                        Pattern::from(vec![left, right]),
                    );
                }
            } else {
                debug!(offset_index = i, ?lr, ?rr, has_left = range_map.get(&lr).is_some(), has_right = range_map.get(&rr).is_some(), "Missing left or right token in range_map");
            }
        }
    }

    /// Print actual VertexData child patterns for tokens.
    ///
    /// This shows what patterns each vertex ACTUALLY contains in its VertexData,
    /// not what we find it through (search cursor patterns).
    fn print_token_vertex_patterns(&mut self, target: Token) {
        info!("=== VERTEX DATA PATTERNS (actual token child patterns) ===");
        
        // Print root patterns
        let root = self.ctx.index;
        let vertex = self.ctx.trav.expect_vertex(root);
        let patterns = vertex.child_patterns();
        info!(
            node = ?root,
            num_patterns = patterns.len(),
            "Root (ababcd) has {} child pattern(s)", patterns.len()
        );
        for (i, (_pattern_id, pattern)) in patterns.iter().enumerate() {
            let tokens_str: Vec<String> = pattern.iter().map(|t| format!("{:?}", t)).collect();
            info!("  Child Pattern {}: [{}]", i, tokens_str.join(", "));
        }
        
        // Print target token patterns
        let vertex = self.ctx.trav.expect_vertex(target);
        let patterns = vertex.child_patterns();
        info!(
            token = ?target,
            num_patterns = patterns.len(),
            "Target token has {} child pattern(s)", patterns.len()
        );
        for (i, (_pattern_id, pattern)) in patterns.iter().enumerate() {
            let tokens_str: Vec<String> = pattern.iter().map(|t| format!("{:?}", t)).collect();
            info!("  Child Pattern {}: [{}]", i, tokens_str.join(", "));
        }
        
        info!("=== END VERTEX DATA PATTERNS ===");
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
}
