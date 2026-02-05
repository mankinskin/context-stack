//! Edge case tests for context-insert
//!
//! These tests verify that context-insert properly rejects invalid inputs
//! that could cause panics or undefined behavior.
//!
//! Failure modes discovered through context-read testing:
//! 1. InitInterval with end_bound = 0 (no atoms matched)
//! 2. Empty patterns passed to search/insert
//!
//! NOTE: These tests are currently marked as #[ignore] because they document
//! bugs that need to be fixed. Once the fixes are implemented:
//! 1. Remove the #[ignore] attribute
//! 2. Update tests to verify proper error handling instead of panics

use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
};
use context_search::*;
use context_trace::{
    graph::{
        Hypergraph,
        HypergraphRef,
        vertex::atom::Atom,
    },
    *,
};

/// Test that InitInterval with end_bound = 0 is rejected
///
/// This edge case occurs when:
/// - Search finds a token that partially matches the query
/// - But no atoms were confirmed as matching (checkpoint_position = 0)
/// - context-read then tries to insert with end_bound = 0
///
/// Expected behavior after fix: Return an error instead of panicking
/// Current behavior: Panics at splits.rs:63 with "called `Option::unwrap()` on a `None` value"
#[test]
#[ignore = "Bug: InitInterval with end_bound=0 causes panic - needs fix to return error"]
fn reject_init_interval_with_zero_end_bound() {
    let graph = Hypergraph::default();
    let [a, b, c, d] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
        Atom::Element('d'),
    ])[..] else {
        panic!()
    };

    // Create a pattern in the graph: abcd
    let abcd = graph.insert_pattern(vec![a, b, c, d]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an InitInterval with end_bound = 0
    // This simulates a search that found a partial match but confirmed nothing
    let invalid_init = InitInterval {
        root: abcd,
        cache: TraceCache::default(), // Empty cache
        end_bound: 0.into(),          // Zero end bound - invalid!
    };

    // TODO: Once fixed, this should return an error, not panic
    // For now, this test documents the panic behavior
    let _token: Token = graph.insert_init((), invalid_init);

    // After fix: uncomment and verify error handling
    // assert!(result.is_err(), "Expected error for InitInterval with end_bound=0");
}

/// Test that empty patterns are rejected during search
///
/// This edge case occurs when:
/// - context-read creates an ExpansionCtx with an empty pattern after processing
/// - The empty pattern is passed to search
///
/// Expected behavior after fix: Return an error instead of panicking
/// Current behavior: Panics at pattern_range.rs:175 with "called `Option::unwrap()` on a `None` value"
#[test]
#[ignore = "Bug: Empty pattern search causes panic - needs fix to return error"]
fn reject_empty_pattern_search() {
    let graph = Hypergraph::default();
    let [a, b] =
        graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
    else {
        panic!()
    };

    // Create a pattern in the graph
    let _ab = graph.insert_pattern(vec![a, b]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an empty pattern
    let empty_pattern: Pattern = Pattern::default();

    // TODO: Once fixed, this should return an error, not panic
    let _result: Result<Response, _> = graph.find_ancestor(empty_pattern);

    // After fix: uncomment and verify error handling
    // assert!(result.is_err(), "Expected error for empty pattern search");
}

/// Test that insert with empty pattern is rejected
///
/// Similar to reject_empty_pattern_search but tests the insert path directly
///
/// Expected behavior after fix: Return an error instead of panicking
#[test]
#[ignore = "Bug: Empty pattern insert causes panic - needs fix to return error"]
fn reject_empty_pattern_insert() {
    let graph = Hypergraph::default();
    let [a, b] =
        graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
    else {
        panic!()
    };

    // Create a pattern in the graph
    let _ab = graph.insert_pattern(vec![a, b]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an empty pattern
    let empty_pattern: Pattern = Pattern::default();

    // TODO: Once fixed, this should return an error, not panic
    let _result: Result<Token, _> = graph.insert(empty_pattern);

    // After fix: uncomment and verify error handling
    // assert!(result.is_err(), "Expected error for empty pattern insert");
}

/// Integration test: Simulates the context-read failure scenario
///
/// This test reproduces the exact scenario from read_sequence1:
/// - Graph has "hypergraph" pattern partially built
/// - Search for pattern [p, h] where p exists but h doesn't match next position
/// - Search returns checkpoint_position = 0
/// - insert_or_get_complete should handle this gracefully
///
/// Expected behavior after fix: Return appropriate error/result without panicking
/// Current behavior: Panics at splits.rs:63
#[test]
#[ignore = "Bug: Partial match with no checkpoint causes panic in insert_or_get_complete"]
fn integration_partial_match_no_checkpoint() {
    let graph = Hypergraph::default();

    // Build a graph similar to what context-read creates
    let [h, y, p, e, r, g, a] = graph.insert_atoms([
        Atom::Element('h'),
        Atom::Element('y'),
        Atom::Element('p'),
        Atom::Element('e'),
        Atom::Element('r'),
        Atom::Element('g'),
        Atom::Element('a'),
    ])[..] else {
        panic!()
    };

    // Create "hypergra" pattern (partial hypergraph)
    let _hypergra = graph.insert_pattern(vec![h, y, p, e, r, g, r, a]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [p, h] - p will be found in hypergra, but h won't match 'e'
    let query = vec![p, h];

    // TODO: Once fixed, this should handle gracefully
    // This mimics what context-read does: insert_or_get_complete
    let _result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    // After fix: verify it returns an appropriate error or result
    // without panicking
}

/// Test: Single token pattern with mismatch at first position
///
/// Tests the boundary case where a single-token pattern exists in graph
/// but the search query doesn't match at the first position
#[test]
fn single_token_mismatch_at_start() {
    let graph = Hypergraph::default();

    let [a, b, c] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
    ])[..] else {
        panic!()
    };

    // Create pattern [a, b]
    let _ab = graph.insert_pattern(vec![a, b]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [c, b] - c is different from a
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
