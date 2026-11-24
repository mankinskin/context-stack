//! Errors of positioned search (plan 2.0, Decision 7.1).
//!
//! Same meanings as the corresponding legacy `ErrorReason` variants; no
//! parent match is not an error but an `EntireRoot` response for the first
//! query token, as in legacy.

use context_trace::{
    CommittedIndex,
    Token,
    VertexId,
};

/// Why a positioned search could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError<I: VertexId = CommittedIndex> {
    /// The query is empty (legacy `ErrorReason::EmptyPatterns`).
    EmptyPatterns,
    /// The query is the single token `0`: nothing to search for beyond it
    /// (legacy `ErrorReason::SingleIndex`).
    SingleIndex(Token<I>),
    /// A query atom has no atom vertex (legacy `ErrorReason::UnknownAtom`).
    UnknownAtom,
}

impl<I: VertexId> std::fmt::Display for SearchError<I> {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::EmptyPatterns => f.write_str("empty query"),
            Self::SingleIndex(token) => match token.index.committed() {
                Some(index) =>
                    write!(f, "query is the single token {}", index.0),
                None => write!(f, "query is the single token {}", token.index),
            },
            Self::UnknownAtom => f.write_str("query atom has no vertex"),
        }
    }
}

impl<I: VertexId> std::error::Error for SearchError<I> {}
