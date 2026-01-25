use std::borrow::Borrow;

use crate::{
    interval::IntervalGraph,
    join::context::{
        frontier::FrontierSplitIterator,
        pattern::PatternJoinCtx,
    },
    split::{
        SplitMap,
        cache::vertex::SplitVertexCache,
        vertex::{
            node::{
                AsNodeTraceCtx,
                NodeTraceCtx,
            },
            pattern::{
                GetPatternCtx,
                GetPatternTraceCtx,
                PatternTraceCtx,
            },
        },
    },
};
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};

/// Context for locked frontier operations during join.
///
/// With interior mutability, we only need `&Hypergraph` - mutations happen
/// through per-vertex locks inside the graph.
#[derive(Debug)]
pub struct LockedFrontierCtx<'a> {
    pub trav: &'a Hypergraph,
    pub interval: &'a IntervalGraph,
    pub splits: &'a SplitMap,
}
impl<'a> LockedFrontierCtx<'a> {
    pub fn new(ctx: &'a mut FrontierSplitIterator) -> Self {
        Self {
            trav: &*ctx.trav,
            interval: &ctx.frontier.interval,
            splits: &ctx.splits,
        }
    }
}
#[derive(Debug, Deref, DerefMut)]
pub struct NodeJoinCtx<'a> {
    #[deref]
    #[deref_mut]
    pub ctx: LockedFrontierCtx<'a>,
    pub index: Token,
}
impl HasToken for NodeJoinCtx<'_> {
    fn token(&self) -> Token {
        self.index
    }
}
impl<'a> NodeJoinCtx<'a> {
    pub fn new(
        index: Token,
        ctx: &'a mut FrontierSplitIterator,
    ) -> Self {
        NodeJoinCtx {
            index,
            ctx: LockedFrontierCtx::new(ctx),
        }
    }
}
impl AsNodeTraceCtx for NodeJoinCtx<'_> {
    fn as_trace_context(&self) -> NodeTraceCtx {
        NodeTraceCtx::new(self.patterns(), self.borrow().index)
    }
}
impl GetPatternTraceCtx for NodeJoinCtx<'_> {
    fn get_pattern_trace_context(
        &self,
        pattern_id: &PatternId,
    ) -> PatternTraceCtx {
        PatternTraceCtx::new(
            self.index.to_pattern_location(*pattern_id),
            self.as_trace_context()
                .patterns
                .get(pattern_id)
                .unwrap()
                .clone(),
        )
    }
}
impl GetPatternCtx for NodeJoinCtx<'_> {
    type PatternCtx = PatternJoinCtx;

    fn get_pattern_context(
        &self,
        pattern_id: &PatternId,
    ) -> Self::PatternCtx {
        let ctx = self.get_pattern_trace_context(pattern_id);
        PatternJoinCtx {
            ctx,
            splits: self.splits.clone(),
        }
    }
}
impl NodeJoinCtx<'_> {
    /// Get the child patterns for the current node.
    /// Returns owned data since graph access returns owned values now.
    pub fn patterns(&self) -> ChildPatterns {
        self.ctx.trav.expect_child_patterns(self.index)
    }
}

impl NodeJoinCtx<'_> {
    pub fn vertex_cache(&self) -> &SplitVertexCache {
        self.interval.cache.get(&self.index.vertex_index()).unwrap()
    }
}
