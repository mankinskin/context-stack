pub(crate) mod range;

use auto_impl::auto_impl;
use derive_more::{
    Deref,
    DerefMut,
};

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Deref, DerefMut)]
pub struct RootedRolePath<R: PathRole, Root: PathRoot, N = ChildLocation> {
    pub(crate) root: Root,
    #[deref]
    #[deref_mut]
    pub(crate) role_path: RolePath<R, N>,
}

impl<Root: PathRoot + Default, R: PathRole, N> Default
    for RootedRolePath<R, Root, N>
{
    fn default() -> Self {
        Self {
            root: Root::default(),
            role_path: RolePath::default(),
        }
    }
}
impl<Root: PathRoot, R: PathRole, N> RootedRolePath<R, Root, N> {
    pub fn new(
        root: impl Into<Root>,
        role_path: RolePath<R, N>,
    ) -> Self {
        Self {
            root: root.into(),
            role_path,
        }
    }
}
impl<Root: PathRoot, R: PathRole, N> From<Root> for RootedRolePath<R, Root, N> {
    fn from(root: Root) -> Self {
        Self {
            root,
            role_path: Default::default(),
        }
    }
}

impl<R: PathRole, Root: PathRoot> HasChildPath<R>
    for RootedRolePath<R, Root, ChildLocation>
where
    Self: HasRolePath<R, Node = ChildLocation>,
{
    type Node = ChildLocation;
    fn child_path(&self) -> &Vec<ChildLocation> {
        self.role_path().path()
    }
    fn child_path_mut(&mut self) -> &mut Vec<ChildLocation> {
        self.role_path_mut().path_mut()
    }
}

impl<Root: PathRoot> HasStartPath for RootedStartPath<Root> {
    fn start_path(&self) -> &StartPath {
        &self.role_path
    }
    fn start_path_mut(&mut self) -> &mut StartPath {
        &mut self.role_path
    }
}
impl<Root: PathRoot> HasEndPath for RootedEndPath<Root> {
    fn end_path(&self) -> &EndPath {
        &self.role_path
    }
    fn end_path_mut(&mut self) -> &mut EndPath {
        &mut self.role_path
    }
}

impl<R: PathRole, Root: PathRoot> HasRolePath<R> for RootedRolePath<R, Root> {
    type Node = ChildLocation;
    fn role_path(&self) -> &RolePath<R, ChildLocation> {
        &self.role_path
    }
    fn role_path_mut(&mut self) -> &mut RolePath<R, ChildLocation> {
        &mut self.role_path
    }
}

// New PathAccessor trait implementation
impl<R: PathRole, Root: PathRoot>
    crate::path::accessors::path_accessor::PathAccessor
    for RootedRolePath<R, Root, ChildLocation>
{
    type Role = R;
    type Node = ChildLocation;

    fn path(&self) -> &Vec<ChildLocation> {
        &self.role_path.sub_path.path
    }

    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        &mut self.role_path.sub_path.path
    }
}

// New RootedPathAccessor trait implementation
impl<R: PathRole, Root: PathRoot>
    crate::path::accessors::path_accessor::RootedPathAccessor
    for RootedRolePath<R, Root, ChildLocation>
{
    type Root = Root;

    fn get_root(&self) -> Self::Root {
        self.root.clone()
    }

    fn get_root_mut(&mut self) -> &mut Self::Root {
        &mut self.root
    }
}
impl<R: PathRole, Root: PathRoot> IntoRolePath<R> for RootedRolePath<R, Root> {
    fn into_role_path(self) -> RolePath<R> {
        self.role_path
    }
}
pub(crate) type IndexRolePath<R, N = ChildLocation> =
    RootedRolePath<R, IndexRoot, N>;

pub(crate) type PatternRolePath<R, N = ChildLocation> =
    RootedRolePath<R, Pattern, N>;

pub(crate) type RootedStartPath<R, N = ChildLocation> =
    RootedRolePath<Start, R, N>;
pub(crate) type RootedEndPath<R, N = ChildLocation> = RootedRolePath<End, R, N>;
pub type IndexStartPath = IndexRolePath<Start>;
pub type IndexEndPath = IndexRolePath<End>;
pub type PatternStartPath = PatternRolePath<Start>;
pub type PatternEndPath = PatternRolePath<End>;

impl<R: PathRole> IndexRolePath<R> {
    pub fn new_location(first: ChildLocation) -> Self {
        Self::from(first)
    }
}
impl<R: PathRole> HasLeafToken<R> for IndexRolePath<R>
where
    Self: HasRolePath<R, Node = ChildLocation>
        + HasChildPath<R, Node = ChildLocation>,
{
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        Some(
            R::bottom_up_iter(HasChildPath::<R>::child_path(self).iter())
                .next()
                .cloned()
                .unwrap_or(
                    self.root
                        .location
                        .to_child_location(self.role_path().root_entry),
                ),
        )
    }
    fn leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Option<Token> {
        self.role_path().leaf_token(trav)
    }
}

