//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandChain tracking overlaps as an ordered map.

use crate::{
    context::ReadCtx,
    expansion::ExpansionCtx,
};
use context_trace::*;
use tracing::debug;

/// Context for block-based expansion of patterns.
///
/// Wraps ExpansionCtx to process a known pattern and find the largest
/// bundled token through overlap detection.
#[derive(Debug)]
pub struct BlockExpansionCtx {
    /// The graph context
    ctx: ReadCtx,
    /// The known pattern to process
    known: Pattern,
}

impl BlockExpansionCtx {
    /// Create a new block expansion context for a known pattern.
    pub fn new(
        ctx: ReadCtx,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");
        Self { ctx, known }
    }

    /// Process the known pattern and return the minified pattern.
    ///
    /// Calls find_largest_bundle on an ExpansionCtx and includes any remainder.
    /// The BandChain within ExpansionCtx tracks overlaps as an ordered map.
    pub fn process(self) -> Pattern {
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

        // Create expansion context and get bundled result
        let expansion = ExpansionCtx::new(self.ctx.clone(), &mut cursor)
            .find_largest_bundle();

        assert!(cursor.end_path().is_empty());

        // Build result: [bundled_token, ...remainder]
        let result: Pattern = [
            &[expansion],
            &cursor.path_root()[cursor.role_root_child_index::<End>() + 1..],
        ]
        .concat()
        .into();

        debug!(result = ?result, "BlockExpansionCtx::process complete");
        result
    }
}
