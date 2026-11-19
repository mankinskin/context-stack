use crate::{
    expansion::ExpansionCtx,
    pipeline::root::RootManager,
};
use context_trace::legacy::*;
use tracing::debug;

/// Context for block-based expansion of a known-atom pattern.
///
/// Owns the `RootManager` and drives the `ExpansionCtx` cursor loop,
/// committing each finalized operation before requesting another step.
#[derive(Debug)]
pub(crate) struct BlockExpansionCtx {
    sink: RootManager,
    ctx: ExpansionCtx,
}

impl BlockExpansionCtx {
    /// Continue accumulation through a block containing only known atom tokens.
    pub(crate) fn new(
        sink: RootManager,
        known: Pattern,
    ) -> Self {
        debug!(known_len = known.len(), known = ?known, "BlockExpansionCtx::new");

        let atoms: Vec<Token> = known.iter().copied().collect();
        let expansion_ctx = ExpansionCtx::new(sink.graph.clone(), atoms);

        Self {
            sink,
            ctx: expansion_ctx,
        }
    }

    /// Commit each finalized span immediately so the next search sees the
    /// updated root and graph.
    pub(crate) fn process(&mut self) {
        debug!("BlockExpansionCtx::process start");

        while let Some(span) = self.ctx.next(&self.sink) {
            let receipt = self.sink.commit(span);
            self.ctx.observe_commit(receipt);

            debug!(
                span = ?span,
                receipt = ?receipt,
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

    /// Return the accumulation sink without processing any remaining block atoms.
    pub(crate) fn finish(self) -> RootManager {
        self.sink
    }
}
