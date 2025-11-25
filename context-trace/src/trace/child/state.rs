use crate::{
    AtomPosition,
    DirectedKey,
    GraphRootChild,
    GraphRootPattern,
    HasPath,
    PathNode,
    PathRole,
    PositionAnnotated,
    RootPattern,
    RootedPath,
    TargetKey,
    UpKey,
    graph::vertex::{
        location::child::ChildLocation,
        token::Token,
    },
    path::{
        RolePathUtils,
        accessors::{
            child::HasLeafToken,
            has_path::{
                HasRolePath,
                IntoRootedRolePath,
            },
            path_accessor::PathAccessor,
            role::{
                End,
                Start,
            },
            root::GraphRoot,
        },
        mutators::{
            append::PathAppend,
            move_path::advance::Advance,
        },
        structs::rooted::{
            index_range::IndexRangePath,
            role_path::{
                HasRootChildIndex,
                HasRootChildToken,
                RootedRolePath,
            },
            root::IndexRoot,
        },
    },
    trace::{
        cache::key::props::{
            LeafKey,
            RootKey,
        },
        has_graph::HasGraph,
        state::{
            StateAdvance,
            parent::ParentState,
        },
    },
};
use derive_more::derive::{
    Deref,
    DerefMut,
};
use std::{
    cmp::Ordering,
    fmt::Debug,
};

//impl_cursor_pos! {
//    CursorPosition for ChildState, self => self.cursor.relative_pos
//}
//impl TargetKey for ChildState {
//    fn target_key(&self) -> DirectedKey {
//        self.target.clone().into()
//    }
//}

#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct RootChildState<EndNode: PathNode = ChildLocation> {
    #[deref]
    #[deref_mut]
    pub child_state: ChildState<EndNode>,
    pub root_parent: ParentState,
}

/// State representing a child position (range path with entry position).
/// The `entry_pos` represents the position where we entered the root token of this traversal.
/// The `start_pos` represents the position to use for tracing the start path (for bottom-up edges).
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct ChildState<EndNode: PathNode = ChildLocation> {
    pub entry_pos: AtomPosition,
    pub start_pos: AtomPosition,
    #[deref]
    #[deref_mut]
    pub path: IndexRangePath<ChildLocation, EndNode>,
}

impl<EndNode: PathNode> ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>:
        HasRolePath<Start, Node = ChildLocation> + RootedPath<Root = IndexRoot>,
{
    pub fn parent_state(&self) -> ParentState {
        ParentState {
            path: RootedRolePath::new(
                self.path.path_root(),
                self.path.start_path().clone(),
            ),
            prev_pos: self.entry_pos,
            root_pos: self.entry_pos,
        }
    }
}

impl<EndNode: PathNode> PathAppend for ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>: PathAppend,
{
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.path.path_append(parent_entry);
    }
}

impl<EndNode: PathNode> HasRootChildIndex<End> for ChildState<EndNode> {
    fn root_child_index(&self) -> usize {
        self.path.role_root_child_index::<End>()
    }
}

impl<EndNode: PathNode> HasRootChildToken<End> for ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>: HasRootChildToken<End>,
{
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        HasRootChildToken::<End>::root_child_token(&self.path, trav)
    }
}

impl<EndNode: PathNode> GraphRoot for ChildState<EndNode> {
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
impl<EndNode: PathNode> RootPattern for ChildState<EndNode> {
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> &'g crate::Pattern {
        self.path.root_pattern::<G>(trav)
    }
}
impl<EndNode: PathNode> GraphRootPattern for ChildState<EndNode> {
    fn root_pattern_location(&self) -> crate::PatternLocation {
        self.path.root_pattern_location()
    }
}
impl<EndNode: PathNode> RootedPath for ChildState<EndNode> {
    type Root = <IndexRangePath<ChildLocation, EndNode> as RootedPath>::Root;
    fn path_root(&self) -> Self::Root {
        self.path.path_root()
    }
}
impl GraphRootChild<End> for ChildState<ChildLocation> {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.path.role_root_child_location::<End>()
    }
}
// HasLeafToken only for non-position-annotated ChildState
impl HasLeafToken<End> for ChildState<ChildLocation>
where
    IndexRangePath<ChildLocation, ChildLocation>:
        HasRootChildIndex<End> + HasPath<End, Node = ChildLocation>,
{
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        self.path.role_leaf_token_location::<End>()
    }
}

// Note: Cannot implement LeafToken<End> for ChildState<PositionAnnotated<ChildLocation>>
// because LeafToken requires HasPath<End, Node = ChildLocation>, but PositionAnnotated paths
// have HasPath<End, Node = PositionAnnotated<ChildLocation>>.
// Instead, PrefixStates is implemented directly for this type in context-search.

impl<R: PathRole, EndNode: PathNode> HasPath<R> for ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>: HasRolePath<R>,
{
    type Node =
        <IndexRangePath<ChildLocation, EndNode> as HasRolePath<R>>::Node;
    fn path(&self) -> &Vec<Self::Node> {
        self.path.role_path().path()
    }
    fn path_mut(&mut self) -> &mut Vec<Self::Node> {
        self.path.role_path_mut().path_mut()
    }
}

impl<EndNode: PathNode> StateAdvance for ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>: Advance,
{
    type Next = Self;
    fn advance_state<G: HasGraph>(
        mut self,
        trav: &G,
    ) -> Result<Self, Self> {
        if self.path.advance(trav).is_continue() {
            Ok(self)
        } else {
            Err(self)
        }
    }
}

impl<EndNode: PathNode> Ord for ChildState<EndNode> {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.path.root_parent().cmp(&other.path.root_parent())
    }
}

impl<EndNode: PathNode> PartialOrd for ChildState<EndNode> {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<EndNode: PathNode> RootKey for ChildState<EndNode> {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.path.root_parent(), self.entry_pos.into())
    }
}

impl<EndNode: PathNode> TargetKey for ChildState<EndNode> {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}

// HasTargetPos impl removed - use StatePosition instead

// StatePosition implementation using macro
crate::impl_state_position! {
    for ChildState<EndNode> where [EndNode: PathNode] => {
        prev_pos: start_pos,
        root_pos: entry_pos,
    }
}

// HasTargetPos implementation
// For ChildState, the target offset is the entry_pos - the offset position
// where we entered the root token being examined (position before the token).
impl<EndNode: PathNode> crate::path::accessors::path_accessor::HasTargetOffset
    for ChildState<EndNode>
{
    fn target_offset(&self) -> &crate::AtomPosition {
        &self.entry_pos
    }

    fn target_offset_mut(&mut self) -> &mut crate::AtomPosition {
        &mut self.entry_pos
    }
}

impl LeafKey for ChildState<ChildLocation> {
    fn leaf_location(&self) -> ChildLocation {
        self.path.leaf_location()
    }
}
