//! Store abstraction (staging plan §3.3, §3.5): read traits over vertex
//! objects and the sealed write primitives behind `GraphEdit`.
//!
//! - [`GraphRead`] hands out [`VertexRead`] objects (`graph.vertex(i)`);
//!   lookups accept anything carrying an index of the store
//!   ([`HasVertexIndex`], legacy `VertexSet<I: GetVertexIndex>`).
//! - [`CompositionRead`] is what navigation, insertion and the reader need
//!   from a composition; [`NodeRef`] is either an atom value or one.
//! - [`GraphWrite`] adds the raw write primitives of [`raw::Write`], which
//!   cannot be named outside `context-trace` and whose methods take a
//!   [`raw::Seal`] that cannot be constructed there: only `GraphEdit`
//!   mutates (insert plan D8).

use std::fmt::Debug;

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

use super::composition::{
    Composition,
    Occurrence,
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
/// [`VertexNode`](super::VertexNode)).
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

/// What navigation, insertion and the reader need from a composition.
/// [`Wide::width`] is the end of the last occurrence (the owner's width).
pub trait CompositionRead: Copy + Wide {
    type Index: VertexId;
    fn len(self) -> usize;
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<Self::Index>>;
    /// The unique occurrence at offset 0.
    fn prefix(self) -> Occurrence<Self::Index>;
    /// The unique occurrence ending at `width()` (the last one).
    fn postfix(self) -> Occurrence<Self::Index>;
    /// The last occurrence with `offset ≤ p`; `None` iff `p ≥ width`.
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<Self::Index>>;
    /// The first occurrence with `end ≥ e`; `None` iff `e == 0` or
    /// `e > width`.
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<Self::Index>>;
    /// Occurrences strictly containing `p` (`offset < p < end`), a
    /// contiguous range.
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<Self::Index>>;
    /// Every occurrence, sorted by offset.
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<Self::Index>> + ExactSizeIterator;
}

/// Direct slice access.
impl<I: VertexId> CompositionRead for &Composition<I> {
    type Index = I;
    fn len(self) -> usize {
        self.occurrences().len()
    }
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<I>> {
        self.occurrences().get(k).copied()
    }
    fn prefix(self) -> Occurrence<I> {
        *Composition::prefix(self)
    }
    fn postfix(self) -> Occurrence<I> {
        *Composition::postfix(self)
    }
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<I>> {
        Composition::covering(self, p).copied()
    }
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<I>> {
        Composition::ending_at(self, e).copied()
    }
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<I>> {
        Composition::crossing(self, p).iter().copied()
    }
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<I>> + ExactSizeIterator
    {
        Composition::occurrences(self).iter().copied()
    }
}

/// Raw write primitives (staging plan §3.5). The module is private to
/// `context-trace` and every method takes a [`Seal`](raw::Seal), so other
/// crates can neither name, implement nor call them, even through a
/// [`GraphWrite`] bound.
pub(crate) mod raw {
    use super::GraphRead;
    use crate::{
        AtomOf,
        AtomPosition,
        GraphKind,
        TokenWidth,
        graph::vertex::VertexId,
        positioned::graph::VertexNode,
    };

    /// Proof of being inside `context-trace`.
    #[derive(Clone, Copy, Debug)]
    pub struct Seal(pub(crate) ());

    pub(crate) const SEAL: Seal = Seal(());

    pub trait Write: GraphRead {
        type VertexMut<'a>: VertexWrite<Kind = Self::Kind, Index = Self::Index>
        where
            Self: 'a;
        fn vertex_mut(
            &mut self,
            seal: Seal,
            index: Self::Index,
        ) -> Option<Self::VertexMut<'_>>;
        /// The index the next [`Self::push`] returns.
        fn next_index(
            &self,
            seal: Seal,
        ) -> Self::Index;
        /// A new parentless vertex (staged inside a stage).
        fn push(
            &mut self,
            seal: Seal,
            node: VertexNode<AtomOf<Self::Kind>, Self::Index>,
            width: TokenWidth,
        ) -> Self::Index;
        fn register_atom(
            &mut self,
            seal: Seal,
            value: AtomOf<Self::Kind>,
            index: Self::Index,
        );
        fn bump_generation(
            &mut self,
            seal: Seal,
        );
    }

    /// Write handle of one vertex.
    pub trait VertexWrite {
        type Kind: GraphKind;
        type Index: VertexId;
        fn set_node(
            &mut self,
            node: VertexNode<AtomOf<Self::Kind>, Self::Index>,
        );
        fn set_width(
            &mut self,
            width: TokenWidth,
        );
        fn link_parent(
            &mut self,
            parent: Self::Index,
            offset: AtomPosition,
        );
        fn unlink_parent(
            &mut self,
            parent: Self::Index,
            offset: AtomPosition,
        );
        #[cfg(debug_assertions)]
        fn set_label(
            &mut self,
            label: String,
        );
    }

    impl<T: Write + ?Sized> Write for &mut T {
        type VertexMut<'a>
            = T::VertexMut<'a>
        where
            Self: 'a;
        fn vertex_mut(
            &mut self,
            seal: Seal,
            index: Self::Index,
        ) -> Option<Self::VertexMut<'_>> {
            (**self).vertex_mut(seal, index)
        }
        fn next_index(
            &self,
            seal: Seal,
        ) -> Self::Index {
            (**self).next_index(seal)
        }
        fn push(
            &mut self,
            seal: Seal,
            node: VertexNode<AtomOf<Self::Kind>, Self::Index>,
            width: TokenWidth,
        ) -> Self::Index {
            (**self).push(seal, node, width)
        }
        fn register_atom(
            &mut self,
            seal: Seal,
            value: AtomOf<Self::Kind>,
            index: Self::Index,
        ) {
            (**self).register_atom(seal, value, index)
        }
        fn bump_generation(
            &mut self,
            seal: Seal,
        ) {
            (**self).bump_generation(seal)
        }
    }
}

/// A store `GraphEdit` can write. Implemented for every store with the
/// sealed raw primitives; the primitives themselves stay unnameable outside
/// `context-trace`.
pub trait GraphWrite: GraphRead + raw::Write {}
impl<T: raw::Write + ?Sized> GraphWrite for T {}
