//! Root node merge implementation.
//!
//! This module implements root node joining by reusing the intermediary merge algorithm
//! with protection of non-participating ranges.

use derive_new::new;

use crate::join::context::node::{
    context::NodeJoinCtx,
    merge::shared::MergeMode,
};
use context_trace::*;
use tracing::info;

/// Root merge context - follows same pattern as NodeMergeCtx but extracts target token.
#[derive(Debug, new)]
pub struct RootMergeCtx<'a: 'b, 'b> {
    pub ctx: &'b mut NodeJoinCtx<'a>,
}

impl<'a: 'b, 'b> RootMergeCtx<'a, 'b> {
    /// Main entry point for root node joining.
    ///
    /// Reuses intermediary merge algorithm with protection of non-participating ranges.
    pub fn join(&mut self) -> Token {
        let root_mode = self.ctx.ctx.interval.cache.root_mode;
        let offsets = self.ctx.vertex_cache().clone();
        let root_index = self.ctx.index;
        //let target_partition_range = self.ctx.ctx.interval.target_range.clone();
        info!("Starting root merge join");

        let (target_token, _) = super::shared::merge_partitions_in_range(
            self.ctx,
            &offsets,
            MergeMode::Root(root_mode),
        );

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
