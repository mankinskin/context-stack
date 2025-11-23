//! Movement trait implementations (MoveRootIndex, MovePath)

use std::ops::ControlFlow;

use crate::{
    PathNode,
    direction::{
        Direction,
        Left,
        Right,
        pattern::PatternDirection,
    },
    graph::vertex::location::child::ChildLocation,
    path::{
        accessors::{
            role::End,
            root::RootPattern,
        },
        mutators::{
            append::PathAppend,
            move_path::{
                leaf::MoveLeaf,
                path::MovePath,
                root::MoveRootIndex,
            },
            pop::PathPop,
        },
        structs::{
            rooted::{
                role_path::RootedRolePath,
                root::PathRoot,
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::has_graph::{
        HasGraph,
        TravDir,
    },
};

use super::IndexRangePath;
use crate::path::structs::rooted::role_path::{
    HasRootChildIndex,
    HasRootChildIndexMut,
};

impl<EndNode: PathNode> MoveRootIndex<Right, End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn move_root_index<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = self.root_pattern::<G>(&graph);
        let current_index = HasRootChildIndex::<End>::root_child_index(self);
        if let Some(next) =
            TravDir::<G>::pattern_index_next(pattern, current_index)
        {
            tracing::debug!(
                "IndexRangePath::move_root_index - advancing end.root_entry from {} to {}",
                current_index,
                next
            );
            *self.root_child_index_mut() = next;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl<EndNode: PathNode> MoveRootIndex<Left, End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn move_root_index<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = self.root_pattern::<G>(&graph);
        if let Some(prev) = TravDir::<G>::pattern_index_prev(
            pattern,
            HasRootChildIndex::<End>::root_child_index(self),
        ) {
            *self.root_child_index_mut() = prev;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl<D: Direction, Root: PathRoot> MovePath<D, End>
    for crate::path::structs::rooted::RootedRangePath<
        Root,
        ChildLocation,
        ChildLocation,
    >
where
    Self: MoveRootIndex<D> + PathAppend,
    ChildLocation: MoveLeaf<D>,
{
    type Node = ChildLocation;

    fn path_pop_node(&mut self) -> Option<Self::Node> {
        PathPop::<ChildLocation>::path_pop(self)
    }

    fn move_path_segment<G: HasGraph>(
        &mut self,
        location: &mut ChildLocation,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()> {
        location.move_leaf(trav)
    }
}

// MovePath implementation for position-annotated paths
impl<D: Direction, Root: PathRoot> MovePath<D, End>
    for crate::path::structs::rooted::RootedRangePath<
        Root,
        ChildLocation,
        PositionAnnotated<ChildLocation>,
    >
where
    Self: MoveRootIndex<D> + PathAppend,
    ChildLocation: MoveLeaf<D>,
{
    type Node = PositionAnnotated<ChildLocation>;

    fn path_pop_node(&mut self) -> Option<Self::Node> {
        PathPop::<PositionAnnotated<ChildLocation>>::path_pop(self)
    }

    fn move_path_segment<G: HasGraph>(
        &mut self,
        node: &mut PositionAnnotated<ChildLocation>,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()> {
        // Move the underlying ChildLocation within the annotation
        // Note: Position should be updated when entry_pos is updated in ChildState
        node.node.move_leaf(trav)
    }
}

// PathPop implementation for position-annotated paths
impl<Root: PathRoot> PathPop<PositionAnnotated<ChildLocation>>
    for crate::path::structs::rooted::RootedRangePath<
        Root,
        ChildLocation,
        PositionAnnotated<ChildLocation>,
    >
{
    fn path_pop(&mut self) -> Option<PositionAnnotated<ChildLocation>> {
        self.end.sub_path.path.pop()
    }
}

impl<D: Direction, Root: PathRoot> MovePath<D, End>
    for RootedRolePath<End, Root>
where
    Self: MoveRootIndex<D> + PathAppend,
    ChildLocation: MoveLeaf<D>,
{
    type Node = ChildLocation;

    fn path_pop_node(&mut self) -> Option<Self::Node> {
        PathPop::<ChildLocation>::path_pop(self)
    }

    fn move_path_segment<G: HasGraph>(
        &mut self,
        location: &mut ChildLocation,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()> {
        location.move_leaf(trav)
    }
}
