//! Paths of positioned search (search plan 2.2, canonical rules 2.4).
//!
//! Positioned counterparts of the legacy role paths: a [`StartPath`] climbs
//! from the match start to a root, an [`EndPath`] descends from the root to
//! the token at the match end, a [`RangePath`] combines both with the matched
//! span in absolute atom offsets of the root, and a [`QueryPath`] addresses a
//! position inside a token-sequence query. All steps are
//! [`OccurrenceLocation`]s; navigation uses only compositions
//! ([`PositionedGraph::start_at`], [`PositionedGraph::prefix_child`],
//! [`PositionedGraph::advance_end_path`]), never content.

use std::ops::{
    ControlFlow,
    Range,
};

use super::{
    graph::PositionedGraph,
    location::{
        OccurrenceLocation,
        OccurrencePath,
    },
};
use crate::{
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::wide::Wide,
};

/// How a matched span covers its root (legacy `PathCoverage`, classified
/// from the span alone). The search result type wrapping the paths lives in
/// `context-search` (step 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpanCoverage {
    /// `span == 0..width`.
    EntireRoot,
    /// Starts at the root start, ends inside.
    Prefix,
    /// Starts inside, ends at the root end.
    Postfix,
    /// Starts and ends inside.
    Range,
}

impl SpanCoverage {
    pub fn of(
        span: &Range<AtomPosition>,
        width: TokenWidth,
    ) -> Self {
        match (span.start.0 == 0, span.end.0 == width.0) {
            (true, true) => Self::EntireRoot,
            (true, false) => Self::Prefix,
            (false, true) => Self::Postfix,
            (false, false) => Self::Range,
        }
    }
}

fn offset_sum<'a>(steps: impl IntoIterator<Item = &'a OccurrenceLocation>) -> AtomPosition {
    steps
        .into_iter()
        .fold(AtomPosition::default(), |sum, step| sum + step.offset.0)
}

/// Path from a root down to the token at the match start, stored bottom-up
/// so that a raise is O(1).
///
/// `steps[0]` is the deepest step (its child is [`Self::start`]), the last
/// step is the root entry (its parent is [`Self::root`]). Canonical form
/// (2.4): built from the climbed chain, it stops at the largest climbed token
/// starting at the match start.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StartPath {
    root: Token,
    /// Child of the root entry step.
    entry: Token,
    /// Child of the deepest step; starts at the match start.
    start: Token,
    steps: Vec<OccurrenceLocation>,
}

impl StartPath {
    /// One step: `start` at `offset` inside `root`.
    pub fn new(
        root: Token,
        offset: impl Into<AtomPosition>,
        start: Token,
    ) -> Self {
        Self {
            root,
            entry: start,
            start,
            steps: vec![OccurrenceLocation {
                parent: root.index,
                offset: offset.into(),
                child: start.index,
            }],
        }
    }
    pub fn root(&self) -> Token {
        self.root
    }
    /// The root child the path enters through.
    pub fn entry(&self) -> Token {
        self.entry
    }
    /// The token the path ends at (starts at the match start).
    pub fn start(&self) -> Token {
        self.start
    }
    pub fn root_entry(&self) -> &OccurrenceLocation {
        self.steps.last().expect("start path has a step")
    }
    /// Steps from the deepest to the root entry.
    pub fn bottom_up(&self) -> &[OccurrenceLocation] {
        &self.steps
    }
    /// Steps from the root entry down to the start token.
    pub fn top_down(&self) -> impl DoubleEndedIterator<Item = &OccurrenceLocation> {
        self.steps.iter().rev()
    }
    pub fn to_occurrence_path(&self) -> OccurrencePath {
        OccurrencePath::new(self.top_down().copied().collect())
            .expect("start path is a connected chain")
    }
    /// Absolute offset of [`Self::start`] in the root (the match start).
    pub fn start_offset(&self) -> AtomPosition {
        offset_sum(&self.steps)
    }
    /// Absolute end of [`Self::entry`] in the root.
    pub fn entry_end(&self) -> AtomPosition {
        self.root_entry().offset + self.entry.width().0
    }
    /// Raise to `parent`, where the old root occurs at `offset`.
    ///
    /// Canonical rule (2.4): if the match starts at the old root's start
    /// (`at_root_start`), the old root is the largest climbed token starting
    /// at the match start and the path becomes `[(parent, offset, root)]`;
    /// otherwise the step is prepended.
    pub fn raise(
        &mut self,
        parent: Token,
        offset: impl Into<AtomPosition>,
        at_root_start: bool,
    ) {
        let step = OccurrenceLocation {
            parent: parent.index,
            offset: offset.into(),
            child: self.root.index,
        };
        if at_root_start {
            self.steps.clear();
            self.start = self.root;
        }
        self.steps.push(step);
        self.entry = self.root;
        self.root = parent;
    }
}

