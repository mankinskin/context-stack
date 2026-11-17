//! Promote partition gap tokens and their shared descendants into global labels.
//!
//! A new pass is run for the unlabelled children of one partitioned node, not
//! once for the whole corpus. Its roots are gap-token identities, not necessarily
//! corpus roots. Label mutations are visible to later partition construction.

use context_trace::{
    graph::{
        vertex::{
            data::{
                VertexData,
                VertexDataBuilder,
            },
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            has_vertex_key::HasVertexKey,
            key::VertexKey,
            pattern::Pattern,
            token::Token,
            wide::Wide,
            ChildPatterns,
            VertexIndex,
        },
        Hypergraph,
    },
    HashMap,
    HashSet,
    VertexSet,
};
use itertools::Itertools;
use pretty_assertions::assert_matches;
use range_ext::intersect::Intersect;
use std::{
    cmp::{
        Ordering,
        Reverse,
    },
    collections::VecDeque,
    fmt::{
        Display,
        Formatter,
    },
    hash::Hash,
    num::NonZeroUsize,
    ops::Range,
};
use tokio_util::sync::CancellationToken;

use derive_more::{
    Deref,
    DerefMut,
    IntoIterator,
};
use derive_new::new;

use crate::graph::{
    labelling::LabellingCtx,
    partitions::{
        NodePartitionCtx,
        PartitionsCtx,
    },
    trace,
    traversal::{
        direction::{
            TopDown,
            TraversalDirection,
        },
        pass::{
            RunResult,
            TraversalPass,
        },
        queue::{
            LayeredQueue,
            LinearQueue,
            Queue,
        },
        visited::VisitTracking,
    },
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
            VocabEntry,
        },
        NGramId,
        ProcessStatus,
        Vocabulary,
    },
};

use super::cover::ChildCover;

/// Per-gap search state used to recognize descendants shared with other gaps.
#[derive(Debug, Default)]
pub(crate) struct DedupRoot {
    /// Selected spans that stop further expansion within this gap.
    pub(crate) cover: ChildCover,
    /// Descendant identities seen under this gap, independent of their offsets.
    pub(crate) visited: HashSet<VertexKey>,
}
/// Interleaved downward traversal over a batch of newly required gap tokens.
#[derive(Debug)]
pub(crate) struct ChildDedupPass<'a> {
    /// Global label set and containment vocabulary.
    pub(crate) ctx: &'a mut LabellingCtx,
    /// Search state keyed by distinct supplied gap-token identity.
    pub(crate) roots: HashMap<VertexKey, DedupRoot>,
}

impl<'a> ChildDedupPass<'a> {
    /// Create empty search state, collapsing duplicate supplied root identities.
    pub(crate) fn new(
        ctx: &'a mut LabellingCtx,
        roots: impl IntoIterator<Item = VertexKey>,
    ) -> Self {
        let roots: Vec<_> = roots.into_iter().collect();
        Self {
            ctx,
            roots: roots
                .iter()
                .map(|root| (*root, DedupRoot::default()))
                .collect(),
        }
    }
}
impl TraversalPass for ChildDedupPass<'_> {
    type Node = (VertexKey, usize, NGramId);
    type NextNode = (VertexKey, usize, NGramId);
    type Queue = LinearQueue<Self>;
    /// Expose shared vocabulary and cooperative cancellation to the driver.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Label every supplied gap and queue its proper children in hash-map order.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        Ok(Self::Queue::from_iter(
            //self.roots.iter()
            //    .map(|&root| (0, NGramId::new(root, self.ctx.vocab.expect_vertex(&root).width())))
            self.roots.iter().flat_map(|(root, tree)| {
                let inserted = self.ctx.labels_mut().insert(*root);
                let entry = self.ctx.vocab().expect_vertex(root);
                if inserted {
                    trace::labelled_gap(&entry.ngram);
                }
                TopDown::next_nodes(&entry)
                    .into_iter()
                    .map(|(p, n)| (*root, p, n))
            }),
        ))
    }
    /// Visit each descendant identity once per gap, regardless of its offset.
    fn node_condition(
        &mut self,
        (root, _, node): Self::Node,
    ) -> bool {
        self.roots
            .get_mut(&root)
            .unwrap()
            .visited
            .insert(node.vertex_key())
    }
    /// Stop at labels or covered spans and label descendants seen under another gap.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        let &(root, off, node) = node;
        let entry = self.ctx.vocab().get_vertex(&node).unwrap();
        let (this_tree, other_trees): (Vec<_>, Vec<_>) =
            self.roots.iter_mut().partition(|(k, _)| **k == root);

        let tree = this_tree.into_iter().next().unwrap().1;
        let other_trees = other_trees.into_iter().map(|(k, v)| v).collect_vec();

        // Cross-gap detection uses visits so far, not a precomputed intersection.
        let next = if node.vertex_key() == root {
            {
                tree.cover.insert(off, node);
                Some(())
            }
        } else {
            {
                (!tree.cover.any_covers(off, node))
                    .then_some(())
                    .and_then(|_| {
                        (!other_trees.iter().any(|r| r.visited.contains(&node)))
                            .then_some(())
                            .or_else(|| {
                                if self.ctx.labels_mut().insert(node.vertex_key()) {
                                    trace::labelled_shared_descendant(
                                        self.ctx.vocab(),
                                        node,
                                        root,
                                        off,
                                    );
                                }
                                tree.cover.insert(off, node);
                                None
                            })
                    })
                    .and_then(|_| {
                        (!self.ctx.labels().contains(&node))
                            .then_some(())
                            .or_else(|| {
                                tree.cover.insert(off, node);
                                None
                            })
                    })
            }
        };
        //self.visited.insert(node.vertex_key());
        Ok(next.map(|_| {
            let ne = self.ctx.vocab().get_vertex(&node).unwrap();
            TopDown::next_nodes(&ne)
                .into_iter()
                .map(|(o, c)| (root, o + off, c))
                .collect()
        }))
    }
    //fn finish_run(&mut self) {
    //    self.roots.iter().for_each(|&root| {
    //        self.covers
    //            .get_mut(&root).unwrap()
    //            .insert(
    //                0,
    //                NGramId::new(
    //                    root,
    //                    self.ctx.vocab.expect_vertex(&root).width(),
    //                )
    //            );
    //    })
    //}
}
