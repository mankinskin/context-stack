//! Search start (legacy `state/start`): seed the first parent batch from
//! the first query token.

use std::marker::PhantomData;

use context_trace::{
    positioned::{
        ParentState,
        PositionedGraph,
    },
    GraphKind,
    Token,
};

use crate::{
    cursor::{
        Checkpointed,
        CursorStateMachine,
        Matched,
        QueryCursor,
    },
    policy::{
        DirectedTraversalPolicy,
        SearchKind,
    },
    r#match::{
        iterator::SearchIterator,
        root_cursor::CompareParentBatch,
    },
    search::SearchState,
    SearchError,
};

/// A query cursor at its first token `start_token` (the cursor's leaf, at
/// query position `cursor.path.leaf_offset()`).
#[derive(Debug)]
pub(crate) struct StartCtx<'g, G: GraphKind, K: SearchKind> {
    pub(crate) graph: &'g PositionedGraph<G>,
    pub(crate) start_token: Token,
    pub(crate) cursor: QueryCursor<Matched>,
    pub(crate) _kind: PhantomData<K>,
}

impl<'g, G: GraphKind, K: SearchKind> StartCtx<'g, G, K> {
    pub(crate) fn new(
        graph: &'g PositionedGraph<G>,
        cursor: QueryCursor<Matched>,
    ) -> Self {
        Self {
            graph,
            start_token: cursor.path.leaf(),
            cursor,
            _kind: PhantomData,
        }
    }
    pub(crate) fn into_search(self) -> Result<SearchState<'g, G, K>, SearchError> {
        let batch = self.get_parent_batch()?;
        Ok(SearchState::new(
            SearchIterator::new(self.graph, self.start_token, batch),
            self.cursor,
        ))
    }
    /// Advance the query past the start token (`SingleIndex` if it is the
    /// last one) and build one parent state per occurrence of the start
    /// token, at the start token's query position.
    pub(crate) fn get_parent_batch(&self) -> Result<CompareParentBatch, SearchError> {
        let checkpoint = self.cursor.clone();
        let mut candidate = self.cursor.clone();
        if candidate.advance(self.graph).is_break() {
            return Err(SearchError::SingleIndex(self.start_token));
        }
        let start = self.start_token;
        let position = self.cursor.path.leaf_offset();
        let batch = K::Policy::gen_parent_batch(self.graph, start, |parent, offset| {
            ParentState::new(start, parent, offset, position)
        });
        Ok(CompareParentBatch {
            batch,
            cursor: Checkpointed::with_candidate(checkpoint, candidate.to_candidate()),
        })
    }
}
