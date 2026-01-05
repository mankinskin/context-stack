use std::{
    fmt::Debug,
    ops::{
        Range,
        RangeBounds,
    },
};

use crate::{
    Pattern,
    Token,
    graph::{
        getters::ErrorReason,
        vertex::PatternId,
    },
};

pub(crate) fn get_child_pattern_range<'a, R: PatternRangeIndex>(
    id: &PatternId,
    p: &'a Pattern,
    range: R,
) -> Result<&'a [Token], ErrorReason> {
    p.get(range.clone().into()).ok_or_else(|| {
        ErrorReason::InvalidPatternRange(
            *id,
            p.clone(),
            format!("{:#?}", range),
        )
    })
}

pub trait RangeIndex<T = Token>:
    RangeBounds<usize> + Debug + Clone + Send + Sync
{
}
impl<T, R: RangeBounds<usize> + Debug + Clone + Send + Sync> RangeIndex<T>
    for R
{
}
pub trait PatternRangeIndex<T = Token>:
    RangeIndex<usize>
    + Into<Range<usize>>
    + Iterator<Item = usize>
    + ExactSizeIterator
{
}

impl<
    T,
    R: RangeIndex<usize>
        + Into<Range<usize>>
        + Iterator<Item = usize>
        + ExactSizeIterator,
> PatternRangeIndex<T> for R
{
}
#[allow(dead_code)]
pub(crate) trait StartInclusive {
    fn start(&self) -> usize;
}
impl StartInclusive for std::ops::RangeInclusive<usize> {
    fn start(&self) -> usize {
        *self.start()
    }
}
impl StartInclusive for std::ops::RangeTo<usize> {
    fn start(&self) -> usize {
        0
    }
}
impl StartInclusive for std::ops::RangeFrom<usize> {
    fn start(&self) -> usize {
        self.start
    }
}
impl StartInclusive for std::ops::Range<usize> {
    fn start(&self) -> usize {
        self.start
    }
}
#[allow(dead_code)]
pub(crate) trait EndInclusive {
    fn end(&self) -> usize;
}
impl EndInclusive for std::ops::RangeInclusive<usize> {
    fn end(&self) -> usize {
        *self.end()
    }
}
impl EndInclusive for std::ops::RangeToInclusive<usize> {
    fn end(&self) -> usize {
        self.end
    }
}
impl EndInclusive for std::ops::Range<usize> {
    fn end(&self) -> usize {
        self.end
    }
}