/// Path from a root down to a leaf token, top-down (non-empty).
///
/// During the search the leaf is the next graph token to compare (it starts
/// at the current match end) or, after a match, the last matched token.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EndPath {
    root: Token,
    leaf: Token,
    steps: Vec<OccurrenceLocation>,
}

impl EndPath {
    /// `steps` must be a connected chain from `root` ending at `leaf`.
    pub fn new(
        root: Token,
        (leaf, steps): (Token, Vec<OccurrenceLocation>),
    ) -> Self {
        debug_assert!(steps.first().is_some_and(|s| s.parent == root.index));
        debug_assert!(steps.last().is_some_and(|s| s.child == leaf.index));
        debug_assert!(OccurrencePath::new(steps.clone()).is_some());
        Self { root, leaf, steps }
    }
    /// [`PositionedGraph::start_at`] as an end path.
    pub fn start_at<G: GraphKind>(
        graph: &PositionedGraph<G>,
        root: Token,
        p: impl Into<AtomPosition>,
    ) -> Option<Self> {
        graph.start_at(root, p).map(|found| Self::new(root, found))
    }
    pub fn root(&self) -> Token {
        self.root
    }
    pub fn leaf(&self) -> Token {
        self.leaf
    }
    pub fn steps(&self) -> &[OccurrenceLocation] {
        &self.steps
    }
    pub fn root_exit(&self) -> &OccurrenceLocation {
        &self.steps[0]
    }
    pub fn to_occurrence_path(&self) -> OccurrencePath {
        OccurrencePath::new(self.steps.clone()).expect("end path is a connected chain")
    }
    /// Absolute offset of the leaf in the root.
    pub fn leaf_offset(&self) -> AtomPosition {
        offset_sum(&self.steps)
    }
    /// Absolute end of the leaf in the root.
    pub fn leaf_end(&self) -> AtomPosition {
        self.leaf_offset() + self.leaf.width().0
    }
    /// Move to [`PositionedGraph::start_at`]`(root, p)` with the corrected
    /// pop rule ([`PositionedGraph::advance_end_path`]). `false` (path
    /// unchanged) iff `p ≥ width(root)`.
    pub fn advance<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
        p: impl Into<AtomPosition>,
    ) -> bool {
        let p = p.into();
        if p.0 >= self.root.width().0 {
            return false;
        }
        let (leaf, steps) = graph
            .advance_end_path(self.root, std::mem::take(&mut self.steps), p)
            .expect("p is inside the root");
        self.leaf = leaf;
        self.steps = steps;
        true
    }
    /// Descend into the prefix child of the leaf (graph-side decomposition).
    /// `false` (path unchanged) for an atom leaf.
    pub fn push_prefix<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> bool {
        let Some(prefix) = graph.prefix_child(self.leaf) else {
            return false;
        };
        self.steps.push(OccurrenceLocation {
            parent: self.leaf.index,
            offset: prefix.offset,
            child: prefix.child.index,
        });
        self.leaf = prefix.child;
        true
    }
    /// Canonical end path (2.4, positioned `simplify`): stop at the first
    /// step whose child ends at `end` (the largest token on the path ending
    /// at the match end). Unchanged if no step ends at `end`.
    pub fn simplify<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
        end: impl Into<AtomPosition>,
    ) {
        let end = end.into();
        let mut start = AtomPosition::default();
        for (i, step) in self.steps.iter().enumerate() {
            start += step.offset.0;
            let child = graph.expect_token(step.child);
            if start + child.width().0 == end {
                self.steps.truncate(i + 1);
                self.leaf = child;
                return;
            }
        }
    }
}

/// A matched range inside `root`: `span` in absolute atom offsets of the
/// root, the start path to the token at `span.start` and the end path.
/// Start and end occurrences may overlap.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RangePath {
    pub root: Token,
    pub span: Range<AtomPosition>,
    pub start: StartPath,
    pub end: EndPath,
}

