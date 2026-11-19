//! Optional visit-set interfaces; passes decide when to consult or update them.

use std::collections::VecDeque;

use itertools::Itertools;

use crate::graph::{
    traversal::pass::TraversalPass,
    vocabulary::{
        entry::VertexCtx,
        NGramId,
        Vocabulary,
    },
};
use context_trace::legacy::{
    graph::vertex::{
        token::Token,
        wide::Wide,
        VertexIndex,
    },
    HashSet,
};

use super::pass::PassNode;

/// Expose a pass's visit collection without imposing a traversal policy.
pub(crate) trait VisitTracking: TraversalPass {
    /// Storage used to record this pass's node identities.
    type Collection: VisitorCollection<Self::Node>;
    /// Borrow the visit collection for membership updates.
    fn visited_mut(&mut self) -> &mut Self::Collection;
}
/// Minimal insertion interface for owned, borrowed, or disabled visit tracking.
pub(crate) trait VisitorCollection<N: PassNode> {
    /// Corresponding borrowable collection form.
    type Ref<'t>: VisitorCollection<N>
    where
        N: 't;
    /// Record a node and return whether it should be considered newly visited.
    fn insert(
        &mut self,
        node: N,
    ) -> bool;
}

impl<N: PassNode> VisitorCollection<N> for HashSet<N> {
    type Ref<'t>
        = &'t mut Self
    where
        N: 't;
    /// Forward owned-set insertion through the borrowed-set implementation.
    fn insert(
        &mut self,
        node: N,
    ) -> bool {
        <&mut Self as VisitorCollection<N>>::insert(&mut &mut *self, node)
    }
}
impl<N: PassNode> VisitorCollection<N> for &'_ mut HashSet<N> {
    type Ref<'t>
        = &'t mut HashSet<N>
    where
        N: 't;
    /// Return `true` only when the node identity was absent.
    fn insert(
        &mut self,
        node: N,
    ) -> bool {
        HashSet::insert(*self, node)
    }
}
/// Disabled tracking accepts every visit, including repeats.
impl<N: PassNode> VisitorCollection<N> for () {
    type Ref<'t>
        = Self
    where
        N: 't;
    /// Accept the node unconditionally without recording it.
    fn insert(
        &mut self,
        node: N,
    ) -> bool {
        true
    }
}
