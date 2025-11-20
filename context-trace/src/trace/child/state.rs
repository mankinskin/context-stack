use crate::{
    AtomPosition,
    DirectedKey,
    GraphRootChild,
    GraphRootPattern,
    HasPath,
    PathRole,
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
            has_path::IntoRootedRolePath,
            role::End,
            root::GraphRoot,
        },
        mutators::{
            append::PathAppend,
            move_path::advance::Advance,
        },
        structs::rooted::{
            index_range::IndexRangePath,
            role_path::RootChildToken,
        },
    },
    trace::{
        cache::key::props::{
            LeafKey,
            RootKey,
        },
        has_graph::HasGraph,
        state::{
            HasTargetPos,
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
pub struct RootChildState {
    #[deref]
    #[deref_mut]
    pub child_state: ChildState,
    pub root_parent: ParentState,
}

/// State representing a child position (range path with current target position).
/// The `current_pos` represents the target position being traversed.
#[derive(Clone, Debug, PartialEq, Eq, Deref, DerefMut)]
pub struct ChildState {
    pub current_pos: AtomPosition,
    #[deref]
    #[deref_mut]
    pub path: IndexRangePath,
}

impl ChildState {
    pub fn parent_state(&self) -> ParentState {
        ParentState {
            path: self.path.get_rooted_role_path(),
            prev_pos: self.current_pos,
            root_pos: self.current_pos,
        }
    }
}
impl PathAppend for ChildState {
    fn path_append(
        &mut self,
        parent_entry: ChildLocation,
    ) {
        self.path.path_append(parent_entry);
    }
}
impl RootChildIndex<End> for ChildState {
    fn root_child_index(&self) -> usize {
        self.path.role_root_child_index::<End>()
    }
}
impl RootChildToken<End> for ChildState {
    fn root_child_token<G: HasGraph>(
        &self,
        trav: &G,
    ) -> Token {
        RootChildToken::<End>::root_child_token(&self.path, trav)
    }
}
impl GraphRoot for ChildState {
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
impl RootPattern for ChildState {
    fn root_pattern<'a: 'g, 'b: 'g, 'g, G: HasGraph + 'a>(
        &'b self,
        trav: &'g G::Guard<'a>,
    ) -> &'g crate::Pattern {
        self.path.root_pattern::<G>(trav)
    }
}
impl GraphRootPattern for ChildState {
    fn root_pattern_location(&self) -> crate::PatternLocation {
        self.path.root_pattern_location()
    }
}
impl RootedPath for ChildState {
    type Root = <IndexRangePath as RootedPath>::Root;
    fn path_root(&self) -> Self::Root {
        self.path.path_root()
    }
}
impl GraphRootChild<End> for ChildState {
    fn graph_root_child_location(&self) -> ChildLocation {
        self.path.role_root_child_location::<End>()
    }
}
impl LeafToken<End> for ChildState {
    fn leaf_token_location(&self) -> Option<ChildLocation> {
        self.path.role_leaf_token_location::<End>()
    }
}
impl<R: PathRole> HasPath<R> for ChildState
where
    IndexRangePath: HasPath<R>,
{
    fn path(&self) -> &Vec<ChildLocation> {
        HasPath::<R>::path(&self.path)
    }
    fn path_mut(&mut self) -> &mut Vec<ChildLocation> {
        HasPath::<R>::path_mut(&mut self.path)
    }
}

impl StateAdvance for ChildState {
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

impl Ord for ChildState {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.path.root_parent().cmp(&other.path.root_parent())
    }
}

impl PartialOrd for ChildState {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl RootKey for ChildState {
    fn root_key(&self) -> UpKey {
        UpKey::new(self.path.root_parent(), self.current_pos.into())
    }
}

impl TargetKey for ChildState {
    fn target_key(&self) -> DirectedKey {
        self.root_key().into()
    }
}

impl HasTargetPos for ChildState {
    fn target_pos(&self) -> &AtomPosition {
        &self.current_pos
    }
    fn target_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.current_pos
    }
}

impl LeafKey for ChildState {
    fn leaf_location(&self) -> ChildLocation {
        self.path.leaf_location()
    }
}
