pub(crate) mod index_range;
pub(crate) mod pattern_range;
pub(crate) mod role_path;
pub(crate) mod root;
//pub(crate) mod split_path;

use crate::{
    EndPath,
    HasEndPath,
    HasPath,
    HasStartPath,
    IntoRootedRolePath,
    RootChildIndex,
    StartPath,
    path::{
        accessors::{
            child::RootedLeafToken,
            has_path::HasRolePath,
            role::{
                End,
                Start,
            },
        },
        structs::{
            role_path::RolePath,
            rooted::role_path::{
                RootedEndPath,
                RootedStartPath,
            },
        },
    },
};
use root::{
    PathRoot,
    RootedPath,
};
use std::fmt;
pub(crate) trait RangePath:
    RootedPath
    + IntoRootedRolePath<Start>
    + IntoRootedRolePath<End>
    + RootChildIndex<Start>
    + RootChildIndex<End>
    + RootedLeafToken<Start>
    + RootedLeafToken<End>
{
    //fn new_range(
    //    root: Self::Root,
    //    entry: usize,
    //    exit: usize,
    //) -> Self;
}
#[derive(Clone, PartialEq, Eq)]
pub struct RootedRangePath<Root: PathRoot> {
    pub(crate) root: Root,
    pub(crate) start: RolePath<Start>,
    pub(crate) end: RolePath<End>,
}
impl<Root: PathRoot> RootedRangePath<Root> {
    pub fn new(
        root: impl Into<Root>,
        start: RolePath<Start>,
        end: RolePath<End>,
    ) -> Self {
        Self {
            root: root.into(),
            start,
            end,
        }
    }
    pub fn new_path<O: PathRoot>(
        root: impl Into<Root>,
        path: impl Into<RootedRangePath<O>>,
    ) -> Self {
        let path = path.into();
        Self::new(root, path.start, path.end)
    }
    pub fn new_empty(root: Root) -> Self {
        Self::new(root, Default::default(), Default::default())
    }
}
impl<R: PathRoot> RootedPath for RootedRangePath<R> {
    type Root = R;
    fn path_root(&self) -> Self::Root {
        self.root.clone()
    }
}
impl<R: PathRoot> From<RootedEndPath<R>> for RootedRangePath<R> {
    fn from(value: RootedEndPath<R>) -> Self {
        Self {
            root: value.root,
            start: Default::default(),
            end: value.role_path,
        }
    }
}
impl<R: PathRoot> From<RootedStartPath<R>> for RootedRangePath<R> {
    fn from(value: RootedStartPath<R>) -> Self {
        Self {
            end: RolePath::new_empty(value.role_path.root_child_index()),
            start: value.role_path,
            root: value.root,
        }
    }
}
impl<Root: PathRoot> HasStartPath for RootedRangePath<Root>
where
    RootedRangePath<Root>: HasPath<Start>,
{
    fn start_path(&self) -> &StartPath {
        &self.start
    }
    fn start_path_mut(&mut self) -> &mut StartPath {
        &mut self.start
    }
}
impl<Root: PathRoot> HasEndPath for RootedRangePath<Root>
where
    RootedRangePath<Root>: HasPath<End>,
{
    fn end_path(&self) -> &EndPath {
        &self.end
    }
    fn end_path_mut(&mut self) -> &mut EndPath {
        &mut self.end
    }
}
//impl<R: PathRoot> RootedRangePath<R> {
//    pub fn start_path(&self) -> RootedSplitPathRef<'_, R> {
//        RootedSplitPathRef {
//            root: &self.root,
//            sub_path: &self.start.sub_path,
//        }
//    }
//    pub fn end_path(&self) -> RootedSplitPathRef<'_, R> {
//        RootedSplitPathRef {
//            root: &self.root,
//            sub_path: &self.end.sub_path,
//        }
//    }
//}

impl<R: PathRoot> HasRolePath<Start> for RootedRangePath<R> {
    fn role_path(&self) -> &RolePath<Start> {
        &self.start
    }
    fn role_path_mut(&mut self) -> &mut RolePath<Start> {
        &mut self.start
    }
}
impl<R: PathRoot> HasRolePath<End> for RootedRangePath<R> {
    fn role_path(&self) -> &RolePath<End> {
        &self.end
    }
    fn role_path_mut(&mut self) -> &mut RolePath<End> {
        &mut self.end
    }
}

// Display implementation using compact format with Display trait for nested types
impl<R: PathRoot + fmt::Display> fmt::Display for RootedRangePath<R> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if f.alternate() {
            // Pretty print with indentation when using {:#}
            writeln!(f, "RootedRangePath {{")?;
            writeln!(f, "  root: {},", self.root)?;
            writeln!(f, "  start: {},", self.start)?;
            write!(f, "  end: {}", self.end)?;
            writeln!(f)?;
            write!(f, "}}")
        } else {
            // Compact format for {} - no spaces inside braces
            write!(f, "RootedRangePath{{")?;
            write!(f, "root:{},", self.root)?;
            write!(f, "start:{},", self.start)?;
            write!(f, "end:{}", self.end)?;
            write!(f, "}}")
        }
    }
}

impl<R: PathRoot + fmt::Display> fmt::Debug for RootedRangePath<R>
where
    Self: crate::logging::compact_format::CompactFormat,
{
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        // Use CompactFormat's indented format for Debug
        use crate::logging::compact_format::CompactFormat;
        self.fmt_indented(f, 1)
    }
}
