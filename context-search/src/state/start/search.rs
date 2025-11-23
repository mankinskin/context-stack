use super::core::{
    IntoCursor,
    StartCtx,
    StartFoldPath,
};
use crate::{
    cursor::{
        PatternCursor,
        PatternPrefixCursor,
    },
    r#match::{
        iterator::SearchIterator,
        root_cursor::CompareParentBatch,
    },
    search::{
        searchable::ErrorState,
        SearchState,
    },
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
    Response,
};
use context_trace::{
    logging::format_utils::pretty,
    *,
};
use tracing::{
    debug,
    trace,
};

impl StartCtx {
    pub(crate) fn get_parent_batch<K: SearchKind>(
        &self,
        trav: &K::Trav,
    ) -> Result<CompareParentBatch, ErrorState> {
        let mut cursor = self.cursor.clone();
        debug!(cursor_path = %cursor.path, "get_parent_batch - cursor path before root_child_token");
        let parent = self.cursor.path.role_root_child_token::<End, _>(trav);
        if cursor.advance(trav).is_continue() {
            let batch = K::Policy::gen_parent_batch(trav, parent, |trav, p| {
                parent.into_parent_state(trav, p)
            });

            Ok(CompareParentBatch { batch, cursor })
        } else {
            Err(ErrorState {
                reason: ErrorReason::SingleIndex(Box::new(IndexWithPath {
                    index: parent,
                    path: self.cursor.path.clone(),
                })),
                found: None,
            })
        }
    }
}
