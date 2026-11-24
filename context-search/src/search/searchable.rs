//! Queries a search can start from (legacy `search/searchable.rs`).
//!
//! Token sequences may contain composite tokens (Decision 6 b); the query
//! side navigates them natively. A query cursor (from a previous
//! [`Response`]) continues right after its checkpoint, without re-reading
//! earlier query positions.

use context_trace::{
    positioned::{
        GraphRead,
        QueryPath,
    },
    CommittedIndex,
    GraphKind,
    ToToken,
    Token,
    VertexId,
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

/// A query of a search on a store of kind `G` with indices `I`.
pub trait Searchable<
    G: GraphKind,
    I: VertexId = CommittedIndex,
    K: SearchKind = AncestorSearchTraversal,
>: Sized
{
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>>;

    fn search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<Response<I>, SearchError<I>> {
        stats::reset();
        Ok(self.start_search(graph)?.search())
    }
}

/// Atoms through the atom index; an atom without a vertex is
/// `UnknownAtom`, an empty query `EmptyPatterns`.
impl<G: GraphKind<Atom = char>, I: VertexId, K: SearchKind> Searchable<G, I, K>
    for std::str::Chars<'_>
{
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        let pattern = self
            .map(|atom| {
                graph
                    .atom(&atom)
                    .map(|vertex| vertex.to_token())
                    .ok_or(SearchError::UnknownAtom)
            })
            .collect::<Result<Vec<Token<I>>, _>>()?;
        pattern.start_search(graph)
    }
}

impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for Vec<Token<I>> {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        let path = QueryPath::new(self).ok_or(SearchError::EmptyPatterns)?;
        StartCtx::new(graph, QueryCursor::at(path)).into_search()
    }
}

impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for &[Token<I>] {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.to_vec().start_search(graph)
    }
}

impl<const N: usize, G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K>
    for [Token<I>; N]
{
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.to_vec().start_search(graph)
    }
}

/// Continuation: the query token starting at the checkpoint's end (inside
/// a composite query token if the checkpoint ends inside one) becomes the
/// first query token. `EmptyPatterns` if the checkpoint is at the query
/// end.
impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for QueryCursor<Matched, I> {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        mut self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        if self.advance(graph).is_break() {
            return Err(SearchError::EmptyPatterns);
        }
        StartCtx::new(graph, self).into_search()
    }
}

impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K>
    for &QueryCursor<Matched, I>
{
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.clone().start_search(graph)
    }
}

/// Continues at the checkpoint (the candidate is not needed).
impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for &CheckpointedCursor<I> {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.checkpoint().start_search(graph)
    }
}

impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for &MatchResult<I> {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.checkpoint().start_search(graph)
    }
}

impl<G: GraphKind, I: VertexId, K: SearchKind> Searchable<G, I, K> for &Response<I> {
    fn start_search<S: GraphRead<Kind = G, Index = I> + ?Sized>(
        self,
        graph: &S,
    ) -> Result<SearchState<'_, S, K>, SearchError<I>> {
        self.query_cursor().start_search(graph)
    }
}
