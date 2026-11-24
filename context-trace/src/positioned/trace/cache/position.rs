//! Per-position cache entry (legacy `cache::position`).

use crate::{
    AtomPosition,
    DirectedKey,
    HashMap,
    HashSet,
    graph::vertex::{
        CommittedIndex,
        VertexId,
    },
};

/// Child key → child offset in this vertex.
pub type Bottom<I = CommittedIndex> = HashMap<DirectedKey<I>, AtomPosition>;
pub type Top<I = CommittedIndex> = HashSet<DirectedKey<I>>;

/// Edges of one `(vertex, directed position)`: `bottom` holds the traced
/// children below it, each labelled by its offset in this vertex; `top` is
/// left to consumers (as in legacy, tracing records edges at the parent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionCache<I: VertexId = CommittedIndex> {
    pub top: Top<I>,
    pub bottom: Bottom<I>,
}

impl<I: VertexId> Default for PositionCache<I> {
    fn default() -> Self {
        Self {
            top: Default::default(),
            bottom: Default::default(),
        }
    }
}

impl<I: VertexId> PositionCache<I> {
    pub fn new(
        top: Top<I>,
        bottom: Bottom<I>,
    ) -> Self {
        Self { top, bottom }
    }
    pub fn with_top(top: Top<I>) -> Self {
        Self {
            top,
            bottom: Default::default(),
        }
    }
    pub fn with_bottom(bottom: Bottom<I>) -> Self {
        Self {
            top: Default::default(),
            bottom,
        }
    }
    pub fn bottom(&self) -> &Bottom<I> {
        &self.bottom
    }
    pub fn top(&self) -> &Top<I> {
        &self.top
    }
    pub fn num_parents(&self) -> usize {
        self.top.len()
    }
    pub fn num_bu_edges(&self) -> usize {
        self.bottom.len()
    }
}
