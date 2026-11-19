//! Unused cover-search variant retained outside the active oracle pipeline.
//!
//! Despite its name, this implementation descends into children, like the child
//! cover search; it does not walk upward through containing parents.

use context_trace::legacy::{
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

use super::ChildCover;

/// Standalone downward cover traversal, currently not invoked by corpus parsing.
#[derive(Debug)]
pub(crate) struct ParentCoverPass<'a> {
    /// Vocabulary and current labels used to select descendants.
    pub(crate) ctx: &'a LabellingCtx,
    /// Vertex whose proper children seed the traversal.
    pub(crate) root: VertexKey,
    /// Labelled descendants indexed by root-relative character offset.
    pub(crate) cover: ChildCover,
}
impl<'a> ParentCoverPass<'a> {
    /// Initialize an empty cover for a root.
    pub(crate) fn new(
        ctx: &'a LabellingCtx,
        root: VertexKey,
    ) -> Self {
        Self {
            ctx,
            root,
            cover: Default::default(),
        }
    }
}
impl TraversalPass for ParentCoverPass<'_> {
    type Node = (usize, NGramId);
    type NextNode = (usize, NGramId);
    type Queue = LayeredQueue<Self>;
    /// Borrow the context used by this standalone cover traversal.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Seed positioned children, rather than parents, of the supplied root.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        Ok(Self::Queue::from_iter(TopDown::next_nodes(
            &self.ctx.vocab().expect_vertex(&self.root),
        )))
    }
    /// Prune covered spans, retain labels, and expand other descendants.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        let &(off, node) = node;
        // check if covered
        Ok(if self.cover.any_covers(off, node) {
            None
        } else if self.ctx.labels().contains(&node) {
            self.cover.insert(off, node);
            None
        } else {
            let ne = self.ctx.vocab().get_vertex(&node).unwrap();
            Some(
                TopDown::next_nodes(&ne)
                    .into_iter()
                    .map(|(o, c)| (o + off, c))
                    .collect(),
            )
        })
    }

    /// Drain successive layers with cancellation checks before processing nodes.
    fn run(&mut self) -> RunResult<()> {
        self.begin_run();
        let mut queue = self.start_queue()?;

        while !queue.is_empty() {
            while let Some(node) = queue.pop_front() {
                self.ctx().check_cancelled()?;
                if self.node_condition(node) {
                    if let Some(next) = self.on_node(&node)? {
                        queue.extend_layer(next);
                    }
                }
            }
            queue.finish_layer()
        }
        self.finish_run()
    }
}
