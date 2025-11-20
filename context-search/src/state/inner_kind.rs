use std::cmp::Ordering;

use context_trace::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InnerKind {
    ParentCandidate(ParentState),
    ChildQueue(ChildState),
}
impl Ord for InnerKind {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        match (self, other) {
            (InnerKind::ChildQueue(a), InnerKind::ChildQueue(b)) => a.cmp(b),
            (InnerKind::ParentCandidate(a), InnerKind::ParentCandidate(b)) =>
                a.cmp(b),
            (InnerKind::ChildQueue(_), _) => Ordering::Less,
            (_, InnerKind::ChildQueue(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for InnerKind {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
//impl InnerKind {
//    pub fn unwrap_parent(self) -> ParentState {
//        if let Self::ParentCandidate(p) = self {
//            p
//        } else {
//            panic!();
//        }
//    }
//    pub(crate) fn unwrap_child(self) -> ChildState {
//        if let Self::ChildQueue(c) = self {
//            c
//        } else {
//            panic!();
//        }
//    }
//}
