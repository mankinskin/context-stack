use crate::{
    context::root::RootManager,
    expansion::ExpansionCtx,
};
use context_trace::*;
use tracing::debug;

/// Context for block-based expansion of a known-atom pattern.
///
/// Owns the `RootManager` and drives the `ExpansionCtx` cursor loop,
/// committing each yielded `BandState` to the root before advancing.
#[derive(Debug)]
pub(crate) struct BlockExpansionCtx {
    root: RootManager,
    ctx: ExpansionCtx,
}

impl BlockExpansionCtx {
    pub(crate) fn new(
        root: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");

        let anchor = root.anchor();
        let atoms: Vec<Token> = known.iter().copied().collect();
        let expansion_ctx =
            ExpansionCtx::new(root.graph.clone(), atoms, anchor);

        Self {
            root,
            ctx: expansion_ctx,
        }
    }

    /// Drive the expansion loop: commit each `BandState` immediately and
    /// refresh the anchor so the next step sees the updated left-side context.
    pub(crate) fn process(&mut self) {
        debug!(anchor = ?self.ctx.anchor, "BlockExpansionCtx::process start");

        while let Some(state) = self.ctx.next() {
            self.root.commit_state(state);
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

    pub(crate) fn finish(self) -> RootManager {
        self.root
    }
}
