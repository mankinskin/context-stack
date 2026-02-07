use std::num::NonZeroUsize;

use crate::*;
use context_trace::*;
use derive_new::new;
pub(crate) type Offset = NonZeroUsize;

/// optional offset inside of pattern sub location
#[derive(Clone, Debug, PartialEq, Eq, new)]
pub(crate) struct SubSplitLocation {
    pub(crate) location: SubLocation,
    pub(crate) inner_offset: Option<Offset>,
}

impl From<SubSplitLocation> for (PatternId, TokenTracePos) {
    fn from(sub: SubSplitLocation) -> Self {
        (
            sub.location.pattern_id(),
            TokenTracePos::new(sub.inner_offset(), sub.location.sub_index()),
        )
    }
}

pub(crate) trait HasInnerOffset {
    fn inner_offset(&self) -> Option<NonZeroUsize>;
}
impl HasInnerOffset for SubSplitLocation {
    fn inner_offset(&self) -> Option<NonZeroUsize> {
        self.inner_offset
    }
}
