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
    },
    positioned::store::{
        GraphWrite,
        HasGraphMut,
        raw,
    },
};

/// A store with an open stage: indices are [`StageIndex`], new vertices are
/// staged, unstable staged vertices can be removed and staged vertices can
/// be marked stable. Staged vertices never move: a removal leaves a hole and
/// no index is ever renamed.
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Stabilize {
    /// The live staged vertices in creation order (without removed slots).
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
    /// Whether `index` is a live stable staged vertex. Stable vertices are
    /// never removed (cheap stabilization plan 20261126).
    fn is_stable(
        &self,
        index: StageIndex,
    ) -> bool;
}

/// Staged → committed; committed indices are unchanged. Removed staged
/// slots have no committed index.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Translation {
    slots: Vec<Option<CommittedIndex>>,
    len: usize,
}

impl Translation {
    pub(crate) fn table(slots: Vec<Option<CommittedIndex>>) -> Self {
        let len = slots.iter().flatten().count();
        Self { slots, len }
    }
    /// The committed indices by staged slot (`None` for removed slots).
    pub(crate) fn slots(&self) -> &[Option<CommittedIndex>] {
        &self.slots
    }
    /// # Panics
    /// If `index` is staged but not part of the committed stage (unknown
    /// or removed).
    pub fn index(
        &self,
        index: StageIndex,
    ) -> CommittedIndex {
        match index.split() {
            Either::Left(committed) => committed,
            Either::Right(staged) => self
                .slots
                .get(staged.slot())
                .copied()
                .flatten()
                .unwrap_or_else(|| panic!("staged {index} was not committed")),
        }
    }
    pub fn token(
        &self,
        token: Token<StageIndex>,
    ) -> Token<CommittedIndex> {
        Token::new(self.index(token.index), token.width)
    }
    /// Number of translated (formerly staged, not removed) vertices.
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
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
