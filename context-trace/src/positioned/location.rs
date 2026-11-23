//! Addressing occurrences and intervals inside positioned vertices.

#[cfg(any(debug_assertions, test, feature = "test-api"))]
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

/// Atom range of an owner vertex. A view, explicitly not a token: it has no
/// vertex identity and is never stored in the graph.
#[cfg(any(debug_assertions, test, feature = "test-api"))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntervalView {
    pub owner: VertexIndex,
    pub range: Range<AtomPosition>,
}
