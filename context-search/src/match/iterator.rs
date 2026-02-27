use crate::{
    compare::state::CompareState,
    cursor::Matched,
    policy::SearchKind,
    r#match::{
        root_cursor::CompareParentBatch,
        NodeConsumer,
        NodeResult::{
            self,
            *,
        },
        SearchNode::{
            self,
        },
        SearchQueue,
    },
};
use context_trace::*;
use tracing::{
    debug,
    trace,
};

/// Result of processing a single BFS node from the search queue.
///
/// Returned by [`SearchIterator::pop_and_process_one`] so that the caller
/// (typically [`SearchState`]) can emit graph-op visualization events for
/// each intermediate step.
#[derive(Debug)]
pub(crate) enum BfsStepResult {
    /// Node was consumed but didn't produce a match — more nodes were added
    /// to the queue (parent exploration or child decomposition).
    Expanded {
        /// Vertex index of the node that was processed.
        node_index: usize,
        /// `true` if the node was a `ParentCandidate`, `false` for `ChildCandidate`.
        is_parent: bool,
    },
    /// Found a root match.
    FoundMatch(CompareState<Matched, Matched>),
    /// Node was skipped (comparison mismatch).
    Skipped {
        node_index: usize,
        is_parent: bool,
    },
    /// Queue is empty — no more nodes to process.
    Empty,
}

#[derive(Debug)]
pub(crate) struct SearchIterator<K: SearchKind> {
    pub(crate) trace_ctx: TraceCtx<K::Trav>,
    pub(crate) queue: SearchQueue,
}
impl<K: SearchKind> SearchIterator<K> {
    pub(crate) fn new(
        trav: K::Trav,
        start_index: Token,
        p: CompareParentBatch,
    ) -> Self {
        SearchIterator {
            trace_ctx: TraceCtx {
                trav,
                cache: TraceCache::new(start_index),
            },
            queue: SearchQueue {
                nodes: FromIterator::from_iter(
                    p.into_compare_batch()
                        .into_iter()
                        .map(SearchNode::ParentCandidate),
                ),
            },
        }
    }
}

impl<K: SearchKind> SearchIterator<K>
where
    K::Trav: Clone,
{
    /// Process a single node from the BFS queue.
    ///
    /// Returns a [`BfsStepResult`] describing what happened. On [`Expanded`],
    /// the new nodes have already been pushed into the internal queue.
    pub(crate) fn pop_and_process_one(&mut self) -> BfsStepResult {
        let node = match self.queue.nodes.pop() {
            Some(n) => n,
            None => return BfsStepResult::Empty,
        };

        let node_index = node.root_parent().index.0;
        let is_parent = matches!(node, SearchNode::ParentCandidate(_));
        let queue_remaining = self.queue.nodes.len();

        debug!(
            node_index,
            is_parent,
            queue_remaining,
            "dequeued search node"
        );

        match NodeConsumer::<'_, K>::new(node, &self.trace_ctx.trav).consume() {
            Some(QueueMore(next)) => {
                debug!(
                    node_index,
                    is_parent,
                    num_added = next.len(),
                    "node expanded — queuing new candidates"
                );
                self.queue.nodes.extend(next);
                BfsStepResult::Expanded { node_index, is_parent }
            },
            Some(NodeResult::FoundMatch(matched_state)) => {
                let root = matched_state.child.current().child_state.root_parent();
                debug!(
                    node_index,
                    %root,
                    "found root match"
                );
                BfsStepResult::FoundMatch(*matched_state)
            },
            Some(Skip) => {
                debug!(node_index, is_parent, "node skipped (mismatch)");
                BfsStepResult::Skipped { node_index, is_parent }
            },
            None => {
                debug!(node_index, "node consumed with no result");
                BfsStepResult::Empty
            },
        }
    }

    pub(crate) fn find_next_root_match(
        &mut self
    ) -> Option<CompareState<Matched, Matched>> {
        trace!("finding next root match");
        loop {
            match self.pop_and_process_one() {
                BfsStepResult::Expanded { .. } => continue,
                BfsStepResult::FoundMatch(matched_state) => {
                    return Some(matched_state);
                },
                BfsStepResult::Skipped { .. } => continue,
                BfsStepResult::Empty => {
                    trace!("no root cursor found, iteration complete");
                    return None;
                },
            }
        }
    }
}
impl<K: SearchKind> Iterator for SearchIterator<K>
where
    K::Trav: Clone,
{
    type Item = CompareState<Matched, Matched>;

    fn next(&mut self) -> Option<Self::Item> {
        // Find a root cursor by iterating through the queue
        let matched_state = self.find_next_root_match()?;

        // Clear the queue - all better matches are explored via this root cursor and its parent exploration
        debug!(
            "Found matching root - clearing search queue (will explore via parents)"
        );
        self.queue.nodes.clear();

        let root_parent =
            matched_state.child.current().child_state.root_parent();
        debug!(
            root_parent = %root_parent,
            root_width = root_parent.width.0,
            "found matching root - creating RootCursor"
        );

        // Create RootCursor for this root - matched_state already has Matched cursors from CompareEndResult
        Some(matched_state)
    }
}
