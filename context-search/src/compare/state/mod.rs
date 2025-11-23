mod core;
mod decomposition;
mod transitions;

pub(crate) use core::{
    CompareResult,
    CompareState,
    IndexAdvanceResult,
    MatchedCompareState,
    PathPairMode,
    QueryAdvanceResult,
};
pub(crate) use decomposition::PrefixStates;
