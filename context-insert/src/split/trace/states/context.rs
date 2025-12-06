use std::num::NonZeroUsize;

use derive_more::derive::{
    Deref,
    DerefMut,
};

use crate::*;
use context_trace::*;

#[derive(Debug, Deref, DerefMut)]
pub struct SplitTraceStatesCtx<G: HasGraph> {
    #[deref]
    #[deref_mut]
    pub ctx: SplitTraceCtx<G>,
    pub states: SplitStates,
}
impl<G: HasGraph> SplitTraceStatesCtx<G> {
    pub fn new(
        ctx: TraceCtx<G>,
        root: Token,
        end_bound: AtomPosition,
    ) -> Self {
        Self {
            ctx: SplitTraceCtx {
                ctx,
                root,
                end_bound,
            },
            states: SplitStates::default(),
        }
    }
    pub fn new_split_vertex(
        &mut self,
        index: Token,
        offset: NonZeroUsize,
        prev: PosKey,
    ) -> SplitVertexCache {
        let mut subs = self.completed_splits::<InnerNode>(&index);
        subs.entry(offset).or_insert_with(|| {
            let graph = self.ctx.trav.graph();
            let node = graph.expect_vertex(index);
            //let entry = self.cache.entries.get(&index.index).unwrap();
            cleaned_position_splits(node.child_patterns().iter(), offset)
        });
        let pos_splits =
            self.states.leaves.collect_leaves(&index, subs.clone());
        self.states
            .filter_trace_states(&self.ctx.trav, &index, pos_splits);
        SplitVertexCache {
            positions: subs
                .into_iter()
                .map(|(offset, res)| {
                    (
                        offset,
                        SplitPositionCache::new(
                            prev,
                            res.unwrap_or_else(|location| {
                                vec![SubSplitLocation::new(location, None)]
                            }),
                        ),
                    )
                })
                .collect(),
        }
    }
    pub fn new_split_position(
        &mut self,
        index: Token,
        offset: NonZeroUsize,
        prev: PosKey,
    ) -> SplitPositionCache {
        let splits = {
            let graph = self.ctx.trav.graph();
            let node = graph.expect_vertex(index);
            cleaned_position_splits(node.child_patterns().iter(), offset)
        };

        // handle clean splits
        match splits {
            Ok(subs) => {
                self.states.filter_trace_states(
                    &self.ctx.trav,
                    &index,
                    Vec::from_iter([(offset, subs.clone())]),
                );
                SplitPositionCache::new(prev, subs)
            },
            Err(location) => {
                self.states.leaves.insert(PosKey::new(index, offset));
                SplitPositionCache::new(
                    prev,
                    vec![SubSplitLocation::new(location, None)],
                )
            },
        }
    }
}
