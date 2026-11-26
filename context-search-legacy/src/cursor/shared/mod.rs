//! Frozen copy of the row-independent cursor typestates (formerly promoted
//! to `context_search::cursor`): state markers, the generic [`PathCursor`]
//! and [`Checkpointed`]. Row-specific impls (paths, `ChildCursor`,
//! `StateAdvance`) live in the parent module `crate::cursor`. Kept whole,
//! so parts the row model does not use are allowed to be dead.
#![allow(dead_code, unused_imports)]

use context_trace::AtomPosition;
use std::marker::PhantomData;

pub mod checkpointed;
pub mod state_machine;

pub use checkpointed::{
    AtCheckpoint,
    CandidateState,
    Checkpointed,
    HasCandidate,
    HasCheckpoint,
};
pub use state_machine::CursorStateMachine;

mod sealed {
    pub trait Sealed {}
}

/// Trait for cursor state markers
pub trait CursorState: sealed::Sealed {}

/// Matched state: cursor is at a confirmed matching position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Matched;
impl sealed::Sealed for Matched {}
impl CursorState for Matched {}

/// Candidate state: cursor has advanced to a position that needs comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate;
impl sealed::Sealed for Candidate {}
impl CursorState for Candidate {}

/// Mismatched state: cursor has scanned atoms but encountered a mismatch
/// Behaves like Matched in terms of atom_position (includes scanned atoms)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatched;
impl sealed::Sealed for Mismatched {}
impl CursorState for Mismatched {}

/// A path with the atom position reached on it, in state `State`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathCursor<P, State = Matched> {
    pub(crate) path: P,
    pub(crate) atom_position: AtomPosition,
    pub(crate) _state: PhantomData<State>,
}
impl<P, State> PathCursor<P, State> {
    pub fn path(&self) -> &P {
        &self.path
    }
}

impl<P> From<P> for PathCursor<P> {
    fn from(value: P) -> Self {
        Self {
            path: value,
            atom_position: 0.into(),
            _state: PhantomData,
        }
    }
}

impl<P> PathCursor<P, Matched> {
    /// Convert a Matched cursor to a Candidate by creating a copy
    /// This preserves the matched position for potential revert
    pub fn as_candidate(&self) -> PathCursor<P, Candidate>
    where
        P: Clone,
    {
        PathCursor {
            path: self.path.clone(),
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }
}

/// Trait for marking cursor/state as matched or mismatched
pub trait MarkMatchState {
    type Matched;
    type Mismatched;

    /// Mark as successfully matched
    fn mark_match(self) -> Self::Matched;

    /// Mark as mismatched/failed
    fn mark_mismatch(self) -> Self::Mismatched;
}

impl<P> MarkMatchState for PathCursor<P, Candidate> {
    type Matched = PathCursor<P, Matched>;
    type Mismatched = PathCursor<P, Mismatched>;

    fn mark_match(self) -> Self::Matched {
        PathCursor {
            path: self.path,
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }

    fn mark_mismatch(self) -> Self::Mismatched {
        PathCursor {
            path: self.path,
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }
}

impl<P: Clone> CursorStateMachine for PathCursor<P, Matched> {
    type AsCandidate = PathCursor<P, Candidate>;
    type AsMatched = Self;
    type AsMismatched = PathCursor<P, Mismatched>;

    fn to_candidate(&self) -> Self::AsCandidate {
        self.as_candidate()
    }

    fn to_matched(self) -> Self::AsMatched {
        self // Already matched
    }

    fn to_mismatched(self) -> Self::AsMismatched {
        PathCursor {
            path: self.path,
            atom_position: self.atom_position,
            _state: PhantomData,
        }
    }
}

impl<P: Clone> CursorStateMachine for PathCursor<P, Candidate> {
    type AsCandidate = Self;
    type AsMatched = PathCursor<P, Matched>;
    type AsMismatched = PathCursor<P, Mismatched>;

    fn to_candidate(&self) -> Self::AsCandidate {
        self.clone() // Already candidate
    }

    fn to_matched(self) -> Self::AsMatched {
        self.mark_match()
    }

    fn to_mismatched(self) -> Self::AsMismatched {
        self.mark_mismatch()
    }
}
