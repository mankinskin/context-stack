//! Compare states (legacy `compare/state`).

mod core;
mod decomposition;
mod transitions;

pub(crate) use core::{
    CompareEndResult,
    CompareLeafResult,
    IndexAdvanceResult,
    MatchedCompareState,
    QueryAdvanceResult,
};
pub use core::{
    CompareState,
    PathPairMode,
};
