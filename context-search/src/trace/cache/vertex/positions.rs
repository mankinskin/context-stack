//! Positions of one direction (legacy `cache::vertex::positions`).

use derive_more::derive::{
    Deref,
    DerefMut,
    IntoIterator,
};

use crate::trace::cache::position::PositionCache;
use context_trace::{
    AtomPosition,
    graph::{
        vertex::{
            CommittedIndex,
            VertexId,
        },
    },
    HashMap,
};

#[derive(Clone, Debug, PartialEq, Eq, IntoIterator, Deref, DerefMut)]
pub struct DirectedPositions<I: VertexId = CommittedIndex> {
    entries: HashMap<AtomPosition, PositionCache<I>>,
}

impl<I: VertexId> Default for DirectedPositions<I> {
    fn default() -> Self {
        Self {
            entries: HashMap::default(),
        }
    }
}

impl<I: VertexId> FromIterator<(AtomPosition, PositionCache<I>)> for DirectedPositions<I> {
    fn from_iter<T: IntoIterator<Item = (AtomPosition, PositionCache<I>)>>(iter: T) -> Self {
        Self {
            entries: FromIterator::from_iter(iter),
        }
    }
}

impl<I: VertexId> Extend<(AtomPosition, PositionCache<I>)> for DirectedPositions<I> {
    fn extend<T: IntoIterator<Item = (AtomPosition, PositionCache<I>)>>(
        &mut self,
        iter: T,
    ) {
        for (k, v) in iter {
            let entry = self.entries.entry(k).or_default();
            entry.top.extend(v.top);
            entry.bottom.extend(v.bottom);
        }
    }
}
