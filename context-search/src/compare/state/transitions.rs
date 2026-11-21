//! State transitions of compare states (legacy
//! `compare/state/transitions.rs`): marking matches, advancing the query
//! cursor and the child (index) cursor.

use std::ops::ControlFlow;

use context_trace::{
    positioned::{
        PositionedGraph,
        StateAdvance,
    },
    GraphKind,
};

use super::core::{
    CompareState,
    IndexAdvanceResult,
    QueryAdvanceResult,
};
use crate::cursor::{
    Candidate,
    Checkpointed,
    ChildCursor,
    MarkMatchState,
    Matched,
    Mismatched,
};

impl MarkMatchState for CompareState<Candidate, Candidate> {
    type Matched = CompareState<Matched, Matched>;
    type Mismatched = CompareState<Mismatched, Mismatched>;

    /// Both leaves matched: the query checkpoint moves to the query leaf,
    /// the child span extends over the graph leaf.
    fn mark_match(self) -> Self::Matched {
        CompareState {
            query: self.query.mark_match(),
            child: self.child.mark_match(),
            target: self.target,
            mode: self.mode,
        }
    }

    fn mark_mismatch(self) -> Self::Mismatched {
        CompareState {
            query: self.query.mark_mismatch(),
            child: self.child.mark_mismatch(),
            target: self.target,
            mode: self.mode,
        }
    }
}

impl CompareState<Matched, Matched> {
    /// Advance the query candidate to the next query token.
    /// `Exhausted` (unchanged) at the query end.
    pub(crate) fn advance_query_cursor<G: GraphKind>(
        mut self,
        graph: &PositionedGraph<G>,
    ) -> QueryAdvanceResult {
        match self.query.candidate_mut().advance(graph) {
            ControlFlow::Continue(()) => {
                let candidate = self.query.candidate().clone().with_state::<Candidate>();
                QueryAdvanceResult::Advanced(CompareState {
                    query: Checkpointed::with_candidate(self.query.checkpoint, candidate),
                    child: self.child,
                    target: self.target,
                    mode: self.mode,
                })
            },
            ControlFlow::Break(()) => QueryAdvanceResult::Exhausted,
        }
    }
}

impl CompareState<Candidate, Matched> {
    /// Advance the child candidate to the graph token at the matched end
    /// (corrected pop rule). `Exhausted` (unchanged) at the root end.
    pub(crate) fn advance_index_cursor<G: GraphKind>(
        self,
        graph: &PositionedGraph<G>,
    ) -> IndexAdvanceResult {
        match self
            .child
            .candidate()
            .child_state
            .clone()
            .advance_state(graph)
        {
            Ok(child_state) => {
                crate::search::stats::record_path_lens(
                    child_state.path.start.len(),
                    child_state.path.end.len(),
                );
                let mut state = CompareState {
                    child: Checkpointed::with_candidate(
                        self.child.checkpoint,
                        ChildCursor::new(child_state),
                    ),
                    query: self.query,
                    target: self.target,
                    mode: self.mode,
                };
                state.target = state.current_target();
                IndexAdvanceResult::Advanced(state)
            },
            Err(_) => IndexAdvanceResult::Exhausted(self),
        }
    }
}
