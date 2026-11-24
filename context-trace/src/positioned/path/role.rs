//! Path roles (legacy `path::accessors::role`): which side of a matched
//! span a path addresses.
//!
//! A [`Start`] path leads from a root to the token at the span start, an
//! [`End`] path to the token at the span end. Both are chains of
//! [`OccurrenceLocation`]s from the root down to a leaf; the role fixes
//!
//! - the storage order: start paths are stored bottom-up (a raise pushes),
//!   end paths top-down (a descent pushes); [`PathRole::top_down`] and
//!   [`PathRole::bottom_up`] iterate either way;
//! - the span boundary ([`PathRole::boundary`]) and the border predicate
//!   ([`PathRole::at_border`]) of the canonical rules (raise, coverage);
//! - how a path is located from a root and a position ([`PathRole::locate`]);
//! - the trace direction and key positions of its cache edges.
//!
//! **Dimension readiness.** Position arithmetic of paths (offset sums, span
//! boundaries, border tests, trace key positions) lives in this module and
//! in [`offset_sum`]/[`token_end`]; a later space abstraction (position,
//! extent and span types for multi-dimensional inputs) only has to replace
//! these.

use std::{
    fmt::Debug,
    hash::Hash,
    ops::Range,
};

use crate::{
    AtomPosition,
    Token,
    TokenWidth,
    graph::vertex::VertexId,
    positioned::{
        access::GraphRead,
        composition::Occurrence,
        location::OccurrenceLocation,
        navigate::{
            Located,
            Navigate,
        },
        trace::{
            BottomUp,
            QueryAnchor,
            TopDown,
            TraceDirection,
        },
    },
};

/// Sum of the step offsets: the offset of the last child in the first
/// parent.
pub fn offset_sum<'a, I: VertexId + 'a>(
    steps: impl IntoIterator<Item = &'a OccurrenceLocation<I>>
) -> AtomPosition {
    steps
        .into_iter()
        .fold(AtomPosition::default(), |sum, step| sum + step.offset.0)
}

/// End of a token of `width` at `offset`.
pub fn token_end(
    offset: AtomPosition,
    width: TokenWidth,
) -> AtomPosition {
    offset + width.0
}

pub trait PathRole: 'static + Debug + Clone + Copy + Default + PartialEq + Eq + Hash {
    /// Direction of the trace edges along a path of this role.
    type Direction: TraceDirection;

    /// Stored steps from the root child down to the leaf.
    fn top_down<I: VertexId>(
        steps: &[OccurrenceLocation<I>]
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<I>> + ExactSizeIterator;
    /// Stored steps from the leaf up to the root child.
    fn bottom_up<I: VertexId>(
        steps: &[OccurrenceLocation<I>]
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<I>> + ExactSizeIterator {
        Self::top_down(steps).rev()
    }
    /// Top-down steps in storage order.
    fn from_top_down<I: VertexId>(steps: Vec<OccurrenceLocation<I>>) -> Vec<OccurrenceLocation<I>>;
    /// The step in the root (its child is the root child).
    fn root_step<I: VertexId>(steps: &[OccurrenceLocation<I>]) -> Option<&OccurrenceLocation<I>>;
    /// Add a step above the current root step.
    fn push_root<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    );
    /// Add a step below the current leaf.
    fn push_leaf<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    );

    /// The boundary of `span` this role addresses.
    fn boundary(span: &Range<AtomPosition>) -> AtomPosition;
    /// A token of `width` at `offset` has its border on `boundary` (start
    /// for [`Start`], end for [`End`]).
    fn at_border(
        offset: AtomPosition,
        width: TokenWidth,
        boundary: AtomPosition,
    ) -> bool;
    /// `span` (in a root of `width`) reaches the root border of this role.
    fn spans_border(
        span: &Range<AtomPosition>,
        width: TokenWidth,
    ) -> bool {
        Self::at_border(AtomPosition::default(), width, Self::boundary(span))
    }
    /// The largest proper descendant of `root` with its border at `p` and
    /// the top-down steps to it ([`Navigate::start_at`] for
    /// [`Start`], [`Navigate::end_at`] for [`End`]).
    fn locate<G: GraphRead + ?Sized>(
        graph: &G,
        root: Token<G::Index>,
        p: AtomPosition,
    ) -> Option<Located<G::Index>>;

    /// Position of the trace key of a token of `width` at root offset
    /// `offset`.
    fn trace_pos(
        anchor: &QueryAnchor,
        offset: AtomPosition,
        width: TokenWidth,
    ) -> <Self::Direction as TraceDirection>::Pos;
}

