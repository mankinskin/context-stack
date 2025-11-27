use crate::{
    cursor::{
        Checkpointed,
        PathCursor,
        PatternCursor,
    },
    r#match::{
        iterator::SearchIterator,
        root_cursor::CompareParentBatch,
    },
    search::SearchState,
    traversal::policy::DirectedTraversalPolicy,
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
    warn,
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InputLocation {
    Location(PatternLocation),
    PatternChild { sub_index: usize, token: Token },
}

impl std::fmt::Display for InputLocation {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            InputLocation::Location(loc) => write!(f, "Location({})", loc),
            InputLocation::PatternChild { sub_index, token } => {
                write!(
                    f,
                    "PatternChild{{ sub_index: {}, token: {} }}",
                    sub_index, token
                )
            },
        }
    }
}

impl GraphRoot for InputLocation {
    fn root_parent(&self) -> Token {
        match self {
            InputLocation::Location(loc) => loc.parent,
            InputLocation::PatternChild { token, .. } => *token,
        }
    }
}

pub trait StartFoldPath:
    BaseQuery
    + PathAppend
    + PathPop
    + MoveRootIndex<Right, End>
    + HasRootedLeafToken<End>
    + RootPattern
    + CalcWidth
{
    fn to_range_path(self) -> PatternRangePath;

    fn input_location<G: HasGraph>(
        &self,
        trav: &G,
    ) -> InputLocation {
        trace!("determining input_location for path");

        if let Some(loc) = self.role_leaf_token_location::<End>() {
            debug!(location = %pretty(&loc), "found leaf token location");
            let pattern_loc = loc.into_pattern_location();
            debug!(pattern_location = %pretty(&pattern_loc), "converted to pattern location");
            InputLocation::Location(pattern_loc)
        } else {
            debug!("no leaf token location, getting pattern child");
            let sub_index = self.role_root_child_index::<End>();
            let token = self.role_rooted_leaf_token::<End, _>(trav);
            debug!(token = %pretty(&token), sub_index, "pattern child");

            // This is where the panic will happen - when we try to use this token
            // and it doesn't have children
            trace!("checking token vertex data in graph");
            if let Ok(vertex_data) =
                trav.graph().get_vertex(token.vertex_index())
            {
                trace!(vertex_data = %pretty(vertex_data), "token vertex data");
                let child_patterns = vertex_data.child_patterns();
                if child_patterns.is_empty() {
                    warn!(
                        token = %pretty(&token),
                        "token has no child patterns - will cause panic"
                    );
                    warn!("typically means searching atoms directly without pattern");
                    warn!("consider using find_sequence() instead of find_ancestor()");
                }
            }

            InputLocation::PatternChild { sub_index, token }
        }
    }
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
        debug!("creating search from start context");
        trace!(start_token = %self.start_token, cursor = %self.cursor, "start context details");
        match self.get_parent_batch() {
            Ok(p) => {
                debug!(
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
                })
            },
            Err(err) => {
                debug!(error = %pretty(&err), "failed to get parent batch");
                Err(err)
            },
        }
    }
    pub(crate) fn get_parent_batch(
        &self
    ) -> Result<CompareParentBatch, ErrorState> {
        let mut cursor = self.cursor.clone();
        debug!(cursor_path = %cursor.path, "get_parent_batch - cursor path before root_child_token");
        let start = self.start_token;
        let checkpoint = cursor.clone();
        if cursor.advance(&self.trav).is_continue() {
            let batch =
                K::Policy::gen_parent_batch(&self.trav, start, |_trav, p| {
                    start.into_parent_state(p)
                });

            let cursor = Checkpointed {
                checkpoint,
                candidate: Some(cursor.as_candidate()),
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
