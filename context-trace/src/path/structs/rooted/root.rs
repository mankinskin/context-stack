use derive_more::{
    Deref,
    DerefMut,
    derive::From,
};

use crate::{
    Token,
    graph::vertex::{
        location::{
            HasParent,
            child::ChildLocation,
            pattern::{
                HasPatternLocation,
                IntoPatternLocation,
                PatternLocation,
            },
        },
        pattern::Pattern,
    },
    path::accessors::root::RootPattern,
};

#[derive(Clone, Debug, PartialEq, Eq, From, Deref, DerefMut)]
pub struct IndexRoot {
    pub(crate) location: PatternLocation,
}

impl std::fmt::Display for IndexRoot {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Root({})", self.location)
    }
}

impl HasPatternLocation for IndexRoot {
    fn pattern_location(&self) -> &PatternLocation {
        &self.location
    }
}
impl HasParent for IndexRoot {
    fn parent(&self) -> &Token {
        self.location.parent()
    }
}
impl From<IndexRoot> for PatternLocation {
    fn from(value: IndexRoot) -> Self {
        value.location
    }
}
pub trait PathRoot: Clone + RootPattern {}

impl PathRoot for Pattern {}

impl PathRoot for IndexRoot {}

pub trait RootedPath {
    type Root: PathRoot;
    fn path_root(&self) -> Self::Root;
}
impl RootedPath for ChildLocation {
    type Root = IndexRoot;
    fn path_root(&self) -> Self::Root {
        IndexRoot::from(self.into_pattern_location())
    }
}
