use std::cmp::Ordering;

use context_trace::*;

pub(crate) trait TraversalOrder: Wide + HasSubIndex {
    fn cmp(
        &self,
        other: impl TraversalOrder,
    ) -> Ordering {
        match self.width().cmp(&other.width()) {
            Ordering::Equal => self.sub_index().cmp(&other.sub_index()),
            r => r,
        }
    }
}

impl<T: Wide + HasSubIndex> TraversalOrder for T {}
