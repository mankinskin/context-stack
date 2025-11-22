use crate::{
    compare::parent::ParentCompareState,
    cursor::{
        Candidate,
        Checkpointed,
        ChildCursor,
        CursorState,
        MarkMatchState,
        Matched,
        Mismatched,
        PathCursor,
    },
};
use context_trace::{
    graph::vertex::token::{
        HasSubLocation,
        SubToken,
    },
    path::{
        accessors::child::RootedLeafToken,
        RolePathUtils,
    },
    trace::state::StateAdvance,
    *,
};
use itertools::Itertools;
use std::{
    cmp::Ordering,
    collections::VecDeque,
    fmt::Debug,
    marker::PhantomData,
    ops::ControlFlow::{
        Break,
        Continue,
    },
};
use tracing::{
    debug,
    trace,
};
use PathPairMode::*;

//pub(crate) type CompareQueue = VecDeque<CompareState<Candidate, Candidate>>;

// Type aliases for clarity
//pub(crate) type CandidateCompareState = CompareState<Candidate, Candidate>;
pub(crate) type MatchedCompareState =
    CompareState<Matched, Matched, PositionAnnotated<ChildLocation>>;

// Return type aliases for advance operations
/// Result of advancing only the query cursor: Ok = query advanced, Err = query ended
pub(crate) type QueryAdvanceResult<EndNode = PositionAnnotated<ChildLocation>> =
    Result<
        CompareState<Candidate, Matched, EndNode>,
        CompareState<Matched, Matched, EndNode>,
    >;

/// Result of advancing only the index cursor: Ok = index advanced, Err = index ended
pub(crate) type IndexAdvanceResult<EndNode = PositionAnnotated<ChildLocation>> =
    Result<
        CompareState<Candidate, Candidate, EndNode>,
        CompareState<Candidate, Matched, EndNode>,
    >;

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub(crate) enum PathPairMode {
    GraphMajor,
    QueryMajor,
}

impl std::fmt::Display for PathPairMode {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        match self {
            PathPairMode::GraphMajor => write!(f, "GraphMajor"),
            PathPairMode::QueryMajor => write!(f, "QueryMajor"),
        }
    }
}
/// State for comparing a candidate position against the graph.
///
/// Generic over TWO states `Q` (query) and `I` (index) to track both cursors' processing status:
/// - `CompareState<Candidate, Candidate>` - Both cursors unprocessed, exploring ahead
/// - `CompareState<Matched, Matched>` - Both cursors matched successfully
/// - `CompareState<Mismatched, Mismatched>` - Both cursors failed to match
///
/// # Checkpointed Cursor Architecture
///
/// Each cursor is wrapped in a `Checkpointed<C>` type that encapsulates:
/// - **current**: The position being evaluated (state controlled by generic Q or I)
/// - **checkpoint**: Last confirmed match (always Matched state)
///
/// This ensures cursor and checkpoint are always managed together, preventing
/// desynchronization bugs.
///
/// ## Query Cursor (`query`)
/// - Tracks position in the query pattern being searched for
/// - Uses `PathCursor<PatternRangePath, Q>` to track start/end positions
/// - `atom_position` represents atoms consumed from the pattern
///
/// ## Child Cursor (`child`)
/// - Tracks position in the graph/index path being searched
/// - Uses `ChildCursor<I, EndNode>` wrapping ChildState with IndexRangePath
/// - Position tracked via `child.current().child_state`
///
/// # atom_position Tracking
///
/// The `atom_position` field represents the number of atoms consumed:
/// - Starts at 0 at the beginning of a pattern
/// - Increments by token width when advancing
/// - checkpoint.atom_position ≤ current.atom_position (current explores ahead)
///
/// Example: Matching pattern [a,b,c] where b,c form a composite token "bc":
/// ```text
/// Position:  0    1    2    3
/// Pattern:   a    b    c    
/// Tokens:    a   [bc]
///
/// After matching 'a':
///   query.checkpoint().atom_position = 1 (consumed 'a')
///   query.current().atom_position = 1 (about to test 'bc')
///
/// While testing 'bc':
///   query.checkpoint().atom_position = 1 (still at 'a')
///   query.current().atom_position = 3 (would consume through 'c')
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompareState<
    Q: CursorState = Candidate,
    I: CursorState = Candidate,
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Query cursor with checkpoint (state controlled by generic parameter Q)
    /// Uses PatternRangePath to properly track start/end positions during matching
    pub(crate) query: Checkpointed<PathCursor<PatternRangePath, Q>>,

    /// Index cursor with checkpoint (state controlled by generic parameter I)
    /// The ChildState contains the IndexRangePath being traversed with position-annotated end nodes
    pub(crate) child: Checkpointed<ChildCursor<I, EndNode>>,

    pub(crate) target: DownKey,
    pub(crate) mode: PathPairMode,
}

