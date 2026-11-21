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
        PositionedGraph,
        QueryPath,
    },
    GraphKind,
};

use super::{
    CursorState,
    Matched,
    PathCursor,
};

/// Positioned query cursor (legacy `PatternCursor`).
pub type QueryCursor<State = Matched> = PathCursor<QueryPath, State>;

impl<S: CursorState> PathCursor<QueryPath, S> {
    /// Cursor at the leaf of `path`.
    pub fn at(path: QueryPath) -> Self {
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
    pub fn advance<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> ControlFlow<()> {
        let flow = self.path.advance_leaf(graph);
        self.atom_position = self.path.leaf_end();
        flow
    }
    /// Split the leaf into its prefix child (query-side decomposition).
    pub fn push_border_child<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> bool {
        let split = self.path.push_border_child(graph);
        self.atom_position = self.path.leaf_end();
        split
    }
    /// The same position in another state.
    pub(crate) fn with_state<T: CursorState>(self) -> PathCursor<QueryPath, T> {
        PathCursor {
            path: self.path,
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }
}
