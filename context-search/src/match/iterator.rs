use crate::{
    r#match::{
        root_cursor::CompareParentBatch,
        RootFinder,
        SearchNode::{
            self,
            ParentCandidate,
        },
        SearchQueue,
    },
    state::matched::MatchedEndState,
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
    /// Best checkpoint found so far during hierarchical search
    /// Updated whenever a root matches successfully, even if it needs parent exploration
    pub(crate) best_checkpoint: Option<MatchedEndState>,
}
impl<K: SearchKind> SearchIterator<K> {
    //#[context_trace::instrument_sig(skip(trav), fields(start_index = %start_index))]
    //pub(crate) fn start_index(
    //    trav: K::Trav,
    //    start_index: Token,
    //) -> Self {
    //    debug!("creating match iterator from start index");
    //    SearchIterator {
    //        trace_ctx: TraceCtx {
    //            trav,
    //            cache: TraceCache::new(start_index),
    //        },
    //        queue: SearchQueue::new(),
    //        best_checkpoint: None,
    //    }
    //}

    #[context_trace::instrument_sig(skip(trav, p), fields(start_index = %start_index, parent_count = p.len()))]
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
            best_checkpoint: None,
        }
    }
}

impl<K: SearchKind> SearchIterator<K>
where
    K::Trav: Clone,
{
    pub(crate) fn find_next(&mut self) -> Option<MatchedEndState> {
        trace!("finding next match");
        self.find_map(Some)
    }
}

impl<K: SearchKind> Iterator for SearchIterator<K>
where
    K::Trav: Clone,
{
    type Item = MatchedEndState;

    fn next(&mut self) -> Option<Self::Item> {
        trace!("searching for root cursor");

        match RootFinder::<K>::new(&self.trace_ctx.trav, &mut self.queue)
            .find_root_cursor()
        {
            Some(root_cursor) => {
                let root_parent = root_cursor
                    .state
                    .child_cursor
                    .child_state
                    .path
                    .root_parent();
                debug!(
                    root_parent = %root_parent,
                    root_width = root_parent.width.0,
                    "found root cursor - starting advance_to_end"
                );

                Some(match root_cursor.advance_to_end() {
                    Ok(matched_state) => {
                        // RootCursor found a match - query matched at least partially
                        debug!(
                            is_complete = matched_state.query_exhausted(),
                            root_parent = %matched_state.root_parent(),
                            root_width = matched_state.root_parent().width.0,
                            checkpoint_pos = *matched_state.cursor().atom_position.as_ref(),
                            "found matched state from root"
                        );

                        matched_state
                    },
                    Err((checkpoint_state, root_cursor)) => {
                        // RootCursor reached end of root without conclusion
                        // Need to explore parent tokens to continue comparison
                        // checkpoint_state contains the best match found in this root
                        let current_root = root_cursor
                            .state
                            .child_cursor
                            .child_state
                            .path
                            .root_parent();
                        let checkpoint_pos = *root_cursor
                            .state
                            .checkpoint
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

                        // Update best_checkpoint if this is better (smaller width)
                        let should_update = match &self.best_checkpoint {
                            None => true,
                            Some(prev) => {
                                let prev_checkpoint_pos =
                                    *prev.cursor().atom_position.as_ref();
                                // Keep checkpoint with LARGEST checkpoint_pos (most query tokens matched)
                                checkpoint_pos >= prev_checkpoint_pos
                            },
                        };

                        if should_update {
                            debug!(
                                root = %checkpoint_state.root_parent(),
                                width = checkpoint_state.root_parent().width.0,
                                checkpoint_pos = checkpoint_pos,
                                "Updating best_checkpoint from root needing parent exploration"
                            );
                            self.best_checkpoint = Some(checkpoint_state);
                        } else {
                            debug!(
                                root = %checkpoint_state.root_parent(),
                                width = checkpoint_state.root_parent().width.0,
                                "Not updating best_checkpoint - current is better"
                            );
                        }

                        match root_cursor.next_parents(&&self.trace_ctx.trav) {
                            Err(_end_state) => {
                                // No more parents available - exhausted search without match
                                // Don't return anything, continue to next candidate
                                debug!(
                                    root = %current_root,
                                    "no more parents available for this root - continuing to next candidate"
                                );
                                return self.next();
                            },
                            Ok((parent, batch)) => {
                                debug!(
                                    child_root = %current_root,
                                    child_width = current_root.width.0,
                                    parent_batch_size = batch.len(),
                                    "found parent batch - adding to queue for hierarchical expansion"
                                );

                                let parent_widths: Vec<usize> = batch
                                    .parents
                                    .iter()
                                    .map(|p| p.path.root_parent().width.0)
                                    .collect();
                                debug!(
                                    parent_widths = ?parent_widths,
                                    "parent batch widths (will be prioritized by min-heap)"
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

                                // No match to return yet - continue searching
                                // Recursively call next() to process the parent batch
                                return self.next();
                            },
                        }
                    },
                })
            },
            None => {
                trace!("no root cursor found, iteration complete");
                None
            },
        }
    }
}
