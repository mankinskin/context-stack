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
        iterator::{BfsStepResult, SearchIterator},
        root_cursor::{
            ConclusiveEnd,
            RootAdvanceResult,
            RootCursor,
            RootEndResult,
        },
        CompareInfo,
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
    graph::{
        search_path::{EdgeRef, PathNode, VizPathGraph},
        visualization::{
            GraphOpEvent,
            LocationInfo,
            OperationType,
            QueryInfo,
            Transition,
        },
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
pub(crate) mod events;
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
    /// Unique identifier for this search path (used for graph-op event grouping).
    pub(crate) path_id: String,
    /// Accumulated path graph for incremental visualization.
    pub(crate) viz_path: VizPathGraph,
    /// Highest confirmed cursor position (atom index) for query visualization.
    pub(crate) viz_cursor_pos: usize,
    /// Atom positions confirmed as matched (for query token highlighting).
    pub(crate) viz_matched_positions: Vec<usize>,
    /// Collected graph-op events for testing and inspection.
    pub(crate) collected_events: Vec<GraphOpEvent>,
    /// Unique identifier for the query path event stream.
    pub(crate) query_path_id: String,
    /// Accumulated path graph for query visualization.
    pub(crate) query_viz_path: VizPathGraph,
}

impl<K: SearchKind> SearchState<K>
where
    K::Trav: Clone,
{
    /// Build and emit a [`GraphOpEvent`] for the current algorithm state.
    ///
    /// `current_root` and `matched_nodes` are inferred from the [`Transition`]
    /// variant. The transition also drives [`VizPathGraph`] updates.
    /// `cursor_position` and `matched_positions` are tracked incrementally
    /// for query path visualization.
    fn emit_graph_op(
        &mut self,
        transition: Transition,
        description: impl Into<String>,
    ) {
        let query_pattern = self.query.path_root();
        let query_tokens: Vec<usize> =
            query_pattern.iter().map(|t| t.index.0).collect();
        let query_width: usize = query_pattern.iter().map(|t| *t.width()).sum();

        let (candidate_parents, candidate_children) = self.queue_candidates();

        let step = self.step_counter;
        self.step_counter += 1;

        // Infer current_root and matched_nodes from the transition
        let (current_root, matched_nodes) = Self::infer_location(&transition);

        // Update viz cursor position and matched state from transition
        let (cursor_pos, active_token) = match &transition {
            Transition::ChildMatch { cursor_pos, node } => {
                // Record this position as matched
                if !self.viz_matched_positions.contains(cursor_pos) {
                    self.viz_matched_positions.push(*cursor_pos);
                }
                self.viz_cursor_pos = *cursor_pos;
                (*cursor_pos, Some(node.index))
            },
            Transition::ChildMismatch { cursor_pos, node, .. } => {
                (*cursor_pos, Some(node.index))
            },
            _ => (self.viz_cursor_pos, None),
        };

        // Apply transition to accumulated path graph
        let _ = self.viz_path.apply_transition(&transition);

        let location = LocationInfo {
            selected_node: current_root,
            root_node: current_root,
            trace_path: vec![],
            completed_nodes: matched_nodes,
            pending_parents: candidate_parents,
            pending_children: candidate_children,
        };

        let query = QueryInfo {
            query_tokens: query_tokens.clone(),
            cursor_position: cursor_pos,
            query_width,
            matched_positions: self.viz_matched_positions.clone(),
            active_token,
        };

        let event = GraphOpEvent {
            step,
            op_type: OperationType::Search,
            transition: transition.clone(),
            location,
            query,
            description: description.into(),
            path_id: self.path_id.clone(),
            path_graph: self.viz_path.clone(),
            graph_mutation: None,
        };
        event.emit();
        self.collected_events.push(event);

        // Emit corresponding query path event for certain transitions
        self.emit_query_event(&transition, query_tokens, query_width, cursor_pos);
    }

    /// Emit a query path event corresponding to a search transition.
    ///
    /// Query events track cursor advancement through the query pattern,
    /// emitted on a separate `query_path_id` stream.
    fn emit_query_event(
        &mut self,
        transition: &Transition,
        query_tokens: Vec<usize>,
        query_width: usize,
        cursor_pos: usize,
    ) {
        // Only emit query events for transitions that affect the query cursor
        let query_transition = match transition {
            Transition::ChildMatch { cursor_pos, .. } => {
                // Find the query token at this cursor position
                let query_token_idx = self.query_token_at_pos(*cursor_pos);
                Some(Transition::ChildMatch {
                    node: PathNode { index: query_token_idx, width: 1 }, // Query token
                    cursor_pos: *cursor_pos,
                })
            },
            Transition::ChildMismatch { cursor_pos, expected, .. } => {
                let query_token_idx = self.query_token_at_pos(*cursor_pos);
                Some(Transition::ChildMismatch {
                    node: PathNode { index: query_token_idx, width: 1 },
                    cursor_pos: *cursor_pos,
                    expected: *expected,
                    actual: PathNode { index: query_token_idx, width: 1 },
                })
            },
            Transition::StartNode { .. } => {
                // Query starts at first token
                let first_query_token = query_tokens.first().copied().unwrap_or(0);
                Some(Transition::StartNode {
                    node: PathNode { index: first_query_token, width: 1 },
                })
            },
            Transition::Done { success, .. } => {
                // Query done when search done
                let last_query_token = query_tokens.last().copied();
                Some(Transition::Done {
                    final_node: last_query_token,
                    success: *success,
                })
            },
            _ => None,
        };

        if let Some(query_trans) = query_transition {
            // Apply to query viz path
            let _ = self.query_viz_path.apply_transition(&query_trans);

            let query_info = QueryInfo {
                query_tokens,
                cursor_position: cursor_pos,
                query_width,
                matched_positions: self.viz_matched_positions.clone(),
                active_token: None,
            };

            let query_event = GraphOpEvent {
                step: self.step_counter - 1, // Same step as search event
                op_type: OperationType::Query,
                transition: query_trans,
                location: LocationInfo::default(),
                query: query_info,
                description: "Query path event".to_string(),
                path_id: self.query_path_id.clone(),
                path_graph: self.query_viz_path.clone(),
                graph_mutation: None,
            };
            query_event.emit();
            self.collected_events.push(query_event);
        }
    }

    /// Find the query token index at a given cursor position.
    fn query_token_at_pos(&self, cursor_pos: usize) -> usize {
        let tokens = self.query.path_root();
        let mut pos = 0;
        for token in tokens.iter() {
            pos += *token.width();
            if pos > cursor_pos {
                return token.index.0;
            }
        }
        // Fallback to last token
        tokens.last().map(|t| t.index.0).unwrap_or(0)
    }

    /// Infer `(current_root, matched_nodes)` from a [`Transition`] variant.
    fn infer_location(transition: &Transition) -> (Option<usize>, Vec<usize>) {
        match transition {
            Transition::StartNode { node } => (Some(node.index), vec![]),
            Transition::VisitParent { from, .. } => (Some(from.index), vec![from.index]),
            Transition::CandidateMatch { root, .. } => (Some(root.index), vec![root.index]),
            Transition::VisitChild { from, .. } => (Some(from.index), vec![from.index]),
            Transition::ParentExplore { current_root, .. } => {
                (Some(*current_root), vec![*current_root])
            },
            Transition::Done { final_node, .. } => {
                (*final_node, final_node.iter().copied().collect())
            },
            Transition::ChildMatch { node, .. } => (Some(node.index), vec![node.index]),
            Transition::ChildMismatch { node, .. } => (Some(node.index), vec![]),
            Transition::CandidateMismatch { node, .. } => (Some(node.index), vec![]),
            // Insert-specific transitions — not expected in search
            _ => (None, vec![]),
        }
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

    /// Emit `VisitChild` / `ChildMatch` / `ChildMismatch` events from a
    /// [`CompareInfo`] returned by the BFS node comparison.
    ///
    /// This translates the metadata captured during `compare_leaf_tokens`
    /// into graph-op events so the visualization stays in sync with the
    /// algorithm.
    fn emit_compare_events(&mut self, info: CompareInfo, parent_node: usize) {
        use events::{EventContext, IntoTransitions};

        let ctx = EventContext::new(parent_node, self.viz_path.end_path.is_empty());
        for (transition, description) in info.into_transitions(&ctx) {
            self.emit_graph_op(transition, description);
        }
    }

    /// Emit events from any [`IntoTransitions`] implementor.
    ///
    /// Creates an [`EventContext`] from the current visualization state and
    /// emits all derived transitions.
    fn emit_advance_events<T: events::IntoTransitions>(&mut self, data: T, parent_node: usize) {
        use events::EventContext;

        let ctx = EventContext::new(parent_node, self.viz_path.end_path.is_empty());
        for (transition, description) in data.into_transitions(&ctx) {
            self.emit_graph_op(transition, description);
        }
    }

    /// Compute the `from` vertex for a [`Transition::CandidateMatch`] edge.
    ///
    /// If `root_idx` matches the last start_path entry (the candidate was
    /// pushed by VisitParent), use the entry below it. Otherwise use the
    /// start_path top or start_node as fallback.
    fn compute_candidate_edge_from(&self, root_idx: usize) -> usize {
        let sp = &self.viz_path;
        if sp.start_path.last().map(|n| n.index) == Some(root_idx) {
            if sp.start_path.len() >= 2 {
                sp.start_path[sp.start_path.len() - 2].index
            } else {
                sp.start_node.map(|n| n.index).unwrap_or(self.start_node)
            }
        } else {
            sp.start_path
                .last()
                .map(|n| n.index)
                .unwrap_or(sp.start_node.map(|n| n.index).unwrap_or(self.start_node))
        }
    }

    /// Emit a [`Transition::CandidateMatch`] to signal that the given node
    /// is being explored as a root candidate.
    ///
    /// With the new semantics, `VisitParent` already sets the root on
    /// [`VizPathGraph`], so this transition is informational for parent
    /// candidates. For non-parent candidates (where no `VisitParent` was
    /// emitted), `CandidateMatch` still sets the root as a fallback.
    fn emit_candidate_match(&mut self, node_index: usize) {
        let edge_from = self.compute_candidate_edge_from(node_index);
        self.emit_graph_op(
            Transition::CandidateMatch {
                root: PathNode { index: node_index, width: 1 }, // TODO: get real width
                edge: EdgeRef {
                    from: edge_from,
                    to: node_index,
                    pattern_idx: 0,
                    sub_index: 0,
                },
            },
            format!("Setting candidate root at node {node_index}"),
        );
    }

    pub(crate) fn search(mut self) -> Response {
        trace!(queue = %&self.matches.queue, "initial state");

        // Emit Init event
        let start_width = *self.query.path_root()[0].width();
        self.emit_graph_op(
            Transition::StartNode {
                node: PathNode { index: self.start_node, width: start_width },
            },
            "Search started — initial queue populated",
        );

        // Emit initial ParentExplore for the start node's parent candidates
        let initial_candidates = self.queue_candidates().0.clone();
        if !initial_candidates.is_empty() {
            self.emit_graph_op(
                Transition::ParentExplore {
                    current_root: self.start_node,
                    parent_candidates: initial_candidates,
                },
                format!("Start node {} exploring parent candidates", self.start_node),
            );
        }

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
        let matched_root = end.root_parent().index.0;
        self.emit_graph_op(
            Transition::Done {
                final_node: Some(matched_root),
                success: true,
            },
            "Search complete",
        );

        let trace_ctx = &mut self.matches.trace_ctx;
        end.trace(trace_ctx);

        let response = Response {
            cache: self.matches.trace_ctx.cache,
            end,
            events: self.collected_events,
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

        // The BFS loop now emits CandidateMatch unconditionally for
        // FoundMatch, so root should already be set. This is a safety net
        // for any edge cases where root wasn't set (should not normally fire).
        if self.viz_path.root.is_none() {
            self.emit_candidate_match(root_idx);
        }

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

                    // Extract child info and emit events via IntoTransitions
                    let trav = &self.matches.trace_ctx.trav;
                    let child_state = &next_match.state.child.current().child_state;
                    let child_token = child_state.path.role_rooted_leaf_token::<End, _>(trav);
                    let event_data = events::MatchAdvanceData::new(
                        adv_root,
                        child_token.index.0,
                        child_token.width.0,
                        child_state.root_child_index(),
                        checkpoint_pos,
                    );
                    self.emit_advance_events(event_data, adv_root);

                    // Continue with the new matched cursor
                    last_match = next_match.state;
                },
                RootAdvanceResult::Finished(end_result) => {

                    // Reached an end condition
                    match end_result {
                        RootEndResult::Conclusive(conclusive) => {
                            // Conclusive end - this is the maximum match for the search
                            match conclusive {
                                ConclusiveEnd::Mismatch(candidate_cursor) => {
                                    // Found mismatch after progress - emit child events
                                    trace!(
                                        "Conclusive end: Mismatch - keeping best match"
                                    );
                                    // Extract token info and emit mismatch events via IntoTransitions
                                    let trav = &self.matches.trace_ctx.trav;
                                    let child_state = &candidate_cursor.state.child.candidate().child_state;
                                    let child_token = child_state.path.role_rooted_leaf_token::<End, _>(trav);
                                    let adv_root = child_state.root_parent().index.0;
                                    let query_token = candidate_cursor.state.query.candidate().path.role_rooted_leaf_token::<End, _>(trav);
                                    let cursor_pos = *candidate_cursor.state.query.candidate().atom_position.as_ref();

                                    let event_data = events::MismatchAdvanceData::new(
                                        adv_root,
                                        child_token.index.0,
                                        child_token.width.0,
                                        query_token.index.0,
                                        query_token.width.0,
                                        child_state.root_child_index(),
                                        cursor_pos,
                                    );
                                    self.emit_advance_events(event_data, adv_root);
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

                            let parent_candidates = self.queue_candidates().0.clone();
                            self.emit_graph_op(
                                Transition::ParentExplore { 
                                    current_root: checkpoint_root_idx,
                                    parent_candidates,
                                },
                                format!("Root boundary at node {checkpoint_root_idx} — exploring parents"),
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

        // Drive the BFS loop one step at a time so we can emit graph-op
        // events for intermediate nodes (e.g. a parent that is explored but
        // doesn't produce a root match).
        //
        // Flow: pop → VisitParent (if parent) → process → compare_events → result events
        // BfsStepResult carries node_index and is_parent for event emission.
        let matched_state = loop {
            let step_result = self.matches.pop_and_process_one();

            // Extract node_index and is_parent from all non-Empty variants
            let (node_index, is_parent) = match &step_result {
                BfsStepResult::Expanded { node_index, is_parent, .. } |
                BfsStepResult::FoundMatch { node_index, is_parent, .. } |
                BfsStepResult::Skipped { node_index, is_parent, .. } => (*node_index, *is_parent),
                BfsStepResult::Empty => {
                    trace!("no more matches found");
                    return None;
                },
            };

            // Emit VisitParent BEFORE processing so the visualization shows
            // navigation to the candidate before any child comparison.
            if is_parent {
                let push_from = if let Some(current_root_node) = self.viz_path.root {
                    current_root_node.index
                } else if let Some(top) = self.viz_path.start_path.last() {
                    // After CandidateMismatch clears root, the top of
                    // start_path is the last confirmed ancestor — use it
                    // so the edge connects correctly (e.g. ab → abc).
                    top.index
                } else {
                    self.viz_path.start_node.map(|n| n.index).unwrap_or(self.start_node)
                };
                self.emit_graph_op(
                    Transition::VisitParent {
                        from: PathNode { index: push_from, width: 1 }, // TODO: get real width
                        to: PathNode { index: node_index, width: 1 },   // TODO: get real width
                        entry_pos: 0,
                        edge: EdgeRef {
                            from: push_from,
                            to: node_index,
                            pattern_idx: 0,
                            sub_index: 0,
                        },
                    },
                    format!("Visiting candidate parent {node_index} from {push_from}"),
                );
            }

            match step_result {
                BfsStepResult::Expanded { info, node_index, is_parent } => {
                    // Clear transient end_path from any previous comparison
                    // before starting a new comparison for this candidate.
                    self.viz_path.end_path.clear();
                    self.viz_path.end_edges.clear();
                    self.viz_path.root_exit_edge = None;

                    // Emit child comparison events from the BFS comparison.
                    // Root was already set by VisitParent, so VisitChild
                    // events can populate end_path for arrow visualization.
                    // No CandidateMatch — node is merely explored, not matched.
                    self.emit_compare_events(info, node_index);

                    if is_parent {
                        let candidate_parents = self.queue_candidates().0.clone();
                        self.emit_graph_op(
                            Transition::ParentExplore {
                                current_root: node_index,
                                parent_candidates: candidate_parents,
                            },
                            format!("Node {node_index} boundary reached — exploring parents"),
                        );
                    }
                    // Expanded nodes are explored, not confirmed matches.
                    // Demote the transient root back to start_path so the
                    // next VisitParent uses the correct push_from.
                    if let Some(exp_root) = self.viz_path.root.take() {
                        let exp_edge = self.viz_path.root_entry_edge.take();
                        self.viz_path.start_path.push(exp_root);
                        if let Some(edge) = exp_edge {
                            self.viz_path.start_edges.push(edge);
                        }
                    }
                    continue;
                },
                BfsStepResult::FoundMatch { state, info, node_index, .. } => {
                    // Clear transient end_path from any previous comparison.
                    self.viz_path.end_path.clear();
                    self.viz_path.end_edges.clear();
                    self.viz_path.root_exit_edge = None;

                    // Emit child comparison events first so the visualization
                    // shows the comparison before confirming the match.
                    self.emit_compare_events(info, node_index);

                    // Signal that this candidate is a confirmed root match.
                    // For parent candidates, VisitParent already set root;
                    // CandidateMatch is informational confirmation.
                    // For non-parents, CandidateMatch sets root as fallback.
                    self.emit_candidate_match(node_index);

                    // Clear queue — finish_root_cursor will explore via
                    // RootCursor and re-populate if necessary.
                    debug!(
                        "Found matching root — clearing search queue (will explore via parents)"
                    );
                    self.matches.queue.nodes.clear();

                    let root_parent =
                        state.child.current().child_state.root_parent();
                    debug!(
                        root_parent = %root_parent,
                        root_width = root_parent.width.0,
                        "found matching root — creating RootCursor"
                    );
                    break state;
                },
                BfsStepResult::Skipped { info, node_index, is_parent } => {
                    // Clear transient end_path from any previous comparison.
                    self.viz_path.end_path.clear();
                    self.viz_path.end_edges.clear();
                    self.viz_path.root_exit_edge = None;

                    // Emit child comparison events. Root was set by
                    // VisitParent, so VisitChild arrows appear during
                    // comparison. No CandidateMatch — node is rejected.
                    self.emit_compare_events(info, node_index);

                    self.emit_graph_op(
                        Transition::CandidateMismatch {
                            node: PathNode { index: node_index, width: 1 }, // TODO: get real width
                            queue_remaining: self.matches.queue.nodes.len(),
                            is_parent,
                        },
                        format!("Node {node_index} skipped (mismatch)"),
                    );
                    continue;
                },
                BfsStepResult::Empty => {
                    // Already handled above, but included for exhaustive matching
                    trace!("no more matches found");
                    return None;
                },
            }
        };

        let matched_result = self.finish_root_cursor(matched_state);
        let checkpoint_pos =
            *matched_result.cursor().atom_position.as_ref();

        trace!(
            query_exhausted = matched_result.query_exhausted(),
            checkpoint_pos = checkpoint_pos,
            "found matched state"
        );
        Some(matched_result)
    }
}
