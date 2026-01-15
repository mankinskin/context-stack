use std::fmt::Debug;

use crate::split::{
    context::SplitCacheCtx,
    trace::states::context::SplitTraceStatesCtx,
};
use context_search::*;
use context_trace::*;

use crate::interval::IntervalGraph;

#[derive(Debug, PartialEq, Eq)]
pub struct InitInterval {
    pub root: Token,
    pub cache: TraceCache,
    pub end_bound: AtomPosition,
}
impl From<Response> for InitInterval {
    fn from(state: Response) -> Self {
        let root = state.root_token();
        let end_bound = state.checkpoint_position();
        Self {
            cache: state.cache,
            root,
            end_bound,
        }
    }
}

/// Create IntervalGraph from a graph reference and InitInterval.
/// 
/// With interior mutability, we only need `&G` since graph mutations
/// happen through per-vertex locks inside the graph.
impl<'a, G: HasGraph + 'a> From<(&'a G, InitInterval)>
    for IntervalGraph
{
    fn from((trav, init): (&'a G, InitInterval)) -> Self {
        let InitInterval {
            root,
            cache,
            end_bound,
            ..
        } = init;
        let ctx = TraceCtx { trav, cache };
        let iter = SplitTraceStatesCtx::new(ctx, root, end_bound);
        Self::from(SplitCacheCtx::init(iter))
    }
}
