use std::ops::ControlFlow;

use crate::{
    PathNode,
    TokenWidth,
    direction::{
        Direction,
        Left,
        Right,
        pattern::PatternDirection,
    },
    graph::vertex::{
        location::{
            child::ChildLocation,
            pattern::IntoPatternLocation,
        },
        pattern::{
            pattern_pre_ctx,
            pattern_width,
        },
        token::Token,
        wide::Wide,
    },
    impl_root,
    path::{
        accessors::{
            child::{
                LeafToken,
                root::GraphRootChild,
            },
            has_path::{
                HasPath,
                HasRolePath,
                IntoRolePath,
                IntoRootedRolePath,
            },
            role::{
                End,
                PathRole,
                Start,
            },
            root::{
                GraphRootPattern,
                RootPattern,
            },
        },
        mutators::{
            append::PathAppend,
            lower::PathLower,
            move_path::{
                key::{
                    AdvanceKey,
                    AtomPosition,
                    RetractKey,
                },
                leaf::MoveLeaf,
                path::MovePath,
                root::MoveRootIndex,
            },
            pop::PathPop,
        },
        structs::{
            role_path::RolePath,
            rooted::{
                RangePath,
                RootedRangePath,
                role_path::{
                    RootChildIndex,
                    RootChildIndexMut,
                    RootChildToken,
                    RootedRolePath,
                },
                root::{
                    IndexRoot,
                    PathRoot,
                    RootedPath,
                },
            },
            sub_path::PositionAnnotated,
        },
    },
    trace::{
        cache::key::props::LeafKey,
        has_graph::{
            HasGraph,
            TravDir,
        },
    },
};

pub type IndexRangePath<StartNode = ChildLocation, EndNode = ChildLocation> =
    RootedRangePath<IndexRoot, StartNode, EndNode>;

impl IndexRangePath<ChildLocation, ChildLocation> {
    /// Convert to position-annotated path, accumulating widths as we traverse
    ///
    /// Starting from `entry_position`, walks through each child location in the end path,
    /// calculating the position at which we entered that child by adding the widths of
    /// all preceding tokens in the parent pattern.
    pub fn with_positions<G: HasGraph>(
        self,
        entry_position: AtomPosition,
        trav: &G,
    ) -> IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>> {
        let graph = trav.graph();
        let mut current_position = entry_position;

        let annotated_path: Vec<PositionAnnotated<ChildLocation>> = self
            .end
            .path()
            .iter()
            .map(|&loc| {
                // Get the pattern at this location
                let pattern = graph.expect_pattern_at(loc);

                // The position at which we enter this child
                let entry_pos = current_position;

                // Calculate width of tokens before the child we're entering
                let width_before =
                    pattern_width(pattern_pre_ctx(pattern, loc.sub_index));

                // Update position for next iteration: add width before child + width of child token
                current_position.advance_key(
                    (width_before + pattern[loc.sub_index].width()).0,
                );

                PositionAnnotated::new(loc, entry_pos)
            })
            .collect();

        let annotated_end =
            RolePath::new(self.end.root_child_index(), annotated_path);
        IndexRangePath::new(self.root, self.start, annotated_end)
    }
}

impl IndexRangePath<ChildLocation, PositionAnnotated<ChildLocation>> {
    /// Get the leaf ChildLocation from a position-annotated path
    /// Unwraps the position annotation to access the underlying location
    pub fn leaf_location(&self) -> Option<ChildLocation> {
        self.end.path().last().map(|annotated| annotated.node)
    }

    /// Convert position-annotated path to plain ChildLocation path
    /// This strips the position information, keeping only the locations
    pub fn into_plain(self) -> IndexRangePath<ChildLocation, ChildLocation> {
        let plain_end_path: Vec<ChildLocation> = self
            .end
            .path()
            .iter()
            .map(|annotated| annotated.node)
            .collect();
        IndexRangePath {
            root: self.root,
            start: self.start,
            end: RolePath::new(self.end.root_child_index(), plain_end_path),
        }
    }

    /// Get the rooted leaf token from position-annotated end path
    /// Extracts the ChildLocation and retrieves the token
    pub fn role_rooted_leaf_token<R: PathRole, G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token
    where
        Self: HasRolePath<R, Node = PositionAnnotated<ChildLocation>>
            + RootChildIndex<R>
            + RootChildToken<R>,
    {
        use crate::path::structs::rooted::HasRolePath;

        // Extract leaf location from position-annotated node
        let role_path = HasRolePath::<R>::role_path(self);
        let leaf_loc = role_path
            .path()
            .last()
            .map(|annotated| annotated.node)
            .unwrap_or_else(|| {
                self.root.location.to_child_location(role_path.root_entry)
            });

        // Get token at that location, or fall back to root child token
        trav.graph()
            .get_child_at(leaf_loc)
            .copied()
            .unwrap_or(self.root_child_token(trav))
    }
}

