//! Checkpointed cursor wrapper that encapsulates cursor advancement with checkpoint tracking
//!
//! The `Checkpointed<C>` type wraps a cursor with its checkpoint state, ensuring they're
//! always managed together. This provides:
//! - Single source of truth for cursor + checkpoint state
//! - Type-safe state transitions (Candidate ↔ Matched ↔ Mismatched)
//! - Uniform handling across query and child cursors
//! - Prevention of atom_position desynchronization bugs

use super::{Candidate, ChildCursor, CursorState, MarkMatchState, Matched, Mismatched, PathCursor};
use context_trace::PathNode;
use std::marker::PhantomData;

/// Trait for cursors that can have a checkpoint
///
/// Maps a cursor type to its Matched version for checkpoint storage
pub(crate) trait HasCheckpoint {
    type Checkpoint;
}

impl<P, S: CursorState> HasCheckpoint for PathCursor<P, S> {
    type Checkpoint = PathCursor<P, Matched>;
}

impl<S: CursorState, EndNode: PathNode> HasCheckpoint for ChildCursor<S, EndNode> {
    type Checkpoint = ChildCursor<Matched, EndNode>;
}

/// Encapsulates a cursor with its checkpoint state
///
/// The `current` cursor may be in any state (Candidate/Matched/Mismatched),
/// while the `checkpoint` is always in Matched state, representing the last
/// confirmed match position.
///
/// # Type Parameters
/// - `C`: The cursor type being wrapped (e.g., `PathCursor<P, S>` or `ChildCursor<S, N>`)
///
/// # Invariants
/// - `checkpoint` is always in Matched state
/// - `checkpoint.atom_position <= current.atom_position` (checkpoint never ahead)
/// - Updates to checkpoint only happen via `mark_match()`
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Checkpointed<C: HasCheckpoint> {
    /// Current cursor position (may be Candidate/Matched/Mismatched)
    pub(crate) current: C,

    /// Last confirmed match position (always Matched state)
    /// This is updated only when `mark_match()` is called
    pub(crate) checkpoint: C::Checkpoint,
}

impl<C: HasCheckpoint> Checkpointed<C> {
    /// Get the checkpoint cursor (always Matched state)
    pub(crate) fn checkpoint(&self) -> &C::Checkpoint {
        &self.checkpoint
    }

    /// Get the current cursor
    pub(crate) fn current(&self) -> &C {
        &self.current
    }

    /// Get mutable access to current cursor (for internal use)
    pub(crate) fn current_mut(&mut self) -> &mut C {
        &mut self.current
    }
}

// Implementation for PathCursor<P, S>
impl<P> Checkpointed<PathCursor<P, Matched>>
where
    P: Clone,
{
    /// Create a new checkpointed cursor from an initial matched position
    ///
    /// Both current and checkpoint start at the same position.
    pub(crate) fn new(initial: PathCursor<P, Matched>) -> Self {
        Self {
            checkpoint: initial.clone(),
            current: initial,
        }
    }

    /// Convert current cursor to Candidate state (for next comparison)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self,
    ) -> Checkpointed<PathCursor<P, Candidate>> {
        Checkpointed {
            current: self.current.as_candidate(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}

impl<P> Checkpointed<PathCursor<P, Candidate>>
where
    P: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the current cursor to Matched state and updates
    /// the checkpoint to match the new position.
    pub(crate) fn mark_match(self) -> Checkpointed<PathCursor<P, Matched>> {
        let matched = self.current.mark_match();
        Checkpointed {
            checkpoint: matched.clone(),
            current: matched,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the current cursor to Mismatched state without
    /// updating the checkpoint.
    pub(crate) fn mark_mismatch(
        self,
    ) -> Checkpointed<PathCursor<P, Mismatched>> {
        Checkpointed {
            current: self.current.mark_mismatch(),
            checkpoint: self.checkpoint,
        }
    }
}

impl<P> Checkpointed<PathCursor<P, Mismatched>>
where
    P: Clone,
{
    /// Convert current cursor to Candidate state (for retrying)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self,
    ) -> Checkpointed<PathCursor<P, Candidate>> {
        Checkpointed {
            current: self.current.as_candidate(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}

// Implementation for ChildCursor<S, EndNode>
impl<EndNode: PathNode> Checkpointed<ChildCursor<Matched, EndNode>>
where
    EndNode: Clone,
{
    /// Create a new checkpointed child cursor from an initial matched position
    ///
    /// Both current and checkpoint start at the same position.
    pub(crate) fn new(initial: ChildCursor<Matched, EndNode>) -> Self {
        Self {
            checkpoint: initial.clone(),
            current: initial,
        }
    }

    /// Convert current cursor to Candidate state (for next comparison)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self,
    ) -> Checkpointed<ChildCursor<Candidate, EndNode>> {
        Checkpointed {
            current: self.current.as_candidate(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}

impl<EndNode: PathNode> Checkpointed<ChildCursor<Candidate, EndNode>>
where
    EndNode: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the current cursor to Matched state and updates
    /// the checkpoint to match the new position.
    pub(crate) fn mark_match(
        self,
    ) -> Checkpointed<ChildCursor<Matched, EndNode>> {
        let matched = self.current.mark_match();
        Checkpointed {
            checkpoint: matched.clone(),
            current: matched,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the current cursor to Mismatched state without
    /// updating the checkpoint.
    pub(crate) fn mark_mismatch(
        self,
    ) -> Checkpointed<ChildCursor<Mismatched, EndNode>> {
        Checkpointed {
            current: self.current.mark_mismatch(),
            checkpoint: self.checkpoint,
        }
    }
}

impl<EndNode: PathNode> Checkpointed<ChildCursor<Mismatched, EndNode>>
where
    EndNode: Clone,
{
    /// Convert current cursor to Candidate state (for retrying)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self,
    ) -> Checkpointed<ChildCursor<Candidate, EndNode>> {
        Checkpointed {
            current: self.current.as_candidate(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}
