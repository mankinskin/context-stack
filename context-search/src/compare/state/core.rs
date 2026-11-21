//! Compare states (legacy `compare/state/core.rs`): a query cursor and a
//! child cursor inside one root, each with a checkpoint.

use context_trace::{
    positioned::trace::child::iterator::ChildQueue,
    DownKey,
    DownPosition,
};

use crate::cursor::{
    Candidate,
    Checkpointed,
    ChildCursor,
    CursorState,
    HasCandidate,
    Matched,
    QueryCursor,
};

pub(crate) enum QueryAdvanceResult {
    Advanced(CompareState<Candidate, Matched>),
    Exhausted,
}

pub(crate) enum IndexAdvanceResult {
    Advanced(CompareState<Candidate, Candidate>),
    Exhausted(CompareState<Candidate, Matched>),
}

/// Which side the last decomposition split (Decision 1 b: equal widths
/// split both sides in one step instead of forking).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPairMode {
    GraphMajor,
    QueryMajor,
    BothMajor,
}

impl std::fmt::Display for PathPairMode {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

/// - `query`: checkpoint = last matched query leaf, candidate = query leaf
///   to compare;
/// - `child`: checkpoint = last matched state, candidate = state whose end
///   leaf is the graph token to compare;
/// - `target`: down key of the graph leaf at the query position of the
///   query leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareState<Q: CursorState = Candidate, I: CursorState = Candidate> {
    pub(crate) query: Checkpointed<QueryCursor<Q>, HasCandidate>,
    pub(crate) child: Checkpointed<ChildCursor<I>, HasCandidate>,
    pub(crate) target: DownKey,
    pub(crate) mode: PathPairMode,
}

pub(crate) type MatchedCompareState = CompareState<Matched, Matched>;

impl<Q: CursorState, I: CursorState> CompareState<Q, I> {
    /// Target key of the current graph and query leaves.
    pub(crate) fn current_target(&self) -> DownKey {
        DownKey::new(
            self.child.current().child_state.path.end.leaf(),
            DownPosition(self.query.current().path.leaf_offset()),
        )
    }
}

impl CompareState<Matched, Matched> {
    /// Make the current positions the checkpoints.
    pub(crate) fn update_checkpoint(&mut self) {
        self.query.checkpoint = self.query.candidate().clone();
        self.child.checkpoint = self.child.candidate().clone();
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum CompareLeafResult {
    Finished(CompareEndResult),
    Prefixes(ChildQueue<CompareState<Candidate, Candidate>>),
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum CompareEndResult {
    FoundMatch(CompareState<Matched, Matched>),
    Mismatch,
}

impl From<CompareState<Candidate, Candidate>> for ChildQueue<CompareState<Candidate, Candidate>> {
    fn from(state: CompareState<Candidate, Candidate>) -> Self {
        ChildQueue::from_iter([state])
    }
}
