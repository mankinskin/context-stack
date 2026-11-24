//! Read traits over vertex objects (staging plan §3.3): [`GraphRead`]
//! hands out [`VertexRead`] objects (`graph.vertex(i)`); lookups accept
//! anything carrying an index of the store ([`HasVertexIndex`], legacy
//! `VertexSet<I: GetVertexIndex>`). [`NodeRef`] is either an atom value or
//! a composition.

use std::fmt::Debug;

use super::CompositionRead;
use crate::{
    AtomOf,
    AtomPosition,
    GraphKind,
    Token,
    graph::vertex::{
        VertexId,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
};

/// Whether a vertex's string content is fixed (staging plan §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexState {
    /// String content immutable forever.
    Committed,
    /// Created by the running read; content may change while parentless.
    Staged,
}

/// Staleness stamp of a store (insert plan D9). The committed store has
/// `stage == 0`; a stage adds its own counter to the base generation at
/// open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation {
    pub base: u64,
    pub stage: u64,
}

impl Generation {
    /// The generation of a committed store.
    pub fn committed(base: u64) -> Self {
        Self { base, stage: 0 }
    }
}

/// Read access to a vertex store.
pub trait GraphRead {
    type Kind: GraphKind;
    type Index: VertexId;
    type Vertex<'a>: VertexRead<'a, Kind = Self::Kind, Index = Self::Index>
    where
        Self: 'a;

    /// Lookup by anything carrying an index of this store (index, token,
    /// occurrence, vertex object). `None` for an unknown index.
    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = Self::Index>,
    ) -> Option<Self::Vertex<'_>>;
    /// The atom vertex of `value`, if any (lookup in the atom index).
    fn atom(
        &self,
        value: &AtomOf<Self::Kind>,
    ) -> Option<Self::Vertex<'_>>;
    /// Every vertex, in index order.
    fn vertices(&self) -> impl Iterator<Item = Self::Vertex<'_>> + '_;
    /// Bumped once by the first mutation of every edit.
    fn generation(&self) -> Generation;
}

impl<T: GraphRead + ?Sized> GraphRead for &T {
    type Kind = T::Kind;
    type Index = T::Index;
    type Vertex<'a>
        = T::Vertex<'a>
    where
        Self: 'a;
    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = Self::Index>,
    ) -> Option<Self::Vertex<'_>> {
        (**self).vertex(index)
    }
    fn atom(
        &self,
        value: &AtomOf<Self::Kind>,
    ) -> Option<Self::Vertex<'_>> {
        (**self).atom(value)
    }
    fn vertices(&self) -> impl Iterator<Item = Self::Vertex<'_>> + '_ {
        (**self).vertices()
    }
    fn generation(&self) -> Generation {
        (**self).generation()
    }
}

impl<T: GraphRead + ?Sized> GraphRead for &mut T {
    type Kind = T::Kind;
    type Index = T::Index;
    type Vertex<'a>
        = T::Vertex<'a>
    where
        Self: 'a;
    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = Self::Index>,
    ) -> Option<Self::Vertex<'_>> {
        (**self).vertex(index)
    }
    fn atom(
        &self,
        value: &AtomOf<Self::Kind>,
    ) -> Option<Self::Vertex<'_>> {
        (**self).atom(value)
    }
    fn vertices(&self) -> impl Iterator<Item = Self::Vertex<'_>> + '_ {
        (**self).vertices()
    }
    fn generation(&self) -> Generation {
        (**self).generation()
    }
}

/// A cheap `Copy` reference to one vertex of a store. It carries the store
/// reference, so it can navigate ([`Navigate`](super::Navigate)). Index,
/// width and token come from the shared accessors ([`HasVertexIndex`],
/// [`Wide`], [`ToToken`](crate::ToToken)).
pub trait VertexRead<'a>: Copy + Debug + HasVertexIndex + Wide {
    type Kind: GraphKind;
    type Composition: CompositionRead<Index = Self::Index>;

    /// Exactly one of atom value or composition.
    fn node(self) -> NodeRef<'a, AtomOf<Self::Kind>, Self::Composition>;
    /// Every occurrence of this vertex in a parent as `(parent, offset)`,
    /// unordered.
    fn parents(
        self
    ) -> impl Iterator<Item = (Token<Self::Index>, AtomPosition)>;
    /// Number of parent occurrences.
    fn parent_count(self) -> usize {
        self.parents().count()
    }
    /// Whether this vertex occurs in `parent` at `offset` (link check of
    /// validation; stores override the linear default).
    fn is_parent_at(
        self,
        parent: Self::Index,
        offset: AtomPosition,
    ) -> bool {
        self.parents()
            .any(|(p, o)| p.index == parent && o == offset)
    }
    fn state(self) -> VertexState;
    /// Another vertex of the same store (a child or a parent).
    fn resolve(
        self,
        index: Self::Index,
    ) -> Option<Self>;
    /// Debug-only expanded content, for diagnostics.
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str;
}

/// Borrowed view of a vertex's node (read-side counterpart of the owned
/// [`VertexNode`](crate::positioned::VertexNode)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRef<'a, A, C> {
    Atom(&'a A),
    Composite(C),
}

impl<'a, A, C> NodeRef<'a, A, C> {
    /// The composition; `None` for an atom.
    pub fn composite(self) -> Option<C> {
        match self {
            Self::Composite(composition) => Some(composition),
            Self::Atom(_) => None,
        }
    }
    /// The atom value; `None` for a composite.
    pub fn atom(self) -> Option<&'a A> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Composite(_) => None,
        }
    }
}
