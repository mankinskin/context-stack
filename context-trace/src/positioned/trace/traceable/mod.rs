//! Tracing paths into the cache (legacy `trace::traceable`).

pub mod role;

use crate::{
    AtomPosition,
    DownPosition,
    UpPosition,
    graph::vertex::{
        CommittedIndex,
        VertexId,
    },
    positioned::{
        path::{
            End,
            EndPath,
            HasRolePath,
            RangePath,
            Start,
            StartPath,
        },
        store::GraphRead,
        trace::{
            QueryAnchor,
            TraceCtx,
            child::ChildState,
        },
    },
};
use role::trace_role_path;

#[derive(Debug, Clone)]
pub enum TraceCommand<I: VertexId = CommittedIndex> {
    Postfix(PostfixCommand<I>),
    Prefix(PrefixCommand<I>),
    Range(RangeCommand<I>),
}

/// Anchor of a start path whose root entry child ends at query position
/// `entry_pos`.
fn start_anchor<I: VertexId>(
    path: &StartPath<I>,
    entry_pos: UpPosition,
) -> QueryAnchor {
    let start = path.leaf_offset();
    QueryAnchor {
        root: start,
        query: AtomPosition(entry_pos.0.0 - (path.root_child_end().0 - start.0)),
    }
}

/// Up edges along a start path (match ending at the root end).
#[derive(Debug, Clone)]
pub struct PostfixCommand<I: VertexId = CommittedIndex> {
    pub path: StartPath<I>,
    /// Query position at the end of the root entry child.
    pub entry_pos: UpPosition,
}

/// Down edges along an end path (match starting at the root start).
#[derive(Debug, Clone)]
pub struct PrefixCommand<I: VertexId = CommittedIndex> {
    pub path: EndPath<I>,
    /// Query position at the start of the root exit child.
    pub exit_pos: DownPosition,
}

/// Up edges along the start path and down edges along the end path.
#[derive(Debug, Clone)]
pub struct RangeCommand<I: VertexId = CommittedIndex> {
    pub path: RangePath<crate::Token<I>>,
    /// Query position at the end of the root entry child.
    pub entry_pos: UpPosition,
}

impl<I: VertexId> From<ChildState<I>> for RangeCommand<I> {
    fn from(state: ChildState<I>) -> Self {
        Self {
            path: state.path,
            entry_pos: state.entry_pos,
        }
    }
}

pub trait Traceable {
    /// Index type of the store the traced paths live in.
    type Index: VertexId;
    fn trace<G: GraphRead<Index = Self::Index> + ?Sized>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    );
}

impl<I: VertexId> Traceable for PostfixCommand<I> {
    type Index = I;
    fn trace<G: GraphRead<Index = I> + ?Sized>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = start_anchor(&self.path, self.entry_pos);
        trace_role_path(ctx, &self.path, anchor);
    }
}

impl<I: VertexId> Traceable for PrefixCommand<I> {
    type Index = I;
    fn trace<G: GraphRead<Index = I> + ?Sized>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = QueryAnchor {
            root: AtomPosition::default(),
            query: AtomPosition(self.exit_pos.0.0 - self.path.root_child_offset().0),
        };
        trace_role_path(ctx, &self.path, anchor);
    }
}

impl<I: VertexId> Traceable for RangeCommand<I> {
    type Index = I;
    fn trace<G: GraphRead<Index = I> + ?Sized>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = start_anchor(&self.path.start, self.entry_pos);
        debug_assert_eq!(anchor.root, self.path.span.start);
        trace_role_path(ctx, HasRolePath::<Start>::role_path(&self.path), anchor);
        trace_role_path(ctx, HasRolePath::<End>::role_path(&self.path), anchor);
    }
}

impl<I: VertexId> Traceable for TraceCommand<I> {
    type Index = I;
    fn trace<G: GraphRead<Index = I> + ?Sized>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        match self {
            Self::Postfix(cmd) => cmd.trace(ctx),
            Self::Prefix(cmd) => cmd.trace(ctx),
            Self::Range(cmd) => cmd.trace(ctx),
        }
    }
}
