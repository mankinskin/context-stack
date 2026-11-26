//! Position-based local navigation on vertex objects (search plan 2.1,
//! staging plan §3.4).
//!
//! Replaces the row model's "next sibling" and "prefix children": a token
//! starting at a position is found by [`Navigate::start_at`], retained
//! prefixes by the [`Navigate::prefix_child`] chain ([`Navigate::prefixes`]),
//! postfixes by the [`Navigate::postfix_child`] chain
//! ([`Navigate::postfixes`]). The search-only navigation of end paths
//! (`end_at`, `advance_end_path`, search plan 2.4) is
//! `context_search::path::PathNavigate`. No content is read; only
//! compositions and parent links are used, so the bodies are written once
//! for every store.

use std::fmt::Display;
#[cfg(any(debug_assertions, test, feature = "test-api"))]
use std::ops::Range;

use super::{
    CompositionRead,
    VertexRead,
};
#[cfg(any(debug_assertions, test, feature = "test-api"))]
use crate::positioned::model::IntervalView;
use crate::{
    AtomOf,
    AtomPosition,
    Token,
    positioned::model::{
        Occurrence,
        OccurrenceLocation,
    },
};

/// A path found by navigation: the token reached and the occurrence steps
/// from the start vertex down to it.
pub type Located<I> = (Token<I>, Vec<OccurrenceLocation<I>>);

/// Navigation on a vertex object (blanket over [`VertexRead`]).
pub trait Navigate<'a>: VertexRead<'a> {
    /// The largest retained proper descendant starting at `p`, and the
    /// occurrence steps from `self` down to it (offsets relative to each
    /// parent, summing to `p`).
    ///
    /// Descends through the covering occurrence (last with `offset ≤ p`)
    /// until one starts exactly at `p`. `None` iff `p ≥ width`, or `self`
    /// is an atom (it has no proper descendants).
    fn start_at(
        self,
        p: impl Into<AtomPosition>,
    ) -> Option<Located<Self::Index>> {
        descend(self, p.into(), Vec::new())
    }

    /// The unique occurrence at offset 0; `None` for atoms (end of the
    /// chain). Every retained proper prefix lies on the chain
    /// ([`Self::prefixes`]).
    fn prefix_child(self) -> Option<Occurrence<Self::Index>> {
        Some(self.node().composite()?.prefix())
    }

    /// The unique occurrence ending at the width; `None` for atoms (end of
    /// the chain). Every retained proper postfix lies on the chain
    /// ([`Self::postfixes`]).
    fn postfix_child(self) -> Option<Occurrence<Self::Index>> {
        Some(self.node().composite()?.postfix())
    }

    /// Prefix chain largest first, down to an atom; `self` is not included.
    /// Each item is the occurrence location in its predecessor on the chain
    /// and the prefix token (legacy `prefix_iter`).
    fn prefixes(
        self
    ) -> impl Iterator<Item = (OccurrenceLocation<Self::Index>, Token<Self::Index>)>
    {
        chain(self, Self::prefix_child)
    }

    /// Postfix chain largest first, down to an atom; `self` is not included.
    /// Every postfix token lies on this chain (reader plan §3.2). Items as
    /// in [`Self::prefixes`] (legacy `postfix_iter`).
    fn postfixes(
        self
    ) -> impl Iterator<Item = (OccurrenceLocation<Self::Index>, Token<Self::Index>)>
    {
        chain(self, Self::postfix_child)
    }

    /// Every occurrence in a parent as `(parent, offset)`, ordered by
    /// parent width, parent index, offset.
    fn parent_occurrences(
        self
    ) -> impl Iterator<Item = (Token<Self::Index>, AtomPosition)> {
        let mut occurrences: Vec<_> = self.parents().collect();
        occurrences.sort_by_key(|(parent, offset)| {
            (parent.width, parent.index, *offset)
        });
        occurrences.into_iter()
    }

    /// `range` of `self` as a view, if it lies within it.
    #[cfg(any(debug_assertions, test, feature = "test-api"))]
    fn interval(
        self,
        range: Range<AtomPosition>,
    ) -> Option<IntervalView<Self::Index>> {
        (range.start <= range.end && range.end.0 <= self.width().0).then_some(
            IntervalView {
                owner: self.vertex_index(),
                range,
            },
        )
    }

