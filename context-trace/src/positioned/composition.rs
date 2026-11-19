//! Positioned child occurrences and the maximal cover of a vertex.

use std::fmt;

use crate::{
    AtomPosition,
    Token,
    TokenWidth,
    graph::vertex::{
        VertexIndex,
        wide::Wide,
    },
};

/// A child token placed at an atom offset inside its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Occurrence {
    pub offset: AtomPosition,
    pub child: Token,
}

impl Occurrence {
    pub fn new(
        offset: impl Into<AtomPosition>,
        child: Token,
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

/// Why a list of occurrences is not a complete composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompositionError {
    /// One occurrence or none: an owner needs at least two narrower children.
    TooFew,
    /// Two occurrences share `(offset, width)`.
    Duplicate(Occurrence),
    /// `inner` lies wholly inside `outer`.
    Contained { inner: Occurrence, outer: Occurrence },
    /// Atom position not covered by any occurrence.
    Gap(AtomPosition),
}

impl fmt::Display for CompositionError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::TooFew => f.write_str("a composition needs at least two occurrences"),
            Self::Duplicate(o) => {
                write!(f, "duplicate occurrence {}@{}", o.child.index.0, o.offset)
            },
            Self::Contained { inner, outer } => write!(
                f,
                "occurrence {}@{} is contained in {}@{}",
                inner.child.index.0, inner.offset, outer.child.index.0, outer.offset
            ),
            Self::Gap(position) => write!(f, "position {position} is not covered"),
        }
    }
}

impl std::error::Error for CompositionError {}

/// Complete composition of a non-atom vertex: occurrences sorted by
/// `(offset, width)`, without duplicates or wholly contained intervals,
/// covering `0..width` without gaps.
///
/// The constructor is the only way to build one, so a `Composition` is
/// complete by construction. Content agreement with the children is checked
/// by [`super::PositionedGraph::insert`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Composition(Vec<Occurrence>);

impl Composition {
    pub fn new(mut occurrences: Vec<Occurrence>) -> Result<Self, CompositionError> {
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
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.0
    }
    /// Owner width implied by the composition (end of the last occurrence).
    pub fn width(&self) -> TokenWidth {
        TokenWidth(self.0.last().map_or(0, |o| o.end().0))
    }
    pub fn children(&self) -> impl Iterator<Item = VertexIndex> + '_ {
        self.0.iter().map(|o| o.child.index)
    }
}
