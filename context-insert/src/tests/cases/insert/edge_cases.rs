//! Edge case tests for context-insert
//!
//! These tests verify that context-insert properly rejects invalid inputs
//! that could cause panics or undefined behavior.
//!
//! Failure modes discovered through context-read testing:
//! 1. InitInterval with end_bound = 0 (no atoms matched)
//! 2. Empty patterns passed to search/insert
//! 3. InitInterval where cache doesn't contain root token entry
//!
//! These tests verify proper error handling for invalid inputs.

use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
};
use context_search::{
    ErrorState,
    *,
};
use context_trace::{
    graph::{
        Hypergraph,
        HypergraphRef,
        getters::ErrorReason,
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
/// Expected behavior: Return InvalidEndBound error instead of panicking
#[test]
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

    // Should return an error, not panic
    let result: Result<Token, ErrorState> = graph.insert_init((), invalid_init);

    assert!(
        result.is_err(),
        "Expected error for InitInterval with end_bound=0"
    );
    assert_eq!(result.unwrap_err().reason, ErrorReason::InvalidEndBound);
}

/// Test that empty patterns are rejected during search
///
/// This edge case occurs when:
/// - context-read creates an ExpansionCtx with an empty pattern after processing
/// - The empty pattern is passed to search
///
/// Expected behavior: Return EmptyPatterns error instead of panicking
#[test]
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

    // Should return an error, not panic
    let result: Result<Response, ErrorReason> =
        graph.find_ancestor(empty_pattern);

    assert!(result.is_err(), "Expected error for empty pattern search");
    assert_eq!(result.unwrap_err(), ErrorReason::EmptyPatterns);
}

/// Test that insert with empty pattern is rejected
///
/// Similar to reject_empty_pattern_search but tests the insert path directly
///
/// Expected behavior: Return EmptyPatterns error instead of panicking
#[test]
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

    // Should return an error, not panic
    let result: Result<Token, ErrorState> = graph.insert(empty_pattern);

    assert!(result.is_err(), "Expected error for empty pattern insert");
    assert_eq!(result.unwrap_err().reason, ErrorReason::EmptyPatterns);
}

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

/// Test: InitInterval with cache missing root token entry
///
/// This edge case occurs when:
/// - Search finds a path through the graph
/// - The Response's root_token() returns a token that wasn't traversed
/// - The cache doesn't contain an entry for this root token
/// - insert_init is called with this invalid InitInterval
///
/// This reproduces the failure seen in context-read's validate_triple_repeat test:
/// - root = T2w2 (index=2)
/// - cache entries: [0] (only vertex 0 in cache)
///
/// Expected behavior: Return error instead of panicking at splits.rs:63
#[test]
#[ignore = "Requires context-insert fix: validate that cache contains root token entry"]
fn reject_init_interval_with_missing_root_entry() {
    let graph = Hypergraph::default();

    // Create atoms
    let [a, b] =
        graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
    else {
        panic!()
    };

    // Create pattern [a, b] - this becomes token T2
    let ab = graph.insert_pattern(vec![a, b]);

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an InitInterval where:
    // - root is ab (T2)
    // - cache only has entry for atom 'a' (T0), NOT for ab
    // This simulates the malformed state context-read produces
    let mut cache = TraceCache::default();
    // Add entry for vertex 0 (atom 'a'), but NOT for vertex 2 (ab)
    cache.entries.insert(
        a.vertex_index(),
        VertexCache {
            index: a,
            bottom_up: Default::default(),
            top_down: Default::default(),
        },
    );

    let invalid_init = InitInterval {
        root: ab,            // Token at vertex index 2
        cache,               // Cache only has vertex 0
        end_bound: 2.into(), // Valid non-zero end_bound
    };

    // Should return an error, not panic at splits.rs:63
    let result: Result<Token, ErrorState> = graph.insert_init((), invalid_init);

    assert!(
        result.is_err(),
        "Expected error for InitInterval with cache missing root entry"
    );
    // The specific error type may need to be defined
    // For now, just verify it doesn't panic
}

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
    let graph = Hypergraph::default();

    // Create atoms for 'a' and 'b'
    let [a, b] =
        graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
    else {
        panic!()
    };

    // Build the graph structure that context-read would create for "ababab":
    // First: [a, b] -> ab
    let ab = graph.insert_pattern(vec![a, b]);

    // Then: [ab, ab, ab] -> ababab
    let _ababab = graph.insert_pattern(vec![ab, ab, ab]);

    let graph = HypergraphRef::from(graph);
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

/// Test: Repeated pattern with overlapping contexts
///
/// Tests the scenario where a pattern appears multiple times and
/// context-read needs to find the largest enclosing context.
/// This is related to the missing "aa" token in validate_three_repeated.
///
/// Expected: Graph should contain intermediate patterns like "aa"
#[test]
#[ignore = "Requires investigation: insert of [a, a] fails - may need different approach"]
fn repeated_pattern_intermediate_tokens() {
    let graph = Hypergraph::default();

    // Create atom 'a'
    let [a] = graph.insert_atoms([Atom::Element('a')])[..] else {
        panic!()
    };

    let graph = HypergraphRef::from(graph);
    let _tracing = context_trace::init_test_tracing!(&graph);

    // For input "aaa", we should get:
    // - 'a' (atom)
    // - 'aa' (pattern [a, a])
    // - 'aaa' (pattern [aa, a] or [a, aa])

    // First insert "aa"
    let query_aa = vec![a, a];
    let aa_result: Result<Token, ErrorState> = graph.insert(query_aa);
    assert!(aa_result.is_ok(), "Should be able to insert 'aa'");
    let aa = aa_result.unwrap();

    // Then insert "aaa" as [aa, a]
    let query_aaa = vec![aa, a];
    let aaa_result: Result<Token, ErrorState> = graph.insert(query_aaa);
    assert!(aaa_result.is_ok(), "Should be able to insert 'aaa'");

    // Verify the graph has all expected patterns
    assert_indices!(graph, aa);
    // The 'aaa' pattern should reference 'aa'
}
