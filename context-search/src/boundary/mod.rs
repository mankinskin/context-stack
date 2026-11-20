//! Boundary search over the positioned model: retained occurrences that
//! start or end at a position of an owner, found transitively through its
//! descendants, each with an [`OccurrencePath`] as evidence.
//!
//! Prefixes and postfixes are the boundary searches at `0` and at the owner's
//! width. Independent of `legacy`.

use std::collections::BTreeSet;

use context_trace::{
    AtomPosition,
    GraphKind,
    Token,
    VertexIndex,
    positioned::{
        OccurrenceLocation,
        OccurrencePath,
        PositionedGraph,
    },
};

/// A retained occurrence found by a boundary search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundaryOccurrence {
    pub token: Token,
    /// Descent from the searched owner to `token`; its offset is the
    /// occurrence's offset in the owner.
    pub path: OccurrencePath,
}

impl BoundaryOccurrence {
    /// Offset of the occurrence in the searched owner.
    pub fn offset(&self) -> AtomPosition {
        self.path.offset()
    }
    /// Exclusive end of the occurrence in the searched owner.
    pub fn end(&self) -> AtomPosition {
        self.offset() + self.token.width.0
    }
}

/// Which end of an occurrence must touch the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// Occurrences starting at the position.
    Start,
    /// Occurrences ending at the position.
    End,
}

/// Retained proper descendants of `owner` that start (or end) at `position`,
/// longest first, one per token with one evidence path each.
///
/// `None` if `owner` is unknown or `position` exceeds its width.
pub fn boundary_occurrences<G: GraphKind>(
    graph: &PositionedGraph<G>,
    owner: VertexIndex,
    position: AtomPosition,
    boundary: Boundary,
) -> Option<Vec<BoundaryOccurrence>> {
    let width = graph.vertex(owner)?.width().0;
    let position = usize::from(position);
    if position > width {
        return None;
    }
    let mut found: Vec<BoundaryOccurrence> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = vec![(owner, 0, Vec::<OccurrenceLocation>::new())];
    while let Some((vertex, base, path)) = stack.pop() {
        let Some(composition) = graph.vertex(vertex).and_then(|v| v.composition())
        else {
            continue;
        };
        for occurrence in composition.occurrences() {
            let start = base + usize::from(occurrence.offset);
            let end = start + occurrence.child.width.0;
            // Only children containing the boundary side can contain a match.
            let (relevant, matches) = match boundary {
                Boundary::Start =>
                    (start <= position && position < end, start == position),
                Boundary::End => (start < position && position <= end, end == position),
            };
            // The subtree below `child@start` is the same on every path.
            if !relevant || !visited.insert((occurrence.child.index, start)) {
                continue;
            }
            let mut steps = path.clone();
            steps.push(OccurrenceLocation {
                parent: vertex,
                offset: occurrence.offset,
                child: occurrence.child.index,
            });
            if matches && seen.insert(occurrence.child) {
                found.push(BoundaryOccurrence {
                    token: occurrence.child,
                    path: OccurrencePath::new(steps.clone())
                        .expect("descent steps form a chain"),
                });
            }
            stack.push((occurrence.child.index, start, steps));
        }
    }
    found.sort_by(|a, b| {
        b.token
            .width
            .cmp(&a.token.width)
            .then(a.token.index.cmp(&b.token.index))
    });
    Some(found)
}

/// Retained proper prefixes of `owner`, longest first.
pub fn prefixes<G: GraphKind>(
    graph: &PositionedGraph<G>,
    owner: VertexIndex,
) -> Option<Vec<BoundaryOccurrence>> {
    boundary_occurrences(graph, owner, AtomPosition::default(), Boundary::Start)
}

/// Retained proper postfixes of `owner`, longest first.
pub fn postfixes<G: GraphKind>(
    graph: &PositionedGraph<G>,
    owner: VertexIndex,
) -> Option<Vec<BoundaryOccurrence>> {
    let width = graph.vertex(owner)?.width();
    boundary_occurrences(graph, owner, width.into(), Boundary::End)
}
