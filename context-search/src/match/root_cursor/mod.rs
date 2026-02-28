mod advance;
mod core;
//mod state;

pub(crate) use advance::AdvanceOutcome;
pub(crate) use core::{
    CompareParentBatch,
    ConclusiveEnd,
    RootAdvanceResult,
    RootCursor,
    RootEndResult,
};
