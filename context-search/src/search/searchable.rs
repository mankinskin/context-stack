use context_trace::{
    logging::pretty,
    *,
};
use derive_new::new;
use tracing::debug;

use crate::{
    cursor::{
        PatternCursor,
        PatternPrefixCursor,
    },
    search::{
        context::AncestorSearchTraversal,
        SearchState,
    },
    state::{
        response::Response,
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

#[derive(Debug, new, PartialEq, Eq)]
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

pub trait Searchable<K: SearchKind = AncestorSearchTraversal>: Sized {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState>;

    #[context_trace::instrument_sig(level = "debug", skip(self, trav))]
    fn search(
        self,
        trav: K::Trav,
    ) -> Result<Response, ErrorState>
    where
        K::Trav: Clone,
    {
        debug!("starting search");
        match self.start_search(trav) {
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

impl<K: SearchKind> Searchable<K> for PatternCursor {
    #[context_trace::instrument_sig(level = "debug", skip(self, trav))]
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        // Get the starting token from the query pattern for the SearchIterator
        let start_token = self.path.role_root_child_token::<End, _>(&trav);
        debug!(start_token = %&start_token, "starting search from token");

        StartCtx {
            trav,
            start_token,
            cursor: self.clone(),
        }
        .into_search()
    }
}

// Implement Searchable for common atom iterator types
// Note: We implement for specific iterator types rather than a blanket impl
// to avoid trait coherence conflicts with Token-based implementations

impl<K: SearchKind> Searchable<K> for std::str::Chars<'_>
where
    char: AsAtom<AtomOf<TravKind<K::Trav>>>,
{
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        // Pass the iterator directly to get_atom_children
        // char implements AsAtom<char> so this works for char-based graphs
        let pattern = trav.graph().get_atom_children(self)?;

        pattern.start_search(trav)
    }
}

impl<K: SearchKind, I> Searchable<K> for std::iter::Map<I, fn(&str) -> char>
where
    I: Iterator<Item = &'static str>,
    char: AsAtom<AtomOf<TravKind<K::Trav>>>,
{
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        // Pass the iterator directly to get_atom_children
        let pattern = trav.graph().get_atom_children(self)?;

        pattern.start_search(trav)
    }
}

impl<K: SearchKind, T: Searchable<K> + Clone> Searchable<K> for &T {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.clone().start_search(trav)
    }
}

impl<const N: usize, K: SearchKind> Searchable<K> for [Token; N] {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.as_slice().start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for &'_ [Token] {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        if self.is_empty() {
            return Err(ErrorReason::EmptyPatterns.into());
        }
        PatternRangePath::from(self).start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for Pattern {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        if self.is_empty() {
            return Err(ErrorReason::EmptyPatterns.into());
        }
        PatternRangePath::from(self).start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for Vec<Token> {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        if self.is_empty() {
            return Err(ErrorReason::EmptyPatterns.into());
        }
        Pattern::from(self).start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for PatternEndPath {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        self.to_range_path().into_cursor(&trav).start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for PatternRangePath {
    #[context_trace::instrument_sig(level = "trace", skip(self, trav), fields(path = ?self))]
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        let range_path = self.to_range_path();
        let cursor = range_path.into_cursor(&trav);
        cursor.start_search(trav)
    }
}

impl<K: SearchKind> Searchable<K> for PatternPrefixCursor {
    fn start_search(
        self,
        trav: K::Trav,
    ) -> Result<SearchState<K>, ErrorState> {
        PatternCursor::from(self).start_search(trav)
    }
}
