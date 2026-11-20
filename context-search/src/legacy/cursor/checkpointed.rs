//! Row-specific `Checkpointed` impls (`ChildCursor`, `StateAdvance`).
//! The generic wrapper lives in [`crate::cursor::checkpointed`].

pub use crate::cursor::checkpointed::{
    AtCheckpoint,
    Checkpointed,
    HasCandidate,
    HasCheckpoint,
};

use super::{
    Candidate,
    ChildCursor,
    CursorStateMachine,
    Matched,
    Mismatched,
    PathCursor,
};
use context_trace::legacy::{
    trace::state::StateAdvance,
    Advance,
    HasGraph,
    PathNode,
};
use std::{
    marker::PhantomData,
    ops::ControlFlow,
};

impl<S: super::CursorState, EndNode: PathNode> HasCheckpoint
    for ChildCursor<S, EndNode>
where
    EndNode: Clone,
{
    type Checkpoint = ChildCursor<Matched, EndNode>;

    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
        // Convert Matched cursor to target state
        ChildCursor {
            child_state: checkpoint.child_state.clone(),
            _state: std::marker::PhantomData,
        }
    }
}

// Implementation for ChildCursor<S, EndNode>
impl<EndNode: PathNode>
    Checkpointed<ChildCursor<Matched, EndNode>, AtCheckpoint>
where
    EndNode: Clone,
{
    /// Convert checkpoint cursor to Candidate state with candidate
    ///
    /// The checkpoint remains unchanged at the last matched position.
    /// Returns HasCandidate state with the new candidate cursor.
    #[allow(dead_code)]
    pub(crate) fn as_candidate(
        &self
    ) -> Checkpointed<ChildCursor<Candidate, EndNode>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: CursorStateMachine::to_candidate(&self.checkpoint),
            _state: PhantomData,
        }
    }
}

impl<EndNode: PathNode>
    Checkpointed<ChildCursor<Candidate, EndNode>, HasCandidate>
where
    EndNode: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the candidate cursor to Matched state and updates
    /// the checkpoint. Returns HasCandidate state.
    pub(crate) fn mark_match(
        self
    ) -> Checkpointed<ChildCursor<Matched, EndNode>, HasCandidate> {
        let matched = CursorStateMachine::to_matched(self.candidate);
        Checkpointed {
            checkpoint: matched.clone(),
            candidate: matched,
            _state: PhantomData,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the candidate cursor to Mismatched state without
    /// updating the checkpoint. Stays in HasCandidate state.
    pub(crate) fn mark_mismatch(
        self
    ) -> Checkpointed<ChildCursor<Mismatched, EndNode>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint,
            candidate: CursorStateMachine::to_mismatched(self.candidate),
            _state: PhantomData,
        }
    }
}

// ============================================================================
// StateAdvance implementations
// ============================================================================

impl<P> StateAdvance for Checkpointed<PathCursor<P, Matched>, AtCheckpoint>
where
    P: Clone,
    PathCursor<P, Candidate>: Advance,
{
    type Next = Checkpointed<PathCursor<P, Matched>, HasCandidate>;

    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        // Convert checkpoint to candidate and advance
        let mut candidate = CursorStateMachine::to_candidate(&self.checkpoint);
        match candidate.advance(trav) {
            ControlFlow::Continue(()) => {
                // Successfully advanced - convert to matched and return with new candidate
                use super::MarkMatchState;
                Ok(Checkpointed {
                    checkpoint: self.checkpoint,
                    candidate: candidate.mark_match(),
                    _state: PhantomData,
                })
            },
            ControlFlow::Break(()) => {
                // Cannot advance from checkpoint
                Err(self)
            },
        }
    }
}

impl<EndNode: PathNode> StateAdvance
    for Checkpointed<ChildCursor<Matched, EndNode>, AtCheckpoint>
where
    EndNode: Clone,
    context_trace::legacy::ChildState<EndNode>:
        StateAdvance<Next = context_trace::legacy::ChildState<EndNode>>,
{
    type Next = Checkpointed<ChildCursor<Matched, EndNode>, HasCandidate>;

    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        // Advance checkpoint's child_state
        let child_state = self.checkpoint.child_state.clone();
        match child_state.advance_state(trav) {
            Ok(advanced_state) => Ok(Checkpointed {
                checkpoint: self.checkpoint,
                candidate: ChildCursor {
                    child_state: advanced_state,
                    _state: std::marker::PhantomData,
                },
                _state: PhantomData,
            }),
            Err(_failed_state) => {
                // Cannot advance from checkpoint
                Err(self)
            },
        }
    }
}
