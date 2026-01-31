use linked_hash_map::LinkedHashMap;
use tracing::info;

use crate::{
    join::context::node::merge::{
        PartitionRange,
        context::MergeCtx,
    },
    split::{
        Split,
        cache::position::PosKey,
    },
};
use context_trace::*;

impl<'a> MergeCtx<'a> {
    /// Merge an intermediary node, returning splits for each offset.
    ///
    /// This uses the same merge algorithm as root nodes - `merge_sub_partitions`
    /// handles all pattern creation and replacement. This function just extracts
    /// the Split results from the range_map for use by parent nodes.
    ///
    /// Splits for merged tokens are added to the shared splits map during merging,
    /// so they're available for subsequent pattern operations.
    pub fn merge_node(&mut self) -> LinkedHashMap<PosKey, Split> {
        // Merge all sub-partitions - this handles all pattern creation
        // and adds splits for merged tokens to the shared map
        let result = self.merge_sub_partitions(None);

        // Extract splits for each offset from the range_map
        let len = self.offsets.len();
        let index = self.ctx.index;

        // Collect offsets first to avoid borrow issues
        let offsets: Vec<_> = self
            .offsets
            .iter()
            .enumerate()
            .map(|(i, (offset, _))| (i, *offset))
            .collect();

        let mut split_result = LinkedHashMap::new();
        let merges = result.range_map;

        for (i, offset) in offsets {
            // Left partition: from start (0) to current offset position (i)
            // Right partition: from after current offset (i+1) to end (len)
            let lr = PartitionRange::new(0..=i);
            let rr = PartitionRange::new((i + 1)..=len);
            let left = *merges.get(&lr).unwrap();
            let right = *merges.get(&rr).unwrap();
            let key = PosKey::new(index, offset);
            let split = Split::new(left, right);
            // Add to shared map so parent nodes can access
            self.add_split(key, split.clone());
            // Also collect for return
            split_result.insert(key, split);
        }

        split_result
    }

    /// Merge the root node, returning the target token.
    ///
    /// Uses the same merge algorithm as intermediary nodes, but with a
    /// target_range that identifies which partition range contains the
    /// token being inserted.
    ///
    /// After merging, adds a new pattern to the root node that includes
    /// the target token (for Prefix, Postfix, and Infix modes).
    pub fn merge_root(&mut self) -> Token {
        let root_index = self.ctx.index;
        // Use the target range computed during split phase
        let target_range = self.ctx.ctx.interval.target_range.clone();
        info!("Starting root merge join");

        let result = self.merge_sub_partitions(Some(target_range.clone()));
        let target_token = result.target_token;
        let range_map = result.range_map;
        let had_perfect_replacement = result.had_perfect_replacement;

        info!(?target_token, "Target token extracted from range_map");

        // Add root pattern that includes the target token
        // This connects the target token to the root in the graph
        // SKIP if a perfect replacement already modified the pattern in place
        if !had_perfect_replacement {
            self.add_root_pattern(&range_map, target_token);
        } else {
            info!("Skipping add_root_pattern - pattern was already modified by replace_in_pattern");
        }

        // Debug: Print actual VertexData child patterns
        self.print_token_vertex_patterns(target_token);
        self.print_token_vertex_patterns(root_index);

        info!(?target_token, "Root join complete - returning target token");

        target_token
    }

    /// Print actual VertexData child patterns for tokens (debug helper).
    fn print_token_vertex_patterns(
        &self,
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
}

// RangeMap is now imported from the shared merge::RangeMap module
