pub(crate) mod policy;

use crate::{
    container::StateContainer,
    state::end::{
        postfix::PostfixEnd,
        PathCoverage,
    },
};
use context_trace::{
    path::accessors::has_path::HasPath,
    RootedStartPathAccessor,
    *,
};
use policy::DirectedTraversalPolicy;
use std::fmt::Debug;

pub trait SearchKind: TraceKind {
    type Container: StateContainer;
    type Policy: DirectedTraversalPolicy<Trav = Self::Trav>;
    type EndNode: PathNode;
}
impl<'a, K: SearchKind> SearchKind for &'a K {
    type Container = K::Container;
    type Policy = &'a K::Policy;
    type EndNode = K::EndNode;
}
//#[derive(Debug, Clone, Copy)]
//pub(crate) enum OptGen<Y> {
//    Yield(Y),
//    Skip,
//}

#[derive(Clone, Debug)]
pub(crate) struct TraceStart<'a> {
    pub(crate) end: &'a crate::state::matched::MatchResult,
    pub(crate) pos: usize,
}

impl Traceable for TraceStart<'_> {
    fn trace<G: HasGraph>(
        self,
        ctx: &mut TraceCtx<G>,
    ) {
        if let Some(mut p) = match self.end.path().clone() {
            PathCoverage::Postfix(p) => Some(p),
            PathCoverage::Range(p) => Some(PostfixEnd {
                path: p.path.into_rooted_role_path(),
                root_pos: p.root_pos,
            }),
            _ => None,
        } {
            HasPath::<Start>::path_mut(p.start_role_path_mut())
                .drain(0..self.pos);
            p.trace(ctx);
        }
    }
}

//impl<K: SearchKind> HasGraph for &'_ TraversalCtx<K> {
//    type Kind = TravKind<K::Trav>;
//    type Guard<'g>
//        = <K::Trav as HasGraph>::Guard<'g>
//    where
//        Self: 'g;
//    fn graph(&self) -> Self::Guard<'_> {
//        self.match_iter.0.trav.graph()
//    }
//}
//
//impl<K: SearchKind> HasGraph for &mut TraversalCtx<K> {
//    type Kind = TravKind<K::Trav>;
//    type Guard<'g>
//        = <K::Trav as HasGraph>::Guard<'g>
//    where
//        Self: 'g;
//    fn graph(&self) -> Self::Guard<'_> {
//        self.match_iter.0.trav.graph()
//    }
//}
