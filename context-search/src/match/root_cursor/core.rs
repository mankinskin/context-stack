use crate::{
    compare::{
        parent::ParentCompareState,
        state::CompareState,
    },
    cursor::{
        Candidate,
        CursorState,
        Matched,
        PatternCursor,
    },
    state::matched::MatchResult,
    traversal::SearchKind,
};
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use std::collections::VecDeque;

/// Result of advancing both query and child cursors
///
/// All variants represent valid outcomes of attempting to advance both cursors:
/// - `BothAdvanced`: Both cursors successfully moved forward
/// - `QueryExhausted`: Query pattern fully matched (no more tokens to match)
/// - `ChildExhausted`: Child path ended but query continues (need parent exploration)
pub(crate) enum AdvanceCursorsResult<K: SearchKind> {
    /// Both cursors advanced successfully
    BothAdvanced(RootCursor<K, Candidate, Candidate>),
    /// Query cursor exhausted - complete match found
    QueryExhausted,
    /// Child cursor exhausted - query continues, needs parent exploration
    ChildExhausted(RootCursor<K, Candidate, Matched>),
}

/// Result of advancing a cursor to completion (QueryExhausted or Mismatch)
///
/// Both variants represent valid outcomes:
/// - `Completed`: Cursor reached a conclusive end state (match found)
/// - `NeedsParentExploration`: Cursor needs to explore parent tokens to continue
pub(crate) enum AdvanceToEndResult<K: SearchKind> {
    /// Cursor completed with a match (QueryExhausted or partial match with Mismatch)
    Completed(MatchResult),
    /// Cursor needs parent exploration to continue matching
    /// Contains the best match found so far and the cursor needing parent exploration
    NeedsParentExploration {
        checkpoint: MatchResult,
        cursor: RootCursor<K, Candidate, Matched>,
    },
}

#[derive(Debug)]
pub(crate) struct RootCursor<
    K: SearchKind,
    Q: CursorState = Matched,
    I: CursorState = Matched,
> {
    pub(crate) state: Box<CompareState<Q, I>>,
    pub(crate) trav: K::Trav,
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub(crate) struct CompareParentBatch {
    #[deref]
    #[deref_mut]
    pub(crate) batch: ParentBatch,
    pub(crate) cursor: PatternCursor,
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
