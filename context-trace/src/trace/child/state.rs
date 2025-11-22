use crate::{
    AtomPosition,
    DirectedKey,
    GraphRootChild,
    GraphRootPattern,
    HasPath,
    PathRole,
    PositionAnnotated,
    RootChildIndex,
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
            child::LeafToken,
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
                RootChildToken,
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
pub struct RootChildState<
    EndNode: Debug + Clone + PartialEq + Eq = ChildLocation,
> {
    #[deref]
    #[deref_mut]
    pub child_state: ChildState<EndNode>,
    pub root_parent: ParentState,
}

/// State representing a child position (range path with entry position).
/// The `entry_pos` represents the position where we entered the root token of this traversal.
/// The `start_pos` represents the position to use for tracing the start path (for bottom-up edges).
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct ChildState<EndNode: Debug + Clone + PartialEq + Eq = ChildLocation> {
    pub entry_pos: AtomPosition,
    pub start_pos: AtomPosition,
    #[deref]
    #[deref_mut]
    pub path: IndexRangePath<ChildLocation, EndNode>,
}

impl<EndNode: Debug + Clone + PartialEq + Eq> ChildState<EndNode>
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

impl<EndNode: Debug + Clone + PartialEq + Eq> PathAppend for ChildState<EndNode>
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

impl<EndNode: Debug + Clone + PartialEq + Eq> RootChildIndex<End>
    for ChildState<EndNode>
{
    fn root_child_index(&self) -> usize {
        self.path.role_root_child_index::<End>()
    }
}

impl<EndNode: Debug + Clone + PartialEq + Eq> RootChildToken<End>
    for ChildState<EndNode>
where
    IndexRangePath<ChildLocation, EndNode>: RootChildToken<End>,
{
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        RootChildToken::<End>::root_child_token(&self.path, trav)
    }
}

impl<EndNode: Debug + Clone + PartialEq + Eq> GraphRoot
    for ChildState<EndNode>
{
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
impl<EndNode: Debug + Clone + PartialEq + Eq> RootPattern
    for ChildState<EndNode>
{
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> &'g crate::Pattern {
        self.path.root_pattern::<G>(trav)
    }
}
impl<EndNode: Debug + Clone + PartialEq + Eq> GraphRootPattern
    for ChildState<EndNode>
{
    fn root_pattern_location(&self) -> crate::PatternLocation {
        self.path.root_pattern_location()
    }
}
impl<EndNode: Debug + Clone + PartialEq + Eq> RootedPath
    for ChildState<EndNode>
{
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
// LeafToken only for non-position-annotated ChildState
impl LeafToken<End> for ChildState<ChildLocation>
where
    IndexRangePath<ChildLocation, ChildLocation>:
        RootChildIndex<End> + HasPath<End, Node = ChildLocation>,
{
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        self.path.role_leaf_token_location::<End>()
    }
}

// Note: Cannot implement LeafToken<End> for ChildState<PositionAnnotated<ChildLocation>>
// because LeafToken requires HasPath<End, Node = ChildLocation>, but PositionAnnotated paths
// have HasPath<End, Node = PositionAnnotated<ChildLocation>>.
// Instead, PrefixStates is implemented directly for this type in context-search.

impl<R: PathRole, EndNode: Debug + Clone + PartialEq + Eq> HasPath<R>
    for ChildState<EndNode>
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

impl<EndNode: Debug + Clone + PartialEq + Eq> StateAdvance
    for ChildState<EndNode>
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

impl<EndNode: Debug + Clone + PartialEq + Eq> Ord for ChildState<EndNode> {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.path.root_parent().cmp(&other.path.root_parent())
    }
}

impl<EndNode: Debug + Clone + PartialEq + Eq> PartialOrd
    for ChildState<EndNode>
{
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<EndNode: Debug + Clone + PartialEq + Eq> RootKey for ChildState<EndNode> {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.path.root_parent(), self.entry_pos.into())
    }
}

impl<EndNode: Debug + Clone + PartialEq + Eq> TargetKey
    for ChildState<EndNode>
{
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}

// HasTargetPos impl removed - use StatePosition instead

// New StatePosition trait implementation
impl<EndNode: Debug + Clone + PartialEq + Eq>
    crate::path::accessors::path_accessor::StatePosition
    for ChildState<EndNode>
{
    fn prev_pos(&self) -> &AtomPosition {
        &self.start_pos
    }

    fn root_pos(&self) -> &AtomPosition {
        &self.entry_pos
    }

    fn target_pos(&self) -> Option<&AtomPosition> {
        Some(&self.entry_pos)
    }

    fn prev_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.start_pos
    }

    fn root_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.entry_pos
    }

    fn target_pos_mut(&mut self) -> Option<&mut AtomPosition> {
        Some(&mut self.entry_pos)
    }
}

impl LeafKey for ChildState<ChildLocation> {
    fn leaf_location(&self) -> ChildLocation {
        self.path.leaf_location()
    }
}
