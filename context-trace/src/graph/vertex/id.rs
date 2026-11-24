//! Typed vertex indices (staging plan §3.2): every store has its own index
//! type, so a committed index cannot address a stage's own vertices and a
//! staged index cannot address the committed graph.

use std::{
    fmt,
    hash::Hash,
};

use either::Either;

use crate::graph::vertex::{
    VertexIndex,
    has_vertex_index::HasVertexIndex,
};

mod sealed {
    pub trait Id {}
}

/// An index type of a vertex store. Implemented only in `context-trace`.
pub trait VertexId:
    Copy
    + Eq
    + Ord
    + Hash
    + fmt::Debug
    + fmt::Display
    + Send
    + Sync
    + HasVertexIndex<Index = Self>
    + sealed::Id
{
    /// The committed index, if this index addresses a committed vertex.
    fn committed(self) -> Option<CommittedIndex>;
    /// The staged index, if this index addresses a staged vertex.
    fn staged(self) -> Option<StagedIndex>;
}

/// A vertex of the committed graph.
///
/// The same type as the row model's untyped [`VertexIndex`], so the shared
/// [`Token`](crate::Token) keeps one type for both models; the typed boundary
/// is between committed and staged indices.
pub type CommittedIndex = VertexIndex;

impl sealed::Id for CommittedIndex {}
impl VertexId for CommittedIndex {
    fn committed(self) -> Option<CommittedIndex> {
        Some(self)
    }
    fn staged(self) -> Option<StagedIndex> {
        None
    }
}

/// A vertex created in the current stage; valid in no other stage and not in
/// the committed graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StagedIndex(usize);

impl StagedIndex {
    /// # Panics
    /// If `slot` has the [`StageIndex`] tag bit set.
    pub fn new(slot: usize) -> Self {
        assert!(slot & STAGED_BIT == 0, "staged slot {slot} out of range");
        Self(slot)
    }
    pub fn slot(self) -> usize {
        self.0
    }
}

impl sealed::Id for StagedIndex {}
impl VertexId for StagedIndex {
    fn committed(self) -> Option<CommittedIndex> {
        None
    }
    fn staged(self) -> Option<StagedIndex> {
        Some(self)
    }
}

impl fmt::Display for StagedIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

impl fmt::Debug for StagedIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Displays an index without name resolution: the plain slot for committed
/// indices (as `VertexIndex::0`), else the index's own display (`s<slot>`).
pub(crate) struct Plain<I>(pub I);

impl<I: VertexId> fmt::Display for Plain<I> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self.0.committed() {
            Some(committed) => write!(f, "{}", committed.0),
            None => fmt::Display::fmt(&self.0, f),
        }
    }
}

const STAGED_BIT: usize = 1 << (usize::BITS - 1);

/// Index space of a stage: a committed or a staged index, packed into one
/// `usize` (top bit = staged).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StageIndex(usize);

impl StageIndex {
    /// [`From<CommittedIndex>`] without the range check (hot paths of
    /// stores whose committed indices are dense slots).
    #[inline]
    pub(crate) fn lift(index: CommittedIndex) -> Self {
        debug_assert!(index.0 & STAGED_BIT == 0, "committed index {} out of range", index.0);
        Self(index.0)
    }
    /// The slot of a store addressing staged and committed vertices in one
    /// vector (tag stage), with the tag bit set iff `staged`.
    #[inline]
    pub(crate) fn tagged(
        slot: usize,
        staged: bool,
    ) -> Self {
        debug_assert!(slot & STAGED_BIT == 0, "slot {slot} out of range");
        Self(slot | if staged { STAGED_BIT } else { 0 })
    }
    /// The index without the tag bit.
    #[inline]
    pub(crate) fn slot(self) -> usize {
        self.0 & !STAGED_BIT
    }
    pub fn is_staged(self) -> bool {
        self.0 & STAGED_BIT != 0
    }
    pub fn split(self) -> Either<CommittedIndex, StagedIndex> {
        if self.is_staged() {
            Either::Right(StagedIndex(self.0 & !STAGED_BIT))
        } else {
            Either::Left(VertexIndex(self.0))
        }
    }
}

impl sealed::Id for StageIndex {}
impl VertexId for StageIndex {
    fn committed(self) -> Option<CommittedIndex> {
        self.split().left()
    }
    fn staged(self) -> Option<StagedIndex> {
        self.split().right()
    }
}

impl From<CommittedIndex> for StageIndex {
    /// # Panics
    /// If the committed index has the tag bit set.
    fn from(index: CommittedIndex) -> Self {
        assert!(
            index.0 & STAGED_BIT == 0,
            "committed index {} out of range",
            index.0
        );
        Self(index.0)
    }
}

impl From<StagedIndex> for StageIndex {
    fn from(index: StagedIndex) -> Self {
        Self(index.0 | STAGED_BIT)
    }
}

impl TryFrom<StageIndex> for CommittedIndex {
    /// The staged index, which only a stage's commit translates.
    type Error = StagedIndex;
    fn try_from(index: StageIndex) -> Result<Self, Self::Error> {
        index.split().either(Ok, Err)
    }
}

impl TryFrom<StageIndex> for StagedIndex {
    type Error = CommittedIndex;
    fn try_from(index: StageIndex) -> Result<Self, Self::Error> {
        index.split().either(Err, Ok)
    }
}

impl fmt::Display for StageIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self.split() {
            Either::Left(committed) => fmt::Display::fmt(&committed, f),
            Either::Right(staged) => fmt::Display::fmt(&staged, f),
        }
    }
}

impl fmt::Debug for StageIndex {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_index_round_trips() {
        let committed = VertexIndex(7);
        let staged = StagedIndex::new(7);
        let (c, s) = (StageIndex::from(committed), StageIndex::from(staged));
        assert_ne!(c, s);
        assert!(!c.is_staged() && s.is_staged());
        assert_eq!(c.split(), Either::Left(committed));
        assert_eq!(s.split(), Either::Right(staged));
        assert_eq!(CommittedIndex::try_from(c), Ok(committed));
        assert_eq!(CommittedIndex::try_from(s), Err(staged));
        assert_eq!(StagedIndex::try_from(s), Ok(staged));
        assert_eq!(c.committed(), Some(committed));
        assert_eq!(s.committed(), None);
        assert_eq!(
            std::mem::size_of::<StageIndex>(),
            std::mem::size_of::<VertexIndex>()
        );
    }
}
