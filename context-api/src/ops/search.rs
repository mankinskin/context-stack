//! Search and its views (plan 01 §1.5).

use super::View;
use crate::{
    error::ApiError,
    types::{
        Limits,
        SearchResult,
        SearchView,
    },
};

/// `max_visited_cap`: the session's upper bound for `Limits::max_visited`.
pub(crate) fn search(
    view: &View<'_>,
    query: &str,
    search_view: SearchView,
    limits: &Limits,
    max_visited_cap: Option<usize>,
) -> Result<SearchResult, ApiError> {
    let _ = (view, query, search_view, limits, max_visited_cap);
    Err(ApiError::Unsupported("search is not implemented yet".into()))
}
