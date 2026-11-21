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

use super::graph::PositionedGraph;
use crate::{
    AtomPosition,
    DirectedKey,
    DownKey,
    DownPosition,
    GraphKind,
    HasAtomPosition,
    Token,
    UpKey,
    UpPosition,
};
use cache::{
    TraceCache,
    new::NewTraceEdge,
};
use traceable::{
    TraceCommand,
    Traceable,
};

#[derive(Debug)]
pub struct TraceCtx<'g, G: GraphKind> {
    pub graph: &'g PositionedGraph<G>,
    pub cache: TraceCache,
}

impl<'g, G: GraphKind> TraceCtx<'g, G> {
    pub fn new(
        graph: &'g PositionedGraph<G>,
        cache: TraceCache,
    ) -> Self {
        Self { graph, cache }
    }
    pub fn trace_command(
        &mut self,
        command: TraceCommand,
    ) {
        command.trace(self)
    }
}

pub trait TraceKey:
    HasAtomPosition + Debug + Clone + Copy + Into<DirectedKey>
{
}
impl<T: HasAtomPosition + Debug + Clone + Copy + Into<DirectedKey>> TraceKey for T {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottomUp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopDown;

/// Direction of trace edges (legacy `TraceDirection`), chosen by the path
/// role ([`PathRole::Direction`](super::path::PathRole::Direction)).
pub trait TraceDirection: Sized {
    type Pos: Copy;
    type Key: TraceKey;
    fn key(
        token: Token,
        pos: Self::Pos,
    ) -> Self::Key;
    /// The edge between `parent` and its `child` at `offset`.
    fn edge(
        parent: Self::Key,
        child: Self::Key,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self>;
    /// Reorder edges collected top-down into trace order.
    fn trace_order<T>(top_down: &mut [T]);
}
/// Edges from the child (`prev`) to the parent (`target`), traced from the
/// leaf upwards.
impl TraceDirection for BottomUp {
    type Pos = UpPosition;
    type Key = UpKey;
    fn key(
        token: Token,
        pos: UpPosition,
    ) -> UpKey {
        UpKey::new(token, pos)
    }
    fn edge(
        parent: UpKey,
        child: UpKey,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self> {
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
    type Key = DownKey;
    fn key(
        token: Token,
        pos: DownPosition,
    ) -> DownKey {
        DownKey::new(token, pos)
    }
    fn edge(
        parent: DownKey,
        child: DownKey,
        offset: AtomPosition,
    ) -> NewTraceEdge<Self> {
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
