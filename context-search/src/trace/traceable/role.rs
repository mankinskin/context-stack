//! Role paths into the cache (legacy `traceable::role` and
//! `traceable::root`): one walk for both roles. Start paths emit up edges,
//! end paths down edges ([`PathRole::Direction`]); the role supplies the
//! key positions ([`PathRole::trace_pos`]).

use crate::{
    path::{
        PathRole,
        RolePath,
    },
    trace::{
        cache::{
            EditKind,
            NewTraceEdge,
        },
        QueryAnchor,
        TraceCtx,
        TraceDirection,
    },
};
use context_trace::{
    AtomPosition,
    graph::vertex::wide::Wide,
    positioned::{
        check::expect_token,
        GraphRead,
    },
    Token,
};

/// Edges along `path` including the root edge, in trace order (bottom-up
/// for start paths, top-down for end paths).
///
/// Every token's key position is the role position of the token
/// ([`PathRole::trace_pos`]: up keys at the token end, down keys at the
/// token start clamped to the match start). The root key takes the
/// position of the root child's key. Edges are labelled by the child
/// offset in its parent.
pub(crate) fn trace_role_path<R: PathRole, G: GraphRead + ?Sized>(
    ctx: &mut TraceCtx<'_, G>,
    path: &RolePath<R, Token<G::Index>>,
    anchor: QueryAnchor,
) where
    NewTraceEdge<R::Direction, G::Index>: Into<EditKind<G::Index>>,
{
    let mut edges = Vec::with_capacity(path.len());
    let mut parent = (path.root(), None);
    let mut start = AtomPosition::default();
    for step in path.top_down() {
        let child = expect_token(ctx.graph, step.child);
        start += step.offset.0;
        let pos = R::trace_pos(&anchor, start, child.width());
        let (parent_token, parent_pos) = parent;
        edges.push(R::Direction::edge(
            R::Direction::key(parent_token, parent_pos.unwrap_or(pos)),
            R::Direction::key(child, pos),
            step.offset,
        ));
        parent = (child, Some(pos));
    }
    R::Direction::trace_order(&mut edges);
    for edge in edges {
        ctx.cache.add_state(edge);
    }
}
