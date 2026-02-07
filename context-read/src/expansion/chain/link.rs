use context_trace::*;
use derive_more::From;

use crate::expansion::chain::band::Band;

#[derive(Clone, Debug)]
pub(crate) struct OverlapLink {
    pub(crate) postfix_path: RolePath<End>, // location of postfix/overlap in first index
    pub(crate) prefix_path: RolePath<Start>, // location of prefix/overlap in second index
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
