//! Select substrings whose occurrence counts exceed a selected ancestor's count.
//!
//! This is a relative frequency criterion, not a configurable minimum count.

use std::collections::VecDeque;

use derive_more::{
    Deref,
    DerefMut,
    From,
};
use derive_new::new;
use itertools::Itertools;
use tokio_util::sync::CancellationToken;

use crate::graph::{
    containment::TextLocation,
    labelling::LabellingCtx,
    traversal::{
        direction::{
            BottomUp,
            TopDown,
            TraversalDirection,
        },
        pass::{
            RunResult,
            TraversalPass,
        },
        queue::{
            Queue,
            SortedQueue,
        },
    },
    utils::cover::frequency::FrequencyCover,
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
        },
        NGramId,
        ProcessStatus,
        Vocabulary,
    },
};
use context_trace::{
    graph::vertex::{
        has_vertex_index::HasVertexIndex,
        has_vertex_key::HasVertexKey,
        key::VertexKey,
        location::child::ChildLocation,
        token::Token,
        wide::Wide,
        VertexIndex,
    },
    HashSet,
};

/// Top-down labelling pass using a decreasing-width work queue.
#[derive(Debug, Deref, new, DerefMut)]
pub(crate) struct FrequencyCtx<'b> {
    /// Vocabulary and shared label set updated by this pass.
    #[deref]
    #[deref_mut]
    pub(crate) ctx: &'b mut LabellingCtx,
}

impl TraversalPass for FrequencyCtx<'_> {
    type Node = VertexKey;
    type NextNode = NGramId;
    type Queue = SortedQueue;
    /// Expose the common context to traversal cancellation checks.
    fn ctx(&self) -> &LabellingCtx {
        self.ctx
    }
    /// Expand roots to seed the queue, then ensure every root is labelled.
    fn start_queue(&mut self) -> RunResult<Self::Queue> {
        let start = TopDown::starting_nodes(self.vocab());

        let mut queue = SortedQueue::default();
        for node in start.iter() {
            queue.extend_layer(self.on_node(node)?.unwrap_or_default());
        }
        self.corpus
            .image
            .labels
            .extend(start.iter().map(HasVertexKey::vertex_key));

        self.status.next_pass(
            ProcessStatus::Frequency,
            0,
            self.image.vocab.containment.vertex_count(),
        );
        Ok(queue)
    }
    /// Test an unlabelled candidate and schedule its immediate smaller substrings.
    ///
    /// Already-labelled nodes are not expanded by this invocation.
    fn on_node(
        &mut self,
        node: &Self::Node,
    ) -> RunResult<Option<Vec<Self::NextNode>>> {
        *self.status.steps_mut() += 1;
        Ok(if self.labels().contains(node) {
            None
        } else {
            let entry = self.vocab().get_vertex(node).unwrap();
            let next = self.entry_next(&entry);
            if self.entry_is_frequent(&entry)? {
                let key = entry.data.vertex_key();
                self.labels_mut().insert(key);
            }
            Some(next)
        })
    }
    /// Announce the stage on stderr without modifying the queue.
    fn begin_run(&mut self) {
        eprintln!("Frequency Pass");
    }
    /// Retain every atom regardless of its relative occurrence count.
    fn finish_run(&mut self) -> RunResult<()> {
        let bottom = BottomUp::starting_nodes(self.vocab());
        self.labels_mut()
            .extend(bottom.iter().map(HasVertexKey::vertex_key));
        Ok(())
    }
}
impl FrequencyCtx<'_> {
    /// Collect immediate smaller substrings, discarding positional offsets.
    pub(crate) fn entry_next(
        &self,
        entry: &VertexCtx,
    ) -> Vec<NGramId> {
        TopDown::next_nodes(entry)
            .into_iter()
            .map(|(_, c)| c)
            .collect()
    }
    /// Whether any ancestor in the occurrence cover has fewer occurrences.
    ///
    /// Counts include overlapping positions across the entire corpus. A selected
    /// candidate is also printed to stderr for diagnostics.
    pub(crate) fn entry_is_frequent(
        &self,
        entry: &VertexCtx,
    ) -> RunResult<bool> {
        FrequencyCover::from_entry(self, entry).map(|cover| {
            cover
                .iter()
                .any(|p| {
                    self.vocab().get_vertex(p).unwrap().count() < entry.count()
                })
                .then(|| eprintln!("{}", entry.ngram))
                .is_some()
        })
    }
}
