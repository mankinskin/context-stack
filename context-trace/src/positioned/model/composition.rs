//! Positioned child occurrences and the maximal cover of a vertex.

use std::fmt;

use crate::{
    graph::vertex::id::Plain,
    AtomPosition,
    Token,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        VertexId,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
};

/// A child token placed at an atom offset inside its owner.
///
/// `repr(C)` (as [`Token`]): occurrences of every index type of one store
/// share their layout, so a stage can read committed compositions in its
/// own index space without copying ([`Composition::lifted`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Occurrence<I: VertexId = CommittedIndex> {
    pub offset: AtomPosition,
    pub child: Token<I>,
}

impl<I: VertexId> Occurrence<I> {
    pub fn new(
        offset: impl Into<AtomPosition>,
        child: Token<I>,
    ) -> Self {
        Self {
            offset: offset.into(),
            child,
        }
    }
    /// Exclusive end offset in the owner.
    pub fn end(&self) -> AtomPosition {
        self.offset + self.child.width().0
    }
    fn order_key(&self) -> (AtomPosition, TokenWidth) {
        (self.offset, self.child.width())
    }
}

/// The child's index.
impl<I: VertexId> HasVertexIndex for Occurrence<I> {
    type Index = I;
    fn vertex_index(&self) -> I {
        self.child.index
    }
}

/// The child's width.
impl<I: VertexId> Wide for Occurrence<I> {
    fn width(&self) -> TokenWidth {
        self.child.width()
    }
}

/// Why a list of occurrences is not a complete composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompositionError<I: VertexId = CommittedIndex> {
    /// One occurrence or none: an owner needs at least two narrower children.
    TooFew,
    /// Two occurrences share `(offset, width)`.
    Duplicate(Occurrence<I>),
    /// `inner` lies wholly inside `outer`.
    Contained {
        inner: Occurrence<I>,
        outer: Occurrence<I>,
    },
    /// Atom position not covered by any occurrence.
    Gap(AtomPosition),
}

impl<I: VertexId> fmt::Display for CompositionError<I> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::TooFew => f.write_str("a composition needs at least two occurrences"),
            Self::Duplicate(o) => {
                write!(f, "duplicate occurrence {}@{}", Plain(o.child.index), o.offset)
            },
            Self::Contained { inner, outer } => write!(
                f,
                "occurrence {}@{} is contained in {}@{}",
                Plain(inner.child.index),
                inner.offset,
                Plain(outer.child.index),
                outer.offset
            ),
            Self::Gap(position) => write!(f, "position {position} is not covered"),
        }
    }
}

impl<I: VertexId> std::error::Error for CompositionError<I> {}

/// Complete composition of a non-atom vertex: occurrences sorted by
/// `(offset, width)`, without duplicates or wholly contained intervals,
/// covering `0..width` without gaps.
///
/// The constructor is the only way to build one, so a `Composition` is
/// complete by construction. Content agreement with the children is checked
/// by `GraphEdit::create` (debug and `test-api` builds).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Composition<I: VertexId = CommittedIndex>(Vec<Occurrence<I>>);

impl<I: VertexId> Composition<I> {
    pub fn new(mut occurrences: Vec<Occurrence<I>>) -> Result<Self, CompositionError<I>> {
        occurrences.sort_by_key(Occurrence::order_key);
        if occurrences.len() < 2 {
            return Err(CompositionError::TooFew);
        }
        let mut covered = AtomPosition::default();
        for pair in occurrences.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            if prev.order_key() == next.order_key() {
                return Err(CompositionError::Duplicate(next));
            }
            // Sorted by start; `next` is contained iff it ends no later, and
            // `prev` iff both share the start.
            if next.end() <= prev.end() {
                return Err(CompositionError::Contained {
                    inner: next,
                    outer: prev,
                });
            }
            if next.offset == prev.offset {
                return Err(CompositionError::Contained {
                    inner: prev,
                    outer: next,
                });
            }
        }
        for occurrence in &occurrences {
            if occurrence.offset > covered {
                return Err(CompositionError::Gap(covered));
            }
            covered = occurrence.end();
        }
        Ok(Self(occurrences))
    }
    pub fn occurrences(&self) -> &[Occurrence<I>] {
        &self.0
    }
    /// The same composition with every child index mapped by `f` (index
    /// spaces of the same graph: order, offsets and widths are kept).
    pub(crate) fn map_index<J: VertexId>(
        &self,
        mut f: impl FnMut(I) -> J,
    ) -> Composition<J> {
        Composition(
            self.0
                .iter()
                .map(|o| Occurrence::new(o.offset, Token::new(f(o.child.index), o.child.width)))
                .collect(),
        )
    }
    /// Owner width implied by the composition (end of the last occurrence).
    pub fn width(&self) -> TokenWidth {
        TokenWidth(self.0.last().map_or(0, |o| o.end().0))
    }
    pub fn children(&self) -> impl Iterator<Item = I> + '_ {
        self.0.iter().map(|o| o.child.index)
    }
    /// The occurrence covering `position`: the last one with
    /// `offset ≤ position` (binary search). Ends increase strictly with
    /// offsets, so it reaches furthest among those; by gap-freedom it covers
    /// `position`. `None` iff `position ≥ width`.
    pub fn covering(
        &self,
        position: impl Into<AtomPosition>,
    ) -> Option<&Occurrence<I>> {
        slice::covering(&self.0, position.into())
    }
    /// The occurrence reaching `end` from furthest left: the first one with
    /// `end() ≥ end` (binary search; ends increase strictly with offsets).
    /// `None` iff `end == 0` or `end > width`.
    pub fn ending_at(
        &self,
        end: impl Into<AtomPosition>,
    ) -> Option<&Occurrence<I>> {
        slice::ending_at(&self.0, end.into())
    }
    /// Occurrences strictly containing `position` (`offset < position < end`),
    /// a contiguous range because offsets and ends both increase strictly
    /// (two binary searches). Empty if `position` lies on a child border
    /// between non-overlapping occurrences or outside `(0, width)`.
    pub fn crossing(
        &self,
        position: impl Into<AtomPosition>,
    ) -> &[Occurrence<I>] {
        slice::crossing(&self.0, position.into())
    }
    /// The unique occurrence at offset 0.
    pub fn prefix(&self) -> &Occurrence<I> {
        &self.0[0]
    }
    /// The unique occurrence ending at the width: ends increase strictly
    /// with offsets, so it is the last one.
    pub fn postfix(&self) -> &Occurrence<I> {
        self.0.last().expect("a composition has at least two occurrences")
    }
}

