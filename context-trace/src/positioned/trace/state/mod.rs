//! Trace states (legacy `trace::state`).

pub mod parent;

use crate::{
    AtomPosition,
    GraphKind,
    Token,
    positioned::graph::PositionedGraph,
};
use parent::ParentState;

pub trait StateAdvance: Sized + Clone {
    type Next;
    fn advance_state<G: GraphKind>(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<Self::Next, Self>;
}

pub trait IntoParentState: Sized {
    /// Seed state: `self` (the first query token, at query position 0)
    /// occurs in `parent` at `offset`.
    fn into_parent_state(
        self,
        parent: Token,
        offset: AtomPosition,
    ) -> ParentState;
}

impl IntoParentState for Token {
    fn into_parent_state(
        self,
        parent: Token,
        offset: AtomPosition,
    ) -> ParentState {
        ParentState::new(self, parent, offset, AtomPosition::default())
    }
}
