//! Block-based expansion for known patterns with overlap detection.
//!
//! This module provides BlockExpansionCtx which wraps ExpansionCtx for
//! processing blocks of known patterns. It uses the expansion mechanism
//! to detect overlaps, with the BandChain tracking overlaps as an ordered map.

use crate::{
    bands::HasTokenRoleIters,
    context::root::RootManager,
    expansion::{
        chain::band,
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
    /// Uses overlap expansion and commits the full band chain with decompositions.
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

        // Create expansion context
        let mut ctx = ExpansionCtx::new(self.root.graph.clone(), &mut cursor);

        let first = ctx.chain.start_token();
        debug!(chain = ?ctx.chain, ?first, "expansion chain before processing");

        // Process all expansions by consuming the iterator
        while ctx.next().is_some() {}

        debug!(
            final_chain = ?ctx.chain,
            "BlockExpansionCtx::process complete"
        );

        // Check for overlaps with root.
        // If postfixes of the last token in the first band match the root, add overlap band.
        // For "ababab" with root="ab" and known processed to [ab, ab]:
        // - Postfix of "ab" (last element) matches root "ab"
        // - Add overlap band
        if let Some(root_token) = self.root.root {
            let first_band = ctx.chain.bands.first().unwrap();
            let last_in_band = *first_band.pattern.last().unwrap();

            // Check if any postfix of last_in_band matches root
            for (_, postfix) in last_in_band.postfix_iter(self.root.graph.clone())
            {
                if postfix.vertex_index() == root_token.vertex_index() {
                    // Verify the overlap is valid: swapped order must produce same string
                    use context_trace::graph::vertex::has_vertex_index::HasVertexIndex;
                    let root_str =
                        self.root.graph.index_string(root_token.vertex_index());
                    let last_str =
                        self.root.graph.index_string(last_in_band.vertex_index());

                    // Check if last + root == root + last (i.e., commutative)
                    let forward = format!("{}{}", root_str, last_str);
                    let swapped = format!("{}{}", last_str, root_str);

                    if forward == swapped {
                        debug!(
                            overlap_band = ?[last_in_band, root_token],
                            postfix = ?postfix,
                            "adding overlap band via postfix match with root"
                        );
                        // Add [last_in_band, root_token] as overlap band
                        ctx.chain
                            .append_front_complement(last_in_band, root_token);
                    }
                    break;
                }
            }
        }

        // Take the chain and commit to root manager
        let chain = std::mem::take(&mut ctx.chain);
        self.root.commit_chain(chain);
    }

    /// Finish processing and return the RootManager.
    pub(crate) fn finish(self) -> RootManager {
        self.root
    }
}
