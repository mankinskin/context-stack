//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandState tracking the current expansion state.
//!

use crate::{
    context::root::RootManager,
    expansion::{
        ExpansionCtx, chain::BandState,
    },
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
    /// The known pattern to process
    known: Pattern,
    ctx: ExpansionCtx,
}

impl<'a> BlockExpansionCtx {
    /// Create a new block expansion context.
    /// Takes ownership of RootManager to manage block commits.
    pub(crate) fn new(
        root: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");

        debug!(
            known_len = known.len(),
            "BlockExpansionCtx::process starting"
        );

        assert!(!known.is_empty(), "Cannot process empty pattern");

        // Set up cursor for the known pattern
        let path = PatternEndPath::new(known.clone(), Default::default());
        let cursor = path.into_range(0);

        // Get the last token from the existing root (if any) to use as overlap anchor
        let root_last_token = root.last_child_token();
        debug!(
            has_root = root.root.is_some(),
            root_last_token = ?root_last_token,
            "Starting block expansion"
        );

        // Create expansion context with root's last token for overlap detection
        let graph = root.graph.clone();
        Self {
            root,
            known,
            ctx: ExpansionCtx::new(graph, cursor, root_last_token.map(BandState::new)),
        }
    }

    /// Process the known pattern and commit the result to the root.
    /// Uses overlap expansion and commits when an overlap is found.
    pub(crate) fn process(&mut self) {

        let first = self.ctx.state.anchor_token();
        debug!(state = ?self.ctx.state, ?first, "expansion state before processing");

        // Process expansions - when overlap found, commit and continue
        while let Some(state) = self.ctx.next() {
            self.root.commit_state(state);
        }
        debug!(
            final_state = ?self.ctx.state,
            "BlockExpansionCtx::process complete"
        );
    }

    /// Finish processing and return the RootManager.
    pub(crate) fn finish(self) -> RootManager {
        self.root
    }
}
