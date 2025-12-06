use std::{
    cmp::PartialEq,
    fmt::Debug,
    ops::Range,
};

use child::*;
use pattern::*;

use crate::{
    PatternId,
    Token,
};

pub mod child;
pub mod pattern;

pub trait HasParent {
    fn parent(&self) -> &Token;
}
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
pub struct SubLocation {
    pub(crate) pattern_id: PatternId,
    pub(crate) sub_index: usize,
}

impl SubLocation {
    pub fn new(
        pattern_id: PatternId,
        sub_index: usize,
    ) -> Self {
        Self {
            pattern_id,
            sub_index,
        }
    }
}
impl HasSubIndex for SubLocation {
    fn sub_index(&self) -> usize {
        self.sub_index
    }
}

impl From<ChildLocation> for SubLocation {
    fn from(value: ChildLocation) -> Self {
        value.to_sub_location()
    }
}

pub struct SubRangeLocation {
    pub pattern_id: PatternId,
    pub sub_range: Range<usize>,
}

impl SubRangeLocation {
    pub fn new(
        pattern_id: PatternId,
        sub_range: Range<usize>,
    ) -> Self {
        Self {
            pattern_id,
            sub_range,
        }
    }
}
