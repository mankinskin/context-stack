//! [`Lifted`]: compositions of an overlay in the stage's index space.

use crate::{
    AtomPosition,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
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

/// [`CompositionRead`] of an overlay vertex in the stage's index space: a
/// staged composition, the copy-on-write composition of a committed host,
/// or an unchanged committed composition read in place (child indices
/// lifted without copying, [`Composition::lifted`]).
#[derive(Clone, Copy, Debug)]
pub struct Lifted<'a>(&'a [Occurrence<StageIndex>]);

impl<'a> Lifted<'a> {
    pub(super) fn base(composition: &'a Composition<CommittedIndex>) -> Self {
        Self(composition.lifted())
    }
    pub(super) fn stage(composition: &'a Composition<StageIndex>) -> Self {
        Self(composition.occurrences())
    }
}

impl Wide for Lifted<'_> {
    fn width(&self) -> TokenWidth {
        TokenWidth(slice::width(self.0))
    }
}

impl CompositionRead for Lifted<'_> {
    type Index = StageIndex;
    fn len(self) -> usize {
        self.0.len()
    }
    fn get(
        self,
        k: usize,
    ) -> Option<Occurrence<StageIndex>> {
        self.0.get(k).copied()
    }
    fn prefix(self) -> Occurrence<StageIndex> {
        self.0[0]
    }
    fn postfix(self) -> Occurrence<StageIndex> {
        *self
            .0
            .last()
            .expect("a composition has at least two occurrences")
    }
    fn covering(
        self,
        p: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::covering(self.0, p).copied()
    }
    fn ending_at(
        self,
        e: AtomPosition,
    ) -> Option<Occurrence<StageIndex>> {
        slice::ending_at(self.0, e).copied()
    }
    fn crossing(
        self,
        p: AtomPosition,
    ) -> impl ExactSizeIterator<Item = Occurrence<StageIndex>> {
        slice::crossing(self.0, p).iter().copied()
    }
    fn occurrences(
        self
    ) -> impl DoubleEndedIterator<Item = Occurrence<StageIndex>> + ExactSizeIterator
    {
        self.0.iter().copied()
    }
}
