//! Lock-step advance inside one root (legacy
//! `match/root_cursor/advance.rs`).

use context_trace::GraphKind;

use super::core::{
    ConclusiveEnd,
    RootAdvanceResult,
    RootCursor,
    RootEndResult,
};
use crate::{
    compare::{
        iterator::CompareIterator,
        state::{
            CompareEndResult,
            IndexAdvanceResult,
            QueryAdvanceResult,
        },
    },
    cursor::{
        Candidate,
        Checkpointed,
        CursorStateMachine,
        Matched,
    },
    policy::SearchKind,
    state::matched::{
        CheckpointedCursor,
        MatchResult,
        PathCoverage,
    },
};

impl<'g, G: GraphKind, K: SearchKind> RootCursor<'g, G, K, Matched, Matched> {
    /// Advance the query, then the graph, then compare. Ends:
    /// query exhausted → `Conclusive(Exhausted)`; mismatch →
    /// `Conclusive(Mismatch)`; root exhausted → `Inconclusive`.
    pub(crate) fn advance_to_next_match(self) -> RootAdvanceResult<'g, G, K> {
        let graph = self.graph;
        let state = match self.state.advance_query_cursor(graph) {
            QueryAdvanceResult::Advanced(state) => state,
            QueryAdvanceResult::Exhausted => {
                return RootAdvanceResult::Finished(RootEndResult::Conclusive(
                    ConclusiveEnd::Exhausted,
                ));
            },
        };
        let state = match state.advance_index_cursor(graph) {
            IndexAdvanceResult::Advanced(state) => state,
            IndexAdvanceResult::Exhausted(state) => {
                return RootAdvanceResult::Finished(RootEndResult::Inconclusive(Box::new(
                    RootCursor::new(graph, state),
                )));
            },
        };
        match CompareIterator::new(graph, state).compare() {
            CompareEndResult::FoundMatch(matched) => {
                RootAdvanceResult::Advanced(Box::new(RootCursor::new(graph, matched)))
            },
            CompareEndResult::Mismatch => {
                RootAdvanceResult::Finished(RootEndResult::Conclusive(ConclusiveEnd::Mismatch))
            },
        }
    }
}

impl<G: GraphKind, K: SearchKind> RootCursor<'_, G, K, Candidate, Matched> {
    /// The match up to the root end, with the query cursor advanced past
    /// the checkpoint (legacy: result of an inconclusive root).
    pub(crate) fn create_parent_exploration_state(&self) -> MatchResult {
        let checkpoint = self.state.query.checkpoint().clone();
        let candidate = self.state.query.candidate().clone().to_matched();
        let end_pos = checkpoint.atom_position;
        MatchResult::new(
            PathCoverage::from_child_state(
                self.graph,
                self.state.child.checkpoint().child_state.clone(),
                end_pos,
            ),
            CheckpointedCursor::HasCandidate(Checkpointed::with_candidate(checkpoint, candidate)),
        )
    }
}
