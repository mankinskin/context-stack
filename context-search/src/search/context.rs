use crate::{
    traversal::{
        policy::DirectedTraversalPolicy,
        TraversalKind,
    },
    BftQueue,
};
use context_trace::*;
#[derive(Debug)]
pub struct AncestorPolicy<T: HasGraph>(std::marker::PhantomData<T>);

impl<T: HasGraph> DirectedTraversalPolicy for AncestorPolicy<T> {
    type Trav = T;
}

#[derive(Debug)]
pub(crate) struct ParentPolicy<T: HasGraph>(std::marker::PhantomData<T>);

impl<T: HasGraph> DirectedTraversalPolicy for ParentPolicy<T> {
    type Trav = T;
    fn next_batch(
        _trav: &Self::Trav,
        _state: &ParentState,
    ) -> Option<ParentBatch> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct SearchCtx<T: HasGraph> {
    pub(crate) graph: T,
}

#[derive(Debug)]
pub(crate) struct AncestorSearchTraversal<T: HasGraph>(
    std::marker::PhantomData<T>,
);
impl<T: HasGraph> Default for AncestorSearchTraversal<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: HasGraph> TraversalKind for AncestorSearchTraversal<T> {
    type Trav = SearchCtx<T>;
    type Container = BftQueue;
    type Policy = AncestorPolicy<Self::Trav>;
}
#[derive(Debug)]
pub(crate) struct ParentSearchTraversal<T: HasGraph>(
    std::marker::PhantomData<T>,
);
impl<T: HasGraph> Default for ParentSearchTraversal<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: HasGraph> TraversalKind for ParentSearchTraversal<T> {
    type Trav = SearchCtx<T>;
    type Container = BftQueue;
    type Policy = ParentPolicy<Self::Trav>;
}
impl<T: HasGraph> SearchCtx<T> {
    pub(crate) fn new(graph: T) -> Self {
        Self { graph }
    }
}

impl<T: HasGraph> HasGraph for SearchCtx<T> {
    type Kind = T::Kind;
    type Guard<'a>
        = T::Guard<'a>
    where
        T: 'a;
    fn graph(&self) -> Self::Guard<'_> {
        self.graph.graph()
    }
}

//impl<'g, T: HasGraph> HasGraph for &'g SearchCtx<T> {
//    type Kind = T::Kind;
//    type Guard<'a> = T::Guard<'a> where T: 'a, 'g: 'a;
//    fn graph(&self) -> Self::Guard<'_> {
//        self.graph.graph()
//    }
//}
