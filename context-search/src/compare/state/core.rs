use crate::{
    compare::parent::ParentCompareState,
    cursor::{
        Candidate,
        Checkpointed,
        ChildCursor,
        CursorState,
        Matched,
        Mismatched,
        PathCursor,
    },
};
use context_trace::*;
use std::{
    collections::VecDeque,
    fmt::Debug,
    marker::PhantomData,
};

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

// Type aliases for clarity
pub(crate) type MatchedCompareState =
    CompareState<Matched, Matched, PositionAnnotated<ChildLocation>>;

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

// Generic implementation for any EndNode type - methods that don't require LeafToken<End>
impl<EndNode: PathNode> CompareState<Candidate, Candidate, EndNode> {}

impl From<CompareState<Candidate, Candidate>>
    for ChildQueue<CompareState<Candidate, Candidate>>
{
    fn from(val: CompareState<Candidate, Candidate>) -> Self {
        ChildQueue::from_iter([val])
    }
}
