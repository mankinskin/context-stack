use context_search::Response;
use context_trace::*;

/// Outcome of `insert_next_match` — always resolves to a single token + path.
///
/// Each variant carries the matched/created [`IndexWithPath`] and the search
/// [`Response`] for caching, debugging, and downstream visibility.
///
/// # Variants
///
/// - **`Created`** — A new token was inserted via the split+join pipeline.
///   The query extended beyond what was known in the graph.
/// - **`Complete`** — The query was fully consumed by an existing token.
///   No insertion was needed (idempotent match).
/// - **`NoExpansion`** — The search found an existing token at the start of
///   the query, but the query extends beyond it. No new token was created.
///   The caller should advance by the returned token's width and retry.
#[derive(Debug, Clone)]
pub enum InsertOutcome {
    /// Newly created via split+join pipeline.
    /// The query extended beyond what was known; a new token was inserted.
    Created {
        result: IndexWithPath,
        response: Response,
    },

    /// Full match already existed — the query was fully consumed by an
    /// existing token. No insertion was needed.
    Complete {
        result: IndexWithPath,
        response: Response,
    },

    /// No expansion: the search found an existing token at the start of
    /// the query, but the query extends beyond it. No new token was created.
    /// The caller should advance by the returned token's width and try again.
    NoExpansion {
        result: IndexWithPath,
        response: Response,
    },
}

impl InsertOutcome {
    /// The matched/created token + cursor path (all variants carry this).
    pub fn result(&self) -> &IndexWithPath {
        match self {
            InsertOutcome::Created { result, .. } => result,
            InsertOutcome::Complete { result, .. } => result,
            InsertOutcome::NoExpansion { result, .. } => result,
        }
    }

    /// Consume into the [`IndexWithPath`].
    pub fn into_result(self) -> IndexWithPath {
        match self {
            InsertOutcome::Created { result, .. } => result,
            InsertOutcome::Complete { result, .. } => result,
            InsertOutcome::NoExpansion { result, .. } => result,
        }
    }

    /// The token (shorthand for `result().index`).
    pub fn token(&self) -> Token {
        self.result().index
    }

    /// The search response (for caching, debugging, trace inspection).
    pub fn response(&self) -> &Response {
        match self {
            InsertOutcome::Created { response, .. } => response,
            InsertOutcome::Complete { response, .. } => response,
            InsertOutcome::NoExpansion { response, .. } => response,
        }
    }

    /// Whether a new token was created (split+join pipeline ran).
    pub fn is_expanded(&self) -> bool {
        matches!(self, InsertOutcome::Created { .. })
    }

    /// Whether the query was fully consumed by an existing token.
    pub fn is_complete(&self) -> bool {
        matches!(self, InsertOutcome::Complete { .. })
    }

    /// Whether no expansion occurred (starting token is the best match,
    /// but query extends beyond it).
    pub fn is_no_expansion(&self) -> bool {
        matches!(self, InsertOutcome::NoExpansion { .. })
    }
}
