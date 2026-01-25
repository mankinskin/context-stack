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
    pub fn merge_node(&mut self) -> LinkedHashMap<PosKey, Split> {
        // Merge all sub-partitions - this handles all pattern creation
        let (_, merges) = self.merge_sub_partitions(None);

        // Extract splits for each offset from the range_map
        let len = self.offsets.len();
        let index = self.ctx.index;
        let mut finals = LinkedHashMap::new();

        for (i, (offset, _)) in self.offsets.iter().enumerate() {
            // Left partition: from start (0) to current offset position (i)
            // Right partition: from after current offset (i+1) to end (len)
            let lr = PartitionRange::new(0..=i);
            let rr = PartitionRange::new((i + 1)..=len);
            let left = *merges.get(&lr).unwrap();
            let right = *merges.get(&rr).unwrap();
            finals.insert(PosKey::new(index, *offset), Split::new(left, right));
        }

        finals
    }

    /// Merge the root node, returning the target token.
    ///
    /// Uses the same merge algorithm as intermediary nodes, but with a
    /// target_range that identifies which partition range contains the
    /// token being inserted.
    pub fn merge_root(&mut self) -> Token {
        let root_index = self.ctx.index;
        // Use the target range computed during split phase
        let target_range = self.ctx.ctx.interval.target_range.clone();
        info!("Starting root merge join");

        let (target_token, _) =
            self.merge_sub_partitions(Some(target_range.clone()));

        info!(?target_token, "Target token extracted from range_map");

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
