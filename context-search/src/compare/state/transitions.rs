use super::core::{
    CompareState,
    IndexAdvanceResult,
    QueryAdvanceResult,
};
use crate::cursor::{
    Candidate,
    Checkpointed,
    ChildCursor,
    CursorStateMachine,
    MarkMatchState,
    Matched,
    Mismatched,
};
use context_trace::{
    trace::state::StateAdvance,
    HasRootChildIndex,
    *,
};
use std::{
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
            cursor = %self.query.candidate(),
            "advancing query cursor only"
        );

        // Try to advance the query cursor's candidate position
        match self.query.candidate_mut().advance(trav) {
            Continue(_) => {
                trace!("query cursor advance succeeded");
                // Convert candidate from Matched to Candidate state
                let advanced_candidate = CursorStateMachine::to_candidate(self.query.candidate());
                
                QueryAdvanceResult::Advanced(CompareState {
                    query: Checkpointed::with_candidate(self.query.checkpoint, advanced_candidate),
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

impl CompareState<Candidate, Matched, PositionAnnotated<ChildLocation>> {
    pub(crate) fn advance_index_cursor<G: HasGraph>(
        self,
        trav: &G,
    ) -> IndexAdvanceResult<PositionAnnotated<ChildLocation>> {
        // Advance the child cursor's candidate using its child_state
        let child_state_clone = self.child.candidate().child_state.clone();
        match child_state_clone.advance_state(trav) {
            Ok(child_state) => {
                // Successfully advanced - convert to Candidate state
                let advanced_child = CursorStateMachine::to_candidate(&ChildCursor::<Matched, _> {
                    child_state,
                    _state: PhantomData,
                });
                
                IndexAdvanceResult::Advanced(CompareState {
                    child: Checkpointed::with_candidate(self.child.checkpoint, advanced_child),
                    query: self.query,
                    target: self.target,
                    mode: self.mode,
                })
            },
            Err(_) => {
                // Child cursor cannot advance (at boundary)
                IndexAdvanceResult::Exhausted(self)
            },
        }
    }
}

impl StateAdvance for CompareState<Candidate, Candidate, ChildLocation> {
    type Next = Self;
    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self, Self> {
        let child_state_clone = self.child.candidate().child_state.clone();
        match child_state_clone.advance_state(trav) {
            Ok(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: Some(ChildCursor {
                        child_state,
                        _state: PhantomData,
                    }),
                    _state: PhantomData,
                },
                ..self
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: Some(ChildCursor {
                        child_state,
                        _state: PhantomData,
                    }),
                    _state: PhantomData,
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
        let child_state_clone = self.child.candidate().child_state.clone();
        match child_state_clone.advance_state(trav) {
            Ok(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: Some(ChildCursor {
                        child_state,
                        _state: PhantomData,
                    }),
                    _state: PhantomData,
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: Some(ChildCursor {
                        child_state,
                        _state: PhantomData,
                    }),
                    _state: PhantomData,
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
        }
    }
}
