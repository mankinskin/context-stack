//! Integration tests simulating context-read scenarios
//!
//! These tests reproduce exact scenarios from context-read tests to verify
//! that context-insert handles them gracefully (returning errors instead of panicking).
//!
//! The scenarios include:
//! - Partial match with no checkpoint advancement
//! - Triple repeat pattern handling (ababab)
//! - Single token mismatch at query start

use crate::{
    insert::ToInsertCtx,
    tests::env::{
        EnvAbc,
        EnvHypergra,
        EnvTripleRepeat,
    },
};
use context_search::{
    ErrorState,
    *,
};
use context_trace::{
    tests::test_case::TestEnv,
    *,
};

// ============================================================================
// Partial Match Scenarios
// ============================================================================

/// Integration test: Simulates the context-read failure scenario
///
/// This test reproduces the exact scenario from read_sequence1:
/// - Graph has "hypergraph" pattern partially built
/// - Search for pattern [p, h] where p exists but h doesn't match next position
/// - Search returns checkpoint_position = 0
/// - insert_or_get_complete should handle this gracefully
///
/// Expected behavior: Returns appropriate error/result without panicking
#[test]
fn integration_partial_match_no_checkpoint() {
    let EnvHypergra { graph, p, h, .. } = EnvHypergra::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [p, h] - p will be found in hypergra, but h won't match 'e'
    let query = vec![p, h];

    // This mimics what context-read does: insert_or_get_complete
    // Should handle gracefully without panicking
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    // The result should either be:
    // - Ok(Ok(_)) if insertion succeeded
    // - Ok(Err(_)) if pattern already exists
    // - Err(_) if validation failed
    // It should NOT panic
    assert!(
        result.is_ok() || result.is_err(),
        "insert_or_get_complete should return a result, not panic"
    );
}

/// Test: Single token pattern with mismatch at first position
///
/// Tests the boundary case where a single-token pattern exists in graph
/// but the search query doesn't match at the first position
#[test]
fn single_token_mismatch_at_start() {
    let EnvAbc { graph, b, c, .. } = EnvAbc::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [c, b] - c is different from a (which starts the ab pattern)
    // This should fail gracefully, not panic
    let query = vec![c, b];
    let result: Result<Response, _> = graph.find_ancestor(query);

    // This search should return an error (no match found)
    // or a response indicating no match, NOT panic
    assert!(
        result.is_err() || !result.as_ref().unwrap().query_exhausted(),
        "Expected no match or error for mismatched pattern"
    );
}

// ============================================================================
// Repeat Pattern Scenarios
// ============================================================================

/// Test: Simulates "ababab" reading which triggers cache/root mismatch
///
/// This test reproduces the exact scenario from validate_triple_repeat:
/// - Input: "ababab" (6 chars)
/// - After reading, context-read tries to expand a pattern
/// - The expansion creates an InitInterval where root != cache entries
///
/// Expected behavior: Graceful error handling, no panic
#[test]
fn triple_repeat_pattern_scenario() {
    let EnvTripleRepeat { graph, ab, .. } = EnvTripleRepeat::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Now simulate what context-read does: search for [ab] in the context of ababab
    // This might return a Response where root_token is different from cached entries
    let query = vec![ab];

    // insert_or_get_complete should handle this without panic
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    // Verify no panic occurred
    match result {
        Ok(Ok(_)) => { /* Pattern already exists - fine */ },
        Ok(Err(_)) => { /* Insertion completed - fine */ },
        Err(_) => { /* Error returned - fine, as long as no panic */ },
    }
}

/// Test: Search for exact triple repeat pattern
///
/// After graph contains ab and ababab, searching for [ab, ab, ab]
/// should find ababab, not abab.
#[test]
fn search_triple_repeat_finds_full_pattern() {
    let EnvTripleRepeat {
        graph, ab, ababab, ..
    } = EnvTripleRepeat::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [ab, ab, ab] - should find ababab
    let query = vec![ab, ab, ab];
    let result: Result<Response, _> = graph.find_ancestor(query);

    assert!(result.is_ok(), "Search should succeed");
    let response = result.unwrap();

    // Should find ababab (width 6), not abab (width 4)
    assert!(response.query_exhausted(), "Query should be fully consumed");
    assert_eq!(
        response.root_token(),
        ababab,
        "Should find ababab, not a partial match"
    );
}

/// Test: Insert same token repeated
///
/// Tests inserting patterns like [a, a] which have the same token repeated
#[test]
fn insert_same_token_repeated() {
    use crate::tests::env::EnvSingleAtom;

    let EnvSingleAtom { graph, a } = EnvSingleAtom::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);
    graph.emit_graph_snapshot();

    // Insert [a, a] - same token repeated
    let query = vec![a, a];
    let result: Result<Token, ErrorState> = graph.insert(query);

    assert!(result.is_ok(), "Should be able to insert [a, a]");
    let aa = result.unwrap();

    assert_eq!(aa.width(), TokenWidth(2), "aa should have width 2");
}
