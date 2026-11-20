//! Root edges (legacy `traceable::root`): the edge from the root to its
//! entry child (bottom-up) or exit child (top-down).

use crate::{
    DownKey,
    GraphKind,
    UpKey,
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

/// Edge `root ← entry`; returns the root key (`entry_pos`).
pub(crate) fn trace_root_entry<G: GraphKind>(
    ctx: &mut TraceCtx<'_, G>,
    path: &StartPath,
    anchor: QueryAnchor,
) -> UpKey {
    let entry = path.root_entry();
    let pos = anchor.up(path.entry_end());
    let target = UpKey::new(path.root(), pos);
    let prev = UpKey::new(path.entry(), pos);
    ctx.cache
        .add_state(NewTraceEdge::<BottomUp>::new(prev, target, entry.offset));
    target
}

/// Edge `root → exit`; returns the exit child key.
pub(crate) fn trace_root_exit<G: GraphKind>(
    ctx: &mut TraceCtx<'_, G>,
    path: &EndPath,
    anchor: QueryAnchor,
) -> DownKey {
    let exit = path.root_exit();
    let pos = anchor.down(exit.offset);
    let prev = DownKey::new(path.root(), pos);
    let target = DownKey::new(ctx.graph.expect_token(exit.child), pos);
    ctx.cache
        .add_state(NewTraceEdge::<TopDown>::new(prev, target, exit.offset));
    target
}