    /// Whether every step is a stored occurrence (each step is looked up
    /// in its own parent, reached from `self`).
    #[cfg(any(test, feature = "test-api"))]
    fn contains_path<'s>(
        self,
        steps: impl IntoIterator<Item = &'s OccurrenceLocation<Self::Index>>,
    ) -> bool
    where
        Self::Index: 's,
    {
        steps.into_iter().all(|step| {
            self.resolve(step.parent)
                .and_then(|parent| parent.node().composite())
                .is_some_and(|composition| {
                    composition.occurrences().any(|o| {
                        o.offset == step.offset && o.child.index == step.child
                    })
                })
        })
    }

    /// Atoms of `range` (relative to `self`), in order, each position read
    /// exactly once. Descends through the composition and may cut through
    /// a child; no token is created for the cut. `None` if the range
    /// exceeds `self`.
    #[cfg(any(debug_assertions, test, feature = "test-api"))]
    fn read_interval(
        self,
        range: Range<AtomPosition>,
    ) -> Option<Vec<AtomOf<Self::Kind>>> {
        (range.start <= range.end && range.end.0 <= self.width().0).then(|| {
            let range = range.start.0..range.end.0;
            let mut atoms = Vec::with_capacity(range.len());
            read_into(self, range, &mut atoms);
            atoms
        })
    }
}

impl<'a, V: VertexRead<'a>> Navigate<'a> for V {}

/// The vertex's string, computed by expanding its composition down to
/// atoms (legacy `HasVertexStringRepr`). Works for every store and in
/// release builds; no content is stored.
pub fn string_repr<'a, V: VertexRead<'a>>(vertex: V) -> String
where
    AtomOf<V::Kind>: Display,
{
    let mut atoms = Vec::with_capacity(vertex.width().0);
    read_into(vertex, 0..vertex.width().0, &mut atoms);
    atoms.iter().map(ToString::to_string).collect()
}

/// The location of `occurrence` in `parent` (shared with the search-only
/// navigation of `context_search::path::PathNavigate`).
#[doc(hidden)]
pub fn location<I: crate::VertexId>(
    parent: I,
    occurrence: Occurrence<I>,
) -> OccurrenceLocation<I> {
    OccurrenceLocation {
        parent,
        offset: occurrence.offset,
        child: occurrence.child.index,
    }
}

/// Continue `steps` with `start_at(parent, p)` (shared with
/// `context_search::path::PathNavigate`).
#[doc(hidden)]
pub fn descend<'a, V: VertexRead<'a>>(
    mut parent: V,
    mut p: AtomPosition,
    mut steps: Vec<OccurrenceLocation<V::Index>>,
) -> Option<Located<V::Index>> {
    loop {
        let occurrence = parent.node().composite()?.covering(p)?;
        steps.push(location(parent.vertex_index(), occurrence));
        if occurrence.offset == p {
            return Some((occurrence.child, steps));
        }
        p = AtomPosition(p.0 - occurrence.offset.0);
        parent = parent.resolve(occurrence.child.index)?;
    }
}

/// The border-child chain below `vertex` (`next` is the prefix or postfix
/// child).
fn chain<'a, V: VertexRead<'a>>(
    vertex: V,
    next: fn(V) -> Option<Occurrence<V::Index>>,
) -> impl Iterator<Item = (OccurrenceLocation<V::Index>, Token<V::Index>)> {
    let mut current = Some(vertex);
    std::iter::from_fn(move || {
        let parent = current?;
        let Some(child) = next(parent) else {
            current = None;
            return None;
        };
        current = parent.resolve(child.child.index);
        Some((location(parent.vertex_index(), child), child.child))
    })
}

/// Append the atoms of `range` (relative to `vertex`) to `atoms`.
fn read_into<'a, V: VertexRead<'a>>(
    vertex: V,
    range: std::ops::Range<usize>,
    atoms: &mut Vec<AtomOf<V::Kind>>,
) {
    if range.is_empty() {
        return;
    }
    // Dispatch through the projections, not a match on `NodeRef`: the
    // match makes the composition address depend on the node tag, which
    // serializes this hot recursion (about 40 % slower content reads).
    match vertex.node().composite() {
        None => atoms.push(*vertex.node().atom().expect("not composite")),
        Some(composition) => {
            // Occurrences are sorted by start without containment, so their
            // ends increase too: reading each one from the first unread
            // position emits every position once.
            let mut next = range.start;
            let mut occurrences = composition.occurrences();
            while next < range.end {
                let Some(occurrence) = occurrences.next() else {
                    break;
                };
                let (start, end) = (occurrence.offset.0, occurrence.end().0);
                if end <= next {
                    continue;
                }
                let to = end.min(range.end);
                let child = vertex
                    .resolve(occurrence.child.index)
                    .expect("child of a stored composition");
                read_into(child, next - start..to - start, atoms);
                next = to;
            }
        },
    }
}
