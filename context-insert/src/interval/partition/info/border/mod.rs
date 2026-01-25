use std::num::NonZeroUsize;

use perfect::*;

use crate::*;
use context_trace::*;
pub mod perfect;

pub mod trace;

pub mod info;

pub struct BorderInfo {
    pub sub_index: usize,
    pub pattern_len: usize,
    pub inner_offset: Option<NonZeroUsize>,
    /// start offset of index with border
    pub start_offset: Option<NonZeroUsize>,
}
impl BorderInfo {
    /// Create a BorderInfo by recalculating sub_index AND inner_offset from atom position.
    /// This is more robust when the pattern may have been modified after the
    /// original trace was recorded.
    ///
    /// IMPORTANT: After pattern replacement (e.g., merging tokens), the original
    /// inner_offset from the cache no longer applies to the new token at sub_index.
    /// We must use the inner_offset calculated from trace_child_pos to get the
    /// correct position within the current pattern structure.
    pub fn new_from_atom_pos(
        pattern: &Pattern,
        atom_pos: NonZeroUsize,
    ) -> Self {
        use crate::TraceBack;
        // Recalculate BOTH sub_index and inner_offset from atom position using current pattern
        let trace_pos = TraceBack::trace_child_pos(pattern, atom_pos)
            .expect("atom_pos should be valid within pattern");
        let sub_index = trace_pos.sub_index();
        let offset = End::inner_ctx_width(pattern, sub_index);
        BorderInfo {
            sub_index,
            pattern_len: pattern.len(),
            inner_offset: trace_pos.inner_offset(), // Use recalculated inner_offset!
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
