use std::{
    fmt::Debug,
    num::NonZeroUsize,
    ops::{
        Range,
        RangeBounds,
        RangeFrom,
        RangeTo,
    },
};

use crate::{
    interval::partition::info::range::{
        mode::{
            InVisitMode,
            PostVisitMode,
            PreVisitMode,
        },
        role::{
            In,
            Post,
            Pre,
            RangeRole,
        },
    },
    split::{
        cache::vertex::SplitVertexCache,
        position_splits,
        range_splits,
        vertex::{
            PosSplitCtx,
            ToVertexSplits,
            node::AsNodeTraceCtx,
        },
    },
};
use context_trace::*;
use derivative::Derivative;
use derive_more::{
    Deref,
    DerefMut,
    From,
    Into,
};

pub trait OffsetIndexRange<R: RangeRole>: RangeIndex {
    fn get_splits(
        &self,
        vertex: &SplitVertexCache,
    ) -> R::Splits;
}

impl<M: InVisitMode> OffsetIndexRange<In<M>> for Range<usize> {
    fn get_splits(
        &self,
        vertex: &SplitVertexCache,
    ) -> <In<M> as RangeRole>::Splits {
        let lo = vertex
            .positions
            .iter()
            .map(PosSplitCtx::from)
            .nth(self.start)
            .unwrap();
        let ro = vertex
            .positions
            .iter()
            .map(PosSplitCtx::from)
            .nth(self.end)
            .unwrap();
        (lo.to_vertex_splits(), ro.to_vertex_splits())
    }
}

impl<M: PreVisitMode> OffsetIndexRange<Pre<M>> for RangeTo<usize> {
    fn get_splits(
        &self,
        vertex: &SplitVertexCache,
    ) -> <Pre<M> as RangeRole>::Splits {
        let ro = vertex
            .positions
            .iter()
            .map(PosSplitCtx::from)
            .nth(self.end)
            .unwrap();
        ro.to_vertex_splits()
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Derivative, Deref, DerefMut, From, Into,
)]
pub struct PostfixRangeFrom {
    range: Range<usize>, // end must be initialized from pattern
}
impl PostfixRangeFrom {
    pub fn new(
        start: usize,
        pattern_len: usize,
    ) -> Self {
        Self {
            range: start..pattern_len,
        }
    }
}
impl RangeBounds<usize> for PostfixRangeFrom {
    fn start_bound(&self) -> std::ops::Bound<&usize> {
        self.range.start_bound()
    }
    fn end_bound(&self) -> std::ops::Bound<&usize> {
        self.range.end_bound()
    }
}
impl Iterator for PostfixRangeFrom {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        self.range.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl ExactSizeIterator for PostfixRangeFrom {
    fn len(&self) -> usize {
        self.range.len()
    }
}

impl<M: PostVisitMode> OffsetIndexRange<Post<M>> for RangeFrom<usize> {
    fn get_splits(
        &self,
        vertex: &SplitVertexCache,
    ) -> <Post<M> as RangeRole>::Splits {
        let lo = vertex
            .positions
            .iter()
            .map(PosSplitCtx::from)
            .nth(self.start)
            .unwrap();
        lo.to_vertex_splits()
    }
}
pub trait RangeOffsets<R: RangeRole>: Debug + Clone + Copy {
    fn as_splits<C: AsNodeTraceCtx>(
        &self,
        ctx: C,
    ) -> R::Splits;
}

impl<M: InVisitMode> RangeOffsets<In<M>> for (NonZeroUsize, NonZeroUsize) {
    fn as_splits<C: AsNodeTraceCtx>(
        &self,
        ctx: C,
    ) -> <In<M> as RangeRole>::Splits {
        range_splits(ctx.as_trace_context().patterns.iter(), *self)
    }
}

impl<M: PreVisitMode> RangeOffsets<Pre<M>> for NonZeroUsize {
    fn as_splits<C: AsNodeTraceCtx>(
        &self,
        ctx: C,
    ) -> <Pre<M> as RangeRole>::Splits {
        position_splits(ctx.as_trace_context().patterns.iter(), *self)
    }
}

impl<M: PostVisitMode> RangeOffsets<Post<M>> for NonZeroUsize {
    fn as_splits<C: AsNodeTraceCtx>(
        &self,
        ctx: C,
    ) -> <Post<M> as RangeRole>::Splits {
        position_splits(ctx.as_trace_context().patterns.iter(), *self)
    }
}
