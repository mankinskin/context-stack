use std::borrow::Borrow;

use linked_hash_map::LinkedHashMap;
use tracing::info;

use crate::{
    join::context::node::merge::{
        PartitionRange,
        context::{
            MergeCtx,
            MergeMode,
        },
    },
    split::{
        Split,
        cache::position::PosKey,
        vertex::TokenTracePositions,
    },
};
use context_trace::*;

impl<'a: 'b, 'b: 'c, 'c> MergeCtx<'a> {
    pub fn merge_node(&'c mut self) -> LinkedHashMap<PosKey, Split> {
        let (_, merges) = self.merge_sub_partitions(
            None, // No target range override for Full mode
        );

        let len = self.offsets.len();
        let index = self.index;
        let mut finals = LinkedHashMap::new();
        for (i, (offset, v)) in self.offsets.iter().enumerate() {
            // Ranges now use partition indices: i..(i+1) convention from RangeMap
            // Left partition: from start (0) to current offset (i+1)
            // Right partition: from current offset (i+1) to end (len+1)
            let lr = PartitionRange::new(0..=i);
            let rr = PartitionRange::new((i + 1)..=len);
            let left = *merges.get(&lr).unwrap();
            let right = *merges.get(&rr).unwrap();
            if !lr.is_empty() || !lr.is_empty() {
                if let Some((&pid, _)) = (v.borrow() as &TokenTracePositions)
                    .iter()
                    .find(|(_, s)| s.inner_offset.is_none())
                {
                    self.ctx.trav.replace_pattern(
                        index.to_pattern_location(pid),
                        vec![left, right],
                    );
                } else {
                    self.ctx.trav.add_pattern_with_update(
                        index,
                        Pattern::from(vec![left, right]),
                    );
                }
            }
            finals.insert(PosKey::new(index, *offset), Split::new(left, right));
        }
        finals
    }

    pub fn merge_root(&mut self) -> Token {
        let root_mode = self.ctx.ctx.interval.cache.root_mode;
        let mut offsets = self.ctx.vertex_cache().clone();
        let root_index = self.ctx.index;
        // Use the target range computed during split phase - it correctly accounts
        // for where target positions fall in the pattern structure
        let target_range = self.ctx.ctx.interval.target_range.clone();
        info!("Starting root merge join");

        let (target_token, _) =
            self.merge_sub_partitions(Some(target_range.clone()));

        info!(?target_token, "Target token extracted from range_map");

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
}

// RangeMap is now imported from the shared merge::RangeMap module
