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
        structs::rooted::role_path::RootChildIndex,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Deref, DerefMut)]
pub struct SubPath {
    pub(crate) root_entry: usize,
    #[deref]
    #[deref_mut]
    pub(crate) path: Vec<ChildLocation>,
}

impl SubPath {
    pub fn new_empty(root_entry: usize) -> Self {
        Self {
            root_entry,
            path: Default::default(),
        }
    }
    pub fn new(
        root_entry: usize,
        path: Vec<ChildLocation>,
    ) -> Self {
        Self { root_entry, path }
    }
    //pub(crate) fn pop_while(
    //    &mut self,
    //    condition: impl Fn(&ChildLocation) -> bool,
    //) {
    //    while self.path.last().map(&condition).unwrap_or_default() {
    //        self.path.pop();
    //    }
    //}
}
impl<R: PathRole> RootChildIndex<R> for SubPath {
    fn root_child_index(&self) -> usize {
        self.root_entry
    }
}

impl fmt::Display for SubPath {
    fn fmt(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "[]")
        } else {
            write!(f, "[")?;
            for (i, loc) in self.path.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", loc)?;
            }
            write!(f, "]")
        }
    }
}
