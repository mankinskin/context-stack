use crate::{
    PositionAnnotated,
    TokenWidth,
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        accessors::{
            role::PathRole,
            root::{
                GraphRootPattern,
                PatternRoot,
            },
        },
        structs::rooted::{
            role_path::{
                HasRootChildIndex,
            },
            root::RootedPath,
        },
    },
    trace::has_graph::HasGraph,
};

#[macro_export]
macro_rules! impl_root_child_token {
    {
        RootChildToken for $target:ty, $self_:ident, $trav:ident => $func:expr
    } => {
        impl<R: PathRole> $crate::path::structs::rooted::role_path::HasRootChildToken<R> for $target
            where $target: $crate::path::structs::rooted::role_path::HasRootChildIndex<R>
        {
            fn root_child_token<
                G: HasGraph,
            >(& $self_, $trav: &G) -> $crate::graph::vertex::token::Token {
                $func
            }
        }
    };
}

/// used to get a direct token in a Graph
pub trait GraphRootChild<R: PathRole>: RootedPath + GraphRootPattern {
    fn graph_root_child_location(&self) -> ChildLocation;
    fn graph_root_child<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        *trav.graph().expect_child_at(
            <_ as GraphRootChild<R>>::graph_root_child_location(self),
        )
    }
    fn get_outer_width<G: HasGraph>(
        &self,
        trav: &G,
    ) -> TokenWidth {
        let i = self.graph_root_child_location().sub_index;
        let g = trav.graph();
        let p = self.graph_root_pattern::<G>(&g);
        TokenWidth(R::outer_ctx_width(p, i))
    }
    fn get_inner_width<G: HasGraph>(
        &self,
        trav: &G,
    ) -> TokenWidth {
        let i = self.graph_root_child_location().sub_index;
        let g = trav.graph();
        let p = self.graph_root_pattern::<G>(&g);
        TokenWidth(R::inner_width(p, i))
    }
}
impl<R: PathRole> GraphRootChild<R> for ChildLocation {
    fn graph_root_child_location(&self) -> ChildLocation {
        *self
    }
}
impl<R: PathRole, T: GraphRootChild<R>> GraphRootChild<R> for PositionAnnotated<T> {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.node.graph_root_child_location()
    }
}
// used to get a direct token of a pattern
pub(crate) trait PatternRootChild<R>:
    HasRootChildIndex<R> + PatternRoot
{
    fn pattern_root_child(&self) -> Token {
        PatternRoot::pattern_root_pattern(self)[self.root_child_index()]
    }
}
