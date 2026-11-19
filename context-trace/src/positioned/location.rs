//! Addressing occurrences and intervals inside positioned vertices.

use std::ops::Range;

use crate::{
    AtomPosition,
    graph::vertex::VertexIndex,
};

/// One retained occurrence: `child` at `offset` inside `parent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OccurrenceLocation {
    pub parent: VertexIndex,
    pub offset: AtomPosition,
    pub child: VertexIndex,
}

/// Chain of occurrences descending from `root()`; each step's child is the
/// next step's parent. The absolute offset in the root is the sum of offsets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OccurrencePath(Vec<OccurrenceLocation>);

impl OccurrencePath {
    /// `None` if `steps` is empty or not a connected chain.
    pub fn new(steps: Vec<OccurrenceLocation>) -> Option<Self> {
        let connected = steps.windows(2).all(|w| w[0].child == w[1].parent);
        (!steps.is_empty() && connected).then_some(Self(steps))
    }
    pub fn steps(&self) -> &[OccurrenceLocation] {
        &self.0
    }
    pub fn root(&self) -> VertexIndex {
        self.0[0].parent
    }
    pub fn target(&self) -> VertexIndex {
        self.0[self.0.len() - 1].child
    }
    /// Offset of [`Self::target`] in [`Self::root`].
    pub fn offset(&self) -> AtomPosition {
        self.0
            .iter()
            .fold(AtomPosition::default(), |sum, step| sum + step.offset.0)
    }
}

/// Atom range of an owner vertex. A view, explicitly not a token: it has no
/// vertex identity and is never stored in the graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalView {
    pub owner: VertexIndex,
    pub range: Range<AtomPosition>,
}
