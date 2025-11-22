//! Unified cursor state machine trait
//!
//! Provides a centralized state transition API for all cursor types,
//! eliminating code duplication between PathCursor, ChildCursor, and their
//! Checkpointed wrappers.

/// Unified state machine trait for all cursor types
///
/// This trait centralizes state transition logic that was previously
/// duplicated across PathCursor, ChildCursor, and Checkpointed variants.
///
/// # State Transitions
/// - `Matched` → `to_candidate()` → `Candidate` (non-consuming, creates copy)
/// - `Candidate` → `to_matched()` → `Matched` (consuming, confirms match)
/// - `Candidate` → `to_mismatched()` → `Mismatched` (consuming, marks failure)
/// - `Mismatched` → `to_candidate()` → `Candidate` (non-consuming, for retry)
///
/// # Type Parameters
/// The Self type determines which cursor type is being transitioned.
/// Associated types specify the result types for each transition.
pub trait CursorStateMachine: Sized {
    /// Candidate state version of this cursor
    type AsCandidate;

    /// Matched state version of this cursor
    type AsMatched;

    /// Mismatched state version of this cursor
    type AsMismatched;

    /// Transition to candidate state (non-consuming, for comparison)
    ///
    /// Creates a copy in Candidate state while preserving the original.
    /// Used when we need to advance the cursor speculatively.
    fn to_candidate(&self) -> Self::AsCandidate;

    /// Transition to matched state (consuming, confirms match)
    ///
    /// Consumes self and returns a cursor in Matched state.
    /// Used when comparison succeeds and we want to commit the advance.
    fn to_matched(self) -> Self::AsMatched;

    /// Transition to mismatched state (consuming, marks failure)
    ///
    /// Consumes self and returns a cursor in Mismatched state.
    /// Used when comparison fails but we've scanned atoms.
    fn to_mismatched(self) -> Self::AsMismatched;
}
