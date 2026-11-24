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
    Token,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        wide::Wide,
    },
    positioned::{
        model::OccurrenceLocation,
        store::{
            GraphRead,
            Located,
            Navigate,
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
    root_child: Token<Root::Index>,
    leaf: Token<Root::Index>,
    steps: Vec<OccurrenceLocation<Root::Index>>,
    _role: PhantomData<R>,
}

/// Path from a graph root to the token at the match start.
pub type StartPath<I = CommittedIndex> = RolePath<super::role::Start, Token<I>>;
/// Path from a graph root to a leaf at the match end.
pub type EndPath<I = CommittedIndex> = RolePath<End, Token<I>>;
/// Position in a token-sequence query: an end path rooted at the query.
pub type QueryPath<I = CommittedIndex> = RolePath<End, QueryRoot<I>>;

impl<R: PathRole, Root: PathRoot> RolePath<R, Root> {
    pub fn path_root(&self) -> &Root {
        &self.root
    }
    /// The root child the path passes through.
    pub fn root_child(&self) -> Token<Root::Index> {
        self.root_child
    }
    pub fn leaf(&self) -> Token<Root::Index> {
        self.leaf
    }
    /// Steps in storage order (bottom-up for start, top-down for end
    /// paths).
    pub fn steps(&self) -> &[OccurrenceLocation<Root::Index>] {
        &self.steps
    }
    pub fn len(&self) -> usize {
        self.steps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
    pub fn top_down(
        &self
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<Root::Index>> + ExactSizeIterator {
        R::top_down(&self.steps)
    }
    pub fn bottom_up(
        &self
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<Root::Index>> + ExactSizeIterator {
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
    pub fn push_border_child<G: GraphRead<Index = Root::Index> + ?Sized>(
        &mut self,
        graph: &G,
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
}

impl<R: PathRole, I: VertexId> RolePath<R, Token<I>> {
    /// One step: `leaf` at `offset` inside `root`.
    pub fn new(
        root: Token<I>,
        offset: impl Into<AtomPosition>,
        leaf: Token<I>,
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
    fn from_top_down<G: GraphRead<Index = I> + ?Sized>(
        graph: &G,
        root: Token<I>,
        (leaf, steps): Located<I>,
    ) -> Self {
        debug_assert!(steps.first().is_some_and(|s| s.parent == root.index));
        debug_assert!(steps.last().is_some_and(|s| s.child == leaf.index));
        debug_assert!(steps.windows(2).all(|w| w[0].child == w[1].parent));
        let root_child = root.root_child(graph, steps.first(), leaf);
        Self {
            root,
            root_child,
            leaf,
            steps: R::from_top_down(steps),
            _role: PhantomData,
        }
    }
    /// [`Navigate::start_at`] as a path of this role.
    pub fn start_at<G: GraphRead<Index = I> + ?Sized>(
        graph: &G,
        root: Token<I>,
        p: impl Into<AtomPosition>,
    ) -> Option<Self> {
        graph
            .vertex(root)?
            .start_at(p)
            .map(|found| Self::from_top_down(graph, root, found))
    }
    /// [`Navigate::end_at`] as a path of this role: the descent to
    /// the largest token ending at `e`.
    pub fn end_at<G: GraphRead<Index = I> + ?Sized>(
        graph: &G,
        root: Token<I>,
        e: impl Into<AtomPosition>,
    ) -> Option<Self> {
        graph
            .vertex(root)?
            .end_at(e)
            .map(|found| Self::from_top_down(graph, root, found))
    }
    /// The canonical path of this role to boundary `p` ([`PathRole::locate`]):
    /// the descent to the largest token with its role border at `p`.
    pub fn locate<G: GraphRead<Index = I> + ?Sized>(
        graph: &G,
        root: Token<I>,
        p: impl Into<AtomPosition>,
    ) -> Option<Self> {
        R::locate(graph, root, p.into()).map(|found| Self::from_top_down(graph, root, found))
    }
    pub fn root(&self) -> Token<I> {
        self.root
    }
    /// The occurrence of the root child in the root.
    pub fn root_child_location(&self) -> &OccurrenceLocation<I> {
        R::root_step(&self.steps).expect("graph-rooted path has a step")
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
        parent: Token<I>,
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
    /// ([`Navigate::advance_end_path`], equal to `start_at`), else at
    /// the next root child ([`PathRoot::next_root_child`]). `Break` (path
    /// unchanged) if there is none (`p ≥ width(root)` for a graph root, the
    /// query end for a query root).
    pub fn advance<G: GraphRead<Index = Root::Index> + ?Sized>(
        &mut self,
        graph: &G,
        p: impl Into<AtomPosition>,
    ) -> ControlFlow<()> {
        let p = p.into();
        let token = self.root.steps_root();
        let base = self.root.steps_root_offset();
        if p.0 < token_end(base, token.width()).0 {
            let (leaf, steps) = graph
                .vertex(token)
                .and_then(|root| {
                    root.advance_end_path(
                        std::mem::take(&mut self.steps),
                        AtomPosition(p.0 - base.0),
                    )
                })
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
    pub fn advance_leaf<G: GraphRead<Index = Root::Index> + ?Sized>(
        &mut self,
        graph: &G,
    ) -> ControlFlow<()> {
        self.advance(graph, self.leaf_end())
    }
}

impl<I: VertexId> QueryPath<I> {
    /// At the first query token; `None` for an empty pattern.
    pub fn new(pattern: Vec<Token<I>>) -> Option<Self> {
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
    pub fn pattern(&self) -> &[Token<I>] {
        self.root.pattern()
    }
    /// Index of the query token containing the leaf.
    pub fn entry(&self) -> usize {
        self.root.entry()
    }
}
