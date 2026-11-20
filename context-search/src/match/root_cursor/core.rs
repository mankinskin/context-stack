//! Root cursor types and parent exploration (legacy
//! `match/root_cursor/core.rs`).

use std::collections::VecDeque;

use context_trace::{
    positioned::{
        trace::state::parent::ParentBatch,
        PositionedGraph,
    },
    GraphKind,
};
use derive_more::{
    Deref,
    DerefMut,
};

use crate::{
    compare::{
        parent::ParentCompareState,
        state::CompareState,
    },
    cursor::{
        Candidate,
        Checkpointed,
        CursorState,
        HasCandidate,
        Matched,
        QueryCursor,
    },
    policy::{
        DirectedTraversalPolicy,
        SearchKind,
    },
};

pub(crate) enum RootAdvanceResult<'g, G: GraphKind, K: SearchKind> {
    Advanced(Box<RootCursor<'g, G, K, Matched, Matched>>),
    Finished(RootEndResult<'g, G, K>),
}

pub(crate) enum RootEndResult<'g, G: GraphKind, K: SearchKind> {
    /// The match in this root is maximal for the whole search.
    Conclusive(ConclusiveEnd),
    /// The root ended before the query: continue in its parents.
    Inconclusive(Box<RootCursor<'g, G, K, Candidate, Matched>>),
}

pub(crate) enum ConclusiveEnd {
    Mismatch,
    Exhausted,
}

#[derive(Debug)]
pub(crate) struct RootCursor<
    'g,
    G: GraphKind,
    K: SearchKind,
    Q: CursorState = Matched,
    I: CursorState = Matched,
> {
    pub(crate) state: CompareState<Q, I>,
    pub(crate) graph: &'g PositionedGraph<G>,
    pub(crate) _kind: std::marker::PhantomData<K>,
}

impl<'g, G: GraphKind, K: SearchKind, Q: CursorState, I: CursorState> RootCursor<'g, G, K, Q, I> {
    pub(crate) fn new(
        graph: &'g PositionedGraph<G>,
        state: CompareState<Q, I>,
    ) -> Self {
        Self {
            state,
            graph,
            _kind: std::marker::PhantomData,
        }
    }
}

/// Parent states sharing one query cursor.
#[derive(Debug, Clone, Deref, DerefMut)]
pub(crate) struct CompareParentBatch {
    #[deref]
    #[deref_mut]
    pub(crate) batch: ParentBatch,
    pub(crate) cursor: Checkpointed<QueryCursor<Candidate>, HasCandidate>,
}

impl CompareParentBatch {
    pub(crate) fn into_compare_batch(self) -> VecDeque<ParentCompareState> {
        let cursor = self.cursor;
        self.batch
            .parents
            .into_iter()
            .map(|parent_state| ParentCompareState {
                parent_state,
                cursor: cursor.clone(),
            })
            .collect()
    }
}

impl<G: GraphKind, K: SearchKind> RootCursor<'_, G, K, Candidate, Matched> {
    /// The matched start path, raised into every parent occurrence of the
    /// exhausted root, with the advanced query cursor. `None` if the root
    /// has no parents.
    pub(crate) fn get_parent_batch(self) -> Option<CompareParentBatch> {
        let parent_state = self.state.child.current().child_state.parent_state();
        K::Policy::next_batch(self.graph, &parent_state).map(|batch| CompareParentBatch {
            batch,
            cursor: self.state.query,
        })
    }
}
