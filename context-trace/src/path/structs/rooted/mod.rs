pub(crate) mod index_range;
pub(crate) mod pattern_range;
pub(crate) mod role_path;
pub(crate) mod root;
//pub(crate) mod split_path;

use crate::{
    ChildLocation,
    EndPath,
    HasEndPath,
    HasPath,
    HasStartPath,
    IntoRootedRolePath,
    RootChildIndex,
    StartPath,
    graph::vertex::{
        location::pattern::PatternLocation,
        pattern::Pattern,
        token::Token,
    },
    path::{
        accessors::{
            child::RootedLeafToken,
            has_path::HasRolePath,
            role::{
                End,
                Start,
            },
            root::{
                GraphRoot,
                GraphRootPattern,
                RootPattern,
            },
        },
        mutators::move_path::key::AtomPosition,
        structs::{
            role_path::RolePath,
            rooted::role_path::{
                RootedEndPath,
                RootedStartPath,
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::has_graph::HasGraph,
};
use root::{
    IndexRoot,
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

/// Trait for extracting ChildLocation from path nodes
pub trait IntoChildLocation {
    fn into_child_location(self) -> ChildLocation;
    fn as_child_location(&self) -> ChildLocation;
}

impl IntoChildLocation for ChildLocation {
    fn into_child_location(self) -> ChildLocation {
        self
    }
    fn as_child_location(&self) -> ChildLocation {
        *self
    }
}

impl IntoChildLocation for PositionAnnotated<ChildLocation> {
    fn into_child_location(self) -> ChildLocation {
        self.node
    }
    fn as_child_location(&self) -> ChildLocation {
        self.node
    }
}

pub trait PathNode:
    std::fmt::Debug + Clone + PartialEq + Eq + IntoChildLocation
{
}
impl<T: std::fmt::Debug + Clone + PartialEq + Eq + IntoChildLocation> PathNode
    for T
{
}

#[derive(Clone, PartialEq, Eq)]
pub struct RootedRangePath<
    Root: PathRoot,
    StartNode = ChildLocation,
    EndNode = ChildLocation,
> {
    pub(crate) root: Root,
    pub(crate) start: RolePath<Start, StartNode>,
    pub(crate) end: RolePath<End, EndNode>,
}
impl<Root: PathRoot, StartNode, EndNode>
    RootedRangePath<Root, StartNode, EndNode>
{
    pub fn new(
        root: impl Into<Root>,
        start: RolePath<Start, StartNode>,
        end: RolePath<End, EndNode>,
    ) -> Self {
        Self {
            root: root.into(),
            start,
            end,
        }
    }
    pub fn end_path(&self) -> &RolePath<End, EndNode> {
        &self.end
    }
    pub fn start_path(&self) -> &RolePath<Start, StartNode> {
        &self.start
    }
    pub fn start_path_mut(&mut self) -> &mut RolePath<Start, StartNode> {
        &mut self.start
    }
    pub fn end_path_mut(&mut self) -> &mut RolePath<End, EndNode> {
        &mut self.end
    }
    pub fn new_path<O: PathRoot>(
        root: impl Into<Root>,
        path: impl Into<RootedRangePath<O, StartNode, EndNode>>,
    ) -> Self {
        let path = path.into();
        Self::new(root, path.start, path.end)
    }
    pub fn new_empty(root: Root) -> Self
    where
        RolePath<Start, StartNode>: Default,
        RolePath<End, EndNode>: Default,
    {
        Self::new(root, Default::default(), Default::default())
    }
}

impl<Root: PathRoot, StartNode>
    RootedRangePath<Root, StartNode, PositionAnnotated<ChildLocation>>
{
    /// Get the entry position from the end path (position when entering this range)
    pub fn end_entry_position(&self) -> Option<AtomPosition> {
        self.end.entry_position()
    }
}
impl<R: PathRoot, StartNode, EndNode> RootedPath
    for RootedRangePath<R, StartNode, EndNode>
{
    type Root = R;
    fn path_root(&self) -> Self::Root {
        self.root.clone()
    }
}
impl<R: PathRoot> From<RootedEndPath<R>>
    for RootedRangePath<R, ChildLocation, ChildLocation>
{
    fn from(value: RootedEndPath<R>) -> Self {
        // The EndPath points to the token we want to start from
        // In RangePath, end points to the token after what's been consumed
        // So if EndPath.root_entry = 0 (start from first token),
        // RangePath.end should = 1 (first token consumed, now at second)
        let end_index = value.role_path.root_child_index() + 1;
        Self {
            root: value.root,
            start: Default::default(),
            end: RolePath::new_empty(end_index),
        }
    }
}
impl<R: PathRoot, EndNode> From<RootedStartPath<R>>
    for RootedRangePath<R, ChildLocation, EndNode>
{
    fn from(value: RootedStartPath<R>) -> Self {
        Self {
            end: RolePath::new_empty(value.role_path.root_child_index()),
            start: value.role_path,
            root: value.root,
        }
    }
}
impl<Root: PathRoot, EndNode> HasStartPath
    for RootedRangePath<Root, ChildLocation, EndNode>
where
    RootedRangePath<Root, ChildLocation, EndNode>: HasPath<Start>,
{
    fn start_path(&self) -> &StartPath {
        &self.start
    }
    fn start_path_mut(&mut self) -> &mut StartPath {
        &mut self.start
    }
}
impl<Root: PathRoot, StartNode> HasEndPath
    for RootedRangePath<Root, StartNode, ChildLocation>
where
    RootedRangePath<Root, StartNode, ChildLocation>: HasPath<End>,
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

impl<R: PathRoot, EndNode> HasRolePath<Start>
    for RootedRangePath<R, ChildLocation, EndNode>
{
    type Node = ChildLocation;
    fn role_path(&self) -> &RolePath<Start, ChildLocation> {
        &self.start
    }
    fn role_path_mut(&mut self) -> &mut RolePath<Start, ChildLocation> {
        &mut self.start
    }
}

// Generic implementation for all EndNode types
impl<R: PathRoot, StartNode, EndNode> HasRolePath<End>
    for RootedRangePath<R, StartNode, EndNode>
{
    type Node = EndNode;
    fn role_path(&self) -> &RolePath<End, EndNode> {
        &self.end
    }
    fn role_path_mut(&mut self) -> &mut RolePath<End, EndNode> {
        &mut self.end
    }
}

// Tier 2 trait implementations: Concrete role accessors
impl<R: PathRoot, EndNode> crate::path::accessors::range_accessor::StartPathAccessor
    for RootedRangePath<R, ChildLocation, EndNode>
{
    type Node = ChildLocation;
    
    fn start_path(&self) -> &RolePath<Start, ChildLocation> {
        &self.start
    }
    
    fn start_path_mut(&mut self) -> &mut RolePath<Start, ChildLocation> {
        &mut self.start
    }
}

impl<R: PathRoot, StartNode> crate::path::accessors::range_accessor::EndPathAccessor
    for RootedRangePath<R, StartNode, ChildLocation>
{
    type Node = ChildLocation;
    
    fn end_path(&self) -> &RolePath<End, ChildLocation> {
        &self.end
    }
    
    fn end_path_mut(&mut self) -> &mut RolePath<End, ChildLocation> {
        &mut self.end
    }
}

// RangePathAccessor automatically implemented via blanket impl

impl<EndNode> GraphRoot for RootedRangePath<IndexRoot, ChildLocation, EndNode> {
    fn root_parent(&self) -> Token {
        self.root.location.parent
    }
}

impl<EndNode> GraphRootPattern
    for RootedRangePath<IndexRoot, ChildLocation, EndNode>
{
    fn root_pattern_location(&self) -> PatternLocation {
        self.root.location
    }
}

impl<EndNode> RootPattern
    for RootedRangePath<IndexRoot, ChildLocation, EndNode>
{
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> &'g Pattern {
        self.root.root_pattern::<G>(trav)
    }
}

// Display implementation using compact format with Display trait for nested types
impl<R: PathRoot + fmt::Display> fmt::Display
    for RootedRangePath<R, ChildLocation, ChildLocation>
{
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

// Generic Debug implementation for all RootedRangePath types
impl<R, StartNode, EndNode> fmt::Debug
    for RootedRangePath<R, StartNode, EndNode>
where
    R: PathRoot + fmt::Debug,
    StartNode: fmt::Debug,
    EndNode: fmt::Debug,
    RolePath<Start, StartNode>: fmt::Debug,
    RolePath<End, EndNode>: fmt::Debug,
{
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("RootedRangePath")
            .field("root", &self.root)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}
