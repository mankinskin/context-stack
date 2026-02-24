use itertools::Itertools;
use linked_hash_set::LinkedHashSet;

use crate::{
    interval::IntervalGraph,
    join::context::node::{
        context::NodeJoinCtx,
        merge::context::{
            MergeCtx,
            MergeMode,
        },
    },
    split::{
        SplitMap,
        cache::position::PosKey,
    },
    visualization::emit_insert_node,
};
use context_trace::{
    graph::visualization::Transition,
    *,
};

pub(crate) struct FrontierIterator {
    pub(crate) frontier: LinkedHashSet<PosKey>,
    pub(crate) interval: IntervalGraph,
}
impl Iterator for FrontierIterator {
    type Item = Option<PosKey>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.frontier.pop_front() {
            Some(key) =>
                Some(match (key.index != self.interval.root).then_some(key) {
                    Some(key) => {
                        let top = self
                            .interval
                            .expect(&key)
                            .top
                            .iter()
                            .sorted_by(|a, b| {
                                a.index.width().cmp(&b.index.width())
                            })
                            .cloned();
                        self.frontier.extend(top);
                        Some(key)
                    },
                    None => None,
                }),
            None => None,
        }
    }
}
pub(crate) struct FrontierSplitIterator {
    pub(crate) frontier: FrontierIterator,
    pub(crate) splits: SplitMap,
    pub(crate) trav: HypergraphRef,
}

impl FrontierSplitIterator {
    fn node(
        &mut self,
        index: Token,
    ) -> NodeJoinCtx<'_> {
        NodeJoinCtx::new(index, self)
    }
}
impl Iterator for FrontierSplitIterator {
    type Item = Option<Token>;
    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.frontier.next() {
            Some(Some(key)) => {
                if !self.splits.contains_key(&key) {
                    let node_idx = key.index.index.0;

                    // Emit event for processing this node
                    emit_insert_node(
                        Transition::JoinStep {
                            left: node_idx,
                            right: node_idx,
                            result: node_idx,
                        },
                        format!(
                            "Processing node {} at position {}",
                            node_idx,
                            key.pos.get()
                        ),
                        node_idx,
                    );

                    let ctx = self.node(key.index);
                    // Use shared initial partition creation
                    let partitions =
                        MergeCtx::new(ctx, MergeMode::Full).merge_node();

                    for (key, split) in partitions {
                        self.splits.insert(key, split);
                    }
                }
                None
            },
            Some(None) => None,
            None => Some({
                let root_idx = self.frontier.interval.root.index.0;

                // Emit event for root merge
                emit_insert_node(
                    Transition::JoinStep {
                        left: root_idx,
                        right: root_idx,
                        result: root_idx,
                    },
                    format!("Merging root node {}", root_idx),
                    root_idx,
                );

                let ctx = self.node(self.frontier.interval.root);
                let root_mode = ctx.interval.cache.root_mode;
                MergeCtx::new(ctx, MergeMode::Root(root_mode)).merge_root()
            }),
        })
    }
}
impl From<(HypergraphRef, IntervalGraph)> for FrontierSplitIterator {
    fn from((trav, interval): (HypergraphRef, IntervalGraph)) -> Self {
        let leaves = interval.states.leaves.iter().cloned().rev();
        FrontierSplitIterator {
            frontier: FrontierIterator {
                frontier: LinkedHashSet::from_iter(leaves),
                interval,
            },
            splits: Default::default(),
            trav,
        }
    }
}
