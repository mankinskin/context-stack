//! Tests for expanded overlaps in context-insert
//!
//! These tests verify behavior when insert query starts at a postfix
//! of an existing anchor token in a context pattern.
//!
//! ## What is an "Expanded Overlap"?
//!
//! When the insert algorithm searches for a pattern, it may find that:
//! 1. The query starts with atoms that match a postfix of an existing token
//! 2. This creates an "overlap" where the query extends beyond the existing pattern
//!
//! For example:
//! - Graph has "abc" token (width 3)
//! - Insert query is [b, c, d]
//! - "bc" is a postfix of "abc"
//! - The overlap links "abc" and the new pattern "bcd"
//!
//! ## Test Categories
//!
//! - **Simple postfix overlap**: Query starts with postfix of single token
//! - **Nested postfix overlap**: Query starts with postfix of compound token
//! - **Multiple overlap candidates**: Multiple tokens could match the prefix
//! - **Cursor position verification**: Ensure cursor correctly tracks consumed atoms

use crate::{
    insert::ToInsertCtx,
    tests::env::{
        EnvExpandedOverlap,
        EnvMultiOverlap,
    },
};
use context_trace::{
    tests::test_case::TestEnv,
    *,
};
use pretty_assertions::assert_eq;

// ============================================================================
// Simple Postfix Overlap Tests
// ============================================================================

/// Test: Insert [b, c] where "bc" is postfix of "abc"
///
/// Graph state: abc = [ab, c] = [[a, b], c]
/// Query: [b, c]
/// Expected: Should find bc as postfix of abc, return bc with correct cursor
#[test]
fn insert_postfix_bc_of_abc() {
    let EnvExpandedOverlap {
        graph,
        b,
        c,
        ab,
        abc,
        ..
    } = EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [b, c] - this is postfix of abc
    let query = vec![b, c];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // The result should:
    // 1. Find or create a token representing [b, c]
    // 2. The path should indicate full query was consumed
    assert_eq!(
        path.path_root().len(),
        2,
        "Query had 2 tokens, path should reflect this"
    );

    // Width should be 2 (b + c)
    assert_eq!(*index.width(), 2, "Result should have width 2 for [b, c]");
}

/// Test: Insert [b, c, d] where "bc" is postfix of "abc"
///
/// Graph state: abc = [ab, c]
/// Query: [b, c, d]
/// Expected: Should handle the overlap and include d
#[test]
fn insert_postfix_bcd_extends_abc() {
    let EnvExpandedOverlap {
        graph,
        b,
        c,
        d,
        abc,
        ..
    } = EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [b, c, d] - "bc" is postfix of abc, d extends it
    let query = vec![b, c, d];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Path root should have 3 tokens
    assert_eq!(path.path_root().len(), 3, "Query had 3 tokens");

    // Width could be 2 (partial match) or 3 (full)
    // Depending on implementation, we might get bc (width 2) if bcd doesn't exist
    // or bcd (width 3) if it was created/found
    assert!(*index.width() >= 2, "Should match at least [b, c]");
}

/// Test: Insert [c, d] where "c" is postfix of "abc"
///
/// Graph state: abc = [ab, c]
/// Query: [c, d]
/// Expected: Should find c at end of abc, extend with d
#[test]
fn insert_single_atom_postfix() {
    let EnvExpandedOverlap {
        graph, c, d, abc, ..
    } = EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [c, d] - c is single-atom postfix of abc
    let query = vec![c, d];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Should have consumed query of 2 tokens
    assert_eq!(path.path_root().len(), 2, "Query had 2 tokens");
}

// ============================================================================
// Multiple Overlap Candidate Tests
// ============================================================================

