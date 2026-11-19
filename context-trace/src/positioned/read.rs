//! Content reading: the atoms of an interval, each position exactly once.

use std::ops::Range;

use super::{
    IntervalView,
    PositionedGraph,
    PositionedVertex,
    VertexNode,
};
use crate::{
    AtomOf,
    GraphKind,
};

impl<G: GraphKind> PositionedGraph<G> {
    /// Owner of `view` if `view.range` lies within it.
    pub(crate) fn view_owner(
        &self,
        view: &IntervalView,
    ) -> Option<&PositionedVertex<G>> {
        let owner = self.vertex(view.owner)?;
        (view.range.start <= view.range.end && view.range.end.0 <= owner.width.0)
            .then_some(owner)
    }

    /// Atoms of `view`, in order, each position read exactly once.
    ///
    /// Descends through the composition and may cut through a child; no
    /// token is created for the cut. `None` if the owner is unknown or the
    /// range exceeds it.
    pub fn read_interval(
        &self,
        view: &IntervalView,
    ) -> Option<Vec<AtomOf<G>>> {
        let owner = self.view_owner(view)?;
        let range = view.range.start.0..view.range.end.0;
        let mut atoms = Vec::with_capacity(range.len());
        self.read_into(owner, range, &mut atoms);
        Some(atoms)
    }

    /// Append the atoms of `range` (relative to `vertex`) to `atoms`.
    fn read_into(
        &self,
        vertex: &PositionedVertex<G>,
        range: Range<usize>,
        atoms: &mut Vec<AtomOf<G>>,
    ) {
        if range.is_empty() {
            return;
        }
        match &vertex.node {
            VertexNode::Atom(atom) => atoms.push(*atom),
            VertexNode::Composite(composition) => {
                // Occurrences are sorted by start without containment, so
                // their ends increase too: reading each one from the first
                // unread position emits every position once.
                let mut next = range.start;
                for occurrence in composition.occurrences() {
                    let (start, end) = (occurrence.offset.0, occurrence.end().0);
                    if next >= range.end {
                        break;
                    }
                    if end <= next {
                        continue;
                    }
                    let to = end.min(range.end);
                    let child = &self.vertices[occurrence.child.index.0];
                    self.read_into(child, next - start..to - start, atoms);
                    next = to;
                }
            },
        }
    }
}
