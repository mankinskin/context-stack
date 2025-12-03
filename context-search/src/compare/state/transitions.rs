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
    graph::vertex::location::SubRangeLocation,
    path::accessors::role::{
        End,
        Start,
    },
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
                let advanced_candidate =
                    CursorStateMachine::to_candidate(self.query.candidate());

                QueryAdvanceResult::Advanced(CompareState {
                    query: Checkpointed::with_candidate(
                        self.query.checkpoint,
                        advanced_candidate,
                    ),
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
            Ok(mut child_state) => {
                // If the end path became empty after advancing, we completed matching a root child
                // and should advance exit_pos by the width of that entire child
                if child_state.path.end_path().is_empty() {
                    // Get the current root_exit index (just advanced)
                    let current_end_index =
                        child_state.path.role_root_child_index::<End>();
                    let prev_root_child_token = self
                        .child
                        .candidate()
                        .child_state
                        .path
                        .role_root_child_token::<End, _>(trav);

                    tracing::trace!(
                        current_end_index = current_end_index,
                        prev_root_child_token = %prev_root_child_token,
                        old_exit_pos = ?child_state.exit_pos,
                        "end path became empty, updating exit_pos"
                    );

                    let pattern_location =
                        child_state.path.root_pattern_location();
                    let token_width = prev_root_child_token.width();

                    // Advance exit_pos by the width of the matched child
                    child_state.exit_pos = DownPosition(
                        (*child_state.exit_pos.0 + token_width.0).into(),
                    );

                    tracing::trace!(
                        new_exit_pos = ?child_state.exit_pos,
                        token_width = token_width.0,
                        "updated exit_pos after completing root child"
                    );

                    // Debug assert: exit_pos should equal entry_pos + width of children between start and current
                    #[cfg(debug_assertions)]
                    {
                        let entry_index =
                            child_state.path.role_root_child_index::<Start>();
                        let full_sub_range = SubRangeLocation::new(
                            pattern_location.pattern_id,
                            entry_index + 1..current_end_index,
                        );
                        let inner_width = trav
                            .graph()
                            .expect_vertex(pattern_location.parent)
                            .expect_child_range_offset(&full_sub_range);
                        debug_assert_eq!(
                            child_state.exit_pos.0,
                            (*child_state.entry_pos.0 + inner_width.0).into(),
                            "exit_pos should equal entry_pos + width of inner children"
                        );
                    }
                }

                // Successfully advanced - convert to Candidate state
                let advanced_child =
                    CursorStateMachine::to_candidate(&ChildCursor::<
                        Matched,
                        _,
                    > {
                        child_state,
                        _state: PhantomData,
                    });

                IndexAdvanceResult::Advanced(CompareState {
                    child: Checkpointed::with_candidate(
                        self.child.checkpoint,
                        advanced_child,
                    ),
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
                    candidate: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    _state: PhantomData,
                },
                ..self
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
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
                    candidate: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    _state: PhantomData,
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
            Err(child_state) => Ok(CompareState {
                child: Checkpointed {
                    checkpoint: self.child.checkpoint().clone(),
                    candidate: ChildCursor {
                        child_state,
                        _state: PhantomData,
                    },
                    _state: PhantomData,
                },
                query: self.query,
                target: self.target,
                mode: self.mode,
            }),
        }
    }
}