/// Roles whose paths descend into the child at their border.
pub trait BorderChild: PathRole {
    /// The child of `token` at its role border; `None` for atoms.
    fn border_child<G: GraphRead + ?Sized>(
        graph: &G,
        token: Token<G::Index>,
    ) -> Option<Occurrence<G::Index>>;
}

/// Path to the token at the span start, stored bottom-up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Start;

/// Path to the token at the span end, stored top-down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct End;

impl PathRole for Start {
    type Direction = BottomUp;

    fn top_down<I: VertexId>(
        steps: &[OccurrenceLocation<I>]
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<I>> + ExactSizeIterator {
        steps.iter().rev()
    }
    fn from_top_down<I: VertexId>(
        mut steps: Vec<OccurrenceLocation<I>>
    ) -> Vec<OccurrenceLocation<I>> {
        steps.reverse();
        steps
    }
    fn root_step<I: VertexId>(steps: &[OccurrenceLocation<I>]) -> Option<&OccurrenceLocation<I>> {
        steps.last()
    }
    fn push_root<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    ) {
        steps.push(step);
    }
    fn push_leaf<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    ) {
        steps.insert(0, step);
    }
    fn boundary(span: &Range<AtomPosition>) -> AtomPosition {
        span.start
    }
    fn at_border(
        offset: AtomPosition,
        _width: TokenWidth,
        boundary: AtomPosition,
    ) -> bool {
        offset == boundary
    }
    fn locate<G: GraphRead + ?Sized>(
        graph: &G,
        root: Token<G::Index>,
        p: AtomPosition,
    ) -> Option<Located<G::Index>> {
        graph.vertex(root)?.start_at(p)
    }
    /// Up keys: the query position at the token end.
    fn trace_pos(
        anchor: &QueryAnchor,
        offset: AtomPosition,
        width: TokenWidth,
    ) -> crate::UpPosition {
        anchor.up(token_end(offset, width))
    }
}

impl PathRole for End {
    type Direction = TopDown;

    fn top_down<I: VertexId>(
        steps: &[OccurrenceLocation<I>]
    ) -> impl DoubleEndedIterator<Item = &OccurrenceLocation<I>> + ExactSizeIterator {
        steps.iter()
    }
    fn from_top_down<I: VertexId>(
        steps: Vec<OccurrenceLocation<I>>
    ) -> Vec<OccurrenceLocation<I>> {
        steps
    }
    fn root_step<I: VertexId>(steps: &[OccurrenceLocation<I>]) -> Option<&OccurrenceLocation<I>> {
        steps.first()
    }
    fn push_root<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    ) {
        steps.insert(0, step);
    }
    fn push_leaf<I: VertexId>(
        steps: &mut Vec<OccurrenceLocation<I>>,
        step: OccurrenceLocation<I>,
    ) {
        steps.push(step);
    }
    fn boundary(span: &Range<AtomPosition>) -> AtomPosition {
        span.end
    }
    fn at_border(
        offset: AtomPosition,
        width: TokenWidth,
        boundary: AtomPosition,
    ) -> bool {
        token_end(offset, width) == boundary
    }
    fn locate<G: GraphRead + ?Sized>(
        graph: &G,
        root: Token<G::Index>,
        p: AtomPosition,
    ) -> Option<Located<G::Index>> {
        graph.vertex(root)?.end_at(p)
    }
    /// Down keys: the query position at the token start, clamped to the
    /// match start.
    fn trace_pos(
        anchor: &QueryAnchor,
        offset: AtomPosition,
        _width: TokenWidth,
    ) -> crate::DownPosition {
        anchor.down(offset)
    }
}

/// An end path continues at the next token starting at the match end, so
/// it descends into prefix children.
impl BorderChild for End {
    fn border_child<G: GraphRead + ?Sized>(
        graph: &G,
        token: Token<G::Index>,
    ) -> Option<Occurrence<G::Index>> {
        graph.vertex(token)?.prefix_child()
    }
}
