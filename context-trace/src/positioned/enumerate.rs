//! Occurrence enumeration: every retained occurrence wholly inside an
//! interval, found by descending through intersecting children.

use std::collections::BTreeSet;

use super::{
    IntervalView,
    Occurrence,
    PositionedGraph,
};
use crate::{
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
};

impl<G: GraphKind> PositionedGraph<G> {
    /// Every retained occurrence wholly inside `view`, with offsets relative
    /// to `view.owner`, sorted by `(offset, width)` and deduplicated by
    /// `(token, offset)`. The owner itself is not included.
    ///
    /// Descends into every child intersecting the range, including children
    /// that cross its boundary, so an occurrence nested in a child cut by the
    /// range is still found. A maximal cover loses no retained occurrence,
    /// so the result equals all occurrences of retained tokens in the
    /// interval's content. `None` if the owner is unknown or the range
    /// exceeds it.
    pub fn occurrences_in(
        &self,
        view: &IntervalView,
    ) -> Option<Vec<Occurrence>> {
        let owner = self.view_owner(view)?;
        let range = view.range.start.0..view.range.end.0;
        // `(offset, width, token)` keeps the result in `(offset, width)` order.
        let mut found = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut stack = vec![(owner.index, 0)];
        while let Some((index, base)) = stack.pop() {
            let Some(composition) = self.vertices[index.0].composition() else {
                continue;
            };
            for occurrence in composition.occurrences() {
                let start = base + occurrence.offset.0;
                let end = start + occurrence.child.width.0;
                if end <= range.start || start >= range.end {
                    continue;
                }
                let child = occurrence.child;
                if range.start <= start && end <= range.end {
                    found.insert((start, child.width, child));
                }
                // The subtree below `child@start` is the same on every path.
                if visited.insert((child.index, start)) {
                    stack.push((child.index, start));
                }
            }
        }
        Some(
            found
                .into_iter()
                .map(|(offset, _, child): (usize, TokenWidth, Token)| {
                    Occurrence::new(AtomPosition::from(offset), child)
                })
                .collect(),
        )
    }
}
