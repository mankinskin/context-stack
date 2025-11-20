use std::{
    cmp::PartialEq,
    fmt::Debug,
};

use child::*;
use pattern::*;

use crate::{
    PatternId,
    Token,
};

pub(crate) mod child;
pub(crate) mod pattern;

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
