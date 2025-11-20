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
        let end_bound = state.cursor_position();
        Self {
            cache: state.cache,
            root,
            end_bound,
        }
    }
}
impl<'a, G: HasGraphMut + 'a> From<(&'a mut G, InitInterval)>
    for IntervalGraph
{
    fn from((trav, init): (&'a mut G, InitInterval)) -> Self {
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
