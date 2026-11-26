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
        graph::vertex::{
            StageIndex,
            VertexId,
        },
        positioned::{
            model::VertexNode,
            stage::{
                Moved,
                Swap,
            },
        },
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

    /// Removal of staged vertices (stages only).
    pub trait Remove: Write {
        /// Remove the staged vertex `index`, which has no parent links and
        /// whose children no longer link to it (checked by `GraphEdit`).
        /// Swap-remove: the last staged vertex moves into the freed slot and
        /// every reference to it is renamed before returning; `None` if
        /// `index` was the last slot.
        fn remove(
            &mut self,
            seal: Seal,
            index: StageIndex,
        ) -> Option<Moved>;
    }

    impl<T: Remove + ?Sized> Remove for &mut T {
        fn remove(
            &mut self,
            seal: Seal,
            index: StageIndex,
        ) -> Option<Moved> {
            (**self).remove(seal, index)
        }
    }

    /// Stability marks of staged vertices (cheap stabilization plan 20261126).
    pub trait Stabilize: Remove {
        /// Mark the unstable staged vertex `index` stable: swap it into the
        /// first unstable slot (renaming every reference of both swapped
        /// vertices) and grow the stable prefix by one; `None` if it already
        /// was in that slot. [`Remove::remove`] panics on a stable slot.
        fn stabilize(
            &mut self,
            seal: Seal,
            index: StageIndex,
        ) -> Option<Swap>;
    }

    impl<T: Stabilize + ?Sized> Stabilize for &mut T {
        fn stabilize(
            &mut self,
            seal: Seal,
            index: StageIndex,
        ) -> Option<Swap> {
            (**self).stabilize(seal, index)
        }
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
