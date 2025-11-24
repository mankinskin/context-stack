use crate::{
    compare::state::CompareState,
    cursor::Matched,
    r#match::{
        root_cursor::{
            CompareParentBatch,
            ConclusiveEnd,
            RootAdvanceResult,
            RootCursor,
            RootEndResult,
        },
        NodeConsumer,
        NodeResult::{
            self,
            *,
        },
        SearchNode::{
            self,
            ParentCandidate,
        },
        SearchQueue,
    },
    state::matched::MatchResult,
    traversal::SearchKind,
};
use context_trace::{
    logging::format_utils::pretty,
    *,
};
use derive_new::new;
use tracing::{
    debug,
    trace,
    warn,
};

#[derive(Debug, new)]
pub(crate) struct SearchIterator<K: SearchKind> {
    pub(crate) trace_ctx: TraceCtx<K::Trav>,
    pub(crate) queue: SearchQueue,
}
impl<K: SearchKind> SearchIterator<K> {
    #[context_trace::instrument_sig(level = "debug", skip(trav, p), fields(start_index = %start_index, parent_count = p.len()))]
    pub(crate) fn start_parent(
        trav: K::Trav,
        start_index: Token,
        p: CompareParentBatch,
    ) -> Self {
        debug!("creating match iterator from parent batch");
        trace!(parent_batch.len = p.len(),);
        //trace!(
        //    batch_details = %pretty(&p),
        //    "parent batch composition"
        //);
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