impl<Q: CursorState, I: CursorState, EndNode: PathNode>
    CompareState<Q, I, EndNode>
{
    /// Access the rooted path from the child cursor's state
    pub(crate) fn rooted_path(
        &self
    ) -> &IndexRangePath<ChildLocation, EndNode> {
        &self.child.current().child_state.path
    }

    ///// Get the leaf ChildLocation from the path, extracting from PositionAnnotated if needed
    //pub(crate) fn leaf_child_location(&self) -> Option<ChildLocation>
    //where
    //    EndNode: IntoChildLocation,
    //{
    //    self.rooted_path()
    //        .end_path()
    //        .last()
    //        .map(|node| node.as_child_location())
    //}
}

#[derive(Clone, Debug)]
pub(crate) enum CompareResult<
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Result of comparing the candidate (matched or mismatched)
    /// Both query and index cursors remain in their respective states
    FoundMatch(CompareState<Matched, Matched, EndNode>),
    Mismatch(CompareState<Mismatched, Mismatched, EndNode>),
    /// Candidate needs decomposition into prefixes for comparison
    Prefixes(ChildQueue<CompareState<Candidate, Candidate, EndNode>>),
}

impl<Q: CursorState + Clone, I: CursorState + Clone, EndNode: PathNode>
    CompareState<Q, I, EndNode>
{
    pub(crate) fn parent_state(&self) -> ParentCompareState {
        // IMPORTANT: Use current cursor (not checkpoint) to create parent state
        // The cursor.path is a PatternRangePath that tracks the current query position
        // This is passed to ParentCompareState.cursor and used as the starting point
        // for matching in parent roots, allowing incremental start path tracing:
        // - Each parent match builds on the previous match's cursor position
        // - Start paths can be traced incrementally from last match to new match
        let cursor = PathCursor {
            path: self.query.current().path.clone(),
            atom_position: self.query.current().atom_position,
            _state: PhantomData,
        };

        ParentCompareState {
            parent_state: self.child.current().child_state.parent_state(),
            cursor,
        }
    }
}

impl<EndNode: PathNode> MarkMatchState
    for CompareState<Candidate, Candidate, EndNode>
{
    type Matched = CompareState<Matched, Matched, EndNode>;
    type Mismatched = CompareState<Mismatched, Mismatched, EndNode>;

    fn mark_match(self) -> Self::Matched {
        let cursor_pos = self.query.current().atom_position;
        let old_checkpoint_pos = self.query.checkpoint().atom_position;
        let cursor_end_index =
            RootChildIndex::<End>::root_child_index(&self.query.current().path);

        // Mark both cursors as matched, which updates their checkpoints
        let query_matched = self.query.mark_match();
        let child_matched = self.child.mark_match();

        let matched_end_index = RootChildIndex::<End>::root_child_index(
            &query_matched.current().path,
        );
        tracing::trace!(
            cursor_pos = %cursor_pos,
            cursor_end_index = cursor_end_index,
            old_checkpoint_pos = %old_checkpoint_pos,
            new_checkpoint_pos = %query_matched.current().atom_position,
            matched_end_index = matched_end_index,
            "mark_match: converting to Matched state and updating checkpoints"
        );
        CompareState {
            query: query_matched,
            child: child_matched,
            target: self.target,
            mode: self.mode,
        }
    }

    fn mark_mismatch(self) -> Self::Mismatched {
        // Mark both cursors as mismatched, checkpoints remain unchanged
        CompareState {
            query: self.query.mark_mismatch(),
            child: self.child.mark_mismatch(),
            target: self.target,
            mode: self.mode,
        }
    }
}