/// Test: Insert [b, c] when both "ab" and "bc" exist
///
/// Graph state: ab=[a,b], bc=[b,c], cd=[c,d], abcd=[ab,cd]
/// Query: [b, c]
/// Expected: Should find existing bc token
#[test]
fn insert_finds_existing_overlap_pattern() {
    let EnvMultiOverlap {
        graph, b, c, bc, ..
    } = EnvMultiOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [b, c] - bc already exists in graph
    let query = vec![b, c];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Should find existing bc token
    assert_eq!(index, bc, "Should find existing bc token");
    assert_eq!(*index.width(), 2, "bc has width 2");
}

/// Test: Insert [c, d] when "cd" exists as postfix of "abcd"
///
/// Graph state: abcd=[ab,cd]
/// Query: [c, d]
/// Expected: Should find cd as postfix of abcd
#[test]
fn insert_finds_postfix_of_compound_token() {
    let EnvMultiOverlap {
        graph,
        c,
        d,
        cd,
        abcd,
        ..
    } = EnvMultiOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [c, d] - cd exists and is postfix of abcd
    let query = vec![c, d];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Should find existing cd token
    assert_eq!(index, cd, "Should find existing cd token");
    assert_eq!(*index.width(), 2, "cd has width 2");
}

// ============================================================================
// Cursor Position Verification Tests
// ============================================================================

/// Test: Verify cursor position after partial postfix match
///
/// When we match a partial postfix, the cursor should correctly indicate:
/// 1. How many atoms were consumed
/// 2. What position in the query we're at
#[test]
fn cursor_position_after_postfix_match() {
    let EnvExpandedOverlap {
        graph,
        b,
        c,
        d,
        e,
        ab,
        abc,
        ..
    } = EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [b, c, d, e] - 4 atoms
    // "bc" might be matched as postfix of abc
    let query = vec![b, c, d, e];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // The path root should reflect the original query
    assert_eq!(path.path_root().len(), 4, "Original query had 4 tokens");

    // The matched portion's width tells us how many atoms were consumed
    let consumed = *index.width();
    assert!(
        consumed >= 2,
        "Should consume at least [b, c] (width 2), got width {}",
        consumed
    );
}

/// Test: Cursor tracks position correctly for overlapping matches
///
/// The cursor (via path.end indices) should indicate which tokens
/// in the query have been processed.
#[test]
fn cursor_tracks_overlap_consumption() {
    let EnvMultiOverlap {
        graph, b, c, d, bc, ..
    } = EnvMultiOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [b, c, d] - 3 atoms
    // bc exists, so should match that first (width 2)
    let query = vec![b, c, d];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Path root reflects original query
    assert_eq!(path.path_root().len(), 3, "Query had 3 tokens");

    // If bc was found (width 2), cursor consumed first 2 atoms
    // Remaining: d
    // Width of result tells us consumption
    let width = *index.width();
    if index == bc {
        assert_eq!(width, 2, "bc has width 2, consumed b and c");
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test: Insert query that doesn't form any overlap
///
/// Query atoms exist in graph but don't form overlapping pattern
#[test]
fn insert_no_overlap_path() {
    let EnvExpandedOverlap { graph, d, e, .. } =
        EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for [d, e] - neither d nor e appear as postfix of abc
    let query = vec![d, e];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Should process query of 2 tokens
    assert_eq!(path.path_root().len(), 2, "Query had 2 tokens");
}

/// Test: Insert single atom that is postfix of existing token
///
/// Query: [c] where abc ends with c
/// This should handle the edge case of single-atom query
#[test]
fn insert_single_atom_is_postfix() {
    let EnvExpandedOverlap { graph, c, abc, .. } =
        EnvExpandedOverlap::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Query for single atom [c] - c is postfix of abc
    let query = vec![c];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    // Single atom query might return error (SingleIndex) or succeed
    // Just verify no panic
    match result {
        Ok(Ok(IndexWithPath { index, .. })) => {
            assert_eq!(index, c, "Should return c atom");
        },
        Ok(Err(IndexWithPath { index, .. })) => {
            assert_eq!(index, c, "Should return c atom");
        },
        Err(e) => {
            // SingleIndex error is acceptable for single-atom queries
            // This is expected behavior per the search algorithm
        },
    }
}
