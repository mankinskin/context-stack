pub(crate) mod policy;

use crate::{
    container::StateContainer,
    state::end::{
        postfix::PostfixEnd,
        PathCoverage,
    },
};
use context_trace::{
    path::accessors::has_path::HasRootedRolePath,
    *,
};
use policy::DirectedTraversalPolicy;
use std::fmt::Debug;

pub trait TraversalKind: Debug + Default {
    type Trav: HasGraph;
    type Container: StateContainer;
    type Policy: DirectedTraversalPolicy<Trav = Self::Trav>;
}
//#[derive(Debug, Clone, Copy)]
//pub(crate) enum OptGen<Y> {
//    Yield(Y),
//    Skip,
//}

#[derive(Clone, Debug)]
pub(crate) struct TraceStart<'a> {
    pub(crate) end: &'a crate::state::matched::MatchedEndState,
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
            p.rooted_role_path_mut().drain(0..self.pos);
            p.trace(ctx);
        }
    }
}

//impl<K: TraversalKind> HasGraph for &'_ TraversalCtx<K> {
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
//impl<K: TraversalKind> HasGraph for &mut TraversalCtx<K> {
//    type Kind = TravKind<K::Trav>;
//    type Guard<'g>
//        = <K::Trav as HasGraph>::Guard<'g>
//    where
//        Self: 'g;
//    fn graph(&self) -> Self::Guard<'_> {
//        self.match_iter.0.trav.graph()
//    }
//}
