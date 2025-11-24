use std::marker::PhantomData;

use crate::{
    compare::state::CompareState,
    cursor::{
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
        matched::MatchResult,
    },
    traversal::TraceStart,
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

pub use searchable::Searchable;
use tracing::{
    debug,
    info,
    trace,
};
pub(crate) mod context;
pub(crate) mod ext;
// pub(crate) mod final_state; // Unused - references old EndState type
pub(crate) mod searchable;

pub(crate) type SearchResult = Result<Response, ErrorReason>;
#[allow(dead_code)]
pub trait Find: HasGraph {
    fn ctx(&self) -> SearchCtx<Self>;

    #[context_trace::instrument_sig(
        level = "info",
        skip(self, pattern),
        fields(pattern_len)
    )]
    fn find_sequence(
        &self,
        pattern: impl IntoIterator<Item = impl AsAtom<AtomOf<TravKind<Self>>>>,
    ) -> SearchResult
    where
        Self: Clone,
    {
        let iter = atomizing_iter(pattern.into_iter());
        let atoms: Vec<_> = iter.collect();
        tracing::Span::current().record("pattern_len", atoms.len());
        debug!(pattern_len = atoms.len(), "finding sequence pattern");
        trace!(atoms = %pretty(&atoms), "pattern atoms");

        let pattern = self.graph().get_atom_children(atoms.into_iter())?;
        debug!(pattern = %pretty(&pattern), "created pattern token");

        self.find_ancestor(pattern)
    }

    /// find largest matching parent for pattern
    #[context_trace::instrument_sig(level = "info", skip(self, searchable))]
    fn find_parent(
        &self,
        searchable: impl Searchable,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting parent search");
        let result = searchable
            .search::<ParentSearchTraversal<Self>>(self.ctx())
            .map_err(|err| err.reason);

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
        searchable: impl Searchable,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting ancestor search");
        let result = searchable
            .search::<AncestorSearchTraversal<Self>>(self.ctx())
            .map_err(|err| err.reason);

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

        let mut iteration = 0;
        let mut best_match = None;
        while let Some(matched_state) = self.next() {
            iteration += 1;
            debug!(iteration, "tracing matched state");
            debug!(
                "About to trace MatchResult: query_exhausted={}, path_variant={}",
                matched_state.query_exhausted(),
                match matched_state.path() {
                    PathCoverage::Range(_) => "Range",
                    PathCoverage::Postfix(_) => "Postfix",
                    PathCoverage::Prefix(_) => "Prefix",
                    PathCoverage::EntireRoot(_) => "EntireRoot",
                }
            );
            matched_state.trace(&mut self.matches.trace_ctx);
            best_match = Some(matched_state);
            debug!("Finished tracing MatchResult");
        }

        debug!(iterations = iteration, "fold completed");

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
            // No matches found - create empty mismatch at position 0
            debug!("No matches found, creating empty mismatch");
            let start_token = self.query.path_root()[0];
            let cursor = PatternCursor {
                atom_position: AtomPosition::default(),
                path: self.query.clone(),
                _state: PhantomData,
            };
            let path = PathCoverage::EntireRoot(IndexRangePath::new_empty(
                IndexRoot::from(PatternLocation::new(
                    start_token,
                    PatternId::default(),
                )),
            ));
            MatchResult { path, cursor }
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
                        },
                    }
                    break last_match;
                },
            }
        };
        self.create_result_from_state(final_state)
    }
    /// Create a checkpoint MatchResult from the current matched state
    /// This is used to update best_match after each successful advancement
    pub(crate) fn create_result_from_state(
        &self,
        state: CompareState<Matched, Matched>,
    ) -> MatchResult {
        let result_query = state.query.current();
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
        let root_pos = result_child.child_state.entry_pos;
        let end_pos = result_query.atom_position;

        let target = DownKey::new(target_token, root_pos.into());

        let path_enum = PathCoverage::from_range_path(
            path, root_pos, target, end_pos, trav,
        );

        MatchResult {
            cursor: result_query.clone(),
            path: path_enum,
        }
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

                // Trace start path for new best match
                if let Some(start_path) = matched_state.path().try_start_path()
                {
                    let prev_start_len = matched_state
                        .path()
                        .try_start_path()
                        .map(|p| p.len())
                        .unwrap_or(0);
                    let current_start_len = start_path.len();

                    if current_start_len > prev_start_len {
                        debug!(
                                "Tracing incremental start path: prev_len={}, current_len={}",
                                prev_start_len, current_start_len
                            );
                        TraceStart {
                            end: &matched_state,
                            pos: prev_start_len,
                        }
                        .trace(&mut self.matches.trace_ctx);
                    }
                }

                Some(matched_state)
            },
            None => {
                trace!("no more matches found");
                None
            },
        }
    }
}
