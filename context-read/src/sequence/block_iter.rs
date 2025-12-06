use context_trace::{
    *,
    graph::vertex::atom::{NewAtomIndex, NewAtomIndices},
};
use derive_more::{
    Deref,
    DerefMut,
};
use itertools::Itertools;

use std::fmt::Debug;

#[derive(Debug, Deref, DerefMut, Clone)]
pub struct BlockIter {
    iter: std::iter::Peekable<std::vec::IntoIter<NewAtomIndex>>,
}

#[derive(Debug, Clone)]
pub struct NextBlock {
    pub known: Pattern,
    pub unknown: Pattern,
}
impl Iterator for BlockIter {
    type Item = NextBlock;
    fn next(&mut self) -> Option<Self::Item> {
        let unknown = self.next_pattern_where(|t| t.is_new());
        let known = self.next_pattern_where(|t| t.is_known());
        if unknown.is_empty() && known.is_empty() {
            None
        } else {
            Some(NextBlock { unknown, known })
        }
    }
}

impl BlockIter {
    pub fn new(sequence: NewAtomIndices) -> Self {
        Self {
            iter: sequence.into_iter().peekable(),
        }
    }
    fn next_pattern_where(
        &mut self,
        f: impl FnMut(&NewAtomIndex) -> bool,
    ) -> Pattern {
        self.iter.peeking_take_while(f).map(Token::from).collect()
    }
}
