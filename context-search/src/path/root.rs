//! Path roots (legacy `path::accessors::root`): a graph token or a query
//! pattern.

use std::{
    fmt::Debug,
    hash::Hash,
};

use context_trace::{
    AtomPosition,
    graph::{
        vertex::{
            CommittedIndex,
            VertexId,
            wide::Wide,
        },
    },
    positioned::{
        check::expect_token,
        GraphRead,
        OccurrenceLocation,
    },
    Token,
    TokenWidth,
};

/// Root of a [`RolePath`](super::RolePath).
///
/// The stored steps descend from [`Self::steps_root`]. For a graph root
/// (a [`Token`]) that is the root itself and the first step is in the root;
/// for a [`QueryRoot`] it is the current pattern token (the root child), so
/// the steps may be empty.
pub trait PathRoot: Clone + Debug + PartialEq + Eq + Hash {
    /// Index type of the store the path lives in.
    type Index: VertexId;
    /// The token the stored steps descend from.
    fn steps_root(&self) -> Token<Self::Index>;
    /// Offset of [`Self::steps_root`] in root coordinates.
    fn steps_root_offset(&self) -> AtomPosition;
    /// Total width of the root.
    fn root_width(&self) -> TokenWidth;
    /// The root child of a path with root step `root_step` (top-down first
    /// stored step) and leaf `leaf`.
    fn root_child<G: GraphRead<Index = Self::Index> + ?Sized>(
        &self,
        graph: &G,
        root_step: Option<&OccurrenceLocation<Self::Index>>,
        leaf: Token<Self::Index>,
    ) -> Token<Self::Index>;
    /// Offset of the root child in root coordinates.
    fn root_child_offset(
        &self,
        root_step: Option<&OccurrenceLocation<Self::Index>>,
    ) -> AtomPosition;
    /// Move to the root child after [`Self::steps_root`] and return it;
    /// `None` (unchanged) if there is none.
    fn next_root_child(&mut self) -> Option<Token<Self::Index>>;
}

/// A graph root: steps start with the occurrence of the root child in it;
/// there is no next root child.
impl<I: VertexId> PathRoot for Token<I> {
    type Index = I;
    fn steps_root(&self) -> Token<I> {
        *self
    }
    fn steps_root_offset(&self) -> AtomPosition {
        AtomPosition::default()
    }
    fn root_width(&self) -> TokenWidth {
        Wide::width(self)
    }
    fn root_child<G: GraphRead<Index = I> + ?Sized>(
        &self,
        graph: &G,
        root_step: Option<&OccurrenceLocation<I>>,
        leaf: Token<I>,
    ) -> Token<I> {
        let step = root_step.expect("graph-rooted path has a root step");
        if step.child == leaf.index {
            leaf
        } else {
            expect_token(graph, step.child)
        }
    }
    fn root_child_offset(
        &self,
        root_step: Option<&OccurrenceLocation<I>>,
    ) -> AtomPosition {
        root_step.expect("graph-rooted path has a root step").offset
    }
    fn next_root_child(&mut self) -> Option<Token<I>> {
        None
    }
}

/// A token-sequence query as root (legacy `PatternEndPath` root): the root
/// children are the pattern tokens, `pattern[entry]` at query position
/// `entry_position`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryRoot<I: VertexId = CommittedIndex> {
    pub(crate) pattern: Vec<Token<I>>,
    pub(crate) entry: usize,
    pub(crate) entry_position: AtomPosition,
}

impl<I: VertexId> QueryRoot<I> {
    /// At the first query token; `None` for an empty pattern.
    pub fn new(pattern: Vec<Token<I>>) -> Option<Self> {
        (!pattern.is_empty()).then_some(Self {
            pattern,
            entry: 0,
            entry_position: AtomPosition::default(),
        })
    }
    pub fn pattern(&self) -> &[Token<I>] {
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

impl<I: VertexId> PathRoot for QueryRoot<I> {
    type Index = I;
    fn steps_root(&self) -> Token<I> {
        self.pattern[self.entry]
    }
    fn steps_root_offset(&self) -> AtomPosition {
        self.entry_position
    }
    fn root_width(&self) -> TokenWidth {
        TokenWidth(self.pattern.iter().map(|t| t.width().0).sum())
    }
    fn root_child<G: GraphRead<Index = I> + ?Sized>(
        &self,
        _graph: &G,
        _root_step: Option<&OccurrenceLocation<I>>,
        _leaf: Token<I>,
    ) -> Token<I> {
        self.steps_root()
    }
    fn root_child_offset(
        &self,
        _root_step: Option<&OccurrenceLocation<I>>,
    ) -> AtomPosition {
        self.entry_position
    }
    fn next_root_child(&mut self) -> Option<Token<I>> {
        let next = *self.pattern.get(self.entry + 1)?;
        self.entry_position += self.pattern[self.entry].width().0;
        self.entry += 1;
        Some(next)
    }
}
