//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandState tracking the current expansion state.
//!

use crate::{
    context::root::RootManager,
    expansion::ExpansionCtx,
};
use context_trace::*;
use tracing::debug;

/// Context for block-based expansion of patterns.
///
/// Contains the RootManager and processes known patterns to find the largest
/// bundled token through overlap detection. Manages block commits directly.
#[derive(Debug)]
pub(crate) struct BlockExpansionCtx {
    /// The root manager (owns graph and root token)
    root: RootManager,
    ctx: ExpansionCtx,
}

impl BlockExpansionCtx {
    /// Create a new block expansion context.
    /// Takes ownership of RootManager to manage block commits.
    pub(crate) fn new(
        root: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");

        // Read the anchor from the RootManager — the last committed token used
        // as the left-side context for overlap detection.
        let anchor = root.anchor();
        debug!(
            has_root = root.root.is_some(),
            anchor = ?anchor,
            "Starting block expansion"
        );

        // Convert the known pattern into a plain Vec<Token> for the new
        // ExpansionCtx cursor loop.
        let atoms: Vec<Token> = known.iter().copied().collect();

        let expansion_ctx =
            ExpansionCtx::new(root.graph.clone(), atoms, anchor);

        Self {
            root,
            ctx: expansion_ctx,
        }
    }

    /// Process the known pattern and commit the result to the root.
    ///
    /// Iterates `ExpansionCtx`, committing each yielded `BandState` to the
    /// `RootManager` immediately (PI-5: commit-before-next invariant).
    /// After each commit, the anchor is refreshed from `RootManager` so that
    /// the next `next()` call sees the up-to-date anchor.
    pub(crate) fn process(&mut self) {
        debug!(
            anchor = ?self.ctx.anchor,
            "BlockExpansionCtx::process start"
        );

        while let Some(state) = self.ctx.next() {
            // Commit the state — this updates root.anchor inside commit_state.
            self.root.commit_state(state);

            // Refresh the anchor in ExpansionCtx from RootManager so the next
            // step sees the newly committed anchor (PI-5 / OQ-5).
            self.ctx.anchor = self.root.anchor();

            debug!(
                new_anchor = ?self.ctx.anchor,
                cursor = self.ctx.cursor,
                "BlockExpansionCtx::process: committed, anchor refreshed"
            );
        }

        debug!(
            cursor = self.ctx.cursor,
            "BlockExpansionCtx::process complete"
        );
    }

    /// Finish processing and return the RootManager.
    pub(crate) fn finish(self) -> RootManager {
        self.root
    }
}
