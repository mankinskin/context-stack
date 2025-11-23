//! Type definition and basic conversions for IndexRangePath

use crate::{
    direction::Right,
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::{
            has_path::IntoRolePath,
            role::{
                End,
                PathRole,
                Start,
            },
        },
        structs::{
            role_path::RolePath,
            rooted::{
                RootedRangePath,
                root::IndexRoot,
            },
        },
    },
};

/// Type alias for range paths rooted at an index in a pattern
pub type IndexRangePath<StartNode = ChildLocation, EndNode = ChildLocation> =
    RootedRangePath<IndexRoot, StartNode, EndNode>;

impl From<IndexRoot> for IndexRangePath {
    fn from(value: IndexRoot) -> Self {
        Self {
            root: value,
            start: Default::default(),
            end: Default::default(),
        }
    }
}

impl IntoRolePath<End> for IndexRangePath {
    fn into_role_path(self) -> RolePath<End> {
        self.end
    }
}

impl IntoRolePath<Start> for IndexRangePath {
    fn into_role_path(self) -> RolePath<Start> {
        self.start
    }
}