impl Composition<CommittedIndex> {
    /// The occurrences with their children as [`StageIndex`]es: a stage
    /// reads unchanged committed compositions without copying.
    pub(crate) fn lifted(&self) -> &[Occurrence<StageIndex>] {
        // SAFETY: `Occurrence` and `Token` are `repr(C)`, `VertexIndex` and
        // `StageIndex` are `repr(transparent)` over `usize`, so both
        // occurrence types have the same layout, and every `usize` is a
        // valid `StageIndex`. Committed slots never have the staged bit set
        // (`StageIndex::from` asserts it), so each lifted index addresses
        // the same committed vertex.
        unsafe { std::slice::from_raw_parts(self.0.as_ptr().cast(), self.0.len()) }
    }
}

/// The search primitives of [`Composition`] on its occurrence slice, shared
/// with stage views of compositions.
pub(crate) mod slice {
    use super::Occurrence;
    use crate::{
        AtomPosition,
        graph::vertex::VertexId,
    };

    pub(crate) fn width<I: VertexId>(occurrences: &[Occurrence<I>]) -> usize {
        occurrences.last().map_or(0, |o| o.end().0)
    }
    pub(crate) fn covering<I: VertexId>(
        occurrences: &[Occurrence<I>],
        position: AtomPosition,
    ) -> Option<&Occurrence<I>> {
        if position.0 >= width(occurrences) {
            return None;
        }
        let after = occurrences.partition_point(|o| o.offset <= position);
        Some(&occurrences[after - 1])
    }
    pub(crate) fn ending_at<I: VertexId>(
        occurrences: &[Occurrence<I>],
        end: AtomPosition,
    ) -> Option<&Occurrence<I>> {
        if end.0 == 0 {
            return None;
        }
        occurrences.get(occurrences.partition_point(|o| o.end() < end))
    }
    pub(crate) fn crossing<I: VertexId>(
        occurrences: &[Occurrence<I>],
        position: AtomPosition,
    ) -> &[Occurrence<I>] {
        let lo = occurrences.partition_point(|o| o.end() <= position);
        let hi = occurrences.partition_point(|o| o.offset < position);
        &occurrences[lo..hi]
    }
}

/// The owner width ([`Composition::width`]).
impl<I: VertexId> Wide for Composition<I> {
    fn width(&self) -> TokenWidth {
        Composition::width(self)
    }
}

/// The maximal occurrences of `occurrences`: sorted by `(start, wider
/// first)`, an occurrence is kept only if it ends after all earlier ones.
/// Drops duplicates and occurrences contained in others; partial overlaps
/// are kept.
pub fn maximal<I: VertexId>(mut occurrences: Vec<Occurrence<I>>) -> Vec<Occurrence<I>> {
    occurrences.sort_by(|a, b| {
        a.offset
            .cmp(&b.offset)
            .then(b.child.width().cmp(&a.child.width()))
    });
    let mut reach = AtomPosition::default();
    let mut kept: Vec<Occurrence<I>> = Vec::with_capacity(occurrences.len());
    for occurrence in occurrences {
        if occurrence.end() > reach {
            reach = occurrence.end();
            kept.push(occurrence);
        }
    }
    kept
}
