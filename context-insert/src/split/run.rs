use std::collections::BTreeSet;
use tracing::debug;

use crate::{
    interval::IntervalGraph,
    join::context::node::merge::{
        PartitionRange,
        RequiredPartitions,
    },
    split::{
        context::SplitCacheCtx,
        trace::SplitTraceState,
    },
};
use context_trace::*;

#[derive(Debug)]
pub(crate) struct SplitRunStep;

#[derive(Debug)]
pub(crate) struct SplitRun<G: HasGraph> {
    ctx: SplitCacheCtx<G>,
    incomplete: BTreeSet<Token>,
}
impl<G: HasGraph> SplitRun<G> {
    pub(crate) fn init(
        &mut self
    ) -> (Vec<SplitTraceState>, PartitionRange, RequiredPartitions) {
        self.ctx.cache.augment_root(
            &self.ctx.states_ctx.trav,
            self.ctx.states_ctx.ctx.root,
        )
    }
    pub(crate) fn finish(mut self) -> SplitCacheCtx<G> {
        self.ctx
            .cache
            .augment_nodes(&mut self.ctx.states_ctx, self.incomplete);
        self.ctx
    }
}
impl<G: HasGraph> Iterator for SplitRun<G> {
    type Item = SplitRunStep;
    fn next(&mut self) -> Option<Self::Item> {
        self.ctx.states_ctx.states.next().map(|state| {
            self.ctx.apply_trace_state(&state);
            self.incomplete.insert(state.index);
            let complete = self
                .incomplete
                .split_off(&TokenWidth(*state.index.width() + 1));
            self.ctx
                .cache
                .augment_nodes(&mut self.ctx.states_ctx, complete);
            SplitRunStep
        })
    }
}
impl<G: HasGraph> From<SplitCacheCtx<G>> for SplitRun<G> {
    fn from(ctx: SplitCacheCtx<G>) -> Self {
        Self {
            ctx,
            incomplete: Default::default(),
        }
    }
}
impl<G: HasGraph> From<SplitCacheCtx<G>> for IntervalGraph {
    fn from(cache: SplitCacheCtx<G>) -> Self {
        Self::from(SplitRun::from(cache))
    }
}
impl<G: HasGraph> From<SplitRun<G>> for IntervalGraph {
    fn from(mut run: SplitRun<G>) -> Self {
        debug!("IntervalGraph::from - init");
        let (next, target_range, required) = run.init();
        run.ctx.states.queue.extend(next);
        debug!("IntervalGraph::from - run iterator to end");
        run.all(|_| true); // run iterator to end
        debug!("SplitRun::from - calling finish");
        let cache = run.finish();
        debug!("SplitRun::from - finish done, creating IntervalGraph");
        Self {
            root: cache.states_ctx.ctx.root,
            states: cache.states_ctx.states,
            cache: cache.cache,
            target_range,
            required,
        }
    }
}
