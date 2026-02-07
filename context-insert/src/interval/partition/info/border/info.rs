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

pub(crate) trait InfoBorder<R: RangeRole>: Sized + PartitionBorder<R> {
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

    /// Get the sub_index of the start position.
    /// For In mode, this is the left border's sub_index.
    /// For Pre mode, this is 0.
    /// For Post mode, this is the border's sub_index.
    fn start_sub_index(&self) -> usize;
}

impl<M: PostVisitMode> InfoBorder<Post<M>> for BorderInfo {
    type Splits = TokenTracePos;
    type AtomPos = NonZeroUsize;

    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        _atom_pos: Self::AtomPos,
    ) -> Self {
        // Use splits (delta-adjusted) instead of atom_pos to handle modified patterns
        Self::new_from_trace_pos(pattern, splits)
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
    fn start_sub_index(&self) -> usize {
        self.sub_index
    }
}

impl<M: PreVisitMode> InfoBorder<Pre<M>> for BorderInfo {
    type Splits = TokenTracePos;
    type AtomPos = NonZeroUsize;

    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        _atom_pos: Self::AtomPos,
    ) -> Self {
        // Use splits (delta-adjusted) instead of atom_pos to handle modified patterns
        Self::new_from_trace_pos(pattern, splits)
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
    fn start_sub_index(&self) -> usize {
        0 // Pre mode always starts at 0
    }
}

impl<M: InVisitMode> InfoBorder<In<M>> for (BorderInfo, BorderInfo) {
    type Splits = (
        <BorderInfo as InfoBorder<Post<M>>>::Splits,
        <BorderInfo as InfoBorder<Pre<M>>>::Splits,
    );
    type AtomPos = (NonZeroUsize, NonZeroUsize);

    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        _atom_pos: Self::AtomPos,
    ) -> Self {
        // Use splits (delta-adjusted) instead of atom_pos to handle modified patterns
        let (left_splits, right_splits) = splits;
        (
            BorderInfo::new_from_trace_pos(pattern, left_splits),
            BorderInfo::new_from_trace_pos(pattern, right_splits),
        )
    }

    fn inner_range_offsets(
        &self,
        pattern: &Pattern,
    ) -> Option<OffsetsOf<In<M>>> {
        let a = InfoBorder::<Post<M>>::inner_range_offsets(&self.0, pattern);
        let b = InfoBorder::<Pre<M>>::inner_range_offsets(&self.1, pattern);
        let r = match (a, b) {
            (Some(lio), Some(rio)) => Some((lio, rio)),
            (Some(lio), None) => {
                // Right border has no inner_offset, meaning it's at a clean boundary.
                // The inner range ends at the START of the right border token, not after it.
                // Use start_offset directly, which is the position before the right border token.
                self.1.start_offset.map(|rio| (lio, rio))
            },
            (None, Some(rio)) => {
                // start_offset can be None when at position 0 - need inner_offset
                let lio = self.0.start_offset.or(self.0.inner_offset).expect(
                    "left border must have start_offset or inner_offset",
                );
                Some((lio, rio))
            },
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
    fn start_sub_index(&self) -> usize {
        self.0.sub_index // In mode uses left border's sub_index
    }
}