impl<EndNode: PathNode> CompareState<Matched, Matched, EndNode> {
    /// Advance only the query cursor to the next token.
    /// Returns CompareState with query in Candidate state, index still in Matched state.
    ///
    /// Returns Err if query cursor cannot advance (query pattern ended).
    pub(crate) fn advance_query_cursor<G: HasGraph>(
        mut self,
        trav: &G,
    ) -> QueryAdvanceResult<EndNode> {
        debug!(
            cursor = %self.query.current(),
            "advancing query cursor only"
        );

        // Try to advance the query cursor's current position
        match self.query.current_mut().advance(trav) {
            Continue(_) => {
                trace!("query cursor advance succeeded");
                // Convert query to candidate state (checkpoint remains unchanged)
                let query_candidate = self.query.as_candidate();

                Ok(CompareState {
                    query: query_candidate,
                    child: self.child,
                    target: self.target,
                    mode: self.mode,
                })
            },
            Break(_) => {
                debug!("query cursor cannot advance - query pattern ended");
                Err(self)
            },
        }
    }
}

// Need impl for CompareState<Candidate, Matched> to support advance_index_cursor result
// Implementation for ChildLocation specifically - these methods require LeafToken<End>
// which is only implemented for ChildState<ChildLocation>, not generic EndNode
impl CompareState<Candidate, Candidate, ChildLocation> {
    //fn mode_prefixes<G: HasGraph>(
    //    &self,
    //    trav: &G,
    //    mode: PathPairMode,
    //) -> ChildQueue<CompareState<Candidate, Candidate, ChildLocation>> {
    //    debug!(
    //        old_mode = %self.mode,
    //        new_mode = %mode,
    //        "creating new state with different mode"
    //    );
    //    CompareState {
    //        mode,
    //        ..self.clone()
    //    }
    //    .prefix_states(trav)
    //}

    ///// Generate token states for index prefixes.
    /////
    ///// Decomposes composite tokens into their constituent sub-tokens for finer-grained comparison.
    ///// - GraphMajor mode: Decomposes the graph path token
    ///// - QueryMajor mode: Decomposes the query cursor token (with proper atom_position tracking)
    //pub(crate) fn prefix_states<G: HasGraph>(
    //    &self,
    //    trav: &G,
    //) -> ChildQueue<CompareState<Candidate, Candidate, ChildLocation>> {
    //    debug!(
    //        mode = %self.mode,
    //        child_state = %self.child_cursor.child_state,
    //        cursor = %self.cursor,
    //        "entering prefix_states"
    //    );

    //    match self.mode {
    //        GraphMajor => {
    //            let checkpoint_pos = *self.checkpoint.cursor_pos();
    //            debug!("calling child_state.prefix_states");
    //            let prefixes =
    //                self.child_cursor.child_state.prefix_states(trav);

    //            trace!(
    //                mode = "GraphMajor",
    //                num_prefixes = prefixes.len(),
    //                checkpoint_pos = %checkpoint_pos,
    //                "decomposing graph path token into prefixes"
    //            );

    //            let result: ChildQueue<
    //                CompareState<Candidate, Candidate, ChildLocation>,
    //            > = prefixes
    //                .into_iter()
    //                .enumerate()
    //                .map(|(i, (sub, child_state))| {
    //                    let token = sub.token();
    //                    let target_pos = checkpoint_pos.into();
    //                    debug!(
    //                        prefix_idx = i,
    //                        sub_width = *token.width(),
    //                        "creating prefix state"
    //                    );
    //                    CompareState {
    //                        target: DownKey::new(token, target_pos),
    //                        child_cursor: ChildCursor {
    //                            child_state,
    //                            _state: PhantomData,
    //                        },
    //                        mode: self.mode,
    //                        cursor: self.cursor.clone(),
    //                        checkpoint: self.checkpoint.clone(),
    //                        checkpoint_child: self.checkpoint_child.clone(),
    //                    }
    //                })
    //                .collect();
    //            debug!(
    //                num_results = result.len(),
    //                "exiting prefix_states (GraphMajor)"
    //            );
    //            result
    //        },
    //        QueryMajor => {
    //            // When decomposing the query cursor's token into prefixes, we need to track
    //            // position relative to the checkpoint, not the advanced cursor position
    //            let base_position = self.checkpoint.atom_position;
    //            debug!("calling cursor.prefix_states_from");
    //            let cursor_prefixes =
    //                self.cursor.prefix_states_from(trav, base_position);

