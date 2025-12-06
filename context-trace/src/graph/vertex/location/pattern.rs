use std::{
    cmp::PartialEq,
    fmt::Debug,
    ops::Range,
};

use crate::{
    HasGraph,
    HasPatternId,
    graph::vertex::{
        PatternId,
        location::{
            ChildLocation,
            HasParent,
        },
        pattern::Pattern,
    },
};

use crate::graph::vertex::token::Token;

#[allow(dead_code)]
pub(crate) struct PatternRangeLocation {
    pub parent: Token,
    pub id: PatternId,
    pub range: Range<usize>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Copy)]
pub struct PatternLocation {
    pub parent: Token,
    pub pattern_id: PatternId,
}

impl std::fmt::Display for PatternLocation {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // Token's Display already handles string representation if available
        write!(
            f,
            "PatternLocation({}, {})",
            self.parent,
            &format!("{}", self.pattern_id)[..8]
        )
    }
}

impl HasPatternId for PatternLocation {
    fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }
}
impl HasParent for PatternLocation {
    fn parent(&self) -> &Token {
        &self.parent
    }
}
#[allow(dead_code)]
impl PatternLocation {
    pub fn new(
        parent: Token,
        pattern_id: PatternId,
    ) -> Self {
        Self { parent, pattern_id }
    }
}
#[allow(dead_code)]
impl PatternLocation {
    pub fn to_child_location(
        self,
        sub_index: usize,
    ) -> ChildLocation {
        ChildLocation {
            parent: self.parent,
            pattern_id: self.pattern_id,
            sub_index,
        }
    }
    pub(crate) fn with_range(
        self,
        range: Range<usize>,
    ) -> PatternRangeLocation {
        PatternRangeLocation {
            parent: self.parent,
            id: self.pattern_id,
            range,
        }
    }
    pub(crate) fn get_pattern<'a, Trav: HasGraph + 'a>(
        &'a self,
        trav: &'a Trav,
    ) -> Option<Pattern> {
        trav.graph().get_pattern_at(self).ok().cloned()
    }
    pub(crate) fn expect_pattern<'a, Trav: HasGraph + 'a>(
        &'a self,
        trav: &'a Trav,
    ) -> Pattern {
        trav.graph().expect_pattern_at(self).clone()
    }
    pub(crate) fn get_pattern_in<'a>(
        &self,
        patterns: &'a crate::graph::vertex::ChildPatterns,
    ) -> Option<&'a Pattern> {
        patterns.get(&self.pattern_id)
    }
    pub(crate) fn expect_pattern_in<'a>(
        &self,
        patterns: &'a crate::graph::vertex::ChildPatterns,
    ) -> &'a Pattern {
        self.get_pattern_in(patterns)
            .expect("Expected Pattern not present in TokenPatterns!")
    }
}

pub trait IntoPatternLocation {
    fn into_pattern_location(self) -> PatternLocation;
}

impl<P: IntoPatternLocation + Copy> IntoPatternLocation for &'_ P {
    fn into_pattern_location(self) -> PatternLocation {
        (*self).into_pattern_location()
    }
}
impl IntoPatternLocation for PatternLocation {
    fn into_pattern_location(self) -> PatternLocation {
        self
    }
}

pub trait HasPatternLocation {
    fn pattern_location(&self) -> &PatternLocation;
}
