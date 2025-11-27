//! Checkpointed cursor wrapper that encapsulates cursor advancement with checkpoint tracking
//!
//! The `Checkpointed<C>` type wraps a cursor with its checkpoint state, ensuring they're
//! always managed together. This provides:
//! - Single source of truth for cursor + checkpoint state
//! - Type-safe state transitions (Candidate ↔ Matched ↔ Mismatched)
//! - Uniform handling across query and child cursors
//! - Prevention of atom_position desynchronization bugs

use super::{
    Candidate,
    ChildCursor,
    CursorState,
    CursorStateMachine,
    Matched,
    Mismatched,
    PathCursor,
};
use context_trace::{
    impl_display_via_compact,
    logging::{
        write_indent,
        CompactFormat,
    },
    trace::state::StateAdvance,
    Advance,
    HasGraph,
    PathNode,
};
use std::{
    marker::PhantomData,
    ops::ControlFlow,
};

/// Marker trait for candidate state control
///
/// Controls whether the Checkpointed cursor has an advanced candidate or is at checkpoint.
pub trait CandidateState: 'static {}

/// At checkpoint - no candidate exists, only checkpoint
///
/// Used for finalized results like MatchResult where we store only the confirmed checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtCheckpoint;
impl CandidateState for AtCheckpoint {}

/// Has candidate - candidate cursor exists alongside checkpoint
///
/// Used during active search/comparison when cursors are exploring ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HasCandidate;
impl CandidateState for HasCandidate {}

/// Trait for cursors that can have a checkpoint
///
/// Maps a cursor type to its Matched version for checkpoint storage
pub(crate) trait HasCheckpoint {
    type Checkpoint;

    /// Convert checkpoint (Matched state) to current cursor type
    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self;
}

impl<P, S: CursorState> HasCheckpoint for PathCursor<P, S>
where
    P: Clone,
{
    type Checkpoint = PathCursor<P, Matched>;

    fn from_checkpoint(checkpoint: &Self::Checkpoint) -> Self {
        // Convert Matched cursor to target state
        PathCursor {
            path: checkpoint.path.clone(),
            atom_position: checkpoint.atom_position,
            _state: std::marker::PhantomData,
        }
    }
}

impl<S: CursorState, EndNode: PathNode> HasCheckpoint
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

/// Encapsulates a cursor with its checkpoint state
///
/// Uses space-efficient storage: `candidate: Option<C>` is None when at checkpoint,
/// Some when advanced beyond. This saves 50% space when cursors match their checkpoints.
///
/// # Type Parameters
/// - `C`: The cursor type being wrapped (e.g., `PathCursor<P, S>` or `ChildCursor<S, N>`)
/// - `S`: The candidate state marker (AtCheckpoint or HasCandidate)
///
/// # Invariants
/// - `checkpoint` is always in Matched state
/// - When `S = AtCheckpoint`: `candidate` is None (enforced by construction)
/// - When `S = HasCandidate`: `candidate` is Some (enforced by construction)
/// - `checkpoint.atom_position <= candidate.atom_position` (checkpoint never ahead of candidate)
/// - Updates to checkpoint only happen via `mark_match()`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpointed<C: HasCheckpoint, S: CandidateState = AtCheckpoint> {
    /// Last confirmed match position (always Matched state)
    /// This is updated only when `mark_match()` is called
    pub(crate) checkpoint: C::Checkpoint,

    /// Advanced cursor position beyond checkpoint (None when at checkpoint)
    /// Only Some when cursor has advanced beyond last confirmed match
    pub(crate) candidate: Option<C>,

    /// Phantom data for candidate state marker
    pub(crate) _state: PhantomData<S>,
}

// Common methods available on all Checkpointed regardless of state
impl<C: HasCheckpoint, S: CandidateState> Checkpointed<C, S> {
    /// Get the checkpoint cursor (always Matched state)
    pub(crate) fn checkpoint(&self) -> &C::Checkpoint {
        &self.checkpoint
    }

    /// Check if currently at checkpoint (no advancement)
    pub(crate) fn at_checkpoint(&self) -> bool {
        self.candidate.is_none()
    }
}

// Methods only available when at checkpoint (AtCheckpoint)
impl<C: HasCheckpoint> Checkpointed<C, AtCheckpoint> {
    /// Create a new checkpointed cursor at checkpoint (no candidate)
    pub(crate) fn new(checkpoint: C::Checkpoint) -> Self {
        Self {
            checkpoint,
            candidate: None,
            _state: PhantomData,
        }
    }
}

// Methods only available when has candidate (HasCandidate)
impl<C: HasCheckpoint> Checkpointed<C, HasCandidate> {
    /// Create a checkpointed cursor with an advanced candidate
    pub(crate) fn with_candidate(
        checkpoint: C::Checkpoint,
        candidate: C,
    ) -> Self {
        Self {
            checkpoint,
            candidate: Some(candidate),
            _state: PhantomData,
        }
    }

