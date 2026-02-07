use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

#[derive(Debug, Deref, DerefMut)]
pub struct CursorCtx<'a> {
    #[deref]
    #[deref_mut]
    pub graph: HypergraphRef,
    pub cursor: &'a mut PatternRangePath,
}

impl<'a> CursorCtx<'a> {
    pub fn new(
        graph: HypergraphRef,
        cursor: &'a mut PatternRangePath,
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
