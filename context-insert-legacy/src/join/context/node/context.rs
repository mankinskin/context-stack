use std::ops::Range;

use crate::{
    interval::IntervalGraph,
    join::{
        context::{
            frontier::FrontierSplitIterator,
            pattern::{
                IntervalMap,
                PatternJoinCtx,
            },
        },
        plan::{
            JoinPlan,
            JoinRef,
            NodeOverlay,
            PatternOrigin,
            PlanEdit,
        },
    },
    split::{
        SplitMap,
        cache::vertex::SplitVertexCache,
        vertex::pattern::GetPatternCtx,
    },
};
use context_trace_legacy::*;
use derive_more::{
    Deref,
    DerefMut,
};

/// Context for frontier operations during join planning.
///
/// The graph is only read; proposed structure is recorded in `plan`.
#[derive(Debug)]
pub(crate) struct LockedFrontierCtx<'a> {
    pub(crate) trav: &'a Hypergraph,
    pub(crate) interval: &'a IntervalGraph,
    pub(crate) splits: &'a mut SplitMap,
    pub(crate) intervals: &'a mut IntervalMap,
    pub(crate) plan: &'a mut JoinPlan,
}
impl<'a> LockedFrontierCtx<'a> {
    pub(crate) fn new(ctx: &'a mut FrontierSplitIterator) -> Self {
        Self {
            trav: &*ctx.trav,
            interval: &ctx.frontier.interval,
            splits: &mut ctx.splits,
            intervals: &mut ctx.intervals,
            plan: &mut ctx.plan,
        }
    }
}
#[derive(Debug, Deref, DerefMut)]
pub(crate) struct NodeJoinCtx<'a> {
    #[deref]
    #[deref_mut]
    pub(crate) ctx: LockedFrontierCtx<'a>,
    pub(crate) index: Token,
    /// The node's decompositions with proposed replacements applied.
    pub(crate) overlay: NodeOverlay,
}
impl HasToken for NodeJoinCtx<'_> {
    fn token(&self) -> Token {
        self.index
    }
}
impl<'a> NodeJoinCtx<'a> {
    pub(crate) fn new(
        index: Token,
        ctx: &'a mut FrontierSplitIterator,
    ) -> Self {
        let overlay =
            NodeOverlay::new(index, &ctx.trav.expect_child_patterns(index));
        NodeJoinCtx {
            index,
            ctx: LockedFrontierCtx::new(ctx),
            overlay,
        }
    }
}
impl GetPatternCtx for NodeJoinCtx<'_> {
    type PatternCtx = PatternJoinCtx;

    fn get_pattern_context(
        &self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx {
        PatternJoinCtx {
            loc: self.index.to_pattern_location(*pattern_id),
            pattern: self.overlay.pattern(pattern_id).to_vec(),
            splits: self.splits.clone(),
            intervals: self.intervals.clone(),
        }
    }
}

impl NodeJoinCtx<'_> {
    pub(crate) fn vertex_cache(&self) -> &SplitVertexCache {
        self.interval.cache.get(&self.index.vertex_index()).unwrap()
    }

    /// Propose replacing the overlay range `range` of `pid` by `replacement`.
    pub(crate) fn propose_replacement(
        &mut self,
        pid: PatternId,
        range: Range<usize>,
        replacement: JoinRef,
    ) {
        let original = self.overlay.replace(pid, range, replacement);
        let target = self.index;
        self.ctx.plan.push_edit(PlanEdit::ReplaceRange {
            target,
            pattern_id: pid,
            range: original,
            replacement,
            origin: PatternOrigin::PerfectReplacement,
        });
    }
}
