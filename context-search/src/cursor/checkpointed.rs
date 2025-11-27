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
use std::ops::ControlFlow;

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

/// Reference to current cursor position (either checkpoint or candidate)
///
/// Provides unified access to the current cursor without exposing the internal
/// Option-based storage. Can be pattern matched or converted to references.
///
/// When at checkpoint, owns a temporary cursor converted from checkpoint.
/// When at candidate, borrows the candidate.
#[derive(Debug)]
pub(crate) enum CheckpointedRef<'a, C> {
    /// At checkpoint (owns converted cursor from checkpoint)
    Checkpoint(C),
    /// Advanced beyond checkpoint (borrows candidate)
    Candidate(&'a C),
}

impl<'a, C> CheckpointedRef<'a, C> {
    /// Get reference to the cursor regardless of variant
    pub(crate) fn as_ref(&self) -> &C {
        match self {
            CheckpointedRef::Checkpoint(c) => c,
            CheckpointedRef::Candidate(c) => c,
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
///
/// # Invariants
/// - `checkpoint` is always in Matched state
/// - `candidate.is_none()` when current position equals checkpoint
/// - `candidate.is_some()` when advanced beyond checkpoint
/// - `checkpoint.atom_position <= candidate.atom_position` (checkpoint never ahead of candidate)
/// - Updates to checkpoint only happen via `mark_match()`
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Checkpointed<C: HasCheckpoint> {
    /// Last confirmed match position (always Matched state)
    /// This is updated only when `mark_match()` is called
    pub(crate) checkpoint: C::Checkpoint,

    /// Advanced cursor position beyond checkpoint (None when at checkpoint)
    /// Only Some when cursor has advanced beyond last confirmed match
    pub(crate) candidate: Option<C>,
}

impl<C: HasCheckpoint> Checkpointed<C> {
    /// Get the checkpoint cursor (always Matched state)
    pub(crate) fn checkpoint(&self) -> &C::Checkpoint {
        &self.checkpoint
    }

    /// Get reference to current cursor position (checkpoint or candidate)
    ///
    /// Returns CheckpointedRef enum that can be pattern-matched
    pub(crate) fn current(&self) -> CheckpointedRef<C> {
        match &self.candidate {
            None => CheckpointedRef::Checkpoint(C::from_checkpoint(
                &self.checkpoint,
            )),
            Some(candidate) => CheckpointedRef::Candidate(candidate),
        }
    }

    /// Get mutable reference to current cursor, materializing candidate if needed
    ///
    /// This converts the checkpoint to a candidate if we're currently at checkpoint.
    /// After calling this, `self.candidate` will be `Some`.
    pub(crate) fn current_mut(&mut self) -> &mut C {
        if self.candidate.is_none() {
            // Convert checkpoint to C and store as candidate
            self.candidate = Some(C::from_checkpoint(&self.checkpoint));
        }
        self.candidate.as_mut().unwrap()
    }

    /// Check if currently at checkpoint (no advancement)
    pub(crate) fn at_checkpoint(&self) -> bool {
        self.candidate.is_none()
    }

    /// Get reference to current cursor, extracting from CheckpointedRef
    ///
    /// This is a convenience method that wraps current() and extracts the reference.
    /// The returned reference borrows from the CheckpointedRef, which may own a temporary.
    #[inline]
    pub(crate) fn current_as_ref(
        &self
    ) -> impl std::ops::Deref<Target = C> + '_ {
        self.current()
    }
}

// Implement Deref for CheckpointedRef so it can be used transparently
impl<'a, C> std::ops::Deref for CheckpointedRef<'a, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

// Implement Display for CheckpointedRef by delegating to inner cursor
impl<'a, C: std::fmt::Display> std::fmt::Display for CheckpointedRef<'a, C> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

// Implement CompactFormat for CheckpointedRef by delegating to inner cursor
impl<'a, C: CompactFormat> CompactFormat for CheckpointedRef<'a, C> {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        self.as_ref().fmt_compact(f)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        indent: usize,
    ) -> std::fmt::Result {
        self.as_ref().fmt_indented(f, indent)
    }
}

// Implementation for PathCursor<P, S>
impl<P> Checkpointed<PathCursor<P, Matched>>
where
    P: Clone,
{
    /// Create a new checkpointed cursor from an initial matched position
    ///
    /// Starts with candidate=None (at checkpoint).
    pub(crate) fn new(initial: PathCursor<P, Matched>) -> Self {
        Self {
            checkpoint: initial,
            candidate: None,
        }
    }

    /// Convert current cursor to Candidate state (for next comparison)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self
    ) -> Checkpointed<PathCursor<P, Candidate>> {
        let current_cursor = match &self.candidate {
            None => &self.checkpoint,
            Some(c) => c,
        };
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: Some(CursorStateMachine::to_candidate(current_cursor)),
        }
    }
}

