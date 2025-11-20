use std::ops::ControlFlow;

use crate::{
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
                    AtomPosition,
                    RetractKey,
                },
                leaf::MoveLeaf,
                path::MovePath,
                root::MoveRootIndex,
            },
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

pub type IndexRangePath = RootedRangePath<IndexRoot>;

impl RangePath for IndexRangePath {
    //fn new_range(
    //    root: Self::Root,
    //    entry: usize,
    //    exit: usize,
    //) -> Self {
    //    Self {
    //        root,
    //        start: SubPath::new_empty(entry).into(),
    //        end: SubPath::new_empty(exit).into(),
    //    }
    //}
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
impl_root! { GraphRootPattern for IndexRangePath, self => self.root.location }
impl_root! { GraphRoot for IndexRangePath, self => self.root_pattern_location().parent }
impl_root! { RootPattern for IndexRangePath, self, trav => GraphRootPattern::graph_root_pattern::<G>(self, trav) }

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

impl<R: PathRole> HasPath<R> for IndexRangePath
where
    Self: HasRolePath<R>,
{
    fn path(&self) -> &Vec<ChildLocation> {
        HasRolePath::<R>::role_path(self).path()
    }
    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        HasRolePath::<R>::role_path_mut(self).path_mut()
    }
}
impl<R: PathRole> LeafToken<R> for IndexRangePath
where
    IndexRangePath: HasRolePath<R> + RootChildIndex<R>,
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

impl MoveRootIndex<Right, End> for IndexRangePath {
    fn move_root_index<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = self.root_pattern::<G>(&graph);
        if let Some(next) = TravDir::<G>::pattern_index_next(
            pattern,
            RootChildIndex::<End>::root_child_index(self),
        ) {
            *self.root_child_index_mut() = next;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl MoveRootIndex<Left, End> for IndexRangePath {
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
impl<D: Direction, Root: PathRoot> MovePath<D, End> for RootedRangePath<Root>
where
    Self: MoveRootIndex<D> + PathAppend,
    ChildLocation: MoveLeaf<D>,
{
    fn move_path_segment<G: HasGraph>(
        &mut self,
        location: &mut ChildLocation,
        trav: &G::Guard<'_>,
    ) -> ControlFlow<()> {
        location.move_leaf(trav)
    }
}

impl<D: Direction, Root: PathRoot> MovePath<D, End>
    for RootedRolePath<End, Root>
where
    Self: MoveRootIndex<D> + PathAppend,
    ChildLocation: MoveLeaf<D>,
{
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

impl RootChildIndex<End> for IndexRangePath {
    fn root_child_index(&self) -> usize {
        RootChildIndex::<End>::root_child_index(&self.end)
    }
}

impl RootChildIndexMut<End> for IndexRangePath {
    fn root_child_index_mut(&mut self) -> &mut usize {
        self.end.root_child_index_mut()
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
                    .fold(0, |a, c| a + c.width()),
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
