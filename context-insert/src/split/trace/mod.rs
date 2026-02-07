pub mod states;

use derive_more::derive::{
    Deref,
    DerefMut,
};

use crate::split::{
    cache::position::PosKey,
    vertex::{
        VertexSplitCtx,
        output::NodeType,
        position::Offset,
    },
};
use context_trace::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitTraceState {
    pub(crate) index: Token,
    pub(crate) offset: Offset,
    pub(crate) prev: PosKey,
}

#[derive(Debug, Deref, DerefMut)]
pub(crate) struct SplitTraceCtx<G: HasGraph> {
    pub(crate) root: Token,
    pub(crate) end_bound: AtomPosition,

    #[deref]
    #[deref_mut]
    pub(crate) ctx: TraceCtx<G>,
}

impl<G: HasGraph> SplitTraceCtx<G> {
    pub(crate) fn get_node<'a, N: NodeType>(
        &'a self,
        index: &Token,
    ) -> Option<VertexSplitCtx<'a>> {
        self.cache
            .entries
            .get(&index.vertex_index())
            .map(VertexSplitCtx::new)
    }
    pub(crate) fn completed_splits<N: NodeType>(
        &self,
        index: &Token,
    ) -> N::CompleteSplitOutput {
        self.get_node::<N>(index)
            .map(|ctx| ctx.complete_splits::<_, N>(&self.trav, self.end_bound))
            .unwrap_or_default()
    }
}