    //            trace!(
    //                mode = "QueryMajor",
    //                cursor_pos = %self.cursor.atom_position,
    //                base_pos = %base_position,
    //                num_prefixes = cursor_prefixes.len(),
    //                "decomposing query cursor token into prefixes"
    //            );

    //            let result: ChildQueue<
    //                CompareState<Candidate, Candidate, ChildLocation>,
    //            > = cursor_prefixes
    //                .into_iter()
    //                .enumerate()
    //                .map(|(i, (sub, cursor))| {
    //                    trace!(
    //                        prefix_idx = i,
    //                        sub_width = *sub.token().width(),
    //                        cursor_pos = %cursor.atom_position,
    //                        "created prefix state"
    //                    );
    //                    CompareState {
    //                        target: DownKey::new(
    //                            sub.token(),
    //                            (*self.checkpoint.cursor_pos()).into(),
    //                        ),
    //                        child_cursor: self.child_cursor.clone(),
    //                        mode: self.mode,
    //                        cursor,
    //                        checkpoint: self.checkpoint.clone(),
    //                        checkpoint_child: self.checkpoint_child.clone(),
    //                    }
    //                })
    //                .collect();
    //            debug!(
    //                num_results = result.len(),
    //                "exiting prefix_states (QueryMajor)"
    //            );
    //            result
    //        },
    //    }
    //}

    ///// Compare a candidate against the graph to determine if tokens match.
    /////
    ///// Returns:
    ///// - `FoundMatch(CompareState<Matched, Matched>)` if tokens are identical - both cursors transition to Matched
    ///// - `Mismatch(CompareState<Mismatched, Mismatched>)` if both are atoms (width=1) and don't match
    ///// - `Prefixes(queue)` if tokens need decomposition into sub-tokens for finer comparison
    /////
    ///// The checkpoint (always Matched state) is NOT updated here - that's RootCursor's responsibility
    ///// after determining this match is part of the largest contiguous match.
    //pub(crate) fn compare_leaf_tokens<G: HasGraph>(
    //    self,
    //    trav: &G,
    //) -> CompareResult<ChildLocation> {
    //    use Ordering::*;
    //    let path_leaf =
    //        self.rooted_path().role_rooted_leaf_token::<End, _>(trav);
    //    let query_leaf = self.cursor.role_rooted_leaf_token::<End, _>(trav);

    //    debug!(
    //        path_leaf = %path_leaf,
    //        query_leaf = %query_leaf,
    //        path_width = *path_leaf.width(),
    //        query_width = *query_leaf.width(),
    //        cursor_pos = %self.cursor.atom_position,
    //        checkpoint_pos = %self.checkpoint.atom_position,
    //        mode = %self.mode,
    //        "comparing candidate tokens"
    //    );

    //    if path_leaf == query_leaf {
    //        debug!(
    //            token = *path_leaf.index,
    //            width = *path_leaf.width(),
    //            "tokens matched"
    //        );
    //        // Simplify the child path before marking as matched
    //        // This removes redundant path segments at token borders
    //        let mut state = self;
    //        state
    //            .child_cursor
    //            .child_state
    //            .path
    //            .child_path_mut::<Start, _>()
    //            .simplify(trav);
    //        state
    //            .child_cursor
    //            .child_state
    //            .path
    //            .child_path_mut::<End, _>()
    //            .simplify(trav);

    //        // Mark as matched using trait method
    //        CompareResult::FoundMatch(state.mark_match())
    //    } else if path_leaf.width() == 1 && query_leaf.width() == 1 {
    //        debug!(
    //            path_token = *path_leaf.index,
    //            query_token = *query_leaf.index,
    //            "atom mismatch - both width 1 but different"
    //        );
    //        // Mark as mismatched using trait method (checkpoint not updated here)
    //        CompareResult::Mismatch(self.mark_mismatch())
    //    } else {
    //        debug!(
    //            path_width = *path_leaf.width(),
    //            query_width = *query_leaf.width(),
    //            mode = %self.mode,
    //            "tokens need decomposition - calling mode_prefixes"
    //        );
    //        let prefixes = match path_leaf.width().cmp(&query_leaf.width()) {
    //            Equal => {
    //                debug!(
    //                    "equal width: calling both GraphMajor and QueryMajor"
    //                );
    //                self.mode_prefixes(trav, GraphMajor)
    //                    .into_iter()
    //                    .chain(self.mode_prefixes(trav, QueryMajor))
    //                    .collect()
    //            },
    //            Greater => {
    //                debug!("GraphMajor: path_width > query_width");
    //                self.mode_prefixes(trav, GraphMajor)
    //            },
    //            Less => {
    //                debug!("QueryMajor: path_width < query_width");
    //                self.mode_prefixes(trav, QueryMajor)
    //            },
    //        };
    //        debug!(num_prefixes = prefixes.len(), "returning Prefixes result");
    //        Prefixes(prefixes)
    //    }
    //}

