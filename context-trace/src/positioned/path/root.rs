//! Path roots (legacy `path::accessors::root`): a graph token or a query
//! pattern.

use std::{
    fmt::Debug,
    hash::Hash,
};

use crate::{
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::wide::Wide,
    positioned::{
        graph::PositionedGraph,
        location::OccurrenceLocation,
    },
};

/// Root of a [`RolePath`](super::RolePath).
///
/// The stored steps descend from [`Self::steps_root`]. For a graph root
/// (a [`Token`]) that is the root itself and the first step is in the root;
/// for a [`QueryRoot`] it is the current pattern token (the root child), so
/// the steps may be empty.
pub trait PathRoot: Clone + Debug + PartialEq + Eq + Hash {
    /// The token the stored steps descend from.
    fn steps_root(&self) -> Token;
    /// Offset of [`Self::steps_root`] in root coordinates.
    fn steps_root_offset(&self) -> AtomPosition;
    /// Total width of the root.
    fn root_width(&self) -> TokenWidth;
    /// The root child of a path with root step `root_step` (top-down first
    /// stored step) and leaf `leaf`.
    fn root_child<G: GraphKind>(
        &self,
        graph: &PositionedGraph<G>,
        root_step: Option<&OccurrenceLocation>,
        leaf: Token,
    ) -> Token;
    /// Offset of the root child in root coordinates.
    fn root_child_offset(
        &self,
        root_step: Option<&OccurrenceLocation>,
    ) -> AtomPosition;
    /// Move to the root child after [`Self::steps_root`] and return it;
    /// `None` (unchanged) if there is none.
    fn next_root_child(&mut self) -> Option<Token>;
}

/// A graph root: steps start with the occurrence of the root child in it;
/// there is no next root child.
impl PathRoot for Token {
    fn steps_root(&self) -> Token {
        *self
    }
    fn steps_root_offset(&self) -> AtomPosition {
        AtomPosition::default()
    }
    fn root_width(&self) -> TokenWidth {
        Wide::width(self)
    }
    fn root_child<G: GraphKind>(
        &self,
        graph: &PositionedGraph<G>,
        root_step: Option<&OccurrenceLocation>,
        leaf: Token,
    ) -> Token {
        let step = root_step.expect("graph-rooted path has a root step");
        if step.child == leaf.index {
            leaf
        } else {
            graph.expect_token(step.child)
        }
    }
    fn root_child_offset(
        &self,
        root_step: Option<&OccurrenceLocation>,
    ) -> AtomPosition {
        root_step.expect("graph-rooted path has a root step").offset
    }
    fn next_root_child(&mut self) -> Option<Token> {
        None
    }
}

/// A token-sequence query as root (legacy `PatternEndPath` root): the root
/// children are the pattern tokens, `pattern[entry]` at query position
/// `entry_position`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryRoot {
    pub(crate) pattern: Vec<Token>,
    pub(crate) entry: usize,
    pub(crate) entry_position: AtomPosition,
}

impl QueryRoot {
    /// At the first query token; `None` for an empty pattern.
    pub fn new(pattern: Vec<Token>) -> Option<Self> {
        (!pattern.is_empty()).then_some(Self {
            pattern,
            entry: 0,
            entry_position: AtomPosition::default(),
        })
    }
    pub fn pattern(&self) -> &[Token] {
        &self.pattern
    }
    /// Index of the current query token.
    pub fn entry(&self) -> usize {
        self.entry
    }
    /// Query position of the current query token.
    pub fn entry_position(&self) -> AtomPosition {
        self.entry_position
    }
}

impl PathRoot for QueryRoot {
    fn steps_root(&self) -> Token {
        self.pattern[self.entry]
    }
    fn steps_root_offset(&self) -> AtomPosition {
        self.entry_position
    }
    fn root_width(&self) -> TokenWidth {
        TokenWidth(self.pattern.iter().map(|t| t.width().0).sum())
    }
    fn root_child<G: GraphKind>(
        &self,
        _graph: &PositionedGraph<G>,
        _root_step: Option<&OccurrenceLocation>,
        _leaf: Token,
    ) -> Token {
        self.steps_root()
    }
    fn root_child_offset(
        &self,
        _root_step: Option<&OccurrenceLocation>,
    ) -> AtomPosition {
        self.entry_position
    }
    fn next_root_child(&mut self) -> Option<Token> {
        let next = *self.pattern.get(self.entry + 1)?;
        self.entry_position += self.pattern[self.entry].width().0;
        self.entry += 1;
        Some(next)
    }
}
