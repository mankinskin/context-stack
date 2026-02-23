use crate::{
    cursor::{
        Checkpointed,
        PathCursor,
        PatternCursor,
    },
    policy::DirectedTraversalPolicy,
    r#match::{
        iterator::SearchIterator,
        root_cursor::CompareParentBatch,
    },
    search::SearchState,
    ErrorState,
    SearchKind,
};
use context_trace::{
    logging::format_utils::pretty,
    path::{
        accessors::child::HasRootedLeafToken,
        BaseQuery,
    },
    *,
};
use std::marker::PhantomData;
use tracing::{
    debug,
    trace,
};

pub(crate) trait IntoCursor: StartFoldPath {
    fn into_cursor<G: HasGraph>(
        self,
        trav: &G,
    ) -> PathCursor<Self>;
}

impl<P: StartFoldPath> IntoCursor for P {
    fn into_cursor<G: HasGraph>(
        self,
        trav: &G,
    ) -> PathCursor<Self> {
        // Initialize with first token consumed (to get its parents)
        // Both atom_position and path indices should reflect this
        PathCursor {
            atom_position: (*self.calc_width(trav)).into(),
            path: self,
            _state: std::marker::PhantomData,
        }
    }
}

pub(crate) trait StartFoldPath:
    BaseQuery
    + PathAppend
    + PathPop
    + MoveRootIndex<Right, End>
    + HasRootedLeafToken<End>
    + RootPattern
    + CalcWidth
{
    fn to_range_path(self) -> PatternRangePath;

}

impl StartFoldPath for PatternRangePath {
    fn to_range_path(self) -> PatternRangePath {
        self
    }
}

impl StartFoldPath for PatternEndPath {
    fn to_range_path(self) -> PatternRangePath {
        self.into_range(0)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StartCtx<K: SearchKind> {
    pub(crate) trav: K::Trav,
    pub(crate) start_token: Token,
    pub(crate) cursor: PatternCursor,
}

impl<K: SearchKind> std::fmt::Display for StartCtx<K> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "StartCtx{{ cursor: {} }}", self.cursor)
    }
}
impl<K: SearchKind> StartCtx<K> {
    pub(crate) fn into_search(self) -> Result<SearchState<K>, ErrorState> {
        trace!(start_token = %self.start_token, cursor = %self.cursor, "creating search from start context");
        match self.get_parent_batch() {
            Ok(p) => {
                trace!(
                    batch_len = p.batch.len(),
                    "first parent batch obtained"
                );

                Ok(SearchState {
                    query: self.cursor.path,
                    matches: SearchIterator::new(
                        self.trav,
                        self.start_token,
                        p,
                    ),
                    step_counter: 0,
                    start_node: self.start_token.index.0,
                })
            },
            Err(err) => {
                trace!(error = %pretty(&err), "failed to get parent batch");
                Err(err)
            },
        }
    }
    pub(crate) fn get_parent_batch(
        &self
    ) -> Result<CompareParentBatch, ErrorState> {
        let mut cursor = self.cursor.clone();
        trace!(cursor_path = %cursor.path, "get_parent_batch called");
        let start = self.start_token;
        let checkpoint = cursor.clone();
        if cursor.advance(&self.trav).is_continue() {
            let batch =
                K::Policy::gen_parent_batch(&self.trav, start, |_trav, p| {
                    start.into_parent_state(p)
                });

            let cursor = Checkpointed {
                checkpoint,
                candidate: cursor.as_candidate(),
                _state: PhantomData,
            };
            Ok(CompareParentBatch { batch, cursor })
        } else {
            Err(ErrorState {
                reason: ErrorReason::SingleIndex(Box::new(IndexWithPath {
                    index: start,
                    path: self.cursor.path.clone(),
                })),
                found: None,
            })
        }
    }
}
