use context_trace::legacy::*;
use std::marker::PhantomData;

pub(crate) mod checkpointed;
pub(crate) mod path;
pub(crate) mod position;
pub(crate) mod state_machine;

pub(crate) use checkpointed::{
    Checkpointed,
    HasCandidate,
};
pub(crate) use state_machine::CursorStateMachine;

// Row-independent typestates, promoted to `crate::cursor` (plan 2.0).
pub use crate::cursor::{
    Candidate,
    CursorState,
    MarkMatchState,
    Matched,
    Mismatched,
    PathCursor,
};

impl<P: GraphRoot, State: CursorState> GraphRoot for PathCursor<P, State> {
    fn root_parent(&self) -> Token {
        self.path.root_parent()
    }
}
pub type PatternCursor<State = Matched> = PathCursor<PatternRangePath, State>;
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
    P: context_trace::legacy::logging::compact_format::CompactFormat,
    State: CursorState,
{
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        use context_trace::legacy::logging::compact_format::CompactFormat;
        self.fmt_indented(f, 0)
    }
}
