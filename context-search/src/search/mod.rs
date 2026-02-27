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
        search_path::{EdgeRef, VizPathGraph},
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
}

impl<K: SearchKind> SearchState<K>
where
    K::Trav: Clone,
{
    /// Build and emit a [`GraphOpEvent`] for the current algorithm state.
    ///
    /// `current_root` and `matched_nodes` are inferred from the [`Transition`]
    /// variant. The transition also drives [`VizPathGraph`] updates.
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
            query_tokens,
            cursor_position: 0,
            query_width,
        };

        let event = GraphOpEvent {
            step,
            op_type: OperationType::Search,
            transition,
            location,
            query,
            description: description.into(),
            path_id: self.path_id.clone(),
            path_graph: self.viz_path.clone(),
        };
        event.emit();
    }

    /// Infer `(current_root, matched_nodes)` from a [`Transition`] variant.
    fn infer_location(transition: &Transition) -> (Option<usize>, Vec<usize>) {
        match transition {
            Transition::StartNode { .. } => (None, vec![]),
            Transition::VisitParent { from, .. } => (Some(*from), vec![*from]),
            Transition::RootExplore { root, .. } => (Some(*root), vec![*root]),
            Transition::VisitChild { from, .. } => (Some(*from), vec![*from]),
            Transition::MatchAdvance { root, .. } => (Some(*root), vec![*root]),
            Transition::ParentExplore { current_root, .. } => {
                (Some(*current_root), vec![*current_root])
            },
            Transition::Done { final_node, .. } => {
                (*final_node, final_node.iter().copied().collect())
            },
            Transition::ChildMatch { node, .. } => (Some(*node), vec![*node]),
            Transition::ChildMismatch { node, .. } => (Some(*node), vec![]),
            Transition::Dequeue { node, .. } => (Some(*node), vec![]),
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

    pub(crate) fn search(mut self) -> Response {
        trace!(queue = %&self.matches.queue, "initial state");

        // Emit Init event
        let start_width = *self.query.path_root()[0].width();
        self.emit_graph_op(
            Transition::StartNode {
                node: self.start_node,
                width: start_width,
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

        // Always emit VisitParent + PushParent before SetRoot.
        // For the first parent, `from` is start_node; for subsequent parents,
        // `from` is the current root (which gets demoted into start_path).
        let push_from = if let Some(current_root_node) = self.viz_path.root {
            current_root_node.index
        } else {
            self.viz_path.start_node.map(|n| n.index).unwrap_or(self.start_node)
        };
        self.emit_graph_op(
            Transition::VisitParent {
                from: push_from,
                to: root_idx,
                entry_pos: init_cursor_pos,
                width: 1,
                edge: EdgeRef {
                    from: push_from,
                    to: root_idx,
                    pattern_idx: 0,
                    sub_index: 0,
                },
            },
            format!("Visiting parent {root_idx} from {push_from}"),
        );

        // SetRoot "graduates" the node from start_path to root.
        // apply() detects that root_idx == start_path.last() and pops it,
        // using the popped edge as root_edge.
        let edge_from = {
            let sp = &self.viz_path;
            if sp.start_path.last().map(|n| n.index) == Some(root_idx) {
                // Root matches last start_path entry — use the entry below it
                if sp.start_path.len() >= 2 {
                    sp.start_path[sp.start_path.len() - 2].index
                } else {
                    sp.start_node.map(|n| n.index).unwrap_or(self.start_node)
                }
            } else {
                sp.start_path.last()
                    .map(|n| n.index)
                    .unwrap_or(sp.start_node.map(|n| n.index).unwrap_or(self.start_node))
            }
        };
        self.emit_graph_op(
            Transition::RootExplore {
                root: root_idx,
                width: 1,
                edge: EdgeRef {
                    from: edge_from,
                    to: root_idx,
                    pattern_idx: 0,
                    sub_index: 0,
                },
            },
            format!("Root match found at node {root_idx}"),
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

                    // Extract child info for end_path visualization
                    let trav = &self.matches.trace_ctx.trav;
                    let child_state = &next_match.state.child.current().child_state;
                    let child_token = child_state.path.role_rooted_leaf_token::<End, _>(trav);
                    let child_idx = child_token.index.0;
                    let child_width = child_token.width.0;
                    let child_sub_index = child_state.root_child_index();

                    // Emit VisitChild to show end_path
                    let replace = !self.viz_path.end_path.is_empty();
                    self.emit_graph_op(
                        Transition::VisitChild {
                            from: adv_root,
                            to: child_idx,
                            child_index: child_sub_index,
                            width: child_width,
                            edge: EdgeRef {
                                from: adv_root,
                                to: child_idx,
                                pattern_idx: 0,
                                sub_index: child_sub_index,
                            },
                            replace,
                        },
                        format!("Visiting child {child_idx} from root {adv_root}"),
                    );

                    // Emit MatchAdvance event
                    self.emit_graph_op(
                        Transition::MatchAdvance { 
                            root: adv_root, 
                            prev_pos: init_cursor_pos,
                            new_pos: checkpoint_pos,
                        },
                        format!("Match advanced in root {adv_root} to pos {checkpoint_pos}"),
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
        let matched_state = loop {
            match self.matches.pop_and_process_one() {
                BfsStepResult::Expanded { node_index, is_parent } => {
                    // The node was explored but didn't yield a match —
                    // emit RootExplore + ParentExplore so the visualisation
                    // shows the intermediate candidate.
                    if is_parent {
                        let candidate_parents = self.queue_candidates().0.clone();
                        self.emit_graph_op(
                            Transition::RootExplore {
                                root: node_index,
                                width: 1,
                                edge: EdgeRef {
                                    from: self.start_node,
                                    to: node_index,
                                    pattern_idx: 0,
                                    sub_index: 0,
                                },
                            },
                            format!("Exploring candidate node {node_index} (advance failed — expanding parents)"),
                        );
                        self.emit_graph_op(
                            Transition::ParentExplore {
                                current_root: node_index,
                                parent_candidates: candidate_parents,
                            },
                            format!("Node {node_index} boundary reached — exploring parents"),
                        );
                    }
                    continue;
                },
                BfsStepResult::FoundMatch(state) => {
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
                BfsStepResult::Skipped { node_index, is_parent } => {
                    // Emit a Dequeue event so the visualisation shows the
                    // mismatch.
                    self.emit_graph_op(
                        Transition::Dequeue {
                            node: node_index,
                            queue_remaining: self.matches.queue.nodes.len(),
                            is_parent,
                        },
                        format!("Node {node_index} skipped (mismatch)"),
                    );
                    continue;
                },
                BfsStepResult::Empty => {
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
