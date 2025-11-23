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
// HasPrevPos, HasRootPos, HasTargetPos traits removed - use StatePosition instead

// StatePosition implementations using macro to reduce boilerplate
crate::impl_state_position! {
    for ParentState => {
        prev_pos: prev_pos,
        root_pos: root_pos,
    }
}

crate::impl_state_position! {
    for BaseState<P> where [P: RootedPath] => {
        prev_pos: prev_pos,
        root_pos: root_pos,
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
// HasRootedPath impl removed - use RootedPathAccessor instead

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
        let width = self.width().0.into();
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
