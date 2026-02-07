use std::borrow::Borrow;

use context_trace::*;
use derivative::Derivative;
use derive_more::Deref;

pub struct BandCtx<'a> {
    pub band: &'a Band,
    //pub back_link: Option<&'a OverlapLink>,
    //pub front_link: Option<&'a OverlapLink>,
}
impl From<BandCtx<'_>> for Band {
    fn from(band: BandCtx<'_>) -> Self {
        band.band.clone()
    }
}

#[derive(Clone, Debug, Derivative)]
#[derivative(Ord, Eq, PartialEq, PartialOrd)]
pub struct Band {
    pub pattern: Pattern,
    pub start_bound: AtomPosition,
    pub end_bound: AtomPosition, // key for ordering
}
impl Borrow<AtomPosition> for Band {
    fn borrow(&self) -> &AtomPosition {
        &self.end_bound
    }
}
impl Band {
    pub fn last_token(&self) -> Token {
        *self.pattern.last().unwrap()
    }
    pub fn append(
        &mut self,
        postfix: Token,
    ) {
        let width = self.last_token().width();
        self.start_bound += width.0;
        self.end_bound += width.0;
        self.pattern.push(postfix);
    }
}
//impl From<(usize, Band)> for Band {
//    fn from((_, band): (usize, Band)) -> Self {
//        band
//    }
//}
impl From<Token> for Band {
    fn from(first: Token) -> Self {
        Self {
            start_bound: 0.into(),
            end_bound: first.width().0.into(),
            pattern: Pattern::from(vec![first]),
        }
    }
}
impl From<(AtomPosition, Pattern)> for Band {
    fn from((start_bound, pattern): (AtomPosition, Pattern)) -> Self {
        let end_bound: AtomPosition = start_bound + pattern_width(&pattern).0;
        Self {
            pattern,
            start_bound,
            end_bound,
        }
    }
}

#[derive(Clone, Debug, Eq, Derivative, Deref)]
#[derivative(Ord, PartialOrd, PartialEq)]
pub struct Overlap {
    #[deref]
    pub index: Token,
    pub start_bound: usize, // key for ordering
}
impl Overlap {
    pub fn end_bound(&self) -> AtomPosition {
        (self.start_bound + self.width().0).into()
    }
}
