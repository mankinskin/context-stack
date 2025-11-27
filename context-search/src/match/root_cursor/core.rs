use crate::{
    compare::{
        parent::ParentCompareState,
        state::CompareState,
    },
    cursor::{
        Candidate,
        Checkpointed,
        CursorState,
        HasCandidate,
        Matched,
        PatternCursor,
    },
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
};
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use std::collections::VecDeque;

/// Result of advancing a RootCursor from one matched state to the next
///
/// Represents all possible outcomes when advancing a RootCursor<Matched, Matched>:
/// - `Advanced`: Successfully found another match, returns new RootCursor<Matched, Matched>
/// - `Finished`: Reached an end condition (conclusive or inconclusive)
pub(crate) enum RootAdvanceResult<K: SearchKind> {
    /// Successfully advanced to next matched state
    Advanced(RootCursor<K, Matched, Matched>),
    /// Reached an end condition - either conclusive (found maximum match) or inconclusive (needs parent exploration)
    Finished(RootEndResult<K>),
}

/// Result when RootCursor advancement reaches an end condition
///
/// - `Conclusive`: Found the maximum match for this root (either Mismatch or QueryExhausted)
/// - `Inconclusive`: Root boundary reached, needs parent exploration to continue
pub(crate) enum RootEndResult<K: SearchKind> {
    /// Conclusive end - this is the maximum match for this root
    Conclusive(ConclusiveEnd<K>),
    /// Inconclusive end - needs parent exploration to potentially find longer match
    Inconclusive(RootCursor<K, Candidate, Matched>),
}

/// Conclusive end states - no further matching possible on this root
pub(crate) enum ConclusiveEnd<K: SearchKind> {
    /// Found a mismatch after some progress - this is the maximum match
    /// Returns the mismatched candidate cursor for creating the final MatchResult
    Mismatch(RootCursor<K, Candidate, Candidate>),
    /// Query pattern fully exhausted - complete match found
    /// No cursor returned as there's no next state to advance to
    Exhausted,
}

#[derive(Debug)]
pub(crate) struct RootCursor<
    K: SearchKind,
    Q: CursorState = Matched,
    I: CursorState = Matched,
> {
    pub(crate) state: CompareState<Q, I>,
    pub(crate) trav: K::Trav,
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub(crate) struct CompareParentBatch {
    #[deref]
    #[deref_mut]
    pub(crate) batch: ParentBatch,
    pub(crate) cursor: Checkpointed<PatternCursor<Candidate>, HasCandidate>,
}

impl CompareParentBatch {
    pub(crate) fn into_compare_batch(self) -> VecDeque<ParentCompareState> {
        self.batch
            .parents
            .into_iter()
            .map(|parent_state| ParentCompareState {
                parent_state,
                cursor: self.cursor.clone(),
            })
            .collect()
    }
}

impl<K: SearchKind> RootCursor<K, Candidate, Matched> {
    pub(crate) fn get_parent_batch(
        self,
        trav: &K::Trav,
    ) -> Option<(ParentCompareState, CompareParentBatch)> {
        let parent = ParentCompareState {
            parent_state: self.state.child.current().child_state.parent_state(),
            cursor: self.state.query,
        };
        // Note: The cursor should already be advanced by the `advanced()` method
        // before this function is called. We don't advance it again here.
        if let Some(batch) = K::Policy::next_batch(trav, &parent.parent_state) {
            let batch = CompareParentBatch {
                batch,
                cursor: parent.cursor.clone(),
            };
            Some((parent, batch))
        } else {
            None
        }
    }
}
