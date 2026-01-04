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
                Post,
                Pre,
                RangeOf,
                RangeRole,
            },
        },
    },
    split::vertex::position::HasInnerOffset,
};
use context_trace::*;

pub trait VisitBorders<R: RangeRole>: Sized + PartitionBorder<R> {
    type Splits;
    /// Create border info from pattern and splits.
    /// The atom_pos parameter allows recalculating sub_index from the current pattern,
    /// which is necessary when the pattern may have been modified after tracing.
    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        atom_pos: Option<NonZeroUsize>,
    ) -> Self;
    
    /// Create border info with both single and pair atom positions.
    /// Used by info_borders to pass all available position info.
    fn info_border_with_pos(
        pattern: &Pattern,
        splits: &Self::Splits,
        atom_pos: Option<NonZeroUsize>,
        _atom_pos_pair: Option<(NonZeroUsize, NonZeroUsize)>,
    ) -> Self {
        // Default implementation uses single atom_pos
        Self::info_border(pattern, splits, atom_pos)
    }
    
    fn inner_range_offsets(
        &self,
        pattern: &Pattern,
    ) -> Option<OffsetsOf<R>>;
    fn inner_range(&self) -> RangeOf<R>;
    fn outer_range(&self) -> RangeOf<R>;
}

impl<M: PostVisitMode> VisitBorders<Post<M>> for BorderInfo {
    type Splits = TokenTracePos;
    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        atom_pos: Option<NonZeroUsize>,
    ) -> Self {
        if let Some(pos) = atom_pos {
            Self::new_from_atom_pos(pattern, pos, splits.inner_offset())
        } else {
            Self::new(pattern, splits)
        }
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
    fn inner_range(&self) -> RangeOf<Post<M>> {
        self.sub_index + self.inner_offset.is_some() as usize..
    }
    fn outer_range(&self) -> RangeOf<Post<M>> {
        self.sub_index..
    }
}

impl<M: PreVisitMode> VisitBorders<Pre<M>> for BorderInfo {
    type Splits = TokenTracePos;
    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        atom_pos: Option<NonZeroUsize>,
    ) -> Self {
        if let Some(pos) = atom_pos {
            Self::new_from_atom_pos(pattern, pos, splits.inner_offset())
        } else {
            Self::new(pattern, splits)
        }
    }
    fn inner_range_offsets(
        &self,
        _pattern: &Pattern,
    ) -> Option<OffsetsOf<Pre<M>>> {
        (self.inner_offset.is_some() && self.sub_index > 0)
            .then_some(self.start_offset)
            .flatten()
    }
    fn inner_range(&self) -> RangeOf<Pre<M>> {
        0..self.sub_index
    }
    fn outer_range(&self) -> RangeOf<Pre<M>> {
        0..self.sub_index + self.inner_offset.is_some() as usize
    }
}

impl<M: InVisitMode> VisitBorders<In<M>> for (BorderInfo, BorderInfo) {
    type Splits = (
        <BorderInfo as VisitBorders<Post<M>>>::Splits,
        <BorderInfo as VisitBorders<Pre<M>>>::Splits,
    );
    fn info_border(
        pattern: &Pattern,
        splits: &Self::Splits,
        _atom_pos: Option<NonZeroUsize>,
    ) -> Self {
        // For Infix without position info, fall back to original behavior
        (
            BorderInfo::new(pattern, &splits.0),
            BorderInfo::new(pattern, &splits.1),
        )
    }
    
    fn info_border_with_pos(
        pattern: &Pattern,
        splits: &Self::Splits,
        _atom_pos: Option<NonZeroUsize>,
        atom_pos_pair: Option<(NonZeroUsize, NonZeroUsize)>,
    ) -> Self {
        // Use the pair of atom positions if available
        if let Some((left_pos, right_pos)) = atom_pos_pair {
            (
                BorderInfo::new_from_atom_pos(pattern, left_pos, splits.0.inner_offset()),
                BorderInfo::new_from_atom_pos(pattern, right_pos, splits.1.inner_offset()),
            )
        } else {
            // Fall back to original behavior
            (
                BorderInfo::new(pattern, &splits.0),
                BorderInfo::new(pattern, &splits.1),
            )
        }
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
    fn inner_range(&self) -> RangeOf<In<M>> {
        self.0.sub_index..self.1.sub_index
    }
    fn outer_range(&self) -> RangeOf<In<M>> {
        self.0.sub_index
            ..self.1.sub_index + self.1.inner_offset.is_some() as usize
    }
}
