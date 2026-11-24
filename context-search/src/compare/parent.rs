//! Parent candidates (legacy `compare/parent.rs`): a parent state paired
//! with the query cursor, entered at the next graph token.

use context_trace::{
    positioned::{
        GraphRead,
        ParentState,
        StateAdvance,
    },
    CommittedIndex,
    DownKey,
    DownPosition,
    VertexId,
};
use derive_more::{
    Deref,
    DerefMut,
};

use crate::{
    compare::state::{
        CompareState,
        PathPairMode,
    },
    cursor::{
        Candidate,
        Checkpointed,
        ChildCursor,
        HasCandidate,
        QueryCursor,
    },
};

/// The first compare state in a root and the parent state it came from.
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub(crate) struct CompareRootState<I: VertexId = CommittedIndex> {
    #[deref]
    #[deref_mut]
    pub(crate) candidate: CompareState<Candidate, Candidate, I>,
    pub(crate) root_parent: ParentState<I>,
}

/// A parent state with the query cursor: checkpoint at the last matched
/// query leaf, candidate at the next query token.
#[derive(Clone, Debug, Deref, DerefMut)]
pub(crate) struct ParentCompareState<I: VertexId = CommittedIndex> {
    #[deref]
    #[deref_mut]
    pub(crate) parent_state: ParentState<I>,
    pub(crate) cursor: Checkpointed<QueryCursor<Candidate, I>, HasCandidate>,
}

impl<I: VertexId> ParentCompareState<I> {
    /// Enter the root at the graph token starting at the matched end
    /// ([`ParentState::advance_state`]); `Err(self)` at the root end.
    pub(crate) fn advance_state<G: GraphRead<Index = I> + ?Sized>(
        self,
        graph: &G,
    ) -> Result<CompareRootState<I>, Self> {
        match self.parent_state.advance_state(graph) {
            Ok(next) => {
                let child_state = next.child_state;
                crate::search::stats::record_path_lens(
                    child_state.path.start.len(),
                    child_state.path.end.len(),
                );
                let target = DownKey::new(
                    child_state.path.end.leaf(),
                    DownPosition(self.cursor.current().path.leaf_offset()),
                );
                let candidate = CompareState {
                    query: self.cursor,
                    child: Checkpointed::with_candidate(
                        ChildCursor::new(child_state.clone()),
                        ChildCursor::new(child_state),
                    ),
                    mode: PathPairMode::GraphMajor,
                    target,
                };
                Ok(CompareRootState {
                    candidate,
                    root_parent: next.root_parent,
                })
            },
            Err(parent_state) => Err(Self {
                parent_state,
                cursor: self.cursor,
            }),
        }
    }
}
