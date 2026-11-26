use derive_more::{
    Debug,
    Deref,
    DerefMut,
};
use std::fmt;

use crate::{
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::role::PathRole,
        mutators::move_path::key::AtomPosition,
        structs::rooted::role_path::HasRootChildIndex,
    },
};

/// Wrapper type for position-annotated path nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PositionAnnotated<N> {
    pub node: N,
    pub position: AtomPosition,
}

impl<N> PositionAnnotated<N> {
    pub fn new(
        node: N,
        position: AtomPosition,
    ) -> Self {
        Self { node, position }
    }
}

impl<N: fmt::Display> fmt::Display for PositionAnnotated<N> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "{}@{}", self.node, self.position)
    }
}

// CompactFormat implementation for PositionAnnotated - delegates to inner Display
impl<N: fmt::Display> crate::logging::compact_format::CompactFormat
    for PositionAnnotated<N>
{
    fn fmt_compact(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(f, "{}@{}", self.node, self.position)
    }

    fn fmt_indented(
        &self,
        f: &mut fmt::Formatter,
        _indent: usize,
    ) -> fmt::Result {
        write!(f, "{}@{}", self.node, self.position)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct SubPath<N = ChildLocation> {
    pub(crate) root_entry: usize,
    #[deref]
    #[deref_mut]
    pub(crate) path: Vec<N>,
}

impl<N> Default for SubPath<N> {
    fn default() -> Self {
        Self {
            root_entry: 0,
            path: Vec::new(),
        }
    }
}

impl<N> SubPath<N> {
    pub fn new_empty(root_entry: usize) -> Self {
        Self {
            root_entry,
            path: Vec::new(),
        }
    }
    pub fn new(
        root_entry: usize,
        path: Vec<N>,
    ) -> Self {
        Self { root_entry, path }
    }
}

impl SubPath<PositionAnnotated<ChildLocation>> {
    /// Get the entry position (position when first node was added)
    pub fn entry_position(&self) -> Option<AtomPosition> {
        self.path.first().map(|annotated| annotated.position)
    }
}

impl<N> SubPath<N> {
    //pub(crate) fn pop_while(
    //    &mut self,
    //    condition: impl Fn(&ChildLocation) -> bool,
    //) {
    //    while self.path.last().map(&condition).unwrap_or_default() {
    //        self.path.pop();
    //    }
    //}
}
impl<R: PathRole, N> HasRootChildIndex<R> for SubPath<N> {
    fn root_child_index(&self) -> usize {
        self.root_entry
    }
}

impl<N: fmt::Display> fmt::Display for SubPath<N> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "[]")
        } else {
            write!(f, "[")?;
            for (i, node) in self.path.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", node)?;
            }
            write!(f, "]")
        }
    }
}
