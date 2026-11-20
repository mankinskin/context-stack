//! Per-position cache entry (legacy `cache::position`).

use crate::{
    AtomPosition,
    DirectedKey,
    HashMap,
    HashSet,
};

/// Child key → child offset in this vertex.
pub type Bottom = HashMap<DirectedKey, AtomPosition>;
pub type Top = HashSet<DirectedKey>;

/// Edges of one `(vertex, directed position)`: `bottom` holds the traced
/// children below it, each labelled by its offset in this vertex; `top` is
/// left to consumers (as in legacy, tracing records edges at the parent).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PositionCache {
    pub top: Top,
    pub bottom: Bottom,
}

impl PositionCache {
    pub fn new(
        top: Top,
        bottom: Bottom,
    ) -> Self {
        Self { top, bottom }
    }
    pub fn with_top(top: Top) -> Self {
        Self {
            top,
            bottom: Default::default(),
        }
    }
    pub fn with_bottom(bottom: Bottom) -> Self {
        Self {
            top: Default::default(),
            bottom,
        }
    }
    pub fn bottom(&self) -> &Bottom {
        &self.bottom
    }
    pub fn top(&self) -> &Top {
        &self.top
    }
    pub fn num_parents(&self) -> usize {
        self.top.len()
    }
    pub fn num_bu_edges(&self) -> usize {
        self.bottom.len()
    }
}
