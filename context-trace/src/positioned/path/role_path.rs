//! The role-generic positioned path (legacy `RolePath` /
//! `RootedRolePath`).

use std::{
    marker::PhantomData,
    ops::ControlFlow,
};

use super::{
    role::{
        BorderChild,
        End,
        PathRole,
        offset_sum,
        token_end,
    },
    root::{
        PathRoot,
        QueryRoot,
    },
};
use crate::{
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::wide::Wide,
    positioned::{
        graph::PositionedGraph,
        location::{
            OccurrenceLocation,
            OccurrencePath,
        },
    },
};

/// Path of role `R` from `root` down to `leaf`.
///
/// `steps` is a connected chain of occurrences from
/// [`PathRoot::steps_root`] down to `leaf`, stored in the role's order
/// ([`PathRole::top_down`]). `root_child` is the root child the path passes
/// through (the child of the root step for a graph root, the current
/// pattern token for a [`QueryRoot`]); it is cached so that its width is
/// known without the graph.
///
/// - [`StartPath`]: bottom-up path to the token at the match start; built
///   from the climbed chain (canonical form 2.4: it stops at the largest
///   climbed token starting at the match start, see [`Self::raise`]).
/// - [`EndPath`]: top-down path to the next graph token to compare (it
///   starts at the current match end) or, after a match, the last matched
///   token.
/// - [`QueryPath`]: end path in a token-sequence query (Decision 6 b);
///   composite query tokens are split with [`Self::push_border_child`] and
///   advanced inside with [`Self::advance`], symmetric to the graph side.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RolePath<R: PathRole, Root: PathRoot = Token> {
    root: Root,
    root_child: Token,
    leaf: Token,
    steps: Vec<OccurrenceLocation>,
    _role: PhantomData<R>,
}

/// Path from a graph root to the token at the match start.
pub type StartPath = RolePath<super::role::Start>;
/// Path from a graph root to a leaf at the match end.
pub type EndPath = RolePath<End>;
/// Position in a token-sequence query: an end path rooted at the query.
pub type QueryPath = RolePath<End, QueryRoot>;

impl<R: PathRole, Root: PathRoot> RolePath<R, Root> {
    pub fn path_root(&self) -> &Root {
        &self.root
    }
    /// The root child the path passes through.
    pub fn root_child(&self) -> Token {
        self.root_child
    }
    pub fn leaf(&self) -> Token {
        self.leaf
    }
    /// Steps in storage order (bottom-up for start, top-down for end
    /// paths).
    pub fn steps(&self) -> &[OccurrenceLocation] {
        &self.steps
    }
    pub fn len(&self) -> usize {
        self.steps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
    pub fn top_down(&self) -> impl DoubleEndedIterator<Item = &OccurrenceLocation> + ExactSizeIterator {
        R::top_down(&self.steps)
    }
    pub fn bottom_up(&self) -> impl DoubleEndedIterator<Item = &OccurrenceLocation> + ExactSizeIterator {
        R::bottom_up(&self.steps)
    }
    /// Offset of the root child in root coordinates.
    pub fn root_child_offset(&self) -> AtomPosition {
        self.root.root_child_offset(R::root_step(&self.steps))
    }
    /// End of the root child in root coordinates.
    pub fn root_child_end(&self) -> AtomPosition {
        token_end(self.root_child_offset(), self.root_child.width())
    }
    /// Offset of the leaf in root coordinates.
    pub fn leaf_offset(&self) -> AtomPosition {
        self.root.steps_root_offset() + offset_sum(&self.steps).0
    }
    /// End of the leaf in root coordinates.
    pub fn leaf_end(&self) -> AtomPosition {
        token_end(self.leaf_offset(), self.leaf.width())
    }
    /// Total root width.
    pub fn width(&self) -> TokenWidth {
        self.root.root_width()
    }
    /// The leaf ends at the root end.
    pub fn at_end(&self) -> bool {
        self.leaf_end().0 == self.width().0
    }
    /// Descend into the border child of the leaf ([`BorderChild`]; the
    /// prefix child for end paths). `false` (path unchanged) for an atom
    /// leaf.
    pub fn push_border_child<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> bool
    where
        R: BorderChild,
    {
        let Some(child) = R::border_child(graph, self.leaf) else {
            return false;
        };
        R::push_leaf(&mut self.steps, OccurrenceLocation {
            parent: self.leaf.index,
            offset: child.offset,
            child: child.child.index,
        });
        self.leaf = child.child;
        self.root_child = self.root.root_child(graph, R::root_step(&self.steps), self.leaf);
        true
    }
    /// Positioned `simplify`: stop at the first step (top-down) whose child
    /// has its role border at `boundary` (for end paths the largest token
    /// on this path ending at the match end). Unchanged if no step does.
    /// Search results use the path-independent [`Self::locate`] instead
    /// (2.4).
    pub fn simplify<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
        boundary: impl Into<AtomPosition>,
    ) {
        let boundary = boundary.into();
        let mut offset = self.root.steps_root_offset();
        let found = self.top_down().enumerate().find_map(|(i, step)| {
            offset += step.offset.0;
            let child = graph.expect_token(step.child);
            R::at_border(offset, child.width(), boundary).then_some((i, child))
        });
        if let Some((i, child)) = found {
            R::keep_top(&mut self.steps, i + 1);
            self.leaf = child;
        }
    }
}

