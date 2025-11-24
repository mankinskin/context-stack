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
        trace!(
            batch_details = %pretty(&p),
            "parent batch composition"
        );
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
    pub(crate) fn find_next(&mut self) -> Option<MatchResult> {
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
    ) -> Option<Box<CompareState<Matched, Matched>>> {
        trace!("finding next root match");
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
    type Item = MatchResult;

    fn next(&mut self) -> Option<Self::Item> {
        trace!("searching for root cursor");

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

        // Create RootCursor for this root - matched_state already has Matched cursors from CompareResult
        let mut root_cursor = RootCursor::<K, _, _> {
            trav: self.trace_ctx.trav.clone(),
            state: matched_state,
        };

        // Set initial root match as baseline for this root
        let mut new_match = root_cursor.create_checkpoint_from_state();
        debug!(
            root = %new_match.root_parent(),
            checkpoint_pos = *new_match.cursor().atom_position.as_ref(),
            "New root match - updating best_match checkpoint"
        );

        // Advance the root cursor step by step, updating best_match after each step
        loop {
            match root_cursor.advance_to_next_match() {
                RootAdvanceResult::Advanced(next_matched) => {
                    // Successfully advanced to next match - always update best_match
                    let checkpoint_state =
                        next_matched.create_checkpoint_from_state();
                    let checkpoint_pos =
                        *checkpoint_state.cursor().atom_position.as_ref();
                    debug!(
                        root = %checkpoint_state.root_parent(),
                        checkpoint_pos,
                        "Match advanced - updating best_match"
                    );
                    new_match = checkpoint_state;

                    // Continue with the new matched cursor
                    root_cursor = next_matched;
                },
                RootAdvanceResult::Finished(end_result) => {
                    // Reached an end condition
                    let new_match = match end_result {
                        RootEndResult::Conclusive(conclusive) => {
                            // Conclusive end - this is the maximum match for the search
                            match conclusive {
                                ConclusiveEnd::Mismatch(candidate_cursor) => {
                                    // Found mismatch after progress - create final MatchResult
                                    // Clone the state before consuming
                                    let final_result = candidate_cursor
                                        .create_end_state(
                                        crate::state::end::EndReason::Mismatch,
                                    );
                                    debug!(
                                        root = %final_result.root_parent(),
                                        checkpoint_pos = *final_result.cursor().atom_position.as_ref(),
                                        "Conclusive end: Mismatch - returning final result"
                                    );

                                    // Continue searching from queue (no parent exploration for mismatch)
                                    new_match
                                },
                                ConclusiveEnd::Exhausted => {
                                    // Query exhausted - best_match should have the final result
                                    debug!("Conclusive end: Exhausted - returning best match");
                                    // Return a clone so best_match remains set for the search layer
                                    new_match
                                },
                            }
                        },
                        RootEndResult::Inconclusive(need_parent_cursor) => {
                            // Root boundary reached - need parent exploration
                            let checkpoint_state = need_parent_cursor
                                .create_parent_exploration_state();
                            let checkpoint_pos = *checkpoint_state
                                .cursor()
                                .atom_position
                                .as_ref();
                            debug!(
                                checkpoint_root = %checkpoint_state.root_parent(),
                                checkpoint_pos,
                                "Inconclusive end - updating best_match"
                            );

                            // Get parent batch and continue searching
                            match need_parent_cursor
                                .get_parent_batch(&self.trace_ctx.trav)
                            {
                                Ok((_parent, batch)) => {
                                    self.queue.nodes.extend(
                                        batch
                                            .into_compare_batch()
                                            .into_iter()
                                            .map(ParentCandidate),
                                    );
                                },
                                Err(_) => {
                                    debug!("No parents available - search exhausted");
                                },
                            }

                            // Continue to next candidate from queue
                            checkpoint_state
                        },
                    };
                    return Some(new_match);
                },
            }
        }
    }
}
