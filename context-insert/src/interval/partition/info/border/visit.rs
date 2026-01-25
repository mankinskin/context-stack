use std::num::NonZeroUsize;

use crate::{
    TokenTracePos,
    interval::partition::info::{
        border::{
            BorderInfo,
            PartitionBorder,
        },
        range::{
            mode::{
                InVisitMode,
                PostVisitMode,
                PreVisitMode,
            },
            role::{
                In,
                OffsetsOf,
                PatternRangeOf,
                Post,
                Pre,
                RangeRole,
            },
            splits::PostfixRangeFrom,
        },
    },
};
use context_trace::*;

pub trait VisitBorders<R: RangeRole>: Sized + PartitionBorder<R> {
    type Splits;
    /// The atom position type for this border visitor.
    /// - NonZeroUsize for Pre/Post modes
    /// - (NonZeroUsize, NonZeroUsize) for In mode
    type AtomPos;

    /// Create border info from pattern and splits.
    /// The atom_pos parameter allows recalculating sub_index from the current pattern,
    /// which is necessary when the pattern may have been modified after tracing.
    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        atom_pos: Self::AtomPos,
    ) -> Self;

    fn inner_range_offsets(
        &self,
        pattern: &Pattern,
    ) -> Option<OffsetsOf<R>>;
    fn inner_range(&self) -> PatternRangeOf<R>;
    fn outer_range(&self) -> PatternRangeOf<R>;
}

impl<M: PostVisitMode> VisitBorders<Post<M>> for BorderInfo {
    type Splits = TokenTracePos;
    type AtomPos = NonZeroUsize;

    fn info_border(
        pattern: &Pattern,
        _splits: &Self::Splits,
        atom_pos: Self::AtomPos,
    ) -> Self {
        Self::new_from_atom_pos(pattern, atom_pos)
    }
    fn inner_range_offsets(
        &self,
        pattern: &Pattern,
    ) -> Option<OffsetsOf<Post<M>>> {
        (self.inner_offset.is_some() && pattern.len() - self.sub_index > 1)
            .then(|| {
                let w = *pattern[self.sub_index].width();
                self.start_offset.map(|o| o.get() + w).unwrap_or(w)
            })
            .and_then(NonZeroUsize::new)
    }
    fn inner_range(&self) -> PatternRangeOf<Post<M>> {
        PostfixRangeFrom::new(
            self.sub_index + self.inner_offset.is_some() as usize,
            self.pattern_len,
        )
    }
    fn outer_range(&self) -> PatternRangeOf<Post<M>> {
        PostfixRangeFrom::new(self.sub_index, self.pattern_len)
    }
}

impl<M: PreVisitMode> VisitBorders<Pre<M>> for BorderInfo {
    type Splits = TokenTracePos;
    type AtomPos = NonZeroUsize;

    fn info_border(
        pattern: &Pattern,
        _splits: &Self::Splits,
        atom_pos: Self::AtomPos,
    ) -> Self {
        Self::new_from_atom_pos(pattern, atom_pos)
    }
    fn inner_range_offsets(
        &self,
        _pattern: &Pattern,
    ) -> Option<OffsetsOf<Pre<M>>> {
        (self.inner_offset.is_some() && self.sub_index > 0)
            .then_some(self.start_offset)
            .flatten()
    }
    fn inner_range(&self) -> PatternRangeOf<Pre<M>> {
        0..self.sub_index
    }
    fn outer_range(&self) -> PatternRangeOf<Pre<M>> {
        0..self.sub_index + self.inner_offset.is_some() as usize
    }
}

impl<M: InVisitMode> VisitBorders<In<M>> for (BorderInfo, BorderInfo) {
    type Splits = (
        <BorderInfo as VisitBorders<Post<M>>>::Splits,
        <BorderInfo as VisitBorders<Pre<M>>>::Splits,
    );
    type AtomPos = (NonZeroUsize, NonZeroUsize);

    fn info_border(
        pattern: &Pattern,
        _splits: &Self::Splits,
        atom_pos: Self::AtomPos,
    ) -> Self {
        let (left_pos, right_pos) = atom_pos;
        (
            BorderInfo::new_from_atom_pos(pattern, left_pos),
            BorderInfo::new_from_atom_pos(pattern, right_pos),
        )
    }

    fn inner_range_offsets(
        &self,
        pattern: &Pattern,
    ) -> Option<OffsetsOf<In<M>>> {
        let a = VisitBorders::<Post<M>>::inner_range_offsets(&self.0, pattern);
        let b = VisitBorders::<Pre<M>>::inner_range_offsets(&self.1, pattern);
        let r = match (a, b) {
            (Some(lio), Some(rio)) => Some((lio, rio)),
            (Some(lio), None) => Some((lio, {
                let w = *pattern[self.1.sub_index].width();
                let o = self.1.start_offset.unwrap().get() + w;
                NonZeroUsize::new(o).unwrap()
            })),
            (None, Some(rio)) => Some((self.0.start_offset.unwrap(), rio)),
            (None, None) => None,
        };
        r.filter(|(l, r)| l != r)
    }
    fn inner_range(&self) -> PatternRangeOf<In<M>> {
        self.0.sub_index..self.1.sub_index
    }
    fn outer_range(&self) -> PatternRangeOf<In<M>> {
        self.0.sub_index
            ..self.1.sub_index + self.1.inner_offset.is_some() as usize
    }
}
