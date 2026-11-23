//! Positioned `insert_match` (insert plan rev 3, §2, §7; task 3.4).
//!
//! `insert_match` makes the span of one search response a token, under one
//! write guard: generation check, split ([`SplitGraph`]), join ([`join`])
//! and publication through one `GraphEdit`. It never searches.

use std::fmt;

use context_search::{
    Find,
    Response,
    SearchError,
    Searchable,
};
use context_trace::{
    GraphKind,
    Token,
    positioned::{
        PositionedError,
        PositionedGraphRef,
        Publication,
    },
};

use crate::{
    join::join,
    split::SplitGraph,
};

/// Result of [`insert_match`] (legacy outcome names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    /// `EntireRoot` and the query is exhausted: `N` is the existing root.
    Complete { token: Token, response: Response },
    /// `EntireRoot` with query remaining: the caller continues after `token`.
    NoExpansion { token: Token, response: Response },
    /// The span became a new token.
    Created {
        token: Token,
        response: Response,
        publication: Publication,
    },
}

impl InsertOutcome {
    /// The token of the matched span.
    pub fn token(&self) -> Token {
        match self {
            Self::Complete { token, .. }
            | Self::NoExpansion { token, .. }
            | Self::Created { token, .. } => *token,
        }
    }
    pub fn response(&self) -> &Response {
        match self {
            Self::Complete { response, .. }
            | Self::NoExpansion { response, .. }
            | Self::Created { response, .. } => response,
        }
    }
}

/// Make the span of `response` a token (§2): afterwards the graph equals
/// the oracle graph of its corpus plus the span. `EntireRoot` responses
/// leave the graph unchanged.
pub fn insert_match<G: GraphKind>(
    graph: &PositionedGraphRef<G>,
    response: Response,
) -> Result<InsertOutcome, InsertError> {
    let mut graph = graph.write();
    if response.generation() != graph.generation() {
        return Err(InsertError::StaleResponse);
    }
    if response.is_entire_root() {
        let token = response.root_token();
        return Ok(if response.query_exhausted() {
            InsertOutcome::Complete { token, response }
        } else {
            InsertOutcome::NoExpansion { token, response }
        });
    }
    let split = SplitGraph::from_response(&graph, &response)
        .expect("a response that is not EntireRoot has a split graph");
    let mut edit = graph.edit();
    let token = match join(&mut edit, &split) {
        Ok(token) => token,
        Err(error) => {
            debug_assert!(false, "insert_match: rejected graph edit: {error}");
            return Err(InsertError::Composition(error));
        },
    };
    let publication = edit.finish();
    debug_assert_eq!(publication.created.last().map(|t| t.index), Some(token.index));
    Ok(InsertOutcome::Created {
        token,
        response,
        publication,
    })
}

/// Search `query` and insert the match ([`insert_match`]). Only the matched
/// part is inserted; the caller continues from the response's checkpoint.
pub fn insert_next_match<G: GraphKind>(
    graph: &PositionedGraphRef<G>,
    query: impl Searchable<G>,
) -> Result<InsertOutcome, InsertError> {
    let response = graph.find_ancestor(query)?;
    insert_match(graph, response)
}

/// [`insert_next_match`], returning only the token of the match.
pub fn insert<G: GraphKind>(
    graph: &PositionedGraphRef<G>,
    query: impl Searchable<G>,
) -> Result<Token, InsertError> {
    insert_next_match(graph, query).map(|outcome| outcome.token())
}

/// Why `insert_match` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertError {
    /// The response was found under an older graph generation.
    StaleResponse,
    /// A graph edit was rejected: an invariant was violated, i.e. a bug.
    Composition(PositionedError),
    /// The search of a wrapper ([`insert`], [`insert_next_match`]) failed.
    Search(SearchError),
}

impl fmt::Display for InsertError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::StaleResponse => f.write_str(
                "response is stale: the graph changed after the search",
            ),
            Self::Composition(error) =>
                write!(f, "rejected graph edit: {error}"),
            Self::Search(error) => write!(f, "search failed: {error}"),
        }
    }
}

impl std::error::Error for InsertError {}

impl From<PositionedError> for InsertError {
    fn from(error: PositionedError) -> Self {
        Self::Composition(error)
    }
}

impl From<SearchError> for InsertError {
    fn from(error: SearchError) -> Self {
        Self::Search(error)
    }
}
