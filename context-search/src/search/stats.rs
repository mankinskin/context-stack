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

#[inline]
pub(crate) fn record_compare(_position: AtomPosition) {
    #[cfg(any(test, feature = "test-api"))]
    if !imp::PROBING.with(|probing| probing.get()) {
        imp::STATS.with(|stats| stats.borrow_mut().compared_positions.push(_position));
    }
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