impl From<IndexRoot> for IndexRangePath {
    fn from(value: IndexRoot) -> Self {
        Self {
            root: value,
            start: Default::default(),
            end: Default::default(),
        }
    }
}

impl IntoRolePath<End> for IndexRangePath {
    fn into_role_path(self) -> RolePath<End> {
        self.end
    }
}
impl IntoRolePath<Start> for IndexRangePath {
    fn into_role_path(self) -> RolePath<Start> {
        self.start
    }
}

// Generic HasPath for all IndexRangePath types
impl<R: PathRole, StartNode, EndNode> HasPath<R>
    for IndexRangePath<StartNode, EndNode>
where
    IndexRangePath<StartNode, EndNode>: HasRolePath<R>,
    <IndexRangePath<StartNode, EndNode> as HasRolePath<R>>::Node: Clone,
{
    type Node = <Self as HasRolePath<R>>::Node;
    fn path(&self) -> &Vec<Self::Node> {
        HasRolePath::<R>::role_path(self).path()
    }
    fn path_mut(&mut self) -> &mut Vec<Self::Node> {
        HasRolePath::<R>::role_path_mut(self).path_mut()
    }
}

// Generic LeafToken for IndexRangePath types with ChildLocation nodes
impl<R: PathRole, StartNode, EndNode> LeafToken<R>
    for IndexRangePath<StartNode, EndNode>
where
    IndexRangePath<StartNode, EndNode>: HasRolePath<R, Node = ChildLocation>
        + RootChildIndex<R>
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

//impl HasMatchPaths for IndexRangePath {
//    fn into_paths(self) -> (RolePath<Start>, RolePath<End>) {
//        (self.start, self.end)
//    }
//}

impl<EndNode: PathNode> MoveRootIndex<Right, End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn move_root_index<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = self.root_pattern::<G>(&graph);
        let current_index = RootChildIndex::<End>::root_child_index(self);
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
            RootChildIndex::<End>::root_child_index(self),
        ) {
            *self.root_child_index_mut() = prev;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}
impl<D: Direction, Root: PathRoot> MovePath<D, End>
    for RootedRangePath<Root, ChildLocation, ChildLocation>
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
    for RootedRangePath<Root, ChildLocation, PositionAnnotated<ChildLocation>>
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
    for RootedRangePath<Root, ChildLocation, PositionAnnotated<ChildLocation>>
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
impl RootChildToken<Start> for IndexRangePath {
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

impl RootChildToken<End> for IndexRangePath {
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
impl RootChildToken<End>
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

impl RootChildToken<Start>
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

impl LeafKey for IndexRangePath {
    fn leaf_location(&self) -> ChildLocation {
        self.end.path.last().cloned().unwrap_or(
            self.root
                .location
                .to_child_location(self.end.sub_path.root_entry),
        )
    }
}

impl GraphRootChild<End> for IndexRangePath {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.root.location.to_child_location(self.end.root_entry)
    }
}

impl RootChildIndex<Start> for IndexRangePath {
    fn root_child_index(&self) -> usize {
        RootChildIndex::<Start>::root_child_index(&self.start)
    }
}

impl<EndNode> RootChildIndex<End> for IndexRangePath<ChildLocation, EndNode> {
    fn root_child_index(&self) -> usize {
        self.end.root_entry
    }
}

impl<EndNode> RootChildIndexMut<End>
    for IndexRangePath<ChildLocation, EndNode>
{
    fn root_child_index_mut(&mut self) -> &mut usize {
        &mut self.end.root_entry
    }
}

impl PathLower for (&mut AtomPosition, &mut IndexRangePath) {
    fn path_lower<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let (root_pos, range) = self;
        let (start, end, root) = (
            &mut range.start.sub_path,
            &mut range.end.sub_path,
            &mut range.root,
        );
        if let Some(prev) = start.path.pop() {
            let graph = trav.graph();
            let pattern = graph.expect_pattern_at(prev);
            root_pos.retract_key(
                pattern[prev.sub_index + 1..]
                    .iter()
                    .fold(TokenWidth(0), |a, c| a + c.width())
                    .0,
            );
            start.root_entry = prev.sub_index;
            end.root_entry = pattern.len() - 1;
            end.path.clear();
            root.location = prev.into_pattern_location();

            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}
