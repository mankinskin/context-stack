//! Errors of positioned search (plan 2.0, Decision 7.1).
//!
//! Same meanings as the corresponding legacy `ErrorReason` variants; no
//! parent match is not an error but an `EntireRoot` response for the first
//! query token, as in legacy.

use context_trace::Token;

/// Why a positioned search could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    /// The query is empty (legacy `ErrorReason::EmptyPatterns`).
    EmptyPatterns,
    /// The query is the single token `0`: nothing to search for beyond it
    /// (legacy `ErrorReason::SingleIndex`).
    SingleIndex(Token),
    /// A query atom has no atom vertex (legacy `ErrorReason::UnknownAtom`).
    UnknownAtom,
}

impl std::fmt::Display for SearchError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::EmptyPatterns => f.write_str("empty query"),
            Self::SingleIndex(token) => {
                write!(f, "query is the single token {}", token.index.0)
            },
            Self::UnknownAtom => f.write_str("query atom has no vertex"),
        }
    }
}

impl std::error::Error for SearchError {}
