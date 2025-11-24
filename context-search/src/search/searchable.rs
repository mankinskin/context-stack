use context_trace::{
    logging::pretty,
    *,
};
use derive_new::new;
use tracing::{
    debug,
    trace,
};

use crate::{
    cursor::{
        Checkpointed,
        PatternCursor,
        PatternPrefixCursor,
    },
    r#match::iterator::SearchIterator,
    search::SearchState,
    state::{
        result::Response,
        start::{
            IntoCursor,
            StartCtx,
            StartFoldPath,
        },
    },
    SearchKind,
};
use std::fmt::Debug;

//pub(crate) type FoldResult = Result<Response, ErrorState>;

#[derive(Debug, new)]
pub struct ErrorState {
    pub reason: ErrorReason,
    pub found: Option<Box<Response>>,
}
impl From<ErrorReason> for ErrorState {
    fn from(reason: ErrorReason) -> Self {
        Self {
            reason,
            found: None,
        }
    }
}
impl From<IndexWithPath> for ErrorState {
    fn from(value: IndexWithPath) -> Self {
        ErrorReason::SingleIndex(Box::new(value)).into()
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
        // Get the starting token from the query pattern for the SearchIterator
        let start_token = self.path.role_root_child_token::<End, _>(&trav);
        debug!(start_token = %&start_token, "starting search from token");

        let start = StartCtx {
            cursor: self.clone(),
        };

        match start.get_parent_batch::<K>(&trav) {
            Ok(p) => {
                debug!(
                    batch_len = p.batch.len(),
                    "first parent batch obtained"
                );

                Ok(SearchState {
                    query: self.path.clone(),
                    matches: SearchIterator::start_parent(trav, start_token, p),
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
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.as_slice().start_search::<K>(trav)
    }
}

impl Searchable for &'_ [Token] {
    fn start_search<K: SearchKind>(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
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
        let range_path = self.to_range_path();
        let cursor = range_path.into_cursor(&trav);
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
use crate::{
    r#match::root_cursor::CompareParentBatch,
    traversal::policy::DirectedTraversalPolicy,
};

impl StartCtx {
    pub(crate) fn get_parent_batch<K: SearchKind>(
        &self,
        trav: &K::Trav,
    ) -> Result<CompareParentBatch, ErrorState> {
        let mut cursor = self.cursor.clone();
        debug!(cursor_path = %cursor.path, "get_parent_batch - cursor path before root_child_token");
        let start = self.cursor.path.role_root_child_token::<End, _>(trav);
        let checkpoint = cursor.clone();
        if cursor.advance(trav).is_continue() {
            let batch = K::Policy::gen_parent_batch(trav, start, |_trav, p| {
                start.into_parent_state(p)
            });

            let cursor = Checkpointed {
                checkpoint,
                current: cursor.as_candidate(),
            };
            Ok(CompareParentBatch { batch, cursor })
        } else {
            Err(ErrorState {
                reason: ErrorReason::SingleIndex(Box::new(IndexWithPath {
                    index: start,
                    path: self.cursor.path.clone(),
                })),
                found: None,
            })
        }
    }
}
