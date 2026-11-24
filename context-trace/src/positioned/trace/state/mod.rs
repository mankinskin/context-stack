//! Trace states (legacy `trace::state`).

pub mod parent;

use crate::{
    AtomPosition,
    Token,
    graph::vertex::VertexId,
    positioned::store::GraphRead,
};
use parent::ParentState;

pub trait StateAdvance: Sized + Clone {
    /// Index type of the store the state lives in.
    type Index: VertexId;
    type Next;
    fn advance_state<G: GraphRead<Index = Self::Index> + ?Sized>(
        self,
        graph: &G,
    ) -> Result<Self::Next, Self>;
}

pub trait IntoParentState<I: VertexId>: Sized {
    /// Seed state: `self` (the first query token, at query position 0)
    /// occurs in `parent` at `offset`.
    fn into_parent_state(
        self,
        parent: Token<I>,
        offset: AtomPosition,
    ) -> ParentState<I>;
}

impl<I: VertexId> IntoParentState<I> for Token<I> {
    fn into_parent_state(
        self,
        parent: Token<I>,
        offset: AtomPosition,
    ) -> ParentState<I> {
        ParentState::new(self, parent, offset, AtomPosition::default())
    }
}
