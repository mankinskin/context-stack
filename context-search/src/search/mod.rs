//! Positioned ancestor search (legacy `search/mod.rs`, without
//! visualization events).
//!
//! [`Find::find_ancestor`] turns a query into a [`Response`]: the smallest
//! root containing the longest matched query prefix, with canonical paths,
//! the trace cache of that match and the checkpointed query cursor.

pub mod context;
pub mod searchable;
pub mod stats;

use context::AncestorSearchTraversal;
use context_trace::{
    positioned::{
        PositionedGraph,
        PositionedGraphRef,
        TraceCache,
        TraceCtx,
        Traceable,
    },
    GraphKind,
};
use searchable::Searchable;

use crate::{
    compare::state::MatchedCompareState,
    cursor::{
        Checkpointed,
        Matched,
        QueryCursor,
    },
    policy::SearchKind,
    r#match::{
        iterator::SearchIterator,
        root_cursor::{
            ConclusiveEnd,
            RootAdvanceResult,
            RootCursor,
            RootEndResult,
        },
        SearchNode,
    },
    state::{
        matched::{
            CheckpointedCursor,
            MatchResult,
            PathCoverage,
        },
        response::Response,
    },
    SearchError,
};

pub type SearchResult = Result<Response, SearchError>;

pub trait Find {
    type Kind: GraphKind;
    /// Search the smallest ancestor containing the longest prefix of the
    /// query (legacy `find_ancestor`). One read guard for the whole search.
    fn find_ancestor(
        &self,
        searchable: impl Searchable<Self::Kind, AncestorSearchTraversal>,
    ) -> SearchResult;
}

impl<G: GraphKind> Find for PositionedGraph<G> {
    type Kind = G;
    fn find_ancestor(
        &self,
        searchable: impl Searchable<G, AncestorSearchTraversal>,
    ) -> SearchResult {
        searchable.search(self)
    }
}

impl<G: GraphKind> Find for PositionedGraphRef<G> {
    type Kind = G;
    fn find_ancestor(
        &self,
        searchable: impl Searchable<G, AncestorSearchTraversal>,
    ) -> SearchResult {
        let graph = self.read();
        searchable.search(&graph)
    }
}

/// Outer search loop: yields one [`MatchResult`] per matched root; the last
/// one is the result.
#[derive(Debug)]
pub struct SearchState<'g, G: GraphKind, K: SearchKind> {
    pub(crate) matches: SearchIterator<'g, G, K>,
    /// The query cursor at the first query token.
    pub(crate) query: QueryCursor<Matched>,
}

impl<'g, G: GraphKind, K: SearchKind> SearchState<'g, G, K> {
    pub(crate) fn new(
        matches: SearchIterator<'g, G, K>,
        query: QueryCursor<Matched>,
    ) -> Self {
        Self { matches, query }
    }
    fn graph(&self) -> &'g PositionedGraph<G> {
        self.matches.graph()
    }

    /// Run to the end and trace the final match into the cache.
    pub fn search(mut self) -> Response {
        let mut best_match = None;
        for item in self.by_ref() {
            best_match = Some(item);
        }
        let graph = self.graph();
        let end = best_match.unwrap_or_else(|| {
            // No parent match: the first query token alone.
            let cursor = self.query.clone();
            MatchResult::new(
                PathCoverage::entire_token(graph, cursor.path().leaf(), cursor.path().leaf_offset()),
                CheckpointedCursor::AtCheckpoint(Checkpointed::new(cursor)),
            )
        });
        let cache = std::mem::replace(&mut self.matches.trace_ctx.cache, TraceCache::default());
        let mut ctx = TraceCtx::new(graph, cache);
        (&end).trace(&mut ctx);
        let response = Response {
            cache: ctx.cache,
            end,
        };
        stats::record_path_lens(
            response.path().start_path().map_or(0, |p| p.len()),
            response.path().end_path().map_or(0, |p| p.len()),
        );
        response
    }

    /// Advance in the matched root until the query, the root or the match
    /// ends. An exhausted root queues its parents (the queue was cleared
    /// when the root matched) and yields the match with the query candidate
    /// advanced.
    fn finish_root_cursor(
        &mut self,
        init_state: MatchedCompareState,
    ) -> MatchResult {
        let graph = self.graph();
        let mut last_match = init_state;
        loop {
            last_match.update_checkpoint();
            let root_cursor = RootCursor::<G, K>::new(graph, last_match.clone());
            match root_cursor.advance_to_next_match() {
                RootAdvanceResult::Advanced(next) => last_match = next.state,
                RootAdvanceResult::Finished(RootEndResult::Conclusive(
                    ConclusiveEnd::Mismatch | ConclusiveEnd::Exhausted,
                )) => {
                    return self.create_result_from_state(last_match);
                },
                RootAdvanceResult::Finished(RootEndResult::Inconclusive(need_parent)) => {
                    let result = need_parent.create_parent_exploration_state();
                    if let Some(batch) = need_parent.get_parent_batch() {
                        self.matches.queue.extend(
                            batch
                                .into_compare_batch()
                                .into_iter()
                                .map(SearchNode::ParentCandidate),
                        );
                    }
                    return result;
                },
            }
        }
    }

    /// The checkpoint of a matched state as a result.
    pub(crate) fn create_result_from_state(
        &self,
        state: MatchedCompareState,
    ) -> MatchResult {
        let checkpoint = state.query.checkpoint().clone();
        let end_pos = checkpoint.atom_position();
        MatchResult::new(
            PathCoverage::from_child_state(
                self.graph(),
                state.child.checkpoint().child_state.clone(),
                end_pos,
            ),
            CheckpointedCursor::AtCheckpoint(Checkpointed::new(checkpoint)),
        )
    }
}

impl<G: GraphKind, K: SearchKind> Iterator for SearchState<'_, G, K> {
    type Item = MatchResult;
    /// The next matched root: the first node to match clears the queue
    /// (first match wins), then the root cursor runs.
    fn next(&mut self) -> Option<Self::Item> {
        let matched = self.matches.find_next_root_match()?;
        self.matches.queue.clear();
        Some(self.finish_root_cursor(matched))
    }
}
