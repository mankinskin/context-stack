use crate::{
    direction::Direction,
    path::{
        accessors::role::PathRole,
        mutators::{
            append::PathAppend,
            move_path::root::MoveRootIndex,
        },
        structs::rooted::{
            IntoChildLocation,
            PathNode,
        },
    },
    trace::has_graph::HasGraph,
};
use std::ops::ControlFlow;

pub trait MovePath<D: Direction, R: PathRole>:
    PathAppend + MoveRootIndex<D, R>
{
    type Node: PathNode;

    fn path_pop_node(&mut self) -> Option<Self::Node>;

    fn move_path_segment<G: HasGraph>(
        &mut self,
        node: &mut Self::Node,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()>;

    fn move_path<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()>
    where
        Self::Node: PathNode,
    {
        let graph = trav.graph();
        if let Some(node) = std::iter::from_fn(|| {
            self.path_pop_node().map(|mut node| {
                self.move_path_segment::<G>(&mut node, &graph)
                    .is_continue()
                    .then_some(node)
            })
        })
        .find_map(|node| node)
        {
            // Convert node to ChildLocation for PathAppend
            let location = node.into_child_location();
            self.path_append(location);
            ControlFlow::Continue(())
        } else {
            self.move_root_index(trav)
        }
    }
}
