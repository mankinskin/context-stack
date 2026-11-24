//! Sealed write primitives behind `GraphEdit` (staging plan §3.5).

use super::GraphRead;

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
        positioned::model::VertexNode,
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
