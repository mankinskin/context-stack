use crate::{
    expansion::ExpansionCtx,
    pipeline::root::RootManager,
};
use context_trace::*;
use tracing::debug;

/// Context for block-based expansion of a known-atom pattern.
///
/// Owns the `RootManager` and drives the `ExpansionCtx` cursor loop,
/// appending each finalized token to the root before advancing.
#[derive(Debug)]
pub(crate) struct BlockExpansionCtx {
    sink: RootManager,
    ctx: ExpansionCtx,
}

impl BlockExpansionCtx {
    pub(crate) fn new(
        sink: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");

        let atoms: Vec<Token> = known.iter().copied().collect();
        let expansion_ctx =
            ExpansionCtx::new(sink.graph.clone(), atoms);

        Self {
            sink,
            ctx: expansion_ctx,
        }
    }

    /// Append each finalized span immediately so the next search sees the
    /// updated graph, without using the accumulation root as a local anchor.
    pub(crate) fn process(&mut self) {
        debug!("BlockExpansionCtx::process start");

        while let Some(token) = self.ctx.next() {
            self.sink.append_completed(token);

            debug!(
                root = ?self.sink.root,
                cursor = self.ctx.cursor,
                "BlockExpansionCtx::process: appended completed span"
            );
        }

        debug!(
            cursor = self.ctx.cursor,
            "BlockExpansionCtx::process complete"
        );
    }

    pub(crate) fn finish(self) -> RootManager {
        self.sink
    }
}