impl<P> Checkpointed<PathCursor<P, Candidate>>
where
    P: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the candidate cursor to Matched state and updates
    /// the checkpoint. Sets candidate=None since we're now at checkpoint.
    pub(crate) fn mark_match(self) -> Checkpointed<PathCursor<P, Matched>> {
        let matched = CursorStateMachine::to_matched(
            self.candidate.expect("Candidate cursor must exist"),
        );
        Checkpointed {
            checkpoint: matched,
            candidate: None,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the candidate cursor to Mismatched state without
    /// updating the checkpoint.
    pub(crate) fn mark_mismatch(
        self
    ) -> Checkpointed<PathCursor<P, Mismatched>> {
        Checkpointed {
            checkpoint: self.checkpoint,
            candidate: Some(CursorStateMachine::to_mismatched(
                self.candidate.expect("Candidate cursor must exist"),
            )),
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
    /// Starts with candidate=None (at checkpoint).
    pub(crate) fn new(initial: ChildCursor<Matched, EndNode>) -> Self {
        Self {
            checkpoint: initial,
            candidate: None,
        }
    }

    /// Convert current cursor to Candidate state (for next comparison)
    ///
    /// The checkpoint remains unchanged at the last matched position.
    pub(crate) fn as_candidate(
        &self
    ) -> Checkpointed<ChildCursor<Candidate, EndNode>> {
        let current_cursor = match &self.candidate {
            None => &self.checkpoint,
            Some(c) => c,
        };
        Checkpointed {
            checkpoint: self.checkpoint.clone(),
            candidate: Some(CursorStateMachine::to_candidate(current_cursor)),
        }
    }
}

impl<EndNode: PathNode> Checkpointed<ChildCursor<Candidate, EndNode>>
where
    EndNode: Clone,
{
    /// Mark the current position as matched, updating the checkpoint
    ///
    /// This transitions the candidate cursor to Matched state and updates
    /// the checkpoint. Sets candidate=None since we're now at checkpoint.
    pub(crate) fn mark_match(
        self
    ) -> Checkpointed<ChildCursor<Matched, EndNode>> {
        let matched = CursorStateMachine::to_matched(
            self.candidate.expect("Candidate cursor must exist"),
        );
        Checkpointed {
            checkpoint: matched,
            candidate: None,
        }
    }

    /// Mark the current position as mismatched, keeping the checkpoint
    ///
    /// This transitions the candidate cursor to Mismatched state without
    /// updating the checkpoint.
    pub(crate) fn mark_mismatch(
        self
    ) -> Checkpointed<ChildCursor<Mismatched, EndNode>> {
        Checkpointed {
            checkpoint: self.checkpoint,
            candidate: Some(CursorStateMachine::to_mismatched(
                self.candidate.expect("Candidate cursor must exist"),
            )),
        }
    }
}

// Implement CompactFormat for Checkpointed
impl<T: CompactFormat + HasCheckpoint> CompactFormat for Checkpointed<T>
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

impl<T: CompactFormat + HasCheckpoint> std::fmt::Display for Checkpointed<T>
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

impl<P> StateAdvance for Checkpointed<PathCursor<P, Matched>>
where
    P: Clone,
    PathCursor<P, Candidate>: Advance,
{
    type Next = Self;

    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        // Only advance from checkpoint state
        debug_assert!(
            self.candidate.is_none(),
            "advance_state should only be called when at checkpoint"
        );

        // Convert checkpoint to candidate and advance
        let mut candidate = CursorStateMachine::to_candidate(&self.checkpoint);
        match candidate.advance(trav) {
            ControlFlow::Continue(()) => {
                // Successfully advanced - convert to matched and return with new candidate
                use super::MarkMatchState;
                Ok(Checkpointed {
                    checkpoint: self.checkpoint,
                    candidate: Some(candidate.mark_match()),
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
    for Checkpointed<ChildCursor<Matched, EndNode>>
where
    EndNode: Clone,
    context_trace::ChildState<EndNode>:
        StateAdvance<Next = context_trace::ChildState<EndNode>>,
{
    type Next = Self;

    fn advance_state<G: HasGraph>(
        self,
        trav: &G,
    ) -> Result<Self::Next, Self> {
        // Only advance from checkpoint state
        debug_assert!(
            self.candidate.is_none(),
            "advance_state should only be called when at checkpoint"
        );

        // Advance checkpoint's child_state
        let child_state = self.checkpoint.child_state.clone();
        match child_state.advance_state(trav) {
            Ok(advanced_state) => Ok(Checkpointed {
                checkpoint: self.checkpoint,
                candidate: Some(ChildCursor {
                    child_state: advanced_state,
                    _state: std::marker::PhantomData,
                }),
            }),
            Err(_failed_state) => {
                // Cannot advance from checkpoint
                Err(self)
            },
        }
    }
}
