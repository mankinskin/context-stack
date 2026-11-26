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
    CursorState,
    CursorStateMachine,
    Matched,
    Mismatched,
    PathCursor,
};
use context_trace::CompactFormat;
use std::marker::PhantomData;

/// Marker trait for candidate state control
///
/// Controls whether the Checkpointed cursor has an advanced candidate or is at checkpoint.
/// Uses an associated type to encode the presence/absence of candidate data in the type system.
pub trait CandidateState: 'static {
    /// The type of candidate data stored
    /// - `()` for AtCheckpoint (no candidate)
    /// - `C` for HasCandidate (cursor stored)
    type CandidateData<C>;
}

/// At checkpoint - no candidate exists, only checkpoint
///
/// Used for finalized results like MatchResult where we store only the confirmed checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtCheckpoint;
impl CandidateState for AtCheckpoint {
    type CandidateData<C> = ();
}

/// Has candidate - candidate cursor exists alongside checkpoint
///
/// Used during active search/comparison when cursors are exploring ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HasCandidate;
impl CandidateState for HasCandidate {
    type CandidateData<C> = C;
}

/// Trait for cursors that can have a checkpoint
///
/// Maps a cursor type to its Matched version for checkpoint storage
pub trait HasCheckpoint {
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
            _state: PhantomData,
        }
    }
}

/// Encapsulates a cursor with its checkpoint state
///
/// Uses type-level encoding for candidate presence:
/// - `AtCheckpoint`: candidate field has type `()` (zero-sized)
/// - `HasCandidate`: candidate field has type `C` (cursor stored)
///
/// # Type Parameters
/// - `C`: The cursor type being wrapped (e.g., `PathCursor<P, S>` or `ChildCursor<S, N>`)
/// - `S`: The candidate state marker (AtCheckpoint or HasCandidate)
///
/// # Invariants
/// - `checkpoint` is always in Matched state
/// - Candidate presence is encoded in the type system via `S::CandidateData<C>`
/// - `checkpoint.atom_position <= candidate.atom_position` (checkpoint never ahead of candidate)
/// - Updates to checkpoint only happen via `mark_match()`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpointed<C: HasCheckpoint, S: CandidateState = AtCheckpoint> {
    /// Last confirmed match position (always Matched state)
    /// This is updated only when `mark_match()` is called
    pub(crate) checkpoint: C::Checkpoint,

    /// Advanced cursor position beyond checkpoint
    /// Type depends on S: `()` for AtCheckpoint, `C` for HasCandidate
    pub(crate) candidate: S::CandidateData<C>,

    /// Phantom data for candidate state marker
    pub(crate) _state: PhantomData<S>,
}

// Common methods available on all Checkpointed regardless of state
impl<C: HasCheckpoint, S: CandidateState> Checkpointed<C, S> {
    /// Get the checkpoint cursor (always Matched state)
    pub fn checkpoint(&self) -> &C::Checkpoint {
        &self.checkpoint
    }
}

// Methods only available when at checkpoint (AtCheckpoint)
impl<C: HasCheckpoint> Checkpointed<C, AtCheckpoint> {
    /// Create a new checkpointed cursor at checkpoint (no candidate)
    pub fn new(checkpoint: C::Checkpoint) -> Self {
        Self {
            checkpoint,
            candidate: (),
            _state: PhantomData,
        }
    }

    /// Check if currently at checkpoint (always true for AtCheckpoint)
    #[inline]
    pub fn at_checkpoint(&self) -> bool {
        true
    }
}

// Methods only available when has candidate (HasCandidate)
impl<C: HasCheckpoint> Checkpointed<C, HasCandidate> {
    /// Create a checkpointed cursor with an advanced candidate
    pub fn with_candidate(
        checkpoint: C::Checkpoint,
        candidate: C,
    ) -> Self {
        Self {
            checkpoint,
            candidate,
            _state: PhantomData,
        }
    }

    /// Get reference to the advanced candidate
    /// Guaranteed to exist by type system (stored directly, not in Option)
    pub fn candidate(&self) -> &C {
        &self.candidate
    }

    /// Get mutable reference to the advanced candidate
    /// Guaranteed to exist by type system (stored directly, not in Option)
    pub fn candidate_mut(&mut self) -> &mut C {
        &mut self.candidate
    }

    /// Get reference to current cursor position (alias for candidate)
    #[inline]
    pub fn current(&self) -> &C {
        self.candidate()
    }

    /// Get mutable reference to current cursor position (alias for candidate_mut)
    #[inline]
    pub fn current_mut(&mut self) -> &mut C {
        self.candidate_mut()
    }

    /// Check if currently at checkpoint (always false for HasCandidate)
    #[inline]
    pub fn at_checkpoint(&self) -> bool {
        false
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
    pub fn as_candidate(
        &self
    ) -> Checkpointed<PathCursor<P, Candidate>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: CursorStateMachine::to_candidate(&self.checkpoint),
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
    pub fn mark_match(
        self
    ) -> Checkpointed<PathCursor<P, Matched>, HasCandidate> {
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
    pub fn mark_mismatch(
        self
    ) -> Checkpointed<PathCursor<P, Mismatched>, HasCandidate> {
        Checkpointed {
            checkpoint: self.checkpoint,
            candidate: CursorStateMachine::to_mismatched(self.candidate),
            _state: PhantomData,
        }
    }
}

// ============================================================================
// CompactFormat implementations
// ============================================================================

impl<T: CompactFormat + HasCheckpoint> CompactFormat
    for Checkpointed<T, AtCheckpoint>
where
    T::Checkpoint: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "Checkpointed(at_checkpoint)")
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        writeln!(f, "{}Checkpointed {{", " ".repeat(indent))?;
        writeln!(f, "{}  checkpoint:", " ".repeat(indent))?;
        self.checkpoint.fmt_indented(f, indent + 4)?;
        writeln!(
            f,
            "{}  candidate: (none - at checkpoint)",
            " ".repeat(indent)
        )?;
        writeln!(f, "{}}}", " ".repeat(indent))
    }
}

impl<T: CompactFormat + HasCheckpoint> CompactFormat
    for Checkpointed<T, HasCandidate>
where
    T: CompactFormat,
    T::Checkpoint: CompactFormat,
{
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "Checkpointed(has_candidate)")
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        writeln!(f, "{}Checkpointed {{", " ".repeat(indent))?;
        writeln!(f, "{}  checkpoint:", " ".repeat(indent))?;
        self.checkpoint.fmt_indented(f, indent + 4)?;
        writeln!(f, "{}  candidate:", " ".repeat(indent))?;
        self.candidate.fmt_indented(f, indent + 4)?;
        writeln!(f, "{}}}", " ".repeat(indent))
    }
}

impl<T: CompactFormat + HasCheckpoint> std::fmt::Display
    for Checkpointed<T, AtCheckpoint>
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

impl<T: CompactFormat + HasCheckpoint> std::fmt::Display
    for Checkpointed<T, HasCandidate>
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
