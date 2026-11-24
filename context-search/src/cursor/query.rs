//! Positioned query cursor: a [`PathCursor`] over a [`QueryPath`].
//!
//! As in legacy, `atom_position` is the query position at the end of the
//! leaf (the number of query atoms consumed once the leaf is matched). It is
//! kept equal to [`QueryPath::leaf_end`](context_trace::positioned::RolePath::leaf_end) by every operation here.

use std::{
    marker::PhantomData,
    ops::ControlFlow,
};

use context_trace::{
    positioned::{
        GraphRead,
        QueryPath,
    },
    CommittedIndex,
    VertexId,
};

use super::{
    CursorState,
    Matched,
    PathCursor,
};

/// Positioned query cursor (legacy `PatternCursor`).
pub type QueryCursor<State = Matched, I = CommittedIndex> = PathCursor<QueryPath<I>, State>;

impl<S: CursorState, I: VertexId> PathCursor<QueryPath<I>, S> {
    /// Cursor at the leaf of `path`.
    pub fn at(path: QueryPath<I>) -> Self {
        Self {
            atom_position: path.leaf_end(),
            path,
            _state: PhantomData,
        }
    }
    /// Query position reached (end of the leaf).
    pub fn atom_position(&self) -> context_trace::AtomPosition {
        self.atom_position
    }
    /// Move to the largest query token starting at the leaf end
    /// ([`RolePath::advance_leaf`](context_trace::positioned::RolePath::advance_leaf)).
    /// `Break` (unchanged) at the query end.
    pub fn advance<G: GraphRead<Index = I> + ?Sized>(
        &mut self,
        graph: &G,
    ) -> ControlFlow<()> {
        let flow = self.path.advance_leaf(graph);
        self.atom_position = self.path.leaf_end();
        flow
    }
    /// Split the leaf into its prefix child (query-side decomposition).
    pub fn push_border_child<G: GraphRead<Index = I> + ?Sized>(
        &mut self,
        graph: &G,
    ) -> bool {
        let split = self.path.push_border_child(graph);
        self.atom_position = self.path.leaf_end();
        split
    }
    /// The same position in another state.
    pub(crate) fn with_state<T: CursorState>(self) -> PathCursor<QueryPath<I>, T> {
        PathCursor {
            path: self.path,
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }
}
