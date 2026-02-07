use std::marker::PhantomData;

use crate::{
    compare::state::CompareState,
    cursor::{
        checkpointed::{
            Checkpointed,
            HasCandidate,
        },
        Matched,
        PatternCursor,
    },
    r#match::{
        iterator::SearchIterator,
        root_cursor::{
            ConclusiveEnd,
            RootAdvanceResult,
            RootCursor,
            RootEndResult,
        },
        SearchNode::ParentCandidate,
    },
    state::{
        end::PathCoverage,
        matched::{
            CheckpointedCursor,
            MatchResult,
        },
    },
    Response,
    SearchKind,
};
use context::{
    AncestorSearchTraversal,
    ParentSearchTraversal,
    SearchCtx,
};
use context_trace::{
    logging::format_utils::pretty,
    *,
};
pub(crate) use searchable::Searchable;

use tracing::{
    debug,
    info,
    trace,
};
pub mod context;
pub(crate) mod searchable;

pub(crate) type SearchResult = Result<Response, ErrorReason>;

pub trait Find: HasGraph {
    fn ctx(&self) -> SearchCtx<Self>;

    /// find largest matching parent for pattern
    #[context_trace::instrument_sig(level = "info", skip(self, searchable))]
    fn find_parent(
        &self,
        searchable: impl Searchable<ParentSearchTraversal<Self>>,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting parent search");
        let result = searchable.search(self.ctx()).map_err(|err| err.reason);

        match &result {
            Ok(_response) => debug!("parent search succeeded"),
            Err(reason) =>
                debug!(reason = %pretty(reason), "parent search failed"),
        }
        result
    }

    /// find largest matching ancestor for pattern
    #[context_trace::instrument_sig(level = "info", skip(self, searchable))]
    fn find_ancestor(
        &self,
        searchable: impl Searchable<AncestorSearchTraversal<Self>>,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting ancestor search");
        let result = searchable.search(self.ctx()).map_err(|err| err.reason);

        match &result {
            Ok(_response) => debug!("ancestor search succeeded"),
            Err(reason) =>
                debug!(reason = %pretty(reason), "ancestor search failed"),
        }
        result
    }
}

impl Find for &Hypergraph {
    fn ctx(&self) -> SearchCtx<Self> {
        SearchCtx::new(self)
    }
}

impl Find for HypergraphRef {
    fn ctx(&self) -> SearchCtx<Self> {
        SearchCtx::new(self.clone())
    }
}

/// context for running fold traversal
#[derive(Debug)]
pub struct SearchState<K: SearchKind> {
    pub(crate) matches: SearchIterator<K>,
    /// The query pattern we're searching for
    pub(crate) query: PatternRangePath,
}

