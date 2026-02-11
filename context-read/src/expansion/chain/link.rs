use context_trace::*;
use derive_more::From;

use crate::expansion::chain::band::Band;

/// Represents the overlap between two tokens in a decomposition.
/// 
/// When a postfix of the current root expands into remaining pattern, it creates
/// an overlap region that can be viewed from two perspectives:
/// 
/// 1. From the first token's view: `child_path` - a top-down path from the starting 
///    root to the expandable postfix (the overlap region token)
/// 2. From the second token's view: `search_path` - a bottom-up then top-down path
///    from the expansion (the same overlap region, but from expansion's perspective)
/// 
/// This link helps retrieve or build complement tokens to convert from an overlap
/// chain representation to the full set of decompositions.
#[derive(Clone, Debug)]
pub(crate) struct OverlapLink {
    /// Top-down child path from starting root to the expandable postfix.
    /// This represents the overlap region token from the first token's perspective.
    pub(crate) child_path: IndexEndPath,
    
    /// Bottom-up then top-down search path from the expansion.
    /// This represents the overlap region token from the second token's perspective.
    pub(crate) search_path: IndexStartPath,
    
    /// Position where the overlap starts in the input sequence.
    pub(crate) start_bound: usize,
}

#[derive(Debug, From)]
pub(crate) enum ChainOp {
    Expansion(BandExpansion),
    Cap(BandCap),
}
#[derive(Debug)]
pub(crate) struct BandExpansion {
    pub(crate) expansion: IndexWithPath,
    pub(crate) start_bound: AtomPosition,
    pub(crate) postfix_path: IndexEndPath,
}
impl StartBound for BandExpansion {
    fn start_bound(&self) -> AtomPosition {
        self.start_bound
    }
}
#[derive(Debug)]
pub(crate) struct BandCap {
    pub(crate) postfix_path: IndexEndPath,
    pub(crate) expansion: Token,
    pub(crate) start_bound: AtomPosition,
}

pub(crate) trait StartBound: Sized {
    fn start_bound(&self) -> AtomPosition;
}
pub(crate) trait EndBound: Sized {
    fn end_bound(&self) -> AtomPosition;
}
impl StartBound for (AtomPosition, Band) {
    fn start_bound(&self) -> AtomPosition {
        self.0
    }
}
impl EndBound for (Band, AtomPosition) {
    fn end_bound(&self) -> AtomPosition {
        self.1
    }
}
