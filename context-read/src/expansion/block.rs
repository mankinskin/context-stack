//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandState tracking the current expansion state.

use crate::{
    context::root::RootManager,
    expansion::{
        chain::BandState,
        ExpansionCtx,
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
}

impl BlockExpansionCtx {
    /// Create a new block expansion context.
    /// Takes ownership of RootManager to manage block commits.
    pub(crate) fn new(
        root: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");
        Self { root, known }
    }

    /// Process the known pattern and commit the result to the root.
    /// Uses overlap expansion and commits when an overlap is found.
    pub(crate) fn process(&mut self) {
        debug!(
            known_len = self.known.len(),
            "BlockExpansionCtx::process starting"
        );

        if self.known.is_empty() {
            panic!("Cannot process empty pattern");
        }

        // Set up cursor for the known pattern
        let path = PatternEndPath::new(self.known.clone(), Default::default());
        let mut cursor = path.into_range(0);

        // Get the last token from the existing root (if any) to use as overlap anchor
        let root_last_token = self.root.last_child_token();
        debug!(
            has_root = self.root.root.is_some(),
            root_last_token = ?root_last_token,
            "Starting block expansion"
        );

        // Create expansion context with root's last token for overlap detection
        let mut ctx = ExpansionCtx::new(self.root.graph.clone(), &mut cursor, root_last_token);

        let first = ctx.state.start_token();
        debug!(state = ?ctx.state, ?first, "expansion state before processing");

        // Process expansions - when overlap found, commit and continue
        loop {
            match ctx.next() {
                Some(_token) => {
                    // Check if we now have an overlap - if so, commit it
                    if ctx.state.has_overlap() {
                        debug!("Overlap found, committing state");
                        let state = std::mem::take(&mut ctx.state);
                        self.root.commit_state(state);
                        
                        // For now, break after first overlap commit
                        // TODO: Continue processing remaining pattern
                        break;
                    }
                }
                None => {
                    // No more expansions - commit final state
                    break;
                }
            }
        }

        debug!(
            final_state = ?ctx.state,
            "BlockExpansionCtx::process complete"
        );

        // Commit any remaining state (but not if band is empty - e.g., only external anchor, no cursor tokens)
        if !ctx.state.is_empty() {
            let state = std::mem::take(&mut ctx.state);
            self.root.commit_state(state);
        }
    }

    /// Finish processing and return the RootManager.
    pub(crate) fn finish(self) -> RootManager {
        self.root
    }
}
