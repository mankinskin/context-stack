//! Stages (staging plan §3.6): a store whose own vertices are staged until
//! its handle commits them, and the [`Translation`] of staged into committed
//! indices.
//!
//! The implementation is the physical [`overlay`] next to the read-only
//! committed graph.

pub mod overlay;

use std::fmt;

use either::Either;

use crate::{
    Token,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        StagedIndex,
        VertexId as _,
    },
    positioned::store::{
        GraphWrite,
        HasGraphMut,
        raw,
    },
};

/// A store with an open stage: indices are [`StageIndex`], new vertices are
/// staged, unstable staged vertices can be removed (swap-remove, [`Moved`])
/// and marked stable ([`Swap`]).
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Stabilize {
    /// The staged vertices in slot order (not creation order: removals and
    /// stabilizations move vertices); the first [`Self::stable`] are stable.
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
    /// Length of the stable slot prefix. Stable vertices are never removed
    /// (cheap stabilization plan 20261126).
    fn stable(&self) -> usize;
    /// Whether `index` is a stable staged vertex.
    fn is_stable(
        &self,
        index: StageIndex,
    ) -> bool {
        index
            .staged()
            .is_some_and(|staged| staged.slot() < self.stable())
    }
}

/// Two staged vertices exchanged slots (a stabilization).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swap {
    pub a: StageIndex,
    pub b: StageIndex,
}

impl Swap {
    /// `index` after the swap: `a` and `b` are exchanged, every other index
    /// is unchanged.
    pub fn apply(
        self,
        index: StageIndex,
    ) -> StageIndex {
        if index == self.a {
            self.b
        } else if index == self.b {
            self.a
        } else {
            index
        }
    }
}

/// A staged vertex moved from slot `from` to slot `to` by a removal
/// (swap-remove: `from` was the last slot, `to` the removed vertex's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    pub from: StageIndex,
    pub to: StageIndex,
}

impl Moved {
    /// `index` after the move: `from` becomes `to`, every other index is
    /// unchanged.
    pub fn apply(
        self,
        index: StageIndex,
    ) -> StageIndex {
        if index == self.from { self.to } else { index }
    }
}

/// Staged → committed; committed indices are unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Translation(Vec<CommittedIndex>);

impl Translation {
    pub(crate) fn table(committed: Vec<CommittedIndex>) -> Self {
        Self(committed)
    }
    /// The committed indices by staged slot.
    pub(crate) fn slots(&self) -> &[CommittedIndex] {
        &self.0
    }
    /// # Panics
    /// If `index` is staged but not part of the committed stage.
    pub fn index(
        &self,
        index: StageIndex,
    ) -> CommittedIndex {
        match index.split() {
            Either::Left(committed) => committed,
            Either::Right(staged) => self.0[staged.slot()],
        }
    }
    pub fn token(
        &self,
        token: Token<StageIndex>,
    ) -> Token<CommittedIndex> {
        Token::new(self.index(token.index), token.width)
    }
    /// Number of translated (formerly staged) vertices.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A handle with an open stage; the only way vertices cross the boundary
/// into the committed graph.
pub trait StageHandle: HasGraphMut<Index = StageIndex> {
    /// The write guard as a [`Stage`].
    type View<'a>: Stage<Kind = Self::Kind>
    where
        Self: 'a;
    fn stage(&mut self) -> Self::View<'_>;
    /// Commit every staged vertex and every change of committed vertices to
    /// the committed graph; `root` translated.
    ///
    /// # Panics
    /// If a staged composite other than `root` has fewer than two parent
    /// occurrences (an invalid vertex set, retention invariant G6).
    fn commit(
        self,
        root: Token<StageIndex>,
    ) -> Result<(Token<CommittedIndex>, Translation), CommitError>;
}

/// Why a stage could not be committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The committed graph was edited while the stage was open.
    BaseChanged { opened: u64, now: u64 },
}

impl fmt::Display for CommitError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::BaseChanged { opened, now } => write!(
                f,
                "the committed graph changed while the stage was open (generation {opened} → {now})"
            ),
        }
    }
}

impl std::error::Error for CommitError {}
