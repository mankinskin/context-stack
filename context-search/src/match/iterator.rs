use crate::{
    r#match::{
        root_cursor::{
            AdvanceToEndResult,
            CompareParentBatch,
        },
        NodeConsumer,
        NodeResult::{
            self,
            *,
        },
        RootCursor,
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
    /// Best match found so far during hierarchical search
    /// Updated whenever a root matches successfully, even if it needs parent exploration
    pub(crate) best_match: Option<MatchResult>,
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
            best_match: None,
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

impl<K: SearchKind> Iterator for SearchIterator<K>
where
    K::Trav: Clone,
{
    type Item = MatchResult;

    fn next(&mut self) -> Option<Self::Item> {
        trace!("searching for root cursor");

        // Find a root cursor by iterating through the queue
        let root_cursor = loop {
            match self.queue.nodes.pop().and_then(|node| {
                NodeConsumer::<'_, K>::new(node, &self.trace_ctx.trav).consume()
            }) {
                Some(QueueMore(next)) => {
                    self.queue.nodes.extend(next);
                    continue;
                },
                Some(NodeResult::FoundMatch(matched_state)) => {
                    // Found a root match - create RootCursor
                    break RootCursor::<K, _, _> {
                        trav: self.trace_ctx.trav.clone(),
                        state: Box::new(*matched_state),
                    };
                },
                Some(Skip) => continue,
                None => {
                    trace!("no root cursor found, iteration complete");
                    return None;
                },
            }
        };

        // Clear the queue - all larger matches will be found by
        // exploring parents of this matched root (graph invariant)
        debug!(
            "Found matching root - clearing search queue (larger matches found via parent exploration)"
        );
        self.queue.nodes.clear();

        let root_parent =
            root_cursor.state.child.current().child_state.root_parent();
        debug!(
            root_parent = %root_parent,
            root_width = root_parent.width.0,
            "found root cursor - advancing both cursors and iterating until conclusion"
        );

        // Try to advance both cursors from the matched state
        let candidate_cursor = match root_cursor.advance_both_cursors() {
            Ok(candidate) => candidate,
            Err(Ok(matched_state)) => {
                // Query exhausted immediately after first match
                debug!(
                    root = %matched_state.root_parent(),
                    "Query exhausted immediately - returning complete match"
                );
                return Some(matched_state);
            },
            Err(Err(need_parent)) => {
                // Child exhausted but query continues - need parent exploration
                let checkpoint_state =
                    need_parent.create_parent_exploration_state();
                debug!(
                    checkpoint_root = %checkpoint_state.root_parent(),
                    checkpoint_width = checkpoint_state.root_parent().width.0,
                    "Child exhausted after first match - need parent exploration"
                );

                // Update best_match
                self.best_match = Some(checkpoint_state.clone());

                // Get parent batch and continue
                match need_parent.get_parent_batch(&self.trace_ctx.trav) {
                    Ok((_parent, batch)) => {
                        self.queue.nodes.extend(
                            batch
                                .into_compare_batch()
                                .into_iter()
                                .map(ParentCandidate),
                        );
                    },
                    Err(_) => {
                        debug!("No parents available - continuing to next candidate");
                    },
                }
                return self.next();
            },
        };

        // Now iterate the candidate cursor until conclusion
        match candidate_cursor.iterate_until_conclusion() {
            AdvanceToEndResult::Completed(matched_state) => {
                // RootCursor found a match - query matched at least partially
                debug!(
                    query_exhausted = matched_state.query_exhausted(),
                    root_parent = %matched_state.root_parent(),
                    root_width = matched_state.root_parent().width.0,
                    checkpoint_pos = *matched_state.cursor().atom_position.as_ref(),
                    "found matched state from root"
                );

                // Update best_match
                self.best_match = Some(matched_state.clone());

                Some(matched_state)
            },
            AdvanceToEndResult::NeedsParentExploration {
                checkpoint: checkpoint_state,
                cursor: root_cursor,
            } => {
                // RootCursor reached end of root without conclusion
                // Need to explore parent tokens to continue comparison
                // checkpoint_state contains the best match found in this root
                let current_root =
                    root_cursor.state.child.current().child_state.root_parent();
                let checkpoint_pos = *root_cursor
                    .state
                    .query
                    .checkpoint()
                    .atom_position
                    .as_ref();

                debug!(
                    current_root = %current_root,
                    current_width = current_root.width.0,
                    checkpoint_pos = checkpoint_pos,
                    checkpoint_root = %checkpoint_state.root_parent(),
                    checkpoint_width = checkpoint_state.root_parent().width.0,
                    "root cursor completed without conclusion - need parent exploration"
                );

                // Update best_match
                self.best_match = Some(checkpoint_state.clone());

                match root_cursor.get_parent_batch(&self.trace_ctx.trav) {
                    Err(_end_state) => {
                        // No more parents available - exhausted search without match
                        // Don't return anything, continue to next candidate
                        debug!(
                            root = %current_root,
                            "no more parents available for this root - continuing to next candidate"
                        );
                    },
                    Ok((_parent, batch)) => {
                        debug!(
                            child_root = %current_root,
                            child_width = current_root.width.0,
                            parent_batch_size = batch.len(),
                            "found parent batch - adding to queue for hierarchical expansion"
                        );

                        trace!(
                            batch_details = %pretty(&batch),
                            "parent batch composition details"
                        );

                        assert!(!batch.is_empty());

                        // Add parent batch to queue for next iteration
                        // Nodes are prioritized by token width (smaller first)
                        self.queue.nodes.extend(
                            batch
                                .into_compare_batch()
                                .into_iter()
                                .map(ParentCandidate),
                        );
                    },
                }

                // No match to return yet - continue searching
                // Recursively call next() to process the parent batch
                self.next()
            },
        }
    }
}