    ///// Advance only the index cursor (via child_cursor) to the next token.
    ///// This is used after query cursor has already been advanced.
    ///// Returns CompareState with both cursors in Candidate state.
    /////
    ///// Returns Err if index cursor cannot advance (graph path ended).
    //pub(crate) fn advance_index_cursor<G: HasGraph>(
    //    self,
    //    trav: &G,
    //) -> IndexAdvanceResult<ChildLocation> {
    //    debug!(
    //        child_cursor = ?self.child_cursor,
    //        "advancing index cursor only (query already advanced)"
    //    );

    //    // child_cursor is already Candidate, advance it directly
    //    match self.child_cursor.child_state.advance_state(trav) {
    //        Ok(advanced_child_state) => {
    //            debug!("index cursor advance succeeded");

    //            Ok(CompareState {
    //                child_cursor: ChildCursor {
    //                    child_state: advanced_child_state,
    //                    _state: PhantomData,
    //                },
    //                cursor: self.cursor, // Already in Candidate state
    //                checkpoint: self.checkpoint,
    //                checkpoint_child: self.checkpoint_child,
    //                target: self.target,
    //                mode: self.mode,
    //            })
    //        },
    //        Err(child_state) => {
    //            debug!("index cursor cannot advance - graph path ended");
    //            Err(CompareState {
    //                child_cursor: ChildCursor {
    //                    child_state,
    //                    _state: PhantomData,
    //                },
    //                cursor: self.cursor,
    //                checkpoint: self.checkpoint,
    //                checkpoint_child: self.checkpoint_child,
    //                target: self.target,
    //                mode: self.mode,
    //            })
    //        },
    //    }
    //}
}

// Implementation for PositionAnnotated<ChildLocation> - these methods use the role_rooted_leaf_token helper
impl CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>> {
    /// Compare a candidate with position-annotated paths
    /// Extracts ChildLocations and delegates to token comparison logic
    pub(crate) fn compare_leaf_tokens<G: HasGraph>(
        self,
        trav: &G,
    ) -> CompareResult<PositionAnnotated<ChildLocation>> {
        use Ordering::*;
        let path_leaf =
            self.rooted_path().role_rooted_leaf_token::<End, _>(trav);
        let query_leaf =
            self.query.current().role_rooted_leaf_token::<End, _>(trav);

        let cursor_end_index =
            RootChildIndex::<End>::root_child_index(&self.query.current().path);
        debug!(
            path_leaf = %path_leaf,
            query_leaf = %query_leaf,
            path_width = *path_leaf.width(),
            query_width = *query_leaf.width(),
            cursor_pos = %self.query.current().atom_position,
            cursor_end_index = cursor_end_index,
            checkpoint_pos = %self.query.checkpoint().atom_position,
            mode = %self.mode,
            "comparing candidate tokens (position-annotated)"
        );

        if path_leaf == query_leaf {
            debug!(
                token = *path_leaf.index,
                width = *path_leaf.width(),
                "tokens matched"
            );
            CompareResult::FoundMatch(self.mark_match())
        } else {
            match path_leaf.width().cmp(&query_leaf.width()) {
                Equal if path_leaf.width() == TokenWidth(1) => {
                    trace!("atom mismatch: different atoms");
                    CompareResult::Mismatch(self.mark_mismatch())
                },
                Equal => {
                    trace!("equal width but not matching: need decomposition (both sides)");
                    CompareResult::Prefixes(
                        self.mode_prefixes(trav, GraphMajor),
                    )
                },
                Greater => {
                    trace!("GraphMajor: path_width > query_width");
                    CompareResult::Prefixes(
                        self.mode_prefixes(trav, GraphMajor),
                    )
                },
                Less => {
                    trace!("QueryMajor: path_width < query_width");
                    CompareResult::Prefixes(
                        self.mode_prefixes(trav, QueryMajor),
                    )
                },
            }
        }
    }