    /// Get reference to the advanced candidate
    /// Guaranteed to exist by type system
    pub(crate) fn candidate(&self) -> &C {
        self.candidate
            .as_ref()
            .expect("HasCandidate state guarantees candidate exists")
    }

    /// Get mutable reference to the advanced candidate
    /// Guaranteed to exist by type system
    pub(crate) fn candidate_mut(&mut self) -> &mut C {
        self.candidate
            .as_mut()
            .expect("HasCandidate state guarantees candidate exists")
    }

    /// Get reference to current cursor position (alias for candidate)
    #[inline]
    pub(crate) fn current(&self) -> &C {
        self.candidate()
    }

    /// Get mutable reference to current cursor position (alias for candidate_mut)
    #[inline]
    pub(crate) fn current_mut(&mut self) -> &mut C {
        self.candidate_mut()
    }
}

// Implementation for PathCursor<P, S> - state transitions
impl<P> Checkpointed<PathCursor<P, Matched>, AtCheckpoint>
where
    P: Clone,
{
    /// Convert checkpoint cursor to Candidate state with candidate
    ///
    /// The checkpoint remains unchanged at the last matched position.
    /// Returns HasCandidate state with the new candidate cursor.
    pub(crate) fn as_candidate(
        &self
    ) -> Checkpointed<PathCursor<P, Candidate>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: Some(CursorStateMachine::to_candidate(&self.checkpoint)),
            _state: PhantomData,
        }
    }
}

impl<P> Checkpointed<PathCursor<P, Candidate>, HasCandidate>
where
    P: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the candidate cursor to Matched state and updates
    /// the checkpoint. Returns HasCandidate state (keeps candidate structure).
    pub(crate) fn mark_match(
        self
    ) -> Checkpointed<PathCursor<P, Matched>, HasCandidate> {
        let matched = CursorStateMachine::to_matched(
            self.candidate
                .expect("HasCandidate state guarantees candidate exists"),
        );
        Checkpointed {
            checkpoint: matched.clone(),
            candidate: Some(matched),
            _state: PhantomData,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the candidate cursor to Mismatched state without
    /// updating the checkpoint. Stays in HasCandidate state.
    pub(crate) fn mark_mismatch(
        self
    ) -> Checkpointed<PathCursor<P, Mismatched>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint,
            candidate: Some(CursorStateMachine::to_mismatched(
                self.candidate
                    .expect("HasCandidate state guarantees candidate exists"),
            )),
            _state: PhantomData,
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
    pub(crate) fn as_candidate(
        &self
    ) -> Checkpointed<ChildCursor<Candidate, EndNode>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: Some(CursorStateMachine::to_candidate(&self.checkpoint)),
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
        let matched = CursorStateMachine::to_matched(
            self.candidate
                .expect("HasCandidate state guarantees candidate exists"),
        );
        Checkpointed {
            checkpoint: matched.clone(),
            candidate: Some(matched),
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
            candidate: Some(CursorStateMachine::to_mismatched(
                self.candidate
                    .expect("HasCandidate state guarantees candidate exists"),
            )),
            _state: PhantomData,
        }
    }
}

// Implement CompactFormat for Checkpointed
impl<T: CompactFormat + HasCheckpoint, S: CandidateState> CompactFormat
    for Checkpointed<T, S>
where
    T::Checkpoint: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "Checkpointed {{ ")?;
        write!(f, "checkpoint: ")?;
        self.checkpoint.fmt_compact(f)?;
        if let Some(candidate) = &self.candidate {
            write!(f, ", candidate: ")?;
            candidate.fmt_compact(f)?;
        }
        write!(f, " }}")
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        write_indent(f, indent)?;
        writeln!(f, "Checkpointed {{")?;
        write_indent(f, indent + 1)?;
        write!(f, "checkpoint: ")?;
        self.checkpoint.fmt_compact(f)?;
        writeln!(f, ",")?;
        if let Some(candidate) = &self.candidate {
            write_indent(f, indent + 1)?;
            write!(f, "candidate: ")?;
            candidate.fmt_compact(f)?;
            writeln!(f, ",")?;
        }
        write_indent(f, indent)?;
        write!(f, "}}")
    }
}

impl<T: CompactFormat + HasCheckpoint, S: CandidateState> std::fmt::Display
    for Checkpointed<T, S>
where
    T::Checkpoint: CompactFormat,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        self.fmt_indented(f, 0)
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
                    candidate: Some(candidate.mark_match()),
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
    context_trace::ChildState<EndNode>:
        StateAdvance<Next = context_trace::ChildState<EndNode>>,
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
                candidate: Some(ChildCursor {
                    child_state: advanced_state,
                    _state: std::marker::PhantomData,
                }),
                _state: PhantomData,
            }),
            Err(_failed_state) => {
                // Cannot advance from checkpoint
                Err(self)
            },
        }
    }
}
