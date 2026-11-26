use crate::graph::vertex::{
    token::Token,
    pattern::Pattern,
};

pub(crate) trait Merge {
    fn split_front(self) -> Option<(Token, Pattern)>;
    fn split_back(self) -> Option<(Token, Pattern)>;
}

impl Merge for Token {
    fn split_front(self) -> Option<(Token, Pattern)> {
        Some((self, vec![]))
    }
    fn split_back(self) -> Option<(Token, Pattern)> {
        Some((self, vec![]))
    }
}

impl Merge for Pattern {
    fn split_front(self) -> Option<(Token, Pattern)> {
        let mut p = self.into_iter();
        let first = p.next();
        first.map(|last| (last, p.collect()))
    }
    fn split_back(mut self) -> Option<(Token, Pattern)> {
        let last = self.pop();
        last.map(|last| (last, self))
    }
}
