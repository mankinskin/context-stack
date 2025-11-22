use crate::{
    cursor::PatternCursor,
    r#match::iterator::SearchIterator,
    state::{
        end::PathCoverage,
        matched::MatchResult,
        start::Searchable,
    },
    traversal::TraceStart,
    Response,
    SearchKind,
};
use context::{
    AncestorSearchTraversal,
    ParentSearchTraversal,
    SearchCtx,
};
use context_trace::{
    logging::format_utils::pretty,
    *,
};
use tracing::{
    debug,
    info,
    trace,
};
pub(crate) mod context;
pub(crate) mod ext;
// pub(crate) mod final_state; // Unused - references old EndState type
pub(crate) mod searchable;

pub(crate) type SearchResult = Result<Response, ErrorReason>;
#[allow(dead_code)]
pub trait Find: HasGraph {
    fn ctx(&self) -> SearchCtx<Self>;

    #[context_trace::instrument_sig(
        level = "info",
        skip(self, pattern),
        fields(pattern_len)
    )]
    fn find_sequence(
        &self,
        pattern: impl IntoIterator<Item = impl AsAtom<AtomOf<TravKind<Self>>>>,
    ) -> SearchResult
    where
        Self: Clone,
    {
        let iter = atomizing_iter(pattern.into_iter());
        let atoms: Vec<_> = iter.collect();
        tracing::Span::current().record("pattern_len", atoms.len());
        debug!(pattern_len = atoms.len(), "finding sequence pattern");
        trace!(atoms = %pretty(&atoms), "pattern atoms");

        let pattern = self.graph().get_atom_children(atoms.into_iter())?;
        debug!(pattern = %pretty(&pattern), "created pattern token");

        self.find_ancestor(pattern)
    }

    /// find largest matching parent for pattern
    #[context_trace::instrument_sig(level = "info", skip(self, searchable))]
    fn find_parent(
        &self,
        searchable: impl Searchable,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting parent search");
        let result = searchable
            .search::<ParentSearchTraversal<Self>>(self.ctx())
            .map_err(|err| err.reason);

        match &result {
            Ok(_response) => debug!("parent search succeeded"),
            Err(reason) =>
                debug!(reason = %pretty(reason), "parent search failed"),
        }
        result
    }

    /// find largest matching ancestor for pattern
    #[context_trace::instrument_sig(level = "info", skip(self, searchable))]
    fn find_ancestor(
        &self,
        searchable: impl Searchable,
    ) -> SearchResult
    where
        Self: Clone,
    {
        debug!("starting ancestor search");
        let result = searchable
            .search::<AncestorSearchTraversal<Self>>(self.ctx())
            .map_err(|err| err.reason);

        match &result {
            Ok(_response) => debug!("ancestor search succeeded"),
            Err(reason) =>
                debug!(reason = %pretty(reason), "ancestor search failed"),
        }
        result
    }
}

impl Find for &Hypergraph {
    fn ctx(&self) -> SearchCtx<Self> {
        SearchCtx::new(self)
    }
}

impl Find for HypergraphRef {
    fn ctx(&self) -> SearchCtx<Self> {
        SearchCtx::new(self.clone())
    }
}

/// context for running fold traversal
#[derive(Debug)]
pub struct SearchState<K: SearchKind> {
    pub(crate) matches: SearchIterator<K>,
    /// The query pattern we're searching for
    pub(crate) query: PatternRangePath,
}

//impl<K: SearchKind> SearchState<K> {
//    /// Extract parent batch from a matched state for queue repopulation
//    /// Converts matched root to parent nodes for continued exploration
//    fn extract_parent_batch(
//        &self,
//        matched_state: &MatchResult,
//    ) -> Option<Vec<crate::r#match::SearchNode>> {
//        use crate::{
//            compare::parent::ParentCompareState,
//            r#match::SearchNode,
//        };
//
//        // Extract IndexRangePath and cursor from matched state
//        let (index_path, cursor) = matched_state.to_parent_state()?;
//
//        debug!(
//            "Extracting parents for matched root: {}",
//            index_path.root_parent()
//        );
//
//        // Create ChildState from IndexRangePath
//        // ChildState wraps IndexRangePath with a current_pos for traversal
//        let child_state = ChildState {
//            current_pos: cursor.atom_position,
//            path: index_path,
//        };
//
//        // Get ParentState from ChildState
//        let parent_state = child_state.parent_state();
//
//        // Use traversal policy to get parent batch
//        let batch =
//            K::Policy::next_batch(&self.matches.trace_ctx.trav, &parent_state)?;
//
//        // Convert to SearchNode::ParentCandidate
//        let parent_nodes: Vec<SearchNode> = batch
//            .parents
//            .into_iter()
//            .map(|ps| {
//                SearchNode::ParentCandidate(ParentCompareState {
//                    parent_state: ps,
//                    cursor: cursor.clone(),
//                })
//            })
//            .collect();
//
//        debug!(
//            "Found {} parents for continued exploration",
//            parent_nodes.len()
//        );
//        Some(parent_nodes)
//    }
//}

