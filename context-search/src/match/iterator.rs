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
    pub(crate) fn find_next_root(
        &mut self
    ) -> Option<CompareState<Matched, Matched>> {
        trace!("finding next match");
        self.find_map(Some)
    }
}

impl<K: SearchKind> SearchIterator<K>
where
    K::Trav: Clone,
{
    pub fn find_next_root_match(
        &mut self
    ) -> Option<CompareState<Matched, Matched>> {
        debug!("finding next root match");
        loop {
            match self.queue.nodes.pop().and_then(|node| {
                NodeConsumer::<'_, K>::new(node, &self.trace_ctx.trav).consume()
            }) {
                Some(QueueMore(next)) => {
                    self.queue.nodes.extend(next);
                    continue;
                },
                Some(NodeResult::FoundMatch(matched_state)) => {
                    // Found a root match
                    return Some(matched_state);
                },
                Some(Skip) => continue,
                None => {
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
