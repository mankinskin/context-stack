//! Stages (staging plan §3.6): a store whose own vertices are staged until
//! its handle commits them, and the [`Translation`] of staged into committed
//! indices.
//!
//! Two implementations: the physical [`overlay`] next to the read-only
//! committed graph and the logical [`tagged`] stage inside it.

pub mod overlay;
pub mod tagged;

use std::fmt;

use either::Either;

use crate::{
    Token,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        StagedIndex,
        VertexIndex,
    },
    positioned::store::{
        GraphWrite,
        HasGraphMut,
    },
};

/// A store with an open stage: indices are [`StageIndex`], new vertices are
/// staged.
pub trait Stage: GraphWrite<Index = StageIndex> {
    /// The staged vertices in creation order.
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
}

/// Staged → committed; committed indices are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation(Map);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Map {
    /// Committed index by [`StagedIndex`] slot (overlay: vertices moved).
    Table(Vec<CommittedIndex>),
    /// A staged slot is its committed slot (tag: only the tag bit is
    /// cleared); the number of formerly staged vertices.
    Untag(usize),
}

impl Default for Translation {
    fn default() -> Self {
        Self(Map::Table(Vec::new()))
    }
}

impl Translation {
    pub(crate) fn table(committed: Vec<CommittedIndex>) -> Self {
        Self(Map::Table(committed))
    }
    pub(crate) fn untag(staged: usize) -> Self {
        Self(Map::Untag(staged))
    }
    /// The committed indices of a table, by staged slot.
    pub(crate) fn slots(&self) -> &[CommittedIndex] {
        match &self.0 {
            Map::Table(committed) => committed,
            Map::Untag(_) => &[],
        }
    }
    /// # Panics
    /// If `index` is staged but not part of the committed stage (overlay).
    pub fn index(
        &self,
        index: StageIndex,
    ) -> CommittedIndex {
        match (index.split(), &self.0) {
            (Either::Left(committed), _) => committed,
            (Either::Right(staged), Map::Table(committed)) =>
                committed[staged.slot()],
            (Either::Right(staged), Map::Untag(_)) =>
                VertexIndex(staged.slot()),
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
        match &self.0 {
            Map::Table(committed) => committed.len(),
            &Map::Untag(staged) => staged,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