    fn mode_prefixes<G: HasGraph>(
        &self,
        trav: &G,
        mode: PathPairMode,
    ) -> ChildQueue<
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    > {
        debug!(
            old_mode = %self.mode,
            new_mode = %mode,
            "creating new state with different mode (position-annotated)"
        );
        CompareState {
            mode,
            ..self.clone()
        }
        .prefix_states(trav)
    }

    /// Generate token states for index prefixes with position tracking
    pub(crate) fn prefix_states<G: HasGraph>(
        &self,
        trav: &G,
    ) -> ChildQueue<
        CompareState<Candidate, Candidate, PositionAnnotated<ChildLocation>>,
    > {
        debug!(
            mode = %self.mode,
            child_state = %self.child.current().child_state,
            cursor = %self.query.current(),
            "entering prefix_states (position-annotated)"
        );

        match self.mode {
            GraphMajor => {
                let checkpoint_pos = *self.query.checkpoint().cursor_pos();
                debug!("calling child_state.prefix_states");
                let prefixes =
                    self.child.current().child_state.prefix_states(trav);

                trace!(
                    mode = "GraphMajor",
                    num_prefixes = prefixes.len(),
                    checkpoint_pos = %checkpoint_pos,
                    "decomposing graph path token into prefixes (position-annotated)"
                );

                let result: ChildQueue<
                    CompareState<
                        Candidate,
                        Candidate,
                        PositionAnnotated<ChildLocation>,
                    >,
                > = prefixes
                    .into_iter()
                    .enumerate()
                    .map(|(i, (sub, child_state))| {
                        let token = sub.token();
                        let target_pos = checkpoint_pos.into();
                        debug!(
                            prefix_idx = i,
                            sub_width = *token.width(),
                            "creating prefix state (position-annotated)"
                        );
                        CompareState {
                            target: DownKey::new(token, target_pos),
                            child: Checkpointed {
                                current: ChildCursor {
                                    child_state,
                                    _state: PhantomData,
                                },
                                checkpoint: self.child.checkpoint().clone(),
                            },
                            mode: self.mode,
                            query: self.query.clone(),
                        }
                    })
                    .collect();
                debug!(
                    num_results = result.len(),
                    "exiting prefix_states (GraphMajor, position-annotated)"
                );
                result
            },
            QueryMajor => {
                let base_position = self.query.checkpoint().atom_position;
                debug!("calling cursor.prefix_states_from");
                let cursor_prefixes = self
                    .query
                    .current()
                    .prefix_states_from(trav, base_position);

                trace!(
                    mode = "QueryMajor",
                    cursor_pos = %self.query.current().atom_position,
                    base_pos = %base_position,
                    num_prefixes = cursor_prefixes.len(),
                    "decomposing query cursor token into prefixes (position-annotated)"
                );

                let result: ChildQueue<
                    CompareState<
                        Candidate,
                        Candidate,
                        PositionAnnotated<ChildLocation>,
                    >,
                > = cursor_prefixes
                    .into_iter()
                    .enumerate()
                    .map(|(i, (sub, cursor))| {
                        trace!(
                            prefix_idx = i,
                            sub_width = *sub.token().width(),
                            cursor_pos = %cursor.atom_position,
                            "created prefix state (position-annotated)"
                        );
                        CompareState {
                            target: DownKey::new(
                                sub.token(),
                                (*self.query.checkpoint().cursor_pos()).into(),
                            ),
                            child: self.child.clone(),
                            mode: self.mode,
                            query: Checkpointed {
                                current: cursor,
                                checkpoint: self.query.checkpoint().clone(),
                            },
                        }
                    })
                    .collect();
                debug!(
                    num_results = result.len(),
                    "exiting prefix_states (QueryMajor, position-annotated)"
                );
                result
            },
        }
    }

    //pub(crate) fn advance_index_cursor<G: HasGraph>(
    //    self,
    //    trav: &G,
    //) -> IndexAdvanceResult<PositionAnnotated<ChildLocation>> {
    //    debug!(
    //        child_cursor = ?self.child_cursor,
    //        "advancing index cursor only (query already advanced, position-annotated)"
    //    );

