pub(crate) mod root;

use crate::{
    GraphRootChild,
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        accessors::{
            has_path::HasChildPath,
            role::PathRole,
        },
        structs::rooted::role_path::{
            HasRootChildIndex,
            HasRootChildToken,
        },
    },
    trace::has_graph::HasGraph,
};

/// Access to the leaf token of a path (the token at the path's end point)
///
/// This trait provides methods to access the leaf token's location and value.
pub trait HasLeafToken<R: PathRole>:
    HasRootChildIndex<R> + HasChildPath<R, Node = ChildLocation>
{
    fn leaf_token_location_mut(&mut self) -> Option<&mut ChildLocation> {
        R::bottom_up_iter(self.child_path_mut().iter_mut()).next()
    }
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        R::bottom_up_iter(self.child_path().iter()).next().cloned() as Option<_>
    }
    fn leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Option<Token> {
        self.leaf_token_location()
            .map(|loc| trav.graph().expect_child_at(loc))
    }
}

/// Access to the leaf token with a guaranteed root fallback
pub trait HasRootedLeafTokenLocation<R: PathRole>:
    HasLeafToken<R> + GraphRootChild<R>
{
    fn rooted_leaf_token_location(&self) -> ChildLocation;
}

/// Access to the leaf token value with a guaranteed root fallback
pub trait HasRootedLeafToken<R: PathRole>:
    HasLeafToken<R> + HasRootChildToken<R>
{
    fn rooted_leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token;
}

impl<R: PathRole, T: HasLeafToken<R> + GraphRootChild<R>>
    HasRootedLeafTokenLocation<R> for T
{
    fn rooted_leaf_token_location(&self) -> ChildLocation {
        self.leaf_token_location()
            .unwrap_or_else(|| self.graph_root_child_location())
    }
}

impl<R: PathRole, T: HasLeafToken<R> + HasRootChildToken<R>>
    HasRootedLeafToken<R> for T
{
    fn rooted_leaf_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        self.leaf_token(trav)
            .unwrap_or_else(|| self.root_child_token(trav))
    }
}

//pub(crate) trait LeafTokenPosMut<R>: HasRootChildIndexMut<R> {
//    fn leaf_token_pos_mut(&mut self) -> &mut usize;
//}
