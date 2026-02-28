use crate::{
    compare::{
        iterator::CompareEvent,
        state::CompareState,
    },
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

/// Info about a node popped from the BFS queue, before processing.
#[derive(Debug)]
pub(crate) struct PoppedNode {
    pub node: SearchNode,
    pub node_index: usize,
    pub is_parent: bool,
}

/// Result of processing a popped node (after pop, before any visualization).
#[derive(Debug)]
pub(crate) enum ProcessResult {
    /// Node expanded — new candidates added to queue.
    Expanded(Vec<CompareEvent>),
    /// Found a root match.
    FoundMatch(CompareState<Matched, Matched>, Vec<CompareEvent>),
    /// Node skipped (comparison mismatch).
    Skipped(Vec<CompareEvent>),
    /// Node consumed with no result.
    NoResult,
}

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
        /// Compare events from child decomposition.
        compare_events: Vec<CompareEvent>,
    },
    /// Found a root match.
    FoundMatch(CompareState<Matched, Matched>, Vec<CompareEvent>),
    /// Node was skipped (comparison mismatch).
    Skipped {
        node_index: usize,
        is_parent: bool,
        compare_events: Vec<CompareEvent>,
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
    /// Pop a node from the BFS queue without processing it.
    ///
    /// Returns [`PoppedNode`] with node info so the caller can emit
    /// visualization events (e.g. `VisitParent`) before processing.
    pub(crate) fn pop_node(&mut self) -> Option<PoppedNode> {
        let node = self.queue.nodes.pop()?;
        let node_index = node.root_parent().index.0;
        let is_parent = matches!(node, SearchNode::ParentCandidate(_));

        debug!(
            node_index,
            is_parent,
            queue_remaining = self.queue.nodes.len(),
            "popped search node"
        );

        Some(PoppedNode { node, node_index, is_parent })
    }

    /// Process a previously popped node.
    ///
    /// This performs the comparison and queues any new candidates.
    /// Returns a [`ProcessResult`] — the caller already has node_index/is_parent
    /// from the [`PoppedNode`].
    pub(crate) fn process_node(&mut self, popped: PoppedNode) -> ProcessResult {
        match NodeConsumer::<'_, K>::new(popped.node, &self.trace_ctx.trav).consume() {
            Some(QueueMore(next, compare_events)) => {
                debug!(
                    node_index = popped.node_index,
                    is_parent = popped.is_parent,
                    num_added = next.len(),
                    "node expanded — queuing new candidates"
                );
                self.queue.nodes.extend(next);
                ProcessResult::Expanded(compare_events)
            },
            Some(NodeResult::FoundMatch(matched_state, compare_events)) => {
                let root = matched_state.child.current().child_state.root_parent();
                debug!(
                    node_index = popped.node_index,
                    %root,
                    "found root match"
                );
                ProcessResult::FoundMatch(*matched_state, compare_events)
            },
            Some(Skip(compare_events)) => {
                debug!(node_index = popped.node_index, is_parent = popped.is_parent, "node skipped (mismatch)");
                ProcessResult::Skipped(compare_events)
            },
            None => {
                debug!(node_index = popped.node_index, "node consumed with no result");
                ProcessResult::NoResult
            },
        }
    }

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
            "popped search node"
        );

        match NodeConsumer::<'_, K>::new(node, &self.trace_ctx.trav).consume() {
            Some(QueueMore(next, compare_events)) => {
                debug!(
                    node_index,
                    is_parent,
                    num_added = next.len(),
                    "node expanded — queuing new candidates"
                );
                self.queue.nodes.extend(next);
                BfsStepResult::Expanded { node_index, is_parent, compare_events }
            },
            Some(NodeResult::FoundMatch(matched_state, compare_events)) => {
                let root = matched_state.child.current().child_state.root_parent();
                debug!(
                    node_index,
                    %root,
                    "found root match"
                );
                BfsStepResult::FoundMatch(*matched_state, compare_events)
            },
            Some(Skip(compare_events)) => {
                debug!(node_index, is_parent, "node skipped (mismatch)");
                BfsStepResult::Skipped { node_index, is_parent, compare_events }
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
                BfsStepResult::FoundMatch(matched_state, _events) => {
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