impl<K: SearchKind> Iterator for SearchState<K>
where
    K::Trav: Clone,
{
    type Item = MatchResult;
    fn next(&mut self) -> Option<Self::Item> {
        trace!("searching for next match");
        match self.matches.find_next() {
            Some(matched_state) => {
                debug!(
                    is_complete = matched_state.query_exhausted(),
                    "found matched state"
                );

                // Update best_match if this match is better
                let checkpoint_pos =
                    *matched_state.cursor().atom_position.as_ref();

                let should_update = match &self.matches.best_match {
                    None => {
                        debug!(
                            root = %matched_state.root_parent(),
                            checkpoint_pos = checkpoint_pos,
                            "First match - setting as best_match"
                        );
                        true
                    },
                    Some(prev) => {
                        let prev_checkpoint_pos =
                            *prev.cursor().atom_position.as_ref();

                        if checkpoint_pos > prev_checkpoint_pos {
                            debug!(
                                root = %matched_state.root_parent(),
                                checkpoint_pos = checkpoint_pos,
                                prev_checkpoint_pos = prev_checkpoint_pos,
                                "Better match found (more query tokens matched)"
                            );
                            true
                        } else if checkpoint_pos == prev_checkpoint_pos {
                            // Same progress: prefer Complete over Mismatch
                            let better = matched_state.query_exhausted()
                                && !prev.query_exhausted();
                            if better {
                                debug!(
                                    root = %matched_state.root_parent(),
                                    "Same progress but Complete - updating"
                                );
                            }
                            better
                        } else {
                            false
                        }
                    },
                };

                if should_update {
                    // Trace start path for new best match
                    if let Some(start_path) =
                        matched_state.path().try_start_path()
                    {
                        let prev_start_len = self
                            .matches
                            .best_match
                            .as_ref()
                            .and_then(|p| p.path().try_start_path())
                            .map(|p| p.len())
                            .unwrap_or(0);
                        let current_start_len = start_path.len();

                        if current_start_len > prev_start_len {
                            debug!(
                                "Tracing incremental start path: prev_len={}, current_len={}",
                                prev_start_len, current_start_len
                            );
                            TraceStart {
                                end: &matched_state,
                                pos: prev_start_len,
                            }
                            .trace(&mut self.matches.trace_ctx);
                        }
                    }

                    self.matches.best_match = Some(matched_state.clone());
                }

                Some(matched_state)
            },
            None => {
                trace!("no more matches found");
                None
            },
        }
    }
}

impl<K: SearchKind> SearchState<K>
where
    K::Trav: Clone,
{
    #[context_trace::instrument_sig(level = "info", skip(self))]
    pub(crate) fn search(mut self) -> Response {
        info!("starting fold search");
        debug!(queue = %&self.matches.queue, "initial state");

        let mut iteration = 0;
        while let Some(matched_state) = &mut self.next() {
            iteration += 1;
            debug!(iteration, "tracing matched state");
            debug!(
                "About to trace MatchResult: is_complete={}, path_variant={}",
                matched_state.query_exhausted(),
                match matched_state.path() {
                    PathCoverage::Range(_) => "Range",
                    PathCoverage::Postfix(_) => "Postfix",
                    PathCoverage::Prefix(_) => "Prefix",
                    PathCoverage::EntireRoot(_) => "EntireRoot",
                }
            );
            matched_state.trace(&mut self.matches.trace_ctx);
            debug!("Finished tracing MatchResult");
        }

        debug!(iterations = iteration, "fold completed");

        // Get the final matched state from best_match
        let end = if let Some(checkpoint) = self.matches.best_match {
            debug!(
                root = %checkpoint.root_parent(),
                checkpoint_pos = *checkpoint.cursor().atom_position.as_ref(),
                is_complete = checkpoint.query_exhausted(),
                "Using best_match as final result"
            );
            checkpoint
        } else {
            // No matches found - create empty mismatch at position 0
            debug!("No matches found, creating empty mismatch");
            let start_token = self.query.path_root()[0];
            let cursor = PatternCursor {
                atom_position: AtomPosition::default(),
                path: self.query.clone(),
                _state: std::marker::PhantomData,
            };
            let path = PathCoverage::EntireRoot(IndexRangePath::new_empty(
                IndexRoot::from(PatternLocation::new(
                    start_token,
                    PatternId::default(),
                )),
            ));
            MatchResult { path, cursor }
        };

        trace!(end = %pretty(&end), "final matched state");

        let trace_ctx = &mut self.matches.trace_ctx;
        end.trace(trace_ctx);

        let response = Response {
            cache: self.matches.trace_ctx.cache,
            end,
        };

        info!("search complete");
        response
    }
}
