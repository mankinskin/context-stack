use super::core::{
    IntoCursor,
    StartCtx,
    StartFoldPath,
};
use crate::{
    cursor::{
        PatternCursor,
        PatternPrefixCursor,
    },
    r#match::{
        iterator::SearchIterator,
        root_cursor::CompareParentBatch,
    },
    search::{
        searchable::ErrorState,
        SearchState,
    },
    traversal::{
        policy::DirectedTraversalPolicy,
        SearchKind,
    },
    Response,
};
use context_trace::{
    logging::format_utils::pretty,
    *,
};
use tracing::{
    debug,
    trace,
};

impl StartCtx {
    pub(crate) fn get_parent_batch<K: SearchKind>(
        &self,
        trav: &K::Trav,
    ) -> Result<CompareParentBatch, ErrorState> {
        let mut cursor = self.cursor.clone();
        debug!(cursor_path = %cursor.path, "get_parent_batch - cursor path before root_child_token");
        let parent = self.cursor.path.role_root_child_token::<End, _>(trav);
        if cursor.advance(trav).is_continue() {
            let batch = K::Policy::gen_parent_batch(trav, parent, |trav, p| {
                parent.into_parent_state(trav, p)
            });

            Ok(CompareParentBatch { batch, cursor })
        } else {
            Err(ErrorState {
                reason: ErrorReason::SingleIndex(Box::new(IndexWithPath {
                    index: parent,
                    path: self.cursor.path.clone(),
                })),
                found: None,
            })
        }
    }
}

pub trait Searchable: Sized {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState>;

    #[context_trace::instrument_sig(level = "debug", skip(self, trav))]
    fn search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<Response, ErrorState>
    where
        K::Trav: Clone,
    {
        debug!("starting search");
        match self.start_search::<K>(trav) {
            Ok(ctx) => {
                debug!("start search successful, beginning fold");
                Ok(ctx.search())
            },
            Err(err) => {
                debug!(error = %pretty(&err), "start search failed");
                Err(err)
            },
        }
    }
}

impl Searchable for PatternCursor {
    #[context_trace::instrument_sig(level = "debug", skip(self, trav))]
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        debug!("starting pattern cursor search");
        debug!(path = %self.path, "pattern cursor path");

        // Get the starting token from the query pattern for the SearchIterator
        let start_token = self.path.role_root_child_token::<End, _>(&trav);
        debug!(start_token = %pretty(&start_token), "starting search from token");

        let start = StartCtx {
            cursor: self.clone(),
        };

        match start.get_parent_batch::<K>(&trav) {
            Ok(p) => {
                debug!(
                    batch_len = p.batch.len(),
                    "first parent batch obtained"
                );
                trace!(batch_details = %pretty(&p), "parent batch details");

                Ok(SearchState {
                    query: self.path.clone(),
                    matches: SearchIterator::start_parent(
                        trav,
                        start_token,
                        p,
                    ),
                })
            },
            Err(err) => {
                debug!(error = %pretty(&err), "failed to get parent batch");
                Err(err)
            },
        }
    }
}

impl<T: Searchable + Clone> Searchable for &T {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.clone().start_search(trav)
    }
}

impl<const N: usize> Searchable for &'_ [Token; N] {
    #[context_trace::instrument_sig(level = "trace", skip(self, trav), fields(token_count = N))]
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        debug!(token_count = N, "creating pattern range path from array");
        trace!(tokens = ?self, "token array");
        
        // Delegate to slice implementation which handles atom special case
        self.as_slice().start_search::<K>(trav)
    }
}

impl Searchable for &'_ [Token] {
    #[context_trace::instrument_sig(level = "trace", skip(self, trav), fields(token_count = self.len()))]
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        debug!(token_count = self.len(), "creating pattern range path from slice");
        trace!(tokens = ?self, "token slice");
        
        // Convert the token slice to a PatternRangePath and start the search
        // This works for both atoms and composite patterns now thanks to MatchState::Query
        PatternRangePath::from(self).start_search::<K>(trav)
    }
}

impl Searchable for Pattern {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        PatternRangePath::from(self).start_search::<K>(trav)
    }
}

impl Searchable for Vec<Token> {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        Pattern::from(self).start_search::<K>(trav)
    }
}

impl Searchable for PatternEndPath {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.to_range_path()
            .into_cursor(&trav)
            .start_search::<K>(trav)
    }
}

impl Searchable for PatternRangePath {
    #[context_trace::instrument_sig(level = "trace", skip(self, trav), fields(path = ?self))]
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        debug!("converting pattern range path to cursor");
        trace!(range_path_details = %self, "pattern range path details");
        
        let range_path = self.to_range_path();
        debug!(range_path = %range_path, "converted to range_path");
        
        let width = range_path.calc_width(&trav);
        debug!("calc_width returned: {}", width);
        
        let cursor = range_path.into_cursor(&trav);
        debug!(cursor_atom_pos = *cursor.atom_position, cursor_path = %cursor.path, "created cursor");
        
        cursor.start_search::<K>(trav)
    }
}

impl Searchable for PatternPrefixCursor {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        PatternCursor::from(self).start_search(trav)
    }
}