    //    match self.child_cursor.child_state.advance_state(trav) {
    //        Ok(advanced_child_state) => {
    //            debug!("index cursor advance succeeded");
    //            // TODO: Update positions in the advanced state
    //            Ok(CompareState {
    //                child_cursor: ChildCursor {
    //                    child_state: advanced_child_state,
    //                    _state: PhantomData,
    //                },
    //                cursor: self.cursor,
    //                checkpoint: self.checkpoint,
    //                checkpoint_child: self.checkpoint_child,
    //                target: self.target,
    //                mode: self.mode,
    //            })
    //        },
    //        Err(child_state) => {
    //            debug!("index cursor cannot advance - graph path ended");
    //            Err(CompareState {
    //                child_cursor: ChildCursor {
    //                    child_state,
    //                    _state: PhantomData,
    //                },
    //                cursor: self.cursor,
    //                checkpoint: self.checkpoint,
    //                checkpoint_child: self.checkpoint_child,
    //                target: self.target,
    //                mode: self.mode,
    //            })
    //        },
    //    }
    //}
}

impl CompareState<Candidate, Matched, PositionAnnotated<ChildLocation>> {
    pub(crate) fn advance_index_cursor<G: HasGraph>(
        self,
        trav: &G,
    ) -> IndexAdvanceResult<PositionAnnotated<ChildLocation>> {
        let candidate_child_cursor = self.child.current().as_candidate();
        match candidate_child_cursor.child_state.advance_state(trav) {
            Ok(advanced_child_state) => {
                // TODO: Update positions in the advanced state
                Ok(CompareState {
                    child: Checkpointed {
                        current: ChildCursor {
                            child_state: advanced_child_state,
                            _state: PhantomData,
                        },
                        checkpoint: self.child.checkpoint().clone(),
                    },
                    query: self.query,
                    target: self.target,
                    mode: self.mode,
                })
            },
            Err(failed_child_state) => Err(CompareState {
                child: Checkpointed {
                    current: ChildCursor {
                        child_state: failed_child_state,
                        _state: PhantomData,
                    },
                    checkpoint: self.child.checkpoint().clone(),
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
        }
    }
}

// Generic implementation for any EndNode type - methods that don't require LeafToken<End>
impl<EndNode: PathNode> CompareState<Candidate, Candidate, EndNode> {}

//impl CompareState<Candidate, Matched, ChildLocation> {
//    pub(crate) fn advance_index_cursor<G: HasGraph>(
//        self,
//        trav: &G,
//    ) -> IndexAdvanceResult<ChildLocation> {
//        let candidate_child_cursor = self.child_cursor.as_candidate();
//        match candidate_child_cursor.child_state.advance_state(trav) {
//            Ok(advanced_child_state) => Ok(CompareState {
//                child_cursor: ChildCursor {
//                    child_state: advanced_child_state,
//                    _state: PhantomData,
//                },
//                cursor: self.cursor,
//                checkpoint: self.checkpoint,
//                checkpoint_child: self.checkpoint_child,
//                target: self.target,
//                mode: self.mode,
//            }),
//            Err(failed_child_state) => Err(CompareState {
//                child_cursor: ChildCursor {
//                    child_state: failed_child_state,
//                    _state: PhantomData,
//                },
//                cursor: self.cursor,
//                checkpoint: self.checkpoint,
//                checkpoint_child: self.checkpoint_child,
//                target: self.target,
//                mode: self.mode,
//            }),
//        }
//    }
//}

impl From<CompareState<Candidate, Candidate>>
    for ChildQueue<CompareState<Candidate, Candidate>>
{
    fn from(val: CompareState<Candidate, Candidate>) -> Self {
        ChildQueue::from_iter([val])
    }
}

impl StateAdvance for CompareState<Candidate, Candidate, ChildLocation> {
    type Next = Self;
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self, Self> {
        let child_state_clone = self.child.current().child_state.clone();
        match child_state_clone.advance_state(trav) {
            Ok(child_state) => Ok(CompareState {
                child: Checkpointed {
                    current: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    checkpoint: self.child.checkpoint().clone(),
                },
                ..self
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    current: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    checkpoint: self.child.checkpoint().clone(),
                },
                ..self
            }),
        }
    }
}

impl StateAdvance for CompareState<Matched, Matched, ChildLocation> {
    type Next = Self;
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self, Self> {
        let child_state_clone = self.child.current().child_state.clone();
        match child_state_clone.advance_state(trav) {
            Ok(child_state) => Ok(CompareState {
                child: Checkpointed {
                    current: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    checkpoint: self.child.checkpoint().clone(),
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    current: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    checkpoint: self.child.checkpoint().clone(),
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
        }
    }
}

pub trait PrefixStates: Sized + Clone {
    fn prefix_states<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)>;
}

// Implementation for ChildState with plain ChildLocation paths
impl PrefixStates for ChildState<ChildLocation> {
    fn prefix_states<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.role_rooted_leaf_token::<End, _>(trav);
        debug!(
            leaf = %leaf,
            "getting prefix_children"
        );
        let prefix_children =
            trav.graph().expect_vertex(leaf).prefix_children::<G>();
        debug!(num_children = prefix_children.len(), "got prefix_children");
        let result = prefix_children
            .iter()
            .sorted_unstable_by(|a, b| {
                b.token().width().cmp(&a.token().width())
            })
            .map(|sub| {
                let mut next = self.clone();
                next.path_append(leaf.to_child_location(*sub.sub_location()));
                (sub.clone(), next)
            })
            .collect();
        debug!("returning prefixes");
        result
    }
}

// Specific implementation for ChildState with position-annotated paths
impl PrefixStates for ChildState<PositionAnnotated<ChildLocation>> {
    fn prefix_states<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)> {
        // Get the end leaf by accessing the path directly
        let leaf_location =
            self.path.end_path().last().map(|annotated| annotated.node);

        let leaf = if let Some(loc) = leaf_location {
            *trav.graph().expect_child_at(loc)
        } else {
            // If path is empty, use root child
            self.path.role_root_child_token::<End, _>(trav)
        };

        debug!(
            leaf = %leaf,
            "getting prefix_children (position-annotated)"
        );
        let prefix_children =
            trav.graph().expect_vertex(leaf).prefix_children::<G>();
        debug!(num_children = prefix_children.len(), "got prefix_children");

        // Use entry_pos as the position for appended nodes
        let position = self.entry_pos;

        let result = prefix_children
            .iter()
            .sorted_unstable_by(|a, b| {
                b.token().width().cmp(&a.token().width())
            })
            .map(|sub| {
                let mut next = self.clone();
                // Append with proper position annotation matching entry_pos
                let child_location =
                    leaf.to_child_location(*sub.sub_location());
                let annotated = PositionAnnotated {
                    node: child_location,
                    position,
                };
                // Directly append to the path with the annotated version
                next.path.path_append(annotated);
                (sub.clone(), next)
            })
            .collect();
        debug!("returning prefixes (position-annotated)");
        result
    }
}

// Separate implementation for PathCursor that correctly tracks atom_position
impl<P, S> PathCursor<P, S>
where
    P: RootedLeafToken<End> + PathAppend + Clone,
    S: CursorState,
{
    pub(crate) fn prefix_states_from<G: HasGraph>(
        &self,
        trav: &G,
        base_position: AtomPosition,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.path.role_rooted_leaf_token::<End, _>(trav);

        trav.graph()
            .expect_vertex(leaf)
            .prefix_children::<G>()
            .iter()
            .sorted_unstable_by(|a, b| {
                b.token().width().cmp(&a.token().width())
            })
            .map(|sub| {
                let mut next_path = self.path.clone();
                next_path
                    .path_append(leaf.to_child_location(*sub.sub_location()));

                let next_cursor = PathCursor {
                    path: next_path,
                    atom_position: base_position,
                    _state: PhantomData,
                };

                (sub.clone(), next_cursor)
            })
            .collect()
    }
}
//impl From<ChildState> for EditKind {
//    fn from(state: ChildState) -> Self {
//        match state.path.role_leaf_token_location::<End>() {
//            Some(entry) => DownEdit {
//                target: state.target,
//                entry,
//            }
//            .into(),
//            None => RootEdit {
//                entry_key: state.target,
//                entry_location: entry,
//            }
//            .into(),
//        }
//    }
//}
