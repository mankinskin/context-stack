//! Root cursor (legacy `match/root_cursor`): lock-step advance inside one
//! matched root.

mod advance;
mod core;

pub(crate) use core::{
    CompareParentBatch,
    ConclusiveEnd,
    RootAdvanceResult,
    RootCursor,
    RootEndResult,
};
