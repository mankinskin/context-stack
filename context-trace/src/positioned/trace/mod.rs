//! Trace states and trace cache of positioned search (search plan 2.2).
//!
//! Mirrors `crate::legacy::trace` (`state/parent.rs`, `child/`, `cache/`,
//! `traceable/`); only navigation and addressing differ: steps are
//! [`OccurrenceLocation`](super::OccurrenceLocation)s, spans are absolute
//! atom offsets in the root and cache edges are labelled by the child's
//! offset in its parent.
//!
//! **Query positions.** Cache keys and state positions are query positions
//! (Decision 4 a). A [`QueryAnchor`] maps root offsets to them: the match
//! start `span.start` in the root corresponds to the query position of the
//! first matched atom. Up keys use the position at the token end, down keys
//! the position at the token start, clamped to the match start (with
//! overlapping occurrences an end-path token may start before the match;
//! its true offset is on the edge label).

pub mod cache;
pub mod child;
pub mod state;
pub mod traceable;

use std::fmt::Debug;

use super::access::GraphRead;
use crate::{
    AtomPosition,
    DirectedKey,
    DownKey,
    DownPosition,
    HasAtomPosition,
    Token,
    UpKey,
    UpPosition,
    graph::vertex::VertexId,
};
use cache::{
    TraceCache,
    new::NewTraceEdge,
};
use traceable::{
    TraceCommand,
    Traceable,
};

/// The graph traced and the cache traced into (legacy `TraceCtx<G:
/// HasGraph>`; here borrowed from one guard for the whole search).
pub struct TraceCtx<'g, G: GraphRead + ?Sized> {
    pub graph: &'g G,
    pub cache: TraceCache<G::Index>,
}

impl<G: GraphRead + ?Sized> Debug for TraceCtx<'_, G> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("TraceCtx")
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

impl<'g, G: GraphRead + ?Sized> TraceCtx<'g, G> {
    pub fn new(
        graph: &'g G,
        cache: TraceCache<G::Index>,
    ) -> Self {
        Self { graph, cache }
    }
    pub fn trace_command(
        &mut self,
        command: TraceCommand<G::Index>,
    ) {
        command.trace(self)
    }
}

pub trait TraceKey<I: VertexId>:
    HasAtomPosition + Debug + Clone + Copy + Into<DirectedKey<I>>
{
}
impl<I: VertexId, T: HasAtomPosition + Debug + Clone + Copy + Into<DirectedKey<I>>> TraceKey<I>
    for T
{
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottomUp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopDown;

/// Direction of trace edges (legacy `TraceDirection`), chosen by the path
/// role ([`PathRole::Direction`](super::path::PathRole::Direction)).
pub trait TraceDirection: Sized {
    type Pos: Copy;
    type Key<I: VertexId>: TraceKey<I>;
    fn key<I: VertexId>(
        token: Token<I>,
        pos: Self::Pos,
    ) -> Self::Key<I>;
    /// The edge between `parent` and its `child` at `offset`.
    fn edge<I: VertexId>(
        parent: Self::Key<I>,
        child: Self::Key<I>,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self, I>;
    /// Reorder edges collected top-down into trace order.
    fn trace_order<T>(top_down: &mut [T]);
}
/// Edges from the child (`prev`) to the parent (`target`), traced from the
/// leaf upwards.
impl TraceDirection for BottomUp {
    type Pos = UpPosition;
    type Key<I: VertexId> = UpKey<I>;
    fn key<I: VertexId>(
        token: Token<I>,
        pos: UpPosition,
    ) -> UpKey<I> {
        UpKey::new(token, pos)
    }
    fn edge<I: VertexId>(
        parent: UpKey<I>,
        child: UpKey<I>,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self, I> {
        NewTraceEdge::new(child, parent, offset)
    }
    fn trace_order<T>(top_down: &mut [T]) {
        top_down.reverse();
    }
}
/// Edges from the parent (`prev`) to the child (`target`), traced from the
/// root downwards.
impl TraceDirection for TopDown {
    type Pos = DownPosition;
    type Key<I: VertexId> = DownKey<I>;
    fn key<I: VertexId>(
        token: Token<I>,
        pos: DownPosition,
    ) -> DownKey<I> {
        DownKey::new(token, pos)
    }
    fn edge<I: VertexId>(
        parent: DownKey<I>,
        child: DownKey<I>,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self, I> {
        NewTraceEdge::new(parent, child, offset)
    }
    fn trace_order<T>(_top_down: &mut [T]) {}
}

/// Correspondence between absolute offsets in a root and query positions:
/// `root` (the match start in the root) is the query position `query`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QueryAnchor {
    pub root: AtomPosition,
    pub query: AtomPosition,
}

impl QueryAnchor {
    /// Query position of the root offset `x ≥ root`.
    pub fn query_pos(
        &self,
        x: AtomPosition,
    ) -> AtomPosition {
        debug_assert!(x >= self.root);
        AtomPosition(self.query.0 + (x.0 - self.root.0))
    }
    /// Up position of a token ending at root offset `end`.
    pub fn up(
        &self,
        end: AtomPosition,
    ) -> UpPosition {
        UpPosition(self.query_pos(end))
    }
    /// Down position of a token starting at root offset `start`, clamped to
    /// the match start.
    pub fn down(
        &self,
        start: AtomPosition,
    ) -> DownPosition {
        DownPosition(self.query_pos(start.max(self.root)))
    }
}
