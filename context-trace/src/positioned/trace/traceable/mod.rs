//! Tracing paths into the cache (legacy `trace::traceable`).

pub mod role;
pub mod root;

use crate::{
    AtomPosition,
    DownPosition,
    GraphKind,
    UpPosition,
    positioned::{
        path::{
            EndPath,
            RangePath,
            StartPath,
        },
        trace::{
            QueryAnchor,
            TraceCtx,
            child::state::ChildState,
        },
    },
};
use role::{
    trace_end_sub_path,
    trace_start_sub_path,
};
use root::{
    trace_root_entry,
    trace_root_exit,
};

#[derive(Debug, Clone)]
pub enum TraceCommand {
    Postfix(PostfixCommand),
    Prefix(PrefixCommand),
    Range(RangeCommand),
}

/// Anchor of a start path whose root entry child ends at query position
/// `entry_pos`.
fn start_anchor(
    path: &StartPath,
    entry_pos: UpPosition,
) -> QueryAnchor {
    let start = path.start_offset();
    QueryAnchor {
        root: start,
        query: AtomPosition(entry_pos.0.0 - (path.entry_end().0 - start.0)),
    }
}

/// Up edges along a start path (match ending at the root end).
#[derive(Debug, Clone)]
pub struct PostfixCommand {
    pub path: StartPath,
    /// Query position at the end of the root entry child.
    pub entry_pos: UpPosition,
}

/// Down edges along an end path (match starting at the root start).
#[derive(Debug, Clone)]
pub struct PrefixCommand {
    pub path: EndPath,
    /// Query position at the start of the root exit child.
    pub exit_pos: DownPosition,
}

/// Up edges along the start path and down edges along the end path.
#[derive(Debug, Clone)]
pub struct RangeCommand {
    pub path: RangePath,
    /// Query position at the end of the root entry child.
    pub entry_pos: UpPosition,
}

impl From<ChildState> for RangeCommand {
    fn from(state: ChildState) -> Self {
        Self {
            path: state.path,
            entry_pos: state.entry_pos,
        }
    }
}

pub trait Traceable {
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    );
}

impl Traceable for PostfixCommand {
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = start_anchor(&self.path, self.entry_pos);
        trace_start_sub_path(ctx, &self.path, anchor);
        trace_root_entry(ctx, &self.path, anchor);
    }
}

impl Traceable for PrefixCommand {
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = QueryAnchor {
            root: AtomPosition::default(),
            query: AtomPosition(self.exit_pos.0.0 - self.path.root_exit().offset.0),
        };
        let exit = trace_root_exit(ctx, &self.path, anchor);
        trace_end_sub_path(ctx, &self.path, anchor, exit);
    }
}

impl Traceable for RangeCommand {
    fn trace<G: GraphKind>(
        self,
        ctx: &mut TraceCtx<'_, G>,
    ) {
        let anchor = start_anchor(&self.path.start, self.entry_pos);
        debug_assert_eq!(anchor.root, self.path.span.start);
        trace_start_sub_path(ctx, &self.path.start, anchor);
        trace_root_entry(ctx, &self.path.start, anchor);
        let exit = trace_root_exit(ctx, &self.path.end, anchor);
        trace_end_sub_path(ctx, &self.path.end, anchor, exit);
    }
}

impl Traceable for TraceCommand {
    fn trace<G: GraphKind>(
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
