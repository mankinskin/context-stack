//! [`PositionedError`]: edit and validation failures.

use std::fmt;

use super::{
    CompositionError,
    Occurrence,
};
use crate::{
    AtomPosition,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        id::Plain,
    },
};

/// Edit or validation failure, with the indices of the store `I`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PositionedError<I: VertexId = CommittedIndex> {
    Composition {
        owner: Option<I>,
        error: CompositionError<I>,
    },
    UnknownVertex(I),
    /// A composite-only operation on an atom.
    Atom(I),
    /// Stored or referenced width disagrees with the vertex.
    Width {
        vertex: I,
        expected: TokenWidth,
        found: TokenWidth,
    },
    /// Occurrence `child@offset` in `parent` lacks its reciprocal link (or
    /// the link has no occurrence).
    ParentLink {
        parent: I,
        child: I,
        offset: AtomPosition,
    },
    /// Vertex stored at the wrong slot.
    Index {
        slot: usize,
        vertex: I,
    },
    /// `first` and `second` denote the same content.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    DuplicateContent {
        first: I,
        second: I,
    },
    /// Adjacent occurrences in `owner` denote different atoms on their overlap.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    OverlapMismatch {
        owner: I,
        left: Occurrence<I>,
        right: Occurrence<I>,
    },
    /// Adjacent occurrences in `owner` overlap, but their overlap is not a
    /// vertex (G4).
    #[cfg(any(debug_assertions, feature = "test-api"))]
    OverlapNotVertex {
        owner: I,
        left: Occurrence<I>,
        right: Occurrence<I>,
    },
    /// The vertex has parents, so its width must not change.
    HasParents(I),
    /// The vertex is committed, so its content must not change.
    NotStaged(I),
    /// `occurrence` does not lie where the edit of `vertex` requires it.
    OutsideVertex {
        vertex: I,
        occurrence: Occurrence<I>,
    },
    /// The edit would change the width of `vertex`.
    WidthChange { vertex: I },
}

impl<I: VertexId> fmt::Display for PositionedError<I> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let owner = |owner: &Option<I>| {
            owner.map_or("new vertex".to_string(), |o| format!("vertex {}", Plain(o)))
        };
        match self {
            Self::Composition { owner: o, error } => {
                write!(f, "{}: {error}", owner(o))
            },
            Self::UnknownVertex(v) => write!(f, "unknown vertex {}", Plain(*v)),
            Self::Atom(v) => write!(f, "vertex {} is an atom", Plain(*v)),
            Self::Width {
                vertex,
                expected,
                found,
            } => write!(
                f,
                "vertex {}: width {} expected, found {}",
                Plain(*vertex),
                expected.0,
                found.0
            ),
            Self::ParentLink {
                parent,
                child,
                offset,
            } => write!(
                f,
                "occurrence {}@{offset} in vertex {} has no reciprocal link",
                Plain(*child),
                Plain(*parent)
            ),
            Self::Index { slot, vertex } => {
                write!(f, "vertex {} stored at slot {slot}", Plain(*vertex))
            },
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::DuplicateContent { first, second } => write!(
                f,
                "vertices {} and {} have the same content",
                Plain(*first),
                Plain(*second)
            ),
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::OverlapMismatch { owner, left, right } => write!(
                f,
                "vertex {}: occurrences {}@{} and {}@{} disagree on their overlap",
                Plain(*owner),
                Plain(left.child.index),
                left.offset,
                Plain(right.child.index),
                right.offset
            ),
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::OverlapNotVertex { owner, left, right } => write!(
                f,
                "vertex {}: the overlap of {}@{} and {}@{} is not a vertex",
                Plain(*owner),
                Plain(left.child.index),
                left.offset,
                Plain(right.child.index),
                right.offset
            ),
            Self::HasParents(vertex) => write!(f, "vertex {} has parents", Plain(*vertex)),
            Self::NotStaged(vertex) => write!(f, "vertex {} is committed", Plain(*vertex)),
            Self::OutsideVertex { vertex, occurrence } => write!(
                f,
                "occurrence {}@{} lies outside the editable range of vertex {}",
                Plain(occurrence.child.index),
                occurrence.offset,
                Plain(*vertex)
            ),
            Self::WidthChange { vertex } => {
                write!(f, "edit would change the width of vertex {}", Plain(*vertex))
            },
        }
    }
}

impl<I: VertexId> std::error::Error for PositionedError<I> {}
