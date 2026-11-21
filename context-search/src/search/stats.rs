//! `test-api` counters of the last search on this thread (no-ops in other
//! builds):
//!
//! - `equal_width_candidates`: queued candidates probed for uniqueness;
//! - `equal_width_first_matches`: queue candidates whose root has the width
//!   of a root just matched and which also match at their first
//!   comparison. Uniqueness is structural, so this must stay 0.
//! - `compared_positions`: query positions (leaf starts) of every leaf
//!   comparison, in order. Shows that a continuation never re-reads
//!   positions before its checkpoint.
//! - depth counters: `parent_raises` (states raised into a parent
//!   occurrence), `graph_prefix_descents` (graph-side decompositions,
//!   `RolePath::push_border_child` of the end path), `query_prefix_descents`
//!   (query-side decompositions, `push_border_child` of the query path), and the maximal start / end path
//!   lengths of any state of the search (raised states, entered and advanced
//!   child states, decompositions) and of the response.
//!
//! The uniqueness probe records none of them.

use context_trace::AtomPosition;

#[cfg(any(test, feature = "test-api"))]
mod imp {
    use std::cell::{
        Cell,
        RefCell,
    };

    use super::SearchStats;

    thread_local! {
        pub(super) static STATS: RefCell<SearchStats> = RefCell::new(SearchStats::default());
        pub(super) static PROBING: Cell<bool> = const { Cell::new(false) };
    }
}

/// Counters of one search.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchStats {
    /// Candidates probed for the uniqueness counter.
    pub equal_width_candidates: usize,
    pub equal_width_first_matches: usize,
    pub compared_positions: Vec<AtomPosition>,
    pub parent_raises: usize,
    pub graph_prefix_descents: usize,
    pub query_prefix_descents: usize,
    pub max_start_path_len: usize,
    pub max_end_path_len: usize,
}

/// Counters of the last search started on this thread.
#[cfg(any(test, feature = "test-api"))]
pub fn last_search_stats() -> SearchStats {
    imp::STATS.with(|stats| stats.borrow().clone())
}

#[inline]
pub(crate) fn reset() {
    #[cfg(any(test, feature = "test-api"))]
    imp::STATS.with(|stats| *stats.borrow_mut() = SearchStats::default());
}

/// Apply `_f` to the counters unless probing (no-op outside test builds).
#[inline]
fn update(_f: impl FnOnce(&mut SearchStats)) {
    #[cfg(any(test, feature = "test-api"))]
    if !imp::PROBING.with(|probing| probing.get()) {
        imp::STATS.with(|stats| _f(&mut stats.borrow_mut()));
    }
}

#[inline]
pub(crate) fn record_compare(position: AtomPosition) {
    update(|stats| stats.compared_positions.push(position));
}

/// A state raised into a parent occurrence, with its new start path length.
#[inline]
pub(crate) fn record_raise(start_len: usize) {
    update(|stats| {
        stats.parent_raises += 1;
        stats.max_start_path_len = stats.max_start_path_len.max(start_len);
    });
}

/// A graph-side prefix descent, with the new end path length.
#[inline]
pub(crate) fn record_graph_prefix_descent(end_len: usize) {
    update(|stats| {
        stats.graph_prefix_descents += 1;
        stats.max_end_path_len = stats.max_end_path_len.max(end_len);
    });
}

#[inline]
pub(crate) fn record_query_prefix_descent() {
    update(|stats| stats.query_prefix_descents += 1);
}

/// Path lengths of a state (start path, end path; 0 if absent).
#[inline]
pub(crate) fn record_path_lens(start_len: usize, end_len: usize) {
    update(|stats| {
        stats.max_start_path_len = stats.max_start_path_len.max(start_len);
        stats.max_end_path_len = stats.max_end_path_len.max(end_len);
    });
}

#[cfg(any(test, feature = "test-api"))]
pub(crate) fn record_equal_width_candidate() {
    imp::STATS.with(|stats| stats.borrow_mut().equal_width_candidates += 1);
}

#[cfg(any(test, feature = "test-api"))]
pub(crate) fn record_equal_width_first_match() {
    imp::STATS.with(|stats| stats.borrow_mut().equal_width_first_matches += 1);
}

/// Run `f` without recording compared positions (uniqueness probe).
#[cfg(any(test, feature = "test-api"))]
pub(crate) fn probing<R>(f: impl FnOnce() -> R) -> R {
    let previous = imp::PROBING.with(|probing| probing.replace(true));
    let result = f();
    imp::PROBING.with(|probing| probing.set(previous));
    result
}
