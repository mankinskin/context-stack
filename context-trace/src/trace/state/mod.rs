pub(crate) mod parent;

use crate::{
    graph::vertex::location::pattern::IntoPatternLocation as _,
    path::{
        accessors::has_path::IntoRootedPath,
        structs::rooted::root::RootedPath,
    },
    *,
};
use parent::ParentState;

//pub(crate) trait SearchSpace {
//    fn expand(&mut self) -> Vec<TraceState<Self>>;
//}
// TODO:
// - param K: SearchSpace
// - connect input and internal path with Into<InternalState>
// AVAILABLE DATA FIELDS
// - PathBagMember<K> for each path for pos or id
// - PathBagProps<K> for whole bag for global pos
// AVAILABLE OPERATIONS
// - expand - (compare match, expand)
//

//
// TraceState<K> is either {ChildState<K>, ParentState<K>}
//
pub trait HasPrevPos {
    fn prev_pos(&self) -> &AtomPosition;
    fn prev_pos_mut(&mut self) -> &mut AtomPosition;
}

pub trait HasRootPos {
    fn root_pos(&self) -> &AtomPosition;
    fn root_pos_mut(&mut self) -> &mut AtomPosition;
}

/// Trait for accessing the current/target position in a child state
pub trait HasTargetPos {
    fn target_pos(&self) -> &AtomPosition;
    fn target_pos_mut(&mut self) -> &mut AtomPosition;
}

impl HasPrevPos for ParentState {
    fn prev_pos(&self) -> &AtomPosition {
        &self.prev_pos
    }
    fn prev_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.prev_pos
    }
}

impl HasRootPos for ParentState {
    fn root_pos(&self) -> &AtomPosition {
        &self.root_pos
    }
    fn root_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.root_pos
    }
}

impl<P: RootedPath> HasPrevPos for BaseState<P> {
    fn prev_pos(&self) -> &AtomPosition {
        &self.prev_pos
    }
    fn prev_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.prev_pos
    }
}
impl<P: RootedPath> HasRootPos for BaseState<P> {
    fn root_pos(&self) -> &AtomPosition {
        &self.root_pos
    }
    fn root_pos_mut(&mut self) -> &mut AtomPosition {
        &mut self.root_pos
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseState<P: RootedPath> {
    pub prev_pos: AtomPosition,
    pub root_pos: AtomPosition,
    pub path: P,
}
impl<P: RootedPath> IntoRootedPath<P> for BaseState<P> {
    fn into_rooted_path(self) -> P {
        self.path
    }
}
impl<P: RootedPath> HasRootedPath<P> for BaseState<P> {
    fn rooted_path(&self) -> &P {
        &self.path
    }
    fn rooted_path_mut(&mut self) -> &mut P {
        &mut self.path
    }
}

pub trait StateAdvance: Sized + Clone {
    type Next;
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self>;
}
pub trait IntoParentState: Sized {
    fn into_parent_state<G: HasGraph>(
        self,
        trav: &G,
        parent_entry: ChildLocation,
    ) -> ParentState;
}
impl IntoParentState for Token {
    fn into_parent_state<G: HasGraph>(
        self,
        _trav: &G,
        parent_entry: ChildLocation,
    ) -> ParentState {
        let width = self.width().into();
        ParentState {
            prev_pos: width,
            root_pos: width,
            path: RootedRolePath {
                root: IndexRoot {
                    location: parent_entry.into_pattern_location(),
                },
                role_path: RolePath {
                    sub_path: SubPath {
                        root_entry: parent_entry.sub_index,
                        path: vec![],
                    },
                    _ty: Default::default(),
                },
            },
        }
    }
}
