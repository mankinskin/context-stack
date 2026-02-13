use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

#[derive(Debug, Deref, DerefMut)]
pub(crate) struct CursorCtx {
    #[deref]
    #[deref_mut]
    pub(crate) graph: HypergraphRef,
    pub(crate) cursor: PatternRangePath,
}

impl CursorCtx {
    pub(crate) fn new(
        graph: HypergraphRef,
        cursor: PatternRangePath,
    ) -> Self {
        debug!(
            cursor_root = ?cursor.path_root(),
            start_path = ?cursor.start_path(),
            end_path = ?cursor.end_path(),
            "New CursorCtx"
        );
        Self { graph, cursor }
    }
}
