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
    CommittedIndex,
    Token,
    VertexId,
    positioned::{
        GraphEdit,
        GraphRead,
        HasGraphMut,
        KindOf,
        PositionedError,
        Publication,
    },
};

use crate::{
    join::join,
    split::SplitGraph,
};

/// Result of [`insert_match`] (legacy outcome names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertOutcome<I: VertexId = CommittedIndex> {
    /// `EntireRoot` and the query is exhausted: `N` is the existing root.
    Complete { token: Token<I>, response: Response<I> },
    /// `EntireRoot` with query remaining: the caller continues after `token`.
    NoExpansion { token: Token<I>, response: Response<I> },
    /// The span became a new token.
    Created {
        token: Token<I>,
        response: Response<I>,
        publication: Publication<I>,
    },
}

impl<I: VertexId> InsertOutcome<I> {
    /// The token of the matched span.
    pub fn token(&self) -> Token<I> {
        match self {
            Self::Complete { token, .. }
            | Self::NoExpansion { token, .. }
            | Self::Created { token, .. } => *token,
        }
    }
    pub fn response(&self) -> &Response<I> {
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
///
/// `graph` is a write handle taken by value (`&PositionedGraphRef`, `&mut`
/// view or handle): the write guard is held for the whole insertion.
pub fn insert_match<G: HasGraphMut>(
    mut graph: G,
    response: Response<G::Index>,
) -> Result<InsertOutcome<G::Index>, InsertError<G::Index>> {
    let mut graph = graph.graph_mut();
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
    let mut edit = GraphEdit::new(&mut graph);
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
pub fn insert_next_match<G: HasGraphMut>(
    graph: G,
    query: impl Searchable<KindOf<G>, G::Index>,
) -> Result<InsertOutcome<G::Index>, InsertError<G::Index>> {
    let response = graph.find_ancestor(query)?;
    insert_match(graph, response)
}

/// [`insert_next_match`], returning only the token of the match.
pub fn insert<G: HasGraphMut>(
    graph: G,
    query: impl Searchable<KindOf<G>, G::Index>,
) -> Result<Token<G::Index>, InsertError<G::Index>> {
    insert_next_match(graph, query).map(|outcome| outcome.token())
}

/// Why `insert_match` failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertError<I: VertexId = CommittedIndex> {
    /// The response was found under an older graph generation.
    StaleResponse,
    /// A graph edit was rejected: an invariant was violated, i.e. a bug.
    Composition(PositionedError<I>),
    /// The search of a wrapper ([`insert`], [`insert_next_match`]) failed.
    Search(SearchError<I>),
}

impl<I: VertexId> fmt::Display for InsertError<I> {
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

impl<I: VertexId> std::error::Error for InsertError<I> {}

impl<I: VertexId> From<PositionedError<I>> for InsertError<I> {
    fn from(error: PositionedError<I>) -> Self {
        Self::Composition(error)
    }
}

impl<I: VertexId> From<SearchError<I>> for InsertError<I> {
    fn from(error: SearchError<I>) -> Self {
        Self::Search(error)
    }
}
