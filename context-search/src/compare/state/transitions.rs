use super::core::{
    CompareState,
    IndexAdvanceResult,
    QueryAdvanceResult,
};
use crate::cursor::{
    Candidate,
    Checkpointed,
    ChildCursor,
    CursorState,
    MarkMatchState,
    Matched,
    Mismatched,
};
use context_trace::{
    graph::vertex::token::HasSubLocation,
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
