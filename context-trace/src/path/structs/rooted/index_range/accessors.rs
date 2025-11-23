//! Trait implementations for path accessors (HasPath, HasLeafToken, HasRootChildToken, etc.)

use crate::{
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        accessors::{
            child::{
                HasLeafToken,
                root::GraphRootChild,
            },
            has_path::{
                HasPath,
                HasRolePath,
            },
            role::{
                End,
                PathRole,
                Start,
            },
            root::RootPattern,
        },
        structs::{
            rooted::{
                role_path::{
                    HasRootChildIndex,
                    HasRootChildIndexMut,
                    HasRootChildToken,
                },
                root::RootedPath,
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::{
        cache::key::props::LeafKey,
        has_graph::HasGraph,
    },
};

use super::IndexRangePath;

// Generic HasPath for all IndexRangePath types
impl<R: PathRole, StartNode, EndNode> HasPath<R>
    for IndexRangePath<StartNode, EndNode>
where
    IndexRangePath<StartNode, EndNode>: HasRolePath<R>,
    <IndexRangePath<StartNode, EndNode> as HasRolePath<R>>::Node: Clone,
{
    type Node = <Self as HasRolePath<R>>::Node;
    fn path(&self) -> &Vec<Self::Node> {
        self.role_path().path()
    }
    fn path_mut(&mut self) -> &mut Vec<Self::Node> {
        self.role_path_mut().path_mut()
    }
}

// Generic HasLeafToken for IndexRangePath types with ChildLocation nodes
impl<R: PathRole, StartNode, EndNode> HasLeafToken<R>
    for IndexRangePath<StartNode, EndNode>
where
    IndexRangePath<StartNode, EndNode>: HasRolePath<R, Node = ChildLocation>
        + HasRootChildIndex<R>
        + HasPath<R, Node = ChildLocation>,
{
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        Some(
            R::bottom_up_iter(self.path().iter())
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

impl HasRootChildToken<Start> for IndexRangePath {
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        *trav.graph().expect_child_at(
            self.path_root()
                .location
                .to_child_location(self.start.sub_path.root_entry),
        )
    }
}

impl HasRootChildToken<End> for IndexRangePath {
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        *trav.graph().expect_child_at(
            self.path_root()
                .location
                .to_child_location(self.end.sub_path.root_entry),
        )
    }
}

// RootChildToken implementations for position-annotated paths
impl HasRootChildToken<End>
    for IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>>
{
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        *trav.graph().expect_child_at(
            self.path_root()
                .location
                .to_child_location(self.end.sub_path.root_entry),
        )
    }
}

impl HasRootChildToken<Start>
    for IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>>
{
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        *trav.graph().expect_child_at(
            self.path_root()
                .location
                .to_child_location(self.start.sub_path.root_entry),
        )
    }
}

impl GraphRootChild<Start> for IndexRangePath {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.root.location.to_child_location(self.start.root_entry)
    }
}

impl GraphRootChild<End> for IndexRangePath {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.root.location.to_child_location(self.end.root_entry)
    }
}

impl LeafKey for IndexRangePath {
    fn leaf_location(&self) -> ChildLocation {
        self.end.path.last().cloned().unwrap_or(
            self.root
                .location
                .to_child_location(self.end.sub_path.root_entry),
        )
    }
}

impl HasRootChildIndex<Start> for IndexRangePath {
    fn root_child_index(&self) -> usize {
        HasRootChildIndex::<Start>::root_child_index(&self.start)
    }
}

impl<EndNode> HasRootChildIndex<End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn root_child_index(&self) -> usize {
        self.end.root_entry
    }
}

impl<EndNode> HasRootChildIndexMut<End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn root_child_index_mut(&mut self) -> &mut usize {
        &mut self.end.root_entry
    }
}
