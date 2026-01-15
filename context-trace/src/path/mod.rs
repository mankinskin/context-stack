use std::fmt::Debug;

use accessors::{
    child::{
        HasLeafToken,
        HasRootedLeafToken,
        HasRootedLeafTokenLocation,
        root::GraphRootChild,
    },
    has_path::HasRolePath,
    role::PathRole,
};

use crate::{
    PathNode,
    TokenWidth,
    direction::pattern::PatternDirection,
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        structs::{
            role_path::RolePath,
            rooted::role_path::{
                HasRootChildIndex,
                HasRootChildToken,
            },
        },
    },
    trace::has_graph::HasGraph,
};

pub mod accessors;
pub mod mutators;
pub mod structs;

pub trait BaseQuery:
    Debug + Clone + PartialEq + Eq + Send + Sync + 'static
{
}

impl<T: Debug + Clone + PartialEq + Eq + Send + Sync + 'static> BaseQuery
    for T
{
}

pub trait BasePath:
    Debug + Sized + Clone + PartialEq + Eq + Send + Sync + Unpin + 'static
{
}

impl<T: Debug + Sized + Clone + PartialEq + Eq + Send + Sync + Unpin + 'static>
    BasePath for T
{
}

pub trait RolePathUtils {
    fn role_leaf_token_location<R: PathRole>(&self) -> Option<ChildLocation>
    where
        Self: HasLeafToken<R>,
    {
        HasLeafToken::<R>::leaf_token_location(self)
    }
    fn role_leaf_token<R: PathRole, G: HasGraph>(
        &self,
        trav: &G,
    ) -> Option<Token>
    where
        Self: HasLeafToken<R>,
    {
        HasLeafToken::<R>::leaf_token(self, trav)
    }
    fn role_rooted_leaf_token<R: PathRole, G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token
    where
        Self: HasRootedLeafToken<R>,
    {
        HasRootedLeafToken::<R>::rooted_leaf_token(self, trav)
    }
    fn role_rooted_leaf_token_location<R: PathRole>(&self) -> ChildLocation
    where
        Self: HasRootedLeafTokenLocation<R>,
    {
        HasRootedLeafTokenLocation::<R>::rooted_leaf_token_location(self)
    }
    fn role_root_child_token<R: PathRole, G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token
    where
        Self: HasRootChildToken<R>,
    {
        HasRootChildToken::<R>::root_child_token(self, trav)
    }
    fn role_root_child_index<R: PathRole>(&self) -> usize
    where
        Self: HasRootChildIndex<R>,
    {
        HasRootChildIndex::<R>::root_child_index(self)
    }
    fn role_root_child_location<R: PathRole>(&self) -> ChildLocation
    where
        Self: GraphRootChild<R>,
    {
        GraphRootChild::<R>::graph_root_child_location(self)
    }

    fn role_outer_width<G: HasGraph, R: PathRole>(
        &self,
        trav: &G,
    ) -> TokenWidth
    where
        Self: GraphRootChild<R>,
    {
        self.get_outer_width(trav)
    }
    fn role_inner_width<G: HasGraph, R: PathRole>(
        &self,
        trav: &G,
    ) -> TokenWidth
    where
        Self: GraphRootChild<R>,
    {
        self.get_inner_width(trav)
    }
    fn is_at_border<G: HasGraph, R: PathRole>(
        &self,
        trav: G,
    ) -> bool
    where
        Self: GraphRootChild<R>,
    {
        let graph = trav.graph();
        let location = self.role_root_child_location::<R>();
        let pattern = graph.expect_pattern_at(location);
        <R::BorderDirection as PatternDirection>::pattern_index_next(
            &pattern,
            location.sub_index,
        )
        .is_none()
    }
    fn child_path_mut<R: PathRole, Node: PathNode>(
        &mut self
    ) -> &mut RolePath<R, Node>
    where
        Self: HasRolePath<R, Node = Node>,
    {
        self.role_path_mut()
    }
    fn child_pos<R: PathRole>(&self) -> usize
    where
        Self: HasRolePath<R>,
    {
        self.role_path().root_child_index()
    }
    fn raw_child_path<R: PathRole>(&self) -> &Vec<ChildLocation>
    where
        Self: HasRolePath<R, Node = ChildLocation>,
    {
        self.role_path().path()
    }
    fn raw_child_path_mut<R: PathRole>(&mut self) -> &mut Vec<ChildLocation>
    where
        Self: HasRolePath<R, Node = ChildLocation>,
    {
        self.role_path_mut().path_mut()
    }
}

impl<T> RolePathUtils for T {}
