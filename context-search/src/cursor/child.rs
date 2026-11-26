//! Positioned child cursor: a [`ChildState`] in a cursor state (legacy
//! `ChildCursor`), and its checkpoint transitions.

use std::marker::PhantomData;

use crate::ChildState;
use context_trace::{
    CommittedIndex,
    VertexId,
};

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
pub struct ChildCursor<State = Matched, I: VertexId = CommittedIndex> {
    pub(crate) child_state: ChildState<I>,
    pub(crate) _state: PhantomData<State>,
}

impl<S: CursorState, I: VertexId> ChildCursor<S, I> {
    pub fn new(child_state: ChildState<I>) -> Self {
        Self {
            child_state,
            _state: PhantomData,
        }
    }
    pub fn child_state(&self) -> &ChildState<I> {
        &self.child_state
    }
    pub(crate) fn with_state<T: CursorState>(self) -> ChildCursor<T, I> {
        ChildCursor::new(self.child_state)
    }
}

impl<S: CursorState, I: VertexId> HasCheckpoint for ChildCursor<S, I> {
    type Checkpoint = ChildCursor<Matched, I>;
    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
        ChildCursor::new(checkpoint.child_state.clone())
    }
}

impl<I: VertexId> Checkpointed<ChildCursor<Candidate, I>, HasCandidate> {
    /// The candidate's end leaf matched: extend its span over the leaf and
    /// make it the checkpoint.
    pub fn mark_match(self) -> Checkpointed<ChildCursor<Matched, I>, HasCandidate> {
        let mut matched: ChildCursor<Matched, I> = self.candidate.with_state();
        matched.child_state.match_end_leaf();
        Checkpointed::with_candidate(matched.clone(), matched)
    }
    /// The candidate mismatched; the checkpoint stays.
    pub fn mark_mismatch(self) -> Checkpointed<ChildCursor<Mismatched, I>, HasCandidate> {
        Checkpointed::with_candidate(self.checkpoint, self.candidate.with_state())
    }
}
