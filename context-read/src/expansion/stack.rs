use std::collections::VecDeque;

use context_trace::*;

use crate::expansion::chain::link::StartBound;

#[derive(Debug, Clone)]
pub(crate) enum StackLocation {
    Head,
    Nested {
        nested_index: usize,
        inner_location: Box<StackLocation>,
    },
}
//#[derive(Debug, Clone)]
//pub(crate) struct NestedStack {
//    pub(crate) stack: OverlapStack,
//    pub(crate) back_context: Pattern,
//    pub(crate) start_bound: usize,
//}

#[derive(Debug, Clone)]
pub(crate) struct OverlapStack {
    pub(crate) head: Pattern,
    pub(crate) overlaps: VecDeque<StackBand>,
}

#[derive(Debug, Clone)]
pub(crate) enum StackBandEnd {
    Single(Token),
    Stack(OverlapStack),
}
#[derive(Debug, Clone)]
pub(crate) struct StackBand {
    pub(crate) back_context: Token,
    pub(crate) expansion: StackBandEnd,
}
impl StartBound for StackBand {
    fn start_bound(&self) -> AtomPosition {
        self.back_context.width().into()
    }
}

impl OverlapStack {
    pub(crate) fn new(head_index: Token) -> Self {
        Self {
            head: Pattern::from(vec![head_index]),
            overlaps: VecDeque::default(),
        }
    }

    ///// Find if an expansion can be appended to any band in this stack
    //pub(crate) fn find_appendable_band(
    //    &self,
    //    expansion: &BandExpansion,
    //) -> Option<StackLocation> {
    //    // Check if expansion can be appended to head band
    //    if self.head.pattern_width() == expansion.start_bound {
    //        return Some(StackLocation::Head);
    //    }

    //    // Recursively check nested stacks
    //    for (nested_index, nested) in self.nested_stacks.iter().enumerate() {
    //        if let Some(location) = nested.stack.find_appendable_band(expansion)
    //        {
    //            return Some(StackLocation::Nested {
    //                nested_index,
    //                inner_location: Box::new(location),
    //            });
    //        }
    //    }
    //    None
    //}
}
