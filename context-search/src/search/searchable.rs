//! Queries a search can start from (legacy `search/searchable.rs`).
//!
//! Token sequences may contain composite tokens (Decision 6 b); the query
//! side navigates them natively. A query cursor (from a previous
//! [`Response`]) continues right after its checkpoint, without re-reading
//! earlier query positions.

use context_trace::{
    positioned::{
        PositionedGraph,
        QueryPath,
    },
    GraphKind,
    Token,
};

use crate::{
    cursor::{
        Matched,
        QueryCursor,
    },
    policy::SearchKind,
    search::{
        context::AncestorSearchTraversal,
        stats,
        SearchState,
    },
    state::{
        matched::{
            CheckpointedCursor,
            MatchResult,
        },
        response::Response,
        start::StartCtx,
    },
    SearchError,
};

pub trait Searchable<G: GraphKind, K: SearchKind = AncestorSearchTraversal>: Sized {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError>;

    fn search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<Response, SearchError> {
        stats::reset();
        Ok(self.start_search(graph)?.search())
    }
}

/// Atoms through the atom index; an atom without a vertex is
/// `UnknownAtom`, an empty query `EmptyPatterns`.
impl<G: GraphKind<Atom = char>, K: SearchKind> Searchable<G, K> for std::str::Chars<'_> {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        let pattern = self
            .map(|atom| graph.atom_token(&atom).ok_or(SearchError::UnknownAtom))
            .collect::<Result<Vec<Token>, _>>()?;
        pattern.start_search(graph)
    }
}

impl<G: GraphKind, K: SearchKind> Searchable<G, K> for Vec<Token> {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        let path = QueryPath::new(self).ok_or(SearchError::EmptyPatterns)?;
        StartCtx::new(graph, QueryCursor::at(path)).into_search()
    }
}

impl<G: GraphKind, K: SearchKind> Searchable<G, K> for &[Token] {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.to_vec().start_search(graph)
    }
}

impl<const N: usize, G: GraphKind, K: SearchKind> Searchable<G, K> for [Token; N] {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.to_vec().start_search(graph)
    }
}

/// Continuation: the query token starting at the checkpoint's end (inside
/// a composite query token if the checkpoint ends inside one) becomes the
/// first query token. `EmptyPatterns` if the checkpoint is at the query
/// end.
impl<G: GraphKind, K: SearchKind> Searchable<G, K> for QueryCursor<Matched> {
    fn start_search(
        mut self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        if self.advance(graph).is_break() {
            return Err(SearchError::EmptyPatterns);
        }
        StartCtx::new(graph, self).into_search()
    }
}

impl<G: GraphKind, K: SearchKind> Searchable<G, K> for &QueryCursor<Matched> {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.clone().start_search(graph)
    }
}

/// Continues at the checkpoint (the candidate is not needed).
impl<G: GraphKind, K: SearchKind> Searchable<G, K> for &CheckpointedCursor {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.checkpoint().start_search(graph)
    }
}

impl<G: GraphKind, K: SearchKind> Searchable<G, K> for &MatchResult {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.checkpoint().start_search(graph)
    }
}

impl<G: GraphKind, K: SearchKind> Searchable<G, K> for &Response {
    fn start_search(
        self,
        graph: &PositionedGraph<G>,
    ) -> Result<SearchState<'_, G, K>, SearchError> {
        self.query_cursor().start_search(graph)
    }
}
