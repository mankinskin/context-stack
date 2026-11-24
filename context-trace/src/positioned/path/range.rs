//! Matched ranges (legacy `RootedRangePath`) and their coverage.

use std::ops::Range;

use super::{
    role::{
        End,
        PathRole,
        Start,
    },
    role_path::RolePath,
    root::PathRoot,
};
use crate::{
    AtomPosition,
    Token,
    TokenWidth,
    graph::vertex::VertexId,
    positioned::access::GraphRead,
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
    /// Classified by whether the span reaches the root border of each role.
    pub fn of(
        span: &Range<AtomPosition>,
        width: TokenWidth,
    ) -> Self {
        match (Start::spans_border(span, width), End::spans_border(span, width)) {
            (true, true) => Self::EntireRoot,
            (true, false) => Self::Prefix,
            (false, true) => Self::Postfix,
            (false, false) => Self::Range,
        }
    }
}

/// Role-indexed access to a path of role `R` (legacy `HasRolePath<R>`).
pub trait HasRolePath<R: PathRole> {
    type Root: PathRoot;
    fn role_path(&self) -> &RolePath<R, Self::Root>;
    fn role_path_mut(&mut self) -> &mut RolePath<R, Self::Root>;
}

impl<R: PathRole, Root: PathRoot> HasRolePath<R> for RolePath<R, Root> {
    type Root = Root;
    fn role_path(&self) -> &RolePath<R, Root> {
        self
    }
    fn role_path_mut(&mut self) -> &mut RolePath<R, Root> {
        self
    }
}

/// A matched range inside one root: `span` in absolute offsets of the
/// root, the start path to the token at `span.start` and the end path.
/// Start and end occurrences may overlap.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RangePath<Root: PathRoot = Token> {
    pub span: Range<AtomPosition>,
    pub start: RolePath<Start, Root>,
    pub end: RolePath<End, Root>,
}

impl<Root: PathRoot> HasRolePath<Start> for RangePath<Root> {
    type Root = Root;
    fn role_path(&self) -> &RolePath<Start, Root> {
        &self.start
    }
    fn role_path_mut(&mut self) -> &mut RolePath<Start, Root> {
        &mut self.start
    }
}

impl<Root: PathRoot> HasRolePath<End> for RangePath<Root> {
    type Root = Root;
    fn role_path(&self) -> &RolePath<End, Root> {
        &self.end
    }
    fn role_path_mut(&mut self) -> &mut RolePath<End, Root> {
        &mut self.end
    }
}

impl<Root: PathRoot> RangePath<Root> {
    pub fn new(
        span: Range<AtomPosition>,
        start: RolePath<Start, Root>,
        end: RolePath<End, Root>,
    ) -> Self {
        debug_assert_eq!(start.path_root(), end.path_root());
        debug_assert_eq!(start.leaf_offset(), span.start);
        Self { span, start, end }
    }
    pub fn path_root(&self) -> &Root {
        self.start.path_root()
    }
    pub fn coverage(&self) -> SpanCoverage {
        SpanCoverage::of(&self.span, self.path_root().root_width())
    }
    /// The span boundary of role `R`.
    pub fn boundary<R: PathRole>(&self) -> AtomPosition {
        R::boundary(&self.span)
    }
    /// Extend the span to the end of the end leaf (the leaf matched).
    pub fn match_end_leaf(&mut self) {
        self.span.end = self.end.leaf_end();
    }
}

impl<I: VertexId> RangePath<Token<I>> {
    pub fn root(&self) -> Token<I> {
        self.start.root()
    }
    /// Canonical path of role `R` (2.4): the descent to the largest token
    /// with its role border at the span boundary ([`RolePath::locate`]). A
    /// function of root and span only, so it does not depend on how the
    /// query was split into leaves (atoms or composite tokens). Unchanged if
    /// there is no such token (e.g. an empty span).
    ///
    /// Search results canonicalise the end path only; the start path keeps
    /// the climbed chain, trimmed by the raise rule ([`RolePath::raise`]).
    pub fn canonicalize<R: PathRole, G: GraphRead<Index = I> + ?Sized>(
        &mut self,
        graph: &G,
    ) where
        Self: HasRolePath<R, Root = Token<I>>,
    {
        let path = RolePath::<R, Token<I>>::locate(graph, self.root(), self.boundary::<R>());
        if let Some(path) = path {
            *<Self as HasRolePath<R>>::role_path_mut(self) = path;
        }
    }
}
