//! [`CompositionRead`]: what navigation, insertion and the reader need from
//! a composition, implemented directly for `&Composition`.

use crate::{
    AtomPosition,
    graph::vertex::{
        VertexId,
        wide::Wide,
    },
    positioned::model::{
        Composition,
        Occurrence,
    },
};

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