impl<K: SearchKind> SearchState<K>
where
    K::Trav: Clone,
{
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub(crate) fn search(mut self) -> Response {
        debug!(queue = %&self.matches.queue, "initial state");

        let mut best_match = None;
        for item in self.by_ref() {
            best_match = Some(item);
        }

        // Get the final matched state from best_match
        let end = if let Some(checkpoint) = best_match.take() {
            debug!(
                root = %checkpoint.root_parent(),
                checkpoint_pos = *checkpoint.cursor().atom_position.as_ref(),
                query_exhausted = checkpoint.query_exhausted(),
                "Using best_match as final result"
            );
            checkpoint
        } else {
            // No matches found - create EntireRoot at the token width position
            // (cursor position should equal the entire matched token's width)
            debug!("No matches found, creating EntireRoot at token width");
            let start_token = self.query.path_root()[0];
            let token_width = *start_token.width();
            let raw_cursor = PatternCursor {
                atom_position: AtomPosition::from(token_width),
                path: self.query.clone(),
                _state: PhantomData,
            };
            // Wrap in Checkpointed (at checkpoint, no candidate)
            use crate::cursor::checkpointed::Checkpointed;
            let cursor = Checkpointed::<PatternCursor<_>>::new(raw_cursor);
            let path = PathCoverage::EntireRoot(IndexRangePath::new_empty(
                IndexRoot::from(PatternLocation::new(
                    start_token,
                    PatternId::default(),
                )),
            ));
            MatchResult::new(path, CheckpointedCursor::AtCheckpoint(cursor))
        };

        trace!(end = %pretty(&end), "final matched state");

        let trace_ctx = &mut self.matches.trace_ctx;
        end.trace(trace_ctx);

        let response = Response {
            cache: self.matches.trace_ctx.cache,
            end,
        };

        info!("search complete");
        response
    }
    fn finish_root_cursor(
        &mut self,
        init_state: CompareState<Matched, Matched>,
    ) -> MatchResult {
        // Set initial root match as baseline for this root
        let mut last_match = init_state;
        debug!(
            root = %last_match.child.current().child_state.root_parent(),
            checkpoint_pos = *last_match.query.current().atom_position.as_ref(),
            "New root match - finishing root cursor"
        );

        // Advance the root cursor step by step, updating best_match after each step
        // Option to store MatchResult directly when created during parent exploration
        let mut parent_exploration_result: Option<MatchResult> = None;

        let final_state = loop {
            last_match.update_checkpoint();
            let root_cursor: RootCursor<K, Matched, Matched> = RootCursor {
                trav: self.matches.trace_ctx.trav.clone(),
                state: last_match.clone(),
            };
            match root_cursor.advance_to_next_match() {
                RootAdvanceResult::Advanced(next_match) => {
                    // Successfully advanced to next match - always update best_match
                    let checkpoint_pos = *next_match
                        .state
                        .query
                        .current()
                        .atom_position
                        .as_ref();
                    debug!(
                        root = %next_match.state.child.current().child_state.root_parent(),
                        checkpoint_pos,
                        "Match advanced - updating best_match"
                    );

                    // Continue with the new matched cursor
                    last_match = next_match.state;
                },
                RootAdvanceResult::Finished(end_result) => {
                    // Reached an end condition
                    match end_result {
                        RootEndResult::Conclusive(conclusive) => {
                            // Conclusive end - this is the maximum match for the search
                            match conclusive {
                                ConclusiveEnd::Mismatch(_candidate_cursor) => {
                                    // Found mismatch after progress - create final MatchResult
                                    // Clone the state before consuming
                                    debug!(
                                        "Conclusive end: Mismatch - keeping best match"
                                    );
                                    // Continue searching from queue (no parent exploration for mismatch)
                                },
                                ConclusiveEnd::Exhausted => {
                                    // Query exhausted - best_match should have the final result
                                    debug!("Conclusive end: Exhausted - keeping best match");
                                    // Return a clone so best_match remains set for the search layer
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
                                .get_parent_batch(&self.matches.trace_ctx.trav)
                            {
                                Some((_parent, batch)) => {
                                    self.matches.queue.nodes.extend(
                                        batch
                                            .into_compare_batch()
                                            .into_iter()
                                            .map(ParentCandidate),
                                    );
                                },
                                _ => {
                                    debug!("No parents available - search exhausted");
                                },
                            }
                            // Store the MatchResult with advanced query candidate
                            parent_exploration_result = Some(checkpoint_state);
                        },
                    }
                    break last_match;
                },
            }
        };

        // If we have a parent exploration result, use it directly (preserves candidate)
        // Otherwise, create result from final state (which only has checkpoint)
        parent_exploration_result
            .unwrap_or_else(|| self.create_result_from_state(final_state))
    }
    /// Create a checkpoint MatchResult from the current matched state
    /// This is used to update best_match after each successful advancement
    pub(crate) fn create_result_from_state(
        &self,
        state: CompareState<Matched, Matched>,
    ) -> MatchResult {
        let result_child = state.child.current();

        let mut path = result_child.child_state.path.clone();
        let trav = &self.matches.trace_ctx.trav;
        // Simplify paths
        path.start_path_mut().simplify(trav);
        path.end_path_mut().simplify(trav);

        // Get the target token from the path
        let target_token = path.role_rooted_leaf_token::<End, _>(trav);

        // Use entry_pos from checkpoint_child
        let _start_pos = result_child.child_state.start_pos;
        let entry_pos = result_child.child_state.entry_pos;
        let exit_pos = result_child.child_state.exit_pos;
        // Use checkpoint position for end_pos (confirmed match boundary, not exploratory candidate)
        let end_pos = state.query.checkpoint().atom_position;

        let target = DownKey::new(target_token, exit_pos);

        let path_enum = PathCoverage::from_range_path(
            path, entry_pos, exit_pos, target, end_pos, trav,
        );

        // Clone and simplify the checkpoint and candidate cursors
        let mut simplified_checkpoint = state.query.checkpoint().clone();
        simplified_checkpoint.path.end_path_mut().simplify(trav);

        let mut simplified_candidate = state.query.candidate().clone();
        simplified_candidate.path.end_path_mut().simplify(trav);

        let cursor = if simplified_candidate == simplified_checkpoint {
            // No candidate advancement - create AtCheckpoint
            CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                simplified_checkpoint,
            ))
        } else {
            // Candidate advanced beyond checkpoint - create HasCandidate
            let cursor_state = Checkpointed::<_, HasCandidate>::with_candidate(
                simplified_checkpoint,
                simplified_candidate,
            );
            CheckpointedCursor::HasCandidate(cursor_state)
        };

        MatchResult::new(path_enum, cursor)
    }
}
impl<K: SearchKind> Iterator for SearchState<K>
where
    K::Trav: Clone,
{
    type Item = MatchResult;
    fn next(&mut self) -> Option<Self::Item> {
        trace!("searching for next match");
        match self.matches.find_next_root() {
            Some(state) => {
                let matched_state = self.finish_root_cursor(state);
                let checkpoint_pos =
                    *matched_state.cursor().atom_position.as_ref();

                debug!(
                    query_exhausted = matched_state.query_exhausted(),
                    checkpoint_pos = checkpoint_pos,
                    "found matched state"
                );
                Some(matched_state)
            },
            None => {
                trace!("no more matches found");
                None
            },
        }
    }
}