impl<R: PathRole> From<ChildLocation> for IndexRolePath<R> {
    fn from(first: ChildLocation) -> Self {
        Self {
            role_path: RolePath::new_empty(first.sub_index),
            root: IndexRoot::from(first.into_pattern_location()),
        }
    }
}
impl<Root: PathRoot, R: PathRole> From<(Root, RolePath<R>)>
    for RootedRolePath<R, Root>
{
    fn from((root, role_path): (Root, RolePath<R>)) -> Self {
        Self { root, role_path }
    }
}
impl<R: PathRoot> RootedStartPath<R> {
    pub fn into_range(
        self,
        exit: usize,
    ) -> RootedRangePath<R> {
        RootedRangePath {
            root: self.root,
            start: self.role_path,
            end: RolePath {
                sub_path: SubPath {
                    root_entry: exit,
                    path: vec![],
                },
                _ty: Default::default(),
            },
        }
    }
}
impl<R: PathRoot> RootedEndPath<R> {
    pub fn into_range(
        self,
        entry: usize,
    ) -> RootedRangePath<R> {
        RootedRangePath {
            root: self.root,
            start: RolePath {
                sub_path: SubPath {
                    root_entry: entry,
                    path: vec![],
                },
                _ty: Default::default(),
            },
            end: self.role_path,
        }
    }
}

/// Access to a token at the root child position of a path
#[auto_impl(&, & mut)]
pub trait HasRootChildToken<R> {
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token;
}

/// Access to the index position of a root child in a path
#[auto_impl(&, & mut)]
pub trait HasRootChildIndex<R> {
    fn root_child_index(&self) -> usize;
}

/// Mutable access to the index position of a root child in a path  
pub trait HasRootChildIndexMut<R>: HasRootChildIndex<R> {
    fn root_child_index_mut(&mut self) -> &mut usize;
}

impl<R: PathRole, Root: PathRoot> HasRootChildIndexMut<R>
    for RootedRolePath<R, Root>
{
    fn root_child_index_mut(&mut self) -> &mut usize {
        self.role_path.root_child_index_mut()
    }
}

impl<R: PathRoot> From<RootedRangePath<R>> for RootedRolePath<Start, R> {
    fn from(path: RootedRangePath<R>) -> Self {
        Self {
            root: path.root,
            role_path: path.start,
        }
    }
}
impl<R: PathRoot> From<RootedRangePath<R>> for RootedRolePath<End, R> {
    fn from(path: RootedRangePath<R>) -> Self {
        Self {
            root: path.root,
            role_path: path.end,
        }
    }
}

impl<R: PathRole, Root: PathRoot> RootedPath for RootedRolePath<R, Root> {
    type Root = Root;
    fn path_root(&self) -> Self::Root {
        self.root.clone()
    }
}

impl<R: PathRole> HasLeafToken<R> for RolePath<R> {}

impl_root_child_token! {
    RootChildToken for IndexRolePath<R>, self,
    trav => trav.graph().expect_child_at(
            self.path_root().location.to_child_location(
                HasRootChildIndex::<R>::root_child_index(&self.role_path)
            )
        )
}
impl<R: PathRole> GraphRootChild<R> for RootedRolePath<R, IndexRoot> {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.path_root()
            .location
            .to_child_location(self.role_path.sub_path.root_entry)
    }
}
impl<R: PathRole, Root: PathRoot> HasRootChildIndex<R>
    for RootedRolePath<R, Root>
{
    fn root_child_index(&self) -> usize {
        HasRootChildIndex::<R>::root_child_index(&self.role_path)
    }
}

impl<R: PathRole> GraphRoot for RootedRolePath<R, IndexRoot> {
    fn root_parent(&self) -> Token {
        self.root.location.parent
    }
}

impl<R: PathRole> GraphRootPattern for RootedRolePath<R, IndexRoot> {
    fn root_pattern_location(&self) -> PatternLocation {
        self.root.location
    }
}

//impl<R: PathRole, Root: PathRoot> HasSinglePath for RootedRolePath<R, Root> {
//    fn single_path(&self) -> &[ChildLocation] {
//        self.role_path.sub_path.path.borrow()
//    }
//}

impl<Role: PathRole, Root: PathRoot> RootPattern
    for RootedRolePath<Role, Root>
{
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> Pattern {
        self.root.root_pattern::<G>(trav)
    }
}

//impl_root! { <R: PathRole> PatternRoot for PatternRolePath<R>, self => self.root.borrow() }

//impl RootChildIndex<Start> for PatternEndPath {
//    fn root_child_index(&self) -> usize {
//        0
//    }
//}
//impl<R: PathRole> LeafToken<R> for PatternRolePath<R> where
//    Self: HasChildPath<R> + PatternRootChild<R>
//{
//}
//
//impl<R: PathRole> PatternRootChild<R> for PatternRolePath<R> where
//    PatternEndPath: RootChildIndex<R>
//{
//}

//impl LeafToken<Start> for PatternEndPath {
//    fn path_child_location(&self) -> Option<ChildLocation> {
//        None
//    }
//    fn path_child<G: HasGraph>(
//        &self,
//        trav: &G,
//    ) -> Option<Token> {
//        Some(self.root_child())
//    }
//}
//impl_child! { RootChildToken for PatternRolePath<R>, self, _trav => self.pattern_root_child() }

impl_root_child_token! { RootChildToken for PatternRolePath<R>, self, _trav => self.pattern_root_child() }
