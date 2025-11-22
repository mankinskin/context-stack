pub(crate) mod root;

use crate::{
    GraphRootChild,
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        accessors::{
            has_path::HasPath,
            role::PathRole,
        },
        structs::rooted::role_path::{
            RootChildIndex,
            RootChildToken,
        },
    },
    trace::has_graph::HasGraph,
};

pub trait LeafToken<R: PathRole>:
    RootChildIndex<R> + HasPath<R, Node = ChildLocation>
{
    fn leaf_token_location_mut(&mut self) -> Option<&mut ChildLocation> {
        R::bottom_up_iter(self.path_mut().iter_mut()).next()
    }
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        R::bottom_up_iter(self.path().iter()).next().cloned() as Option<_>
    }
    fn leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Option<Token> {
        self.leaf_token_location()
            .map(|loc| *trav.graph().expect_child_at(loc))
    }
}
pub trait RootedLeafTokenLocation<R: PathRole>:
    LeafToken<R> + GraphRootChild<R>
{
    fn rooted_leaf_token_location(&self) -> ChildLocation;
}
pub trait RootedLeafToken<R: PathRole>:
    LeafToken<R> + RootChildToken<R>
{
    fn rooted_leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token;
}
//impl<R: PathRole, T: LeafToken<R> + GraphRootChild<R>> RootedLeafToken<R>
//    for T
//{
//    fn rooted_leaf_token<G: HasGraph>(
//        &self,
//        trav: &G,
//    ) -> Token {
//        self.leaf_token(trav)
//            .unwrap_or_else(|| self.graph_root_child(trav))
//    }
//    fn rooted_leaf_token_location(&self) -> ChildLocation {
//        self.leaf_token_location()
//            .unwrap_or_else(|| self.graph_root_child_location())
//    }
//}
impl<R: PathRole, T: LeafToken<R> + GraphRootChild<R>>
    RootedLeafTokenLocation<R> for T
{
    fn rooted_leaf_token_location(&self) -> ChildLocation {
        self.leaf_token_location()
            .unwrap_or_else(|| self.graph_root_child_location())
    }
}
impl<R: PathRole, T: LeafToken<R> + RootChildToken<R>> RootedLeafToken<R>
    for T
{
    fn rooted_leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        self.leaf_token(trav)
            .unwrap_or_else(|| self.root_child_token(trav))
    }
}

//pub(crate) trait LeafTokenPosMut<R>: RootChildIndexMut<R> {
//    fn leaf_token_pos_mut(&mut self) -> &mut usize;
//}
