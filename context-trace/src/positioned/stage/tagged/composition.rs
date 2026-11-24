//! [`TaggedComposition`]: compositions of a tagged stage with derived tag bits.

use super::tag_child;
use crate::{
    AtomPosition,
    TokenWidth,
    graph::vertex::{
        StageIndex,
        wide::Wide,
    },
    positioned::{
        model::{
            Composition,
            Occurrence,
            slice,
        },
        store::CompositionRead,
    },
};

/// [`CompositionRead`] of a tagged vertex: the stored composition with the
/// tag bits of its children derived from slot and width.
///
/// Two words (as the overlay's `Lifted`), so a [`NodeRef`](crate::positioned::NodeRef) of it is
/// returned in registers.
#[derive(Clone, Copy, Debug)]
pub struct TaggedComposition<'a> {
    pub(super) composition: &'a Composition,
    pub(super) first: usize,
}

impl<'a> TaggedComposition<'a> {
    #[inline(always)]
    fn stored(self) -> &'a [Occurrence] {
        self.composition.occurrences()
    }
}

impl Wide for TaggedComposition<'_> {
    fn width(&self) -> TokenWidth {
        TokenWidth(slice::width(self.stored()))
    }
}

impl CompositionRead for TaggedComposition<'_> {
    type Index = StageIndex;
    fn len(self) -> usize {
        self.stored().len()
    }
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<StageIndex>> {
        self.stored().get(k).map(|o| tag_child(self.first, o))
    }
    fn prefix(self) -> Occurrence<StageIndex> {
        tag_child(self.first, &self.stored()[0])
    }
    fn postfix(self) -> Occurrence<StageIndex> {
        tag_child(
            self.first,
            self.stored()
                .last()
                .expect("a composition has at least two occurrences"),
        )
    }
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::covering(self.stored(), p).map(|o| tag_child(self.first, o))
    }
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::ending_at(self.stored(), e).map(|o| tag_child(self.first, o))
    }
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<StageIndex>> {
        slice::crossing(self.stored(), p)
            .iter()
            .map(move |o| tag_child(self.first, o))
    }
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<StageIndex>> + ExactSizeIterator
    {
        self.stored()
            .iter()
            .map(move |o| tag_child(self.first, o))
    }
}
