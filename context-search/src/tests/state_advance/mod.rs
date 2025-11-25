//! Unit tests for StateAdvance trait implementations in context-search
//!
//! Tests the advancement behavior for:
//! - ParentCompareState advancing to CompareRootState
//! - CompareState<Candidate, Candidate> advancing
//! - CompareState<Matched, Matched> advancing
//!
//! Each test verifies:
//! - Successful advancement when possible
//! - Proper error handling when advancement fails
//! - State consistency after advancement (cursors, positions, etc.)

mod compare_state;
mod integration;
mod parent_compare_state;
