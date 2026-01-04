use std::num::NonZeroUsize;

use perfect::*;

use crate::*;
use context_trace::*;
pub mod perfect;

pub mod trace;

pub mod visit;

pub struct BorderInfo {
    pub sub_index: usize,
    pub inner_offset: Option<NonZeroUsize>,
    /// start offset of index with border
    pub start_offset: Option<NonZeroUsize>,
}
impl BorderInfo {
    fn new(
        pattern: &Pattern,
        pos: &TokenTracePos,
    ) -> Self {
        let offset = End::inner_ctx_width(pattern, pos.sub_index());
        BorderInfo {
            sub_index: pos.sub_index(),
            inner_offset: pos.inner_offset(),
            start_offset: NonZeroUsize::new(offset),
        }
    }
    
    /// Create a BorderInfo by recalculating sub_index from atom position.
    /// This is more robust when the pattern may have been modified after the
    /// original trace was recorded.
    pub fn new_from_atom_pos(
        pattern: &Pattern,
        atom_pos: NonZeroUsize,
        inner_offset: Option<NonZeroUsize>,
    ) -> Self {
        use crate::TraceBack;
        // Recalculate sub_index from atom position using current pattern
        let trace_pos = TraceBack::trace_child_pos(pattern, atom_pos)
            .expect("atom_pos should be valid within pattern");
        let sub_index = trace_pos.sub_index();
        let offset = End::inner_ctx_width(pattern, sub_index);
        BorderInfo {
            sub_index,
            inner_offset,
            start_offset: NonZeroUsize::new(offset),
        }
    }
}
impl HasInnerOffset for BorderInfo {
    fn inner_offset(&self) -> Option<NonZeroUsize> {
        self.inner_offset
    }
}

pub trait PartitionBorder<R: RangeRole>: Sized {
    fn perfect(&self) -> BooleanPerfectOf<R>;
    fn offsets(&self) -> OffsetsOf<R>;
}

impl<
    P: BorderPerfect<Boolean = bool>,
    R: RangeRole<Perfect = P, Offsets = NonZeroUsize>,
> PartitionBorder<R> for BorderInfo
{
    fn perfect(&self) -> BooleanPerfectOf<R> {
        self.inner_offset.is_none()
    }
    fn offsets(&self) -> OffsetsOf<R> {
        self.start_offset
            .map(|o| {
                self.inner_offset
                    .map(|io| o.checked_add(io.get()).unwrap())
                    .unwrap_or(o)
            })
            .unwrap_or_else(|| self.inner_offset.unwrap())
    }
}

impl<M: InVisitMode> PartitionBorder<In<M>> for (BorderInfo, BorderInfo) {
    fn perfect(&self) -> BooleanPerfectOf<In<M>> {
        (
            <_ as PartitionBorder<Pre<M>>>::perfect(&self.0),
            <_ as PartitionBorder<Post<M>>>::perfect(&self.1),
        )
    }
    fn offsets(&self) -> OffsetsOf<In<M>> {
        (
            <_ as PartitionBorder<Pre<M>>>::offsets(&self.0),
            <_ as PartitionBorder<Post<M>>>::offsets(&self.1),
        )
    }
}
