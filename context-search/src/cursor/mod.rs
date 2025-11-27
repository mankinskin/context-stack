use context_trace::*;
use std::marker::PhantomData;

pub(crate) mod checkpointed;
pub(crate) mod path;
pub(crate) mod position;
pub(crate) mod state_machine;

pub(crate) use checkpointed::{AtCheckpoint, Checkpointed, HasCandidate, HasCheckpoint};
pub(crate) use state_machine::CursorStateMachine;

//pub trait CursorPath: GraphRoot {}
//impl<T: GraphRoot> CursorPath for T {}

// State marker types for PathCursor
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

///// Exhausted state: cursor has reached the end of the pattern
//#[derive(Debug, Clone, Copy, PartialEq, Eq)]
//pub struct Exhausted;
//impl sealed::Sealed for Exhausted {}
//impl CursorState for Exhausted {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathCursor<P, State = Matched> {
    pub(crate) path: P,
    pub(crate) atom_position: AtomPosition,
    pub(crate) _state: PhantomData<State>,
}
impl<P: GraphRoot, State: CursorState> GraphRoot for PathCursor<P, State> {
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
pub(crate) type PatternCursor<State = Matched> =
    PathCursor<PatternRangePath, State>;
//pub(crate) type IndexCursor = PathCursor<IndexRangePath>;

pub(crate) type PatternPrefixCursor = PathCursor<PatternPrefixPath>;

/// Cursor wrapper for ChildState that supports CursorState markers
/// This allows tracking the state (Matched/Candidate/Mismatched) of the index path
/// without duplicating the path information that ChildState already contains.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChildCursor<
    State = Matched,
    EndNode: PathNode = ChildLocation,
> {
    pub(crate) child_state: ChildState<EndNode>,
    pub(crate) _state: PhantomData<State>,
}

impl From<PatternPrefixCursor> for PatternCursor {
    fn from(value: PatternPrefixCursor) -> Self {
        Self {
            path: value.path.into(),
            atom_position: value.atom_position,
            _state: PhantomData,
        }
    }
}
impl From<PatternCursor> for PatternPrefixCursor {
    fn from(value: PatternCursor) -> Self {
        Self {
            path: value.path.into(),
            atom_position: value.atom_position,
            _state: PhantomData,
        }
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

// State transition methods
impl<P> PathCursor<P, Matched> {
    /// Convert a Matched cursor to a Candidate by creating a copy
    /// This preserves the matched position for potential revert
    pub(crate) fn as_candidate(&self) -> PathCursor<P, Candidate>
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

//impl<P> PathCursor<P, Mismatched> {
//    /// Convert a mismatched cursor to matched (for final states)
//    pub(crate) fn as_matched(self) -> PathCursor<P, Matched> {
//        PathCursor {
//            path: self.path,
//            atom_position: self.atom_position,
//            _state: PhantomData,
//        }
//    }
//}

// ChildCursor state transitions
impl<EndNode: PathNode> ChildCursor<Matched, EndNode> {
    /// Convert a Matched cursor to a Candidate by creating a copy
    pub(crate) fn as_candidate(&self) -> ChildCursor<Candidate, EndNode> {
        ChildCursor {
            child_state: self.child_state.clone(),
            _state: PhantomData,
        }
    }
}

impl<EndNode: PathNode> MarkMatchState for ChildCursor<Candidate, EndNode> {
    type Matched = ChildCursor<Matched, EndNode>;
    type Mismatched = ChildCursor<Mismatched, EndNode>;

    fn mark_match(self) -> Self::Matched {
        ChildCursor {
            child_state: self.child_state,
            _state: PhantomData,
        }
    }

    fn mark_mismatch(self) -> Self::Mismatched {
        ChildCursor {
            child_state: self.child_state,
            _state: PhantomData,
        }
    }
}

// ============================================================================
// CursorStateMachine implementations
// ============================================================================

// Implementation for PathCursor<P, Matched>
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

// Implementation for PathCursor<P, Candidate>
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

// Implementation for ChildCursor<Matched, EndNode>
impl<EndNode: PathNode> CursorStateMachine for ChildCursor<Matched, EndNode> {
    type AsCandidate = ChildCursor<Candidate, EndNode>;
    type AsMatched = Self;
    type AsMismatched = ChildCursor<Mismatched, EndNode>;

    fn to_candidate(&self) -> Self::AsCandidate {
        self.as_candidate()
    }

    fn to_matched(self) -> Self::AsMatched {
        self // Already matched
    }

    fn to_mismatched(self) -> Self::AsMismatched {
        ChildCursor {
            child_state: self.child_state,
            _state: PhantomData,
        }
    }
}

// Implementation for ChildCursor<Candidate, EndNode>
impl<EndNode: PathNode + Clone> CursorStateMachine
    for ChildCursor<Candidate, EndNode>
{
    type AsCandidate = Self;
    type AsMatched = ChildCursor<Matched, EndNode>;
    type AsMismatched = ChildCursor<Mismatched, EndNode>;

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

// Display implementation for PathCursor
// Uses CompactFormat if available, otherwise falls back to Debug
impl<P, State> std::fmt::Display for PathCursor<P, State>
where
    P: context_trace::logging::compact_format::CompactFormat,
    State: CursorState,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        use context_trace::logging::compact_format::CompactFormat;
        self.fmt_indented(f, 0)
    }
}
