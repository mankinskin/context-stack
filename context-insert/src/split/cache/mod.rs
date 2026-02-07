use crate::{
    join::context::node::merge::{
        PartitionRange,
        RequiredPartitions,
    },
    split::{
        cache::vertex::SplitVertexCache,
        trace::{
            SplitTraceState,
            states::context::SplitTraceStatesCtx,
        },
        vertex::{
            node::NodeTraceCtx,
            output::RootMode,
        },
    },
};
use context_trace::*;
use derive_more::derive::{
    Deref,
    DerefMut,
};
use derive_new::new;
use std::fmt::Debug;
use tracing::debug;

pub(crate) mod vertex;

pub(crate) mod leaves;
pub mod position;

#[derive(Debug, Deref, DerefMut, new, Clone, PartialEq, Eq)]
pub struct SplitCache {
    pub(crate) root_mode: RootMode,
    #[deref]
    #[deref_mut]
    pub(crate) entries: HashMap<VertexIndex, SplitVertexCache>,
}
impl SplitCache {
    pub(crate) fn augment_node(
        &mut self,
        trav: impl HasGraph,
        index: Token,
    ) -> Vec<SplitTraceState> {
        let graph = trav.graph();
        let ctx = NodeTraceCtx::from_index(&graph, index);
        self.get_mut(&index.vertex_index())
            .unwrap()
            .node_augmentation(ctx)
    }
    /// complete inner range offsets for root
    pub(crate) fn augment_root(
        &mut self,
        trav: impl HasGraph,
        root: Token,
    ) -> (Vec<SplitTraceState>, PartitionRange, RequiredPartitions) {
        let graph = trav.graph();
        let ctx = NodeTraceCtx::from_index(&graph, root);
        let index = root.vertex_index();
        let root_mode = self.root_mode;
        self.get_mut(&index)
            .unwrap()
            .root_augmentation(ctx, root_mode)
    }
    pub(crate) fn augment_nodes<G: HasGraph, I: IntoIterator<Item = Token>>(
        &mut self,
        ctx: &mut SplitTraceStatesCtx<G>,
        nodes: I,
    ) {
        debug!("augment_nodes");
        for c in nodes {
            let new = self.augment_node(&ctx.trav, c);
            // todo: force order
            ctx.states.queue.extend(new.into_iter());
        }
    }
}
