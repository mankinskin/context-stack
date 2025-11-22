pub(crate) mod end;
pub(crate) mod inner_kind;
pub(crate) mod matched;
pub(crate) mod result;
pub(crate) mod start;

use std::cmp::Ordering;

use context_trace::{
    path::accessors::path_accessor::StatePosition,
    *,
};

use inner_kind::InnerKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraversalState {
    pub(crate) prev: DirectedKey,
    pub(crate) kind: InnerKind,
}
impl From<(DirectedKey, ParentState)> for TraversalState {
    fn from((prev, ps): (DirectedKey, ParentState)) -> Self {
        Self {
            prev,
            kind: InnerKind::ParentCandidate(ps),
        }
    }
}
impl From<(DirectedKey, ChildState)> for TraversalState {
    fn from((prev, cs): (DirectedKey, ChildState)) -> Self {
        Self {
            prev,
            kind: InnerKind::ChildQueue(cs),
        }
    }
}
// HasRootPos implementation removed - use StatePosition instead
impl TargetKey for TraversalState {
    fn target_key(&self) -> DirectedKey {
        match &self.kind {
            InnerKind::ParentCandidate(state) => state.target_key(),
            InnerKind::ChildQueue(state) => state.target_key(),
        }
    }
}
impl RootKey for TraversalState {
    fn root_key(&self) -> UpKey {
        match &self.kind {
            InnerKind::ParentCandidate(state) => state.root_key(),
            InnerKind::ChildQueue(state) => state.root_key(),
        }
    }
}
impl Ord for TraversalState {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.kind.cmp(&other.kind)
    }
}

impl PartialOrd for TraversalState {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

//impl TraversalState {
//    pub(crate) fn entry_location(&self) -> Option<ChildLocation> {
//        match &self.kind {
//            InnerKind::ParentCandidate(state) =>
//                Some(state.path.graph_root_child_location()),
//            InnerKind::ChildQueue(state) =>
//                state.path.role_leaf_token_location::<End>(),
//        }
//    }
//    pub(crate) fn prev_key(&self) -> DirectedKey {
//        self.prev.clone()
//    }
//
//    pub(crate) fn state_direction(&self) -> StateDirection {
//        match &self.kind {
//            InnerKind::ParentCandidate(_) => StateDirection::BottomUp,
//            InnerKind::ChildQueue(_) => StateDirection::TopDown,
//        }
//    }
//}
