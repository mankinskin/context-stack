//! Positioned child cursor: a [`ChildState`] in a cursor state (legacy
//! `ChildCursor`), and its checkpoint transitions.

use std::marker::PhantomData;

use context_trace::positioned::ChildState;

use super::{
    Candidate,
    Checkpointed,
    CursorState,
    HasCandidate,
    HasCheckpoint,
    Matched,
    Mismatched,
};

/// A graph-side match state inside one root, in state `State`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildCursor<State = Matched> {
    pub(crate) child_state: ChildState,
    pub(crate) _state: PhantomData<State>,
}

impl<S: CursorState> ChildCursor<S> {
    pub fn new(child_state: ChildState) -> Self {
        Self {
            child_state,
            _state: PhantomData,
        }
    }
    pub fn child_state(&self) -> &ChildState {
        &self.child_state
    }
    pub(crate) fn with_state<T: CursorState>(self) -> ChildCursor<T> {
        ChildCursor::new(self.child_state)
    }
}

impl<S: CursorState> HasCheckpoint for ChildCursor<S> {
    type Checkpoint = ChildCursor<Matched>;
    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
        ChildCursor::new(checkpoint.child_state.clone())
    }
}

impl Checkpointed<ChildCursor<Candidate>, HasCandidate> {
    /// The candidate's end leaf matched: extend its span over the leaf and
    /// make it the checkpoint.
    pub fn mark_match(self) -> Checkpointed<ChildCursor<Matched>, HasCandidate> {
        let mut matched: ChildCursor<Matched> = self.candidate.with_state();
        matched.child_state.match_end_leaf();
        Checkpointed::with_candidate(matched.clone(), matched)
    }
    /// The candidate mismatched; the checkpoint stays.
    pub fn mark_mismatch(self) -> Checkpointed<ChildCursor<Mismatched>, HasCandidate> {
        Checkpointed::with_candidate(self.checkpoint, self.candidate.with_state())
    }
}
