use crate::cursor::{
    Candidate,
    Checkpointed,
    ChildCursor,
    CursorState,
    HasCandidate,
    Matched,
    Mismatched,
    PathCursor,
};
use context_trace::{
    path::accessors::has_path::HasRootedPath,
    *,
};
use std::fmt::Debug;
use tracing::debug;

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
/// Each cursor is wrapped in a `Checkpointed<C, S>` type that encapsulates:
/// - **candidate**: The position being evaluated (state controlled by generic Q or I)
/// - **checkpoint**: Last confirmed match (always Matched state)
/// - **S**: CandidateState - HasCandidate during search, AtCheckpoint in final results
///
/// During active search, all CompareState instances use HasCandidate state to track
/// both checkpoint and candidate positions. Only MatchResult (final result) uses AtCheckpoint.
///
/// ## Query Cursor (`query`)
/// - Tracks position in the query pattern being searched for
/// - Uses `PathCursor<PatternRangePath, Q>` to track start/end positions
/// - `atom_position` represents atoms consumed from the pattern
///
/// ## Child Cursor (`child`)
/// - Tracks position in the graph/index path being searched
/// - Uses `ChildCursor<I, EndNode>` wrapping ChildState with IndexRangePath
/// - Position tracked via `child.candidate().child_state`
///
/// # atom_position Tracking
///
/// The `atom_position` field represents the number of atoms consumed:
/// - Starts at 0 at the beginning of a pattern
/// - Increments by token width when advancing
/// - checkpoint.atom_position ≤ candidate.atom_position (candidate explores ahead)
///
/// Example: Matching pattern [a,b,c] where b,c form a composite token "bc":
/// ```text
/// Position:  0    1    2    3
/// Pattern:   a    b    c    
/// Tokens:    a   [bc]
///
/// After matching 'a':
///   query.checkpoint().atom_position = 1 (consumed 'a')
///   query.candidate().atom_position = 1 (about to test 'bc')
///
/// While testing 'bc':
///   query.checkpoint().atom_position = 1 (still at 'a')
///   query.candidate().atom_position = 3 (would consume through 'c')
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompareState<
    Q: CursorState = Candidate,
    I: CursorState = Candidate,
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Query cursor with checkpoint and candidate (HasCandidate state during search)
    /// Uses PatternRangePath to properly track start/end positions during matching
    pub(crate) query:
        Checkpointed<PathCursor<PatternRangePath, Q>, HasCandidate>,

    /// Index cursor with checkpoint and candidate (HasCandidate state during search)
    /// The ChildState contains the IndexRangePath being traversed with position-annotated end nodes
    pub(crate) child: Checkpointed<ChildCursor<I, EndNode>, HasCandidate>,

    pub(crate) target: DownKey,
    pub(crate) mode: PathPairMode,
}

// Type aliases for clarity
pub(crate) type MatchedCompareState =
    CompareState<Matched, Matched, PositionAnnotated<ChildLocation>>;

impl<Q: CursorState, I: CursorState, EndNode: PathNode>
    HasRootedPath<IndexRangePath<ChildLocation, EndNode>>
    for CompareState<Q, I, EndNode>
{
    /// Access the rooted path from the child cursor's candidate state
    fn rooted_path(&self) -> &IndexRangePath<ChildLocation, EndNode> {
        // Return reference to the path in candidate (HasCandidate state guarantees it exists)
        &self.child.candidate().child_state.path
    }

    /// Access the rooted path from the child cursor's candidate state (mutable)
    fn rooted_path_mut(
        &mut self
    ) -> &mut IndexRangePath<ChildLocation, EndNode> {
        // Return mutable reference to candidate's path (HasCandidate state guarantees it exists)
        &mut self.child.candidate_mut().child_state.path
    }
}
impl<EndNode: PathNode> CompareState<Matched, Matched, EndNode> {
    pub(crate) fn update_checkpoint(&mut self) {
        debug!(
            query.checkpoint = %self.query.checkpoint(),
            query.candidate = %self.query.candidate(),
            "Marking current positions as checkpoints"
        );
        // Update checkpoint from candidate (both are Matched state)
        // Take candidate and convert to checkpoint
        self.query.checkpoint = self.query.candidate().clone();
        self.child.checkpoint = self.child.candidate().clone();

        // Now update candidate to match checkpoint (no advancement)
        *self.query.candidate_mut() = self.query.checkpoint.clone();
        *self.child.candidate_mut() = self.child.checkpoint.clone();

        // After this, checkpoint and candidate are synchronized
    }
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum CompareLeafResult<
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Result of comparing the candidate (matched or mismatched)
    /// Both query and index cursors remain in their respective states
    Finished(CompareEndResult<EndNode>),
    /// Candidate needs decomposition into prefixes for comparison
    Prefixes(ChildQueue<CompareState<Candidate, Candidate, EndNode>>),
}
#[derive(Clone, Debug)]
pub(crate) enum CompareEndResult<
    EndNode: PathNode = PositionAnnotated<ChildLocation>,
> {
    /// Result of comparing the candidate (matched or mismatched)
    /// Both query and index cursors remain in their respective states
    FoundMatch(CompareState<Matched, Matched, EndNode>),
    Mismatch(CompareState<Mismatched, Mismatched, EndNode>),
}

// Generic implementation for any EndNode type - methods that don't require LeafToken<End>
impl<EndNode: PathNode> CompareState<Matched, Matched, EndNode> {}

impl From<CompareState<Candidate, Candidate>>
    for ChildQueue<CompareState<Candidate, Candidate>>
{
    fn from(val: CompareState<Candidate, Candidate>) -> Self {
        ChildQueue::from_iter([val])
    }
}
