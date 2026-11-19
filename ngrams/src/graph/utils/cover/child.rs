//! Find largest labelled descendants without expanding already-covered spans.

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

/// Layered downward search for a positioned cover of a root's proper children.
#[derive(Debug)]
pub(crate) struct ChildCoverPass<'a> {
    /// Read-only vocabulary and label selection for this search.
    pub(crate) ctx: &'a LabellingCtx,
    /// Containing vertex; the root itself is not a candidate.
    pub(crate) root: VertexKey,
    /// Selected descendants accumulated at root-relative character offsets.
    pub(crate) cover: ChildCover,
}
impl<'a> ChildCoverPass<'a> {
    /// Start a search with no selected descendants.
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
impl TraversalPass for ChildCoverPass<'_> {
    type Node = (usize, NGramId);
    type NextNode = (usize, NGramId);
    type Queue = LayeredQueue<Self>;
    /// Borrow the search's read-only labelling context.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Seed the search with the root's immediate positioned substrings.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        Ok(Self::Queue::from_iter(TopDown::next_nodes(
            &self.ctx.vocab().expect_vertex(&self.root),
        )))
    }
    /// Retain a labelled child or descend further if its span remains uncovered.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        let &(off, node) = node;
        // Partial overlaps remain eligible; only full containment prunes a node.
        Ok(if self.cover.any_covers(off, node) {
            None
        } else if self.ctx.labels().contains(&node) {
            trace::selected_child(self.ctx.vocab(), node, off);
            self.cover.insert(off, node);
            None
        } else {
            let ne = self.ctx.vocab().get_vertex(&node).unwrap();
            trace::descend_through_child(&ne.ngram, off);
            // Child offsets are local to node; translate them back to the root.
            Some(
                TopDown::next_nodes(&ne)
                    .into_iter()
                    .map(|(o, c)| (o + off, c))
                    .collect(),
            )
        })
    }
}
