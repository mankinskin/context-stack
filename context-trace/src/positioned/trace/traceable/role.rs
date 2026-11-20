//! Sub paths below the root (legacy `traceable::role`): up edges along the
//! start path, down edges along the end path.

use crate::{
    AtomPosition,
    DownKey,
    GraphKind,
    UpKey,
    graph::vertex::wide::Wide,
    positioned::{
        path::{
            EndPath,
            StartPath,
        },
        trace::{
            BottomUp,
            QueryAnchor,
            TopDown,
            TraceCtx,
            cache::new::NewTraceEdge,
        },
    },
};

/// Up edges below the root entry, from the start token upwards. Every
/// key's position is the query position at its token's end.
pub(crate) fn trace_start_sub_path<G: GraphKind>(
    ctx: &mut TraceCtx<'_, G>,
    path: &StartPath,
    anchor: QueryAnchor,
) {
    // (parent key, child key, offset), top-down.
    let mut edges = Vec::new();
    let mut parent_start = path.root_entry().offset;
    let mut parent = UpKey::new(path.entry(), anchor.up(path.entry_end()));
    for step in path.top_down().skip(1) {
        let child = ctx.graph.expect_token(step.child);
        let start = parent_start + step.offset.0;
        let key = UpKey::new(child, anchor.up(start + child.width().0));
        edges.push((parent, key, step.offset));
        parent = key;
        parent_start = start;
    }
    for (target, prev, offset) in edges.into_iter().rev() {
        ctx.cache.add_state(NewTraceEdge::<BottomUp>::new(prev, target, offset));
    }
}

/// Down edges below the root exit. Every key's position is the query
/// position at its token's start (clamped to the match start).
pub(crate) fn trace_end_sub_path<G: GraphKind>(
    ctx: &mut TraceCtx<'_, G>,
    path: &EndPath,
    anchor: QueryAnchor,
    exit: DownKey,
) -> DownKey {
    let mut parent_start: AtomPosition = path.root_exit().offset;
    let mut prev = exit;
    for step in &path.steps()[1..] {
        let child = ctx.graph.expect_token(step.child);
        let start = parent_start + step.offset.0;
        let target = DownKey::new(child, anchor.down(start));
        ctx.cache.add_state(NewTraceEdge::<TopDown>::new(prev, target, step.offset));
        prev = target;
        parent_start = start;
    }
    prev
}
