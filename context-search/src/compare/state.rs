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
        accessors::child::HasRootedLeafToken,
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

/// Result of advancing only the query cursor
///
/// Both variants represent valid outcomes of attempting to advance:
/// - `Advanced`: Query cursor successfully moved to next token
/// - `Exhausted`: Query cursor reached end of pattern (not an error condition)
pub(crate) enum QueryAdvanceResult<
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Query cursor advanced to next token
    Advanced(CompareState<Candidate, Matched, EndNode>),
    /// Query cursor exhausted (reached end of pattern)
    Exhausted(CompareState<Matched, Matched, EndNode>),
}

/// Result of advancing only the index (child) cursor
///
/// Both variants represent valid outcomes of attempting to advance:
/// - `Advanced`: Index cursor successfully moved to next position
/// - `Exhausted`: Index cursor reached end of available positions (not an error condition)
pub(crate) enum IndexAdvanceResult<
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Index cursor advanced to next position  
    Advanced(CompareState<Candidate, Candidate, EndNode>),
    /// Index cursor exhausted (no more positions)
    Exhausted(CompareState<Candidate, Matched, EndNode>),
}

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
        let cursor_end_index = HasRootChildIndex::<End>::root_child_index(
            &self.query.current().path,
        );

        // Mark both cursors as matched, which updates their checkpoints
        let query_matched = self.query.mark_match();
        let child_matched = self.child.mark_match();

        let matched_end_index = HasRootChildIndex::<End>::root_child_index(
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
    /// Returns `Exhausted` variant if query cursor cannot advance (query pattern ended).
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

                QueryAdvanceResult::Advanced(CompareState {
                    query: query_candidate,
                    child: self.child,
                    target: self.target,
                    mode: self.mode,
                })
            },
            Break(_) => {
                debug!("query cursor cannot advance - query pattern ended");
                QueryAdvanceResult::Exhausted(self)
            },
        }
    }
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

        let cursor_end_index = HasRootChildIndex::<End>::root_child_index(
            &self.query.current().path,
        );
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
        .expand_to_prefix_comparisons(trav)
    }

    /// Generate token states for index prefixes with position tracking
    pub(crate) fn expand_to_prefix_comparisons<G: HasGraph>(
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
                let prefixes = self
                    .child
                    .current()
                    .child_state
                    .decompose_into_prefixes(trav);

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
                    .decompose_at_position(trav, base_position);

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
                IndexAdvanceResult::Advanced(CompareState {
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
            Err(failed_child_state) =>
                IndexAdvanceResult::Exhausted(CompareState {
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

/// Helper function to decompose a token into its prefix children.
/// Reduces code duplication across trait implementations.
fn decompose_token_to_prefixes<G, State>(
    leaf: Token,
    trav: &G,
    update_state: impl Fn(SubToken, ChildLocation) -> State,
) -> VecDeque<(SubToken, State)>
where
    G: HasGraph,
{
    debug!(
        leaf = %leaf,
        "getting prefix_children"
    );
    let prefix_children =
        trav.graph().expect_vertex(leaf).prefix_children::<G>();
    debug!(num_children = prefix_children.len(), "got prefix_children");

    let result = prefix_children
        .iter()
        .sorted_unstable_by(|a, b| b.token().width().cmp(&a.token().width()))
        .map(|sub| {
            let child_location = leaf.to_child_location(*sub.sub_location());
            let next_state = update_state(sub.clone(), child_location);
            (sub.clone(), next_state)
        })
        .collect();
    debug!("returning prefixes");
    result
}

pub trait PrefixStates: Sized + Clone {
    fn decompose_into_prefixes<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)>;
}

// Implementation for ChildState with plain ChildLocation paths
impl PrefixStates for ChildState<ChildLocation> {
    fn decompose_into_prefixes<G: HasGraph>(
        &self,
        trav: &G,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.role_rooted_leaf_token::<End, _>(trav);
        decompose_token_to_prefixes(leaf, trav, |_sub, child_location| {
            let mut next = self.clone();
            next.path_append(child_location);
            next
        })
    }
}

// Specific implementation for ChildState with position-annotated paths
impl PrefixStates for ChildState<PositionAnnotated<ChildLocation>> {
    fn decompose_into_prefixes<G: HasGraph>(
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

        // Use entry_pos as the position for appended nodes
        let position = self.entry_pos;

        decompose_token_to_prefixes(leaf, trav, |_sub, child_location| {
            let mut next = self.clone();
            // Append with proper position annotation matching entry_pos
            let annotated = PositionAnnotated {
                node: child_location,
                position,
            };
            // Directly append to the path with the annotated version
            next.path.path_append(annotated);
            next
        })
    }
}

// Separate implementation for PathCursor that correctly tracks atom_position
impl<P, S> PathCursor<P, S>
where
    P: HasRootedLeafToken<End> + PathAppend + Clone,
    S: CursorState,
{
    pub(crate) fn decompose_at_position<G: HasGraph>(
        &self,
        trav: &G,
        base_position: AtomPosition,
    ) -> VecDeque<(SubToken, Self)> {
        let leaf = self.path.role_rooted_leaf_token::<End, _>(trav);

        decompose_token_to_prefixes(leaf, trav, |_sub, child_location| {
            let mut next_path = self.path.clone();
            next_path.path_append(child_location);

            PathCursor {
                path: next_path,
                atom_position: base_position,
                _state: PhantomData,
            }
        })
    }
}
