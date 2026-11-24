//! Graph access (staging plan §3.6; legacy `trace/has_graph.rs`): one trait
//! abstracts *how* a store is reached (a plain reference or a lock), so
//! search, tracing, insertion and the reader take `G: HasGraph`
//! (`G: HasGraphMut`) and work on views and handles alike.
//!
//! Deviation from legacy: the guards do not `Deref` to a concrete store;
//! they *are* [`GraphRead`] / [`GraphWrite`] views, and the write guard only
//! exposes the sealed write primitives (insert plan D8). The legacy traits
//! of the same name stay untouched in `context_trace::legacy`.

use crate::{
    AtomOf,
    GraphKind,
    graph::vertex::VertexId,
};

use super::access::{
    GraphRead,
    GraphWrite,
};

pub trait HasGraph {
    type Kind: GraphKind;
    type Index: VertexId;
    type Guard<'a>: GraphRead<Kind = Self::Kind, Index = Self::Index>
    where
        Self: 'a;
    fn graph(&self) -> Self::Guard<'_>;
}

pub trait HasGraphMut: HasGraph {
    type GuardMut<'a>: GraphWrite<Kind = Self::Kind, Index = Self::Index>
    where
        Self: 'a;
    fn graph_mut(&mut self) -> Self::GuardMut<'_>;
}

impl<T: HasGraph + ?Sized> HasGraph for &T {
    type Kind = T::Kind;
    type Index = T::Index;
    type Guard<'a>
        = T::Guard<'a>
    where
        Self: 'a;
    fn graph(&self) -> Self::Guard<'_> {
        (**self).graph()
    }
}

impl<T: HasGraph + ?Sized> HasGraph for &mut T {
    type Kind = T::Kind;
    type Index = T::Index;
    type Guard<'a>
        = T::Guard<'a>
    where
        Self: 'a;
    fn graph(&self) -> Self::Guard<'_> {
        (**self).graph()
    }
}

impl<T: HasGraphMut + ?Sized> HasGraphMut for &mut T {
    type GuardMut<'a>
        = T::GuardMut<'a>
    where
        Self: 'a;
    fn graph_mut(&mut self) -> Self::GuardMut<'_> {
        (**self).graph_mut()
    }
}

/// Kind of the graph behind `G` (legacy `TravKind`).
pub type KindOf<G> = <G as HasGraph>::Kind;
/// Index type of the graph behind `G`.
pub type IndexOf<G> = <G as HasGraph>::Index;
/// Atom type of the graph behind `G` (legacy `TravAtom`).
pub type GraphAtom<G> = AtomOf<KindOf<G>>;

/// `HasGraph` for one implementer (legacy `impl_has_graph!`):
///
/// ```text
/// impl_has_graph! {
///     impl<G: GraphKind> for PositionedGraph<G>, Kind = G, Index = CommittedIndex,
///     self => self; <'a> &'a Self
/// }
/// ```
macro_rules! impl_has_graph {
    {
        impl $(< $( $par:ident $( : $bound:path )? ),* >)? for $target:ty,
        Kind = $kind:ty, Index = $index:ty,
        $self_:ident => $func:expr; <$lt:lifetime> $guard:ty
    } => {
        impl $(< $( $par $( : $bound )? ),* >)? $crate::positioned::HasGraph for $target {
            type Kind = $kind;
            type Index = $index;
            type Guard<$lt> = $guard where Self: $lt;
            fn graph(&$self_) -> Self::Guard<'_> {
                $func
            }
        }
    };
}

/// `HasGraphMut` for one implementer (legacy `impl_has_graph_mut!`).
macro_rules! impl_has_graph_mut {
    {
        impl $(< $( $par:ident $( : $bound:path )? ),* >)? for $target:ty,
        $self_:ident => $func:expr; <$lt:lifetime> $guard:ty
    } => {
        impl $(< $( $par $( : $bound )? ),* >)? $crate::positioned::HasGraphMut for $target {
            type GuardMut<$lt> = $guard where Self: $lt;
            fn graph_mut(&mut $self_) -> Self::GuardMut<'_> {
                $func
            }
        }
    };
}

pub(crate) use impl_has_graph;
pub(crate) use impl_has_graph_mut;
