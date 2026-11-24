//! New trace edges (legacy `cache::new`), labelled by the child's offset in
//! its parent instead of a row location.

use derive_more::From;
use derive_new::new;

use super::key::TargetKey;
use crate::{
    AtomPosition,
    DirectedKey,
    graph::vertex::{
        CommittedIndex,
        VertexId,
    },
    positioned::trace::{
        BottomUp,
        TopDown,
        TraceDirection,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, From)]
pub enum EditKind<I: VertexId = CommittedIndex> {
    Parent(NewTraceEdge<BottomUp, I>),
    Child(NewTraceEdge<TopDown, I>),
}

impl<I: VertexId> TargetKey<I> for EditKind<I> {
    fn target_key(&self) -> DirectedKey<I> {
        match self {
            Self::Parent(edge) => edge.target.into(),
            Self::Child(edge) => edge.target.into(),
        }
    }
}

/// Edge between `prev` and `target` in trace direction `D`: bottom-up
/// `prev` is the child and `target` the parent, top-down the reverse.
/// `offset` is the child's offset in the parent.
#[derive(Clone, Debug, PartialEq, Eq, new)]
pub struct NewTraceEdge<D: TraceDirection, I: VertexId = CommittedIndex> {
    pub prev: D::Key<I>,
    pub target: D::Key<I>,
    pub offset: AtomPosition,
}

impl<I: VertexId> EditKind<I> {
    /// `(parent key, child key, child offset)`.
    pub(crate) fn edge(
        &self
    ) -> (DirectedKey<I>, DirectedKey<I>, AtomPosition) {
        match self {
            Self::Parent(e) => (e.target.into(), e.prev.into(), e.offset),
            Self::Child(e) => (e.prev.into(), e.target.into(), e.offset),
        }
    }
}