impl RangePath {
    pub fn new(
        span: Range<AtomPosition>,
        start: StartPath,
        end: EndPath,
    ) -> Self {
        debug_assert_eq!(start.root(), end.root());
        debug_assert_eq!(start.start_offset(), span.start);
        Self {
            root: start.root(),
            span,
            start,
            end,
        }
    }
    pub fn coverage(&self) -> SpanCoverage {
        SpanCoverage::of(&self.span, self.root.width())
    }
    /// Extend the span to the end of the end leaf (the leaf matched).
    pub fn match_end_leaf(&mut self) {
        self.span.end = self.end.leaf_end();
    }
    /// Canonical end path for the current span end (2.4).
    pub fn simplify<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) {
        self.end.simplify(graph, self.span.end);
    }
}

/// Position in a token-sequence query (Decision 6 b).
///
/// The leaf is `pattern[entry]` itself or a descendant of it reached by
/// `end` (top-down steps inside `pattern[entry]`). Composite query tokens are
/// split with [`PositionedGraph::prefix_child`] and advanced inside with
/// [`PositionedGraph::advance_end_path`], symmetric to the graph side;
/// content is never read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QueryPath {
    pattern: Vec<Token>,
    entry: usize,
    /// Query position of `pattern[entry]`.
    entry_position: AtomPosition,
    /// Query position of the leaf.
    position: AtomPosition,
    leaf: Token,
    end: Vec<OccurrenceLocation>,
}

impl QueryPath {
    /// At the first query token; `None` for an empty pattern.
    pub fn new(pattern: Vec<Token>) -> Option<Self> {
        let leaf = *pattern.first()?;
        Some(Self {
            pattern,
            entry: 0,
            entry_position: AtomPosition::default(),
            position: AtomPosition::default(),
            leaf,
            end: Vec::new(),
        })
    }
    pub fn pattern(&self) -> &[Token] {
        &self.pattern
    }
    /// Index of the query token containing the leaf.
    pub fn entry(&self) -> usize {
        self.entry
    }
    /// Query position of the leaf start.
    pub fn position(&self) -> AtomPosition {
        self.position
    }
    /// Query position of the leaf end.
    pub fn leaf_end(&self) -> AtomPosition {
        self.position + self.leaf.width().0
    }
    pub fn leaf(&self) -> Token {
        self.leaf
    }
    /// Steps from `pattern[entry]` down to the leaf (empty if the leaf is
    /// `pattern[entry]`).
    pub fn end(&self) -> &[OccurrenceLocation] {
        &self.end
    }
    /// Total query width.
    pub fn width(&self) -> TokenWidth {
        TokenWidth(self.pattern.iter().map(|t| t.width().0).sum())
    }
    /// The leaf ends at the query end.
    pub fn at_end(&self) -> bool {
        self.leaf_end().0 == self.width().0
    }
    /// Descend into the prefix child of the leaf (query-side
    /// decomposition). `false` (path unchanged) for an atom leaf.
    pub fn prefix_child<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> bool {
        let Some(prefix) = graph.prefix_child(self.leaf) else {
            return false;
        };
        self.end.push(OccurrenceLocation {
            parent: self.leaf.index,
            offset: prefix.offset,
            child: prefix.child.index,
        });
        self.leaf = prefix.child;
        true
    }
    /// Move to the largest token starting at the leaf end: inside the
    /// current query token via the corrected pop rule, else the next query
    /// token. `Break` (path unchanged) at the query end.
    pub fn advance<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> ControlFlow<()> {
        let p = self.leaf_end();
        let token = self.pattern[self.entry];
        let local = AtomPosition(p.0 - self.entry_position.0);
        if local.0 < token.width().0 {
            let (leaf, end) = graph
                .advance_end_path(token, std::mem::take(&mut self.end), local)
                .expect("position inside the query token");
            self.leaf = leaf;
            self.end = end;
        } else if let Some(&next) = self.pattern.get(self.entry + 1) {
            self.entry += 1;
            self.entry_position = p;
            self.leaf = next;
            self.end.clear();
        } else {
            return ControlFlow::Break(());
        }
        self.position = p;
        ControlFlow::Continue(())
    }
}
