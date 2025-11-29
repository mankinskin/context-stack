use context_trace::*;
use derive_more::From;

use crate::expansion::chain::band::Band;

#[derive(Clone, Debug)]
pub struct OverlapLink {
    pub postfix_path: RolePath<End>, // location of postfix/overlap in first index
    pub prefix_path: RolePath<Start>, // location of prefix/overlap in second index
}

#[derive(Debug, From)]
pub enum ChainOp {
    Expansion(BandExpansion),
    Cap(BandCap),
}
#[derive(Debug)]
pub struct BandExpansion {
    pub expansion: IndexWithPath,
    pub start_bound: AtomPosition,
    pub postfix_path: IndexEndPath,
}
impl StartBound for BandExpansion {
    fn start_bound(&self) -> AtomPosition {
        self.start_bound
    }
}
#[derive(Debug)]
pub struct BandCap {
    pub postfix_path: IndexEndPath,
    pub expansion: Token,
    pub start_bound: AtomPosition,
}

pub trait StartBound: Sized {
    fn start_bound(&self) -> AtomPosition;
}
pub trait EndBound: Sized {
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
