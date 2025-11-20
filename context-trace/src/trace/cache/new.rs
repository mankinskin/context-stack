use crate::{
    graph::vertex::location::child::ChildLocation,
    trace::{
        BottomUp,
        TopDown,
        TraceDirection,
        cache::key::{
            directed::DirectedKey,
            props::TargetKey,
        },
    },
};

use derive_more::From;
use derive_new::new;

#[derive(Clone, Debug, PartialEq, Eq, From)]
pub enum EditKind {
    Parent(NewTraceEdge<BottomUp>),
    Child(NewTraceEdge<TopDown>),
}

impl TargetKey for EditKind {
    fn target_key(&self) -> DirectedKey {
        match &self {
            EditKind::Parent(state) => state.target.into(),
            EditKind::Child(state) => state.target.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, new)]
pub struct NewTraceEdge<D: TraceDirection> {
    pub(crate) prev: D::Key,
    pub(crate) target: D::Key,
    pub(crate) location: ChildLocation,
}
//#[derive(Clone, Debug, PartialEq, Eq)]
//pub(crate) struct RootEdit {
//    pub(crate) entry_key: UpKey,
//    pub(crate) entry_location: ChildLocation,
//}