impl<R: PathRole> RolePath<R, Token> {
    /// One step: `leaf` at `offset` inside `root`.
    pub fn new(
        root: Token,
        offset: impl Into<AtomPosition>,
        leaf: Token,
    ) -> Self {
        Self {
            root,
            root_child: leaf,
            leaf,
            steps: vec![OccurrenceLocation {
                parent: root.index,
                offset: offset.into(),
                child: leaf.index,
            }],
            _role: PhantomData,
        }
    }
    /// `steps` must be a connected chain from `root` ending at `leaf`,
    /// top-down.
    fn from_top_down<G: GraphKind>(
        graph: &PositionedGraph<G>,
        root: Token,
        (leaf, steps): (Token, Vec<OccurrenceLocation>),
    ) -> Self {
        debug_assert!(steps.first().is_some_and(|s| s.parent == root.index));
        debug_assert!(steps.last().is_some_and(|s| s.child == leaf.index));
        debug_assert!(OccurrencePath::new(steps.clone()).is_some());
        let root_child = root.root_child(graph, steps.first(), leaf);
        Self {
            root,
            root_child,
            leaf,
            steps: R::from_top_down(steps),
            _role: PhantomData,
        }
    }
    /// [`PositionedGraph::start_at`] as a path of this role.
    pub fn start_at<G: GraphKind>(
        graph: &PositionedGraph<G>,
        root: Token,
        p: impl Into<AtomPosition>,
    ) -> Option<Self> {
        graph
            .start_at(root, p)
            .map(|found| Self::from_top_down(graph, root, found))
    }
    /// [`PositionedGraph::end_at`] as a path of this role: the descent to
    /// the largest token ending at `e`.
    pub fn end_at<G: GraphKind>(
        graph: &PositionedGraph<G>,
        root: Token,
        e: impl Into<AtomPosition>,
    ) -> Option<Self> {
        graph
            .end_at(root, e)
            .map(|found| Self::from_top_down(graph, root, found))
    }
    /// The canonical path of this role to boundary `p` ([`PathRole::locate`]):
    /// the descent to the largest token with its role border at `p`.
    pub fn locate<G: GraphKind>(
        graph: &PositionedGraph<G>,
        root: Token,
        p: impl Into<AtomPosition>,
    ) -> Option<Self> {
        R::locate(graph, root, p.into()).map(|found| Self::from_top_down(graph, root, found))
    }
    pub fn root(&self) -> Token {
        self.root
    }
    /// The occurrence of the root child in the root.
    pub fn root_child_location(&self) -> &OccurrenceLocation {
        R::root_step(&self.steps).expect("graph-rooted path has a step")
    }
    pub fn to_occurrence_path(&self) -> OccurrencePath {
        OccurrencePath::new(self.top_down().copied().collect())
            .expect("role path is a connected chain")
    }
    /// Raise to `parent`, where the root occurs at `offset`; `span` is the
    /// matched span in the old root.
    ///
    /// Canonical rule (2.4): if the span reaches the old root's border of
    /// this role ([`PathRole::spans_border`]; for start paths: the match
    /// starts at the old root's start), the old root is the largest climbed
    /// token at the boundary and the path becomes `[(parent, offset,
    /// root)]`; otherwise the step is added above the root.
    pub fn raise(
        &mut self,
        parent: Token,
        offset: impl Into<AtomPosition>,
        span: &std::ops::Range<AtomPosition>,
    ) {
        let step = OccurrenceLocation {
            parent: parent.index,
            offset: offset.into(),
            child: self.root.index,
        };
        if R::spans_border(span, Wide::width(&self.root)) {
            self.steps.clear();
            self.leaf = self.root;
        }
        R::push_root(&mut self.steps, step);
        self.root_child = self.root;
        self.root = parent;
    }
}

impl<Root: PathRoot> RolePath<End, Root> {
    /// Move to the largest token starting at `p` (root coordinates): inside
    /// the current root child via the corrected pop rule
    /// ([`PositionedGraph::advance_end_path`], equal to `start_at`), else at
    /// the next root child ([`PathRoot::next_root_child`]). `Break` (path
    /// unchanged) if there is none (`p ≥ width(root)` for a graph root, the
    /// query end for a query root).
    pub fn advance<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
        p: impl Into<AtomPosition>,
    ) -> ControlFlow<()> {
        let p = p.into();
        let token = self.root.steps_root();
        let base = self.root.steps_root_offset();
        if p.0 < token_end(base, token.width()).0 {
            let (leaf, steps) = graph
                .advance_end_path(
                    token,
                    std::mem::take(&mut self.steps),
                    AtomPosition(p.0 - base.0),
                )
                .expect("p is inside the root child");
            self.leaf = leaf;
            self.steps = steps;
        } else if let Some(next) = self.root.next_root_child() {
            debug_assert_eq!(self.root.steps_root_offset(), p);
            self.leaf = next;
            self.steps.clear();
        } else {
            return ControlFlow::Break(());
        }
        self.root_child = self.root.root_child(graph, self.steps.first(), self.leaf);
        ControlFlow::Continue(())
    }
    /// [`Self::advance`] to the leaf end: the largest token starting right
    /// after the leaf.
    pub fn advance_leaf<G: GraphKind>(
        &mut self,
        graph: &PositionedGraph<G>,
    ) -> ControlFlow<()> {
        self.advance(graph, self.leaf_end())
    }
}

impl QueryPath {
    /// At the first query token; `None` for an empty pattern.
    pub fn new(pattern: Vec<Token>) -> Option<Self> {
        let root = QueryRoot::new(pattern)?;
        let leaf = root.steps_root();
        Some(Self {
            root,
            root_child: leaf,
            leaf,
            steps: Vec::new(),
            _role: PhantomData,
        })
    }
    pub fn pattern(&self) -> &[Token] {
        self.root.pattern()
    }
    /// Index of the query token containing the leaf.
    pub fn entry(&self) -> usize {
        self.root.entry()
    }
}
