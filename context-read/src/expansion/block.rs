//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandChain tracking overlaps as an ordered map.

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
pub struct BlockExpansionCtx {
    /// The root manager (owns graph and root token)
    root: RootManager,
    /// The known pattern to process
    known: Pattern,
}

impl BlockExpansionCtx {
    /// Create a new block expansion context.
    /// Takes ownership of RootManager to manage block commits.
    pub fn new(
        root: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");
        Self { root, known }
    }

    /// Process the known pattern and commit the result to the root.
    /// Uses overlap expansion and commits the full band chain with decompositions.
    pub fn process(&mut self) {
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

        // Create expansion context
        let mut ctx = ExpansionCtx::new(self.root.graph.clone(), &mut cursor);

        let first = ctx.chain.start_token();
        debug!(chain = ?ctx.chain, ?first, "expansion chain before processing");
        
        // Process all expansions by consuming the iterator
        while ctx.next().is_some() {}

        debug!(
            final_chain = ?ctx.chain,
            final_token = ?ctx.chain.final_token(),
            "BlockExpansionCtx::process complete"
        );
        
        // Take the chain and commit to root manager
        let chain = std::mem::take(&mut ctx.chain);
        self.root.commit_chain(chain);
    }

    /// Finish processing and return the RootManager.
    pub fn finish(self) -> RootManager {
        self.root
    }
}
