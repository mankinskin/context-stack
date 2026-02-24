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
        SearchNode,
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
    graph::visualization::{
        GraphOpEvent,
        LocationInfo,
        OperationType,
        QueryInfo,
        Transition,
    },
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
        trace!("starting parent search");
        let result = searchable.search(self.ctx()).map_err(|err| err.reason);

        match &result {
            Ok(_response) => trace!("parent search succeeded"),
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
        trace!("starting ancestor search");
        let result = searchable.search(self.ctx()).map_err(|err| err.reason);

        match &result {
            Ok(_response) => trace!("ancestor search succeeded"),
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
    /// Monotonically increasing step counter for search visualisation.
    pub(crate) step_counter: usize,
    /// Vertex index of the token where the search started.
    pub(crate) start_node: usize,
}

impl<K: SearchKind> SearchState<K>
where
    K::Trav: Clone,
{
    /// Build and emit a [`GraphOpEvent`] for the current algorithm state.
    fn emit_graph_op(
        &mut self,
        transition: Transition,
        description: impl Into<String>,
        cursor_position: usize,
        matched_nodes: Vec<usize>,
        current_root: Option<usize>,
    ) {
        let query_pattern = self.query.path_root();
        let query_tokens: Vec<usize> =
            query_pattern.iter().map(|t| t.index.0).collect();
        let query_width: usize = query_pattern.iter().map(|t| *t.width()).sum();

        let (candidate_parents, candidate_children) = self.queue_candidates();

        let step = self.step_counter;
        self.step_counter += 1;

        let location = LocationInfo {
            selected_node: current_root,
            root_node: current_root,
            trace_path: vec![], // TODO: populate from path
            completed_nodes: matched_nodes,
            pending_parents: candidate_parents,
            pending_children: candidate_children,
        };

        let query = QueryInfo {
            query_tokens,
            cursor_position,
            query_width,
        };

        let event = GraphOpEvent {
            step,
            op_type: OperationType::Search,
            transition,
            location,
            query,
            description: description.into(),
        };
        event.emit();
    }

    /// Extract parent and child candidate vertex indices from the BFS queue.
    fn queue_candidates(&self) -> (Vec<usize>, Vec<usize>) {
        let mut parents = Vec::new();
        let mut children = Vec::new();
        for node in self.matches.queue.nodes.iter() {
            let idx = node.root_parent().index.0;
            match node {
                SearchNode::ParentCandidate(_) => parents.push(idx),
                SearchNode::ChildCandidate(_) => children.push(idx),
            }
        }
        (parents, children)
    }

    pub(crate) fn search(mut self) -> Response {
        trace!(queue = %&self.matches.queue, "initial state");

        // Emit Init event
        self.emit_graph_op(
            Transition::StartNode {
                node: self.start_node,
            },
            "Search started — initial queue populated",
            0,
            vec![],
            None,
        );

        let mut best_match = None;
        for item in self.by_ref() {
            best_match = Some(item);
        }

        // Get the final matched state from best_match
        let end = if let Some(checkpoint) = best_match.take() {
            trace!(
                root = %checkpoint.root_parent(),
                checkpoint_pos = *checkpoint.cursor().atom_position.as_ref(),
                query_exhausted = checkpoint.query_exhausted(),
                "Using best_match as final result"
            );
            checkpoint
        } else {
            // No matches found - create EntireRoot at the token width position
            // (cursor position should equal the entire matched token's width)
            trace!("No matches found, creating EntireRoot at token width");
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

        // Emit Done event
        let cursor_pos = *end.cursor().atom_position.as_ref();
        let matched_root = end.root_parent().index.0;
        self.emit_graph_op(
            Transition::Done {
                final_node: Some(matched_root),
                success: true,
            },
            "Search complete",
            cursor_pos,
            vec![matched_root],
            Some(matched_root),
        );

        let trace_ctx = &mut self.matches.trace_ctx;
        end.trace(trace_ctx);

        let response = Response {
            cache: self.matches.trace_ctx.cache,
            end,
        };

        info!(end=?response.end, "search complete");
        response
    }
    fn finish_root_cursor(
        &mut self,
        init_state: CompareState<Matched, Matched>,
    ) -> MatchResult {
        // Set initial root match as baseline for this root
        let mut last_match = init_state;
        let root_idx =
            last_match.child.current().child_state.root_parent().index.0;
        let init_cursor_pos =
            *last_match.query.current().atom_position.as_ref();
        trace!(
            root = %last_match.child.current().child_state.root_parent(),
            checkpoint_pos = init_cursor_pos,
            "New root match - finishing root cursor"
        );

        // Emit RootExplore event
        self.emit_graph_op(
            Transition::RootExplore { root: root_idx },
            format!("Root match found at node {root_idx}"),
            init_cursor_pos,
            vec![root_idx],
            Some(root_idx),
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
                    let adv_root = next_match
                        .state
                        .child
                        .current()
                        .child_state
                        .root_parent()
                        .index
                        .0;
                    trace!(
                        root = %next_match.state.child.current().child_state.root_parent(),
                        checkpoint_pos,
                        "Match advanced - updating best_match"
                    );

                    // Emit MatchAdvance event
                    self.emit_graph_op(
                        Transition::MatchAdvance { 
                            root: adv_root, 
                            prev_pos: init_cursor_pos,
                            new_pos: checkpoint_pos,
                        },
                        format!("Match advanced in root {adv_root} to pos {checkpoint_pos}"),
                        checkpoint_pos,
                        vec![adv_root],
                        Some(adv_root),
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
                                    trace!(
                                        "Conclusive end: Mismatch - keeping best match"
                                    );
                                    // Continue searching from queue (no parent exploration for mismatch)
                                },
                                ConclusiveEnd::Exhausted => {
                                    // Query exhausted - best_match should have the final result
                                    trace!("Conclusive end: Exhausted - keeping best match");
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
                            let checkpoint_root_idx =
                                checkpoint_state.root_parent().index.0;
                            trace!(
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
                                    trace!("No parents available - search exhausted");
                                },
                            }

                            // Emit ParentExplore event (after queue is populated)
                            self.emit_graph_op(
                                Transition::ParentExplore { 
                                    current_root: checkpoint_root_idx,
                                    parent_candidates: self.queue_candidates().0.clone(),
                                },
                                format!("Root boundary at node {checkpoint_root_idx} — exploring parents"),
                                checkpoint_pos,
                                vec![checkpoint_root_idx],
                                Some(checkpoint_root_idx),
                            );

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

                trace!(
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
