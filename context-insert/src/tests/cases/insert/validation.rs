//! Input validation tests for context-insert
//!
//! These tests verify that context-insert properly rejects invalid inputs
//! that could cause panics or undefined behavior.
//!
//! Test categories:
//! - InitInterval validation (zero end_bound, missing cache entries)
//! - Empty pattern rejection (search and insert paths)

use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
    tests::env::{EnvAbcd, EnvAb},
};
use context_search::{
    ErrorState,
    *,
};
use context_trace::{
    graph::getters::ErrorReason,
    tests::test_case::TestEnv,
    *,
};

// ============================================================================
// InitInterval Validation Tests
// ============================================================================

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
    let EnvAbcd { graph, abcd, .. } = EnvAbcd::initialize();
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
fn reject_init_interval_with_missing_root_entry() {
    let EnvAb { graph, a, ab, .. } = EnvAb::initialize();
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
}

// ============================================================================
// Empty Pattern Rejection Tests
// ============================================================================

/// Test that empty patterns are rejected during search
///
/// This edge case occurs when:
/// - context-read creates an ExpansionCtx with an empty pattern after processing
/// - The empty pattern is passed to search
///
/// Expected behavior: Return EmptyPatterns error instead of panicking
#[test]
fn reject_empty_pattern_search() {
    let EnvAb { graph, .. } = EnvAb::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an empty pattern
    let empty_pattern: Pattern = Pattern::default();

    // Should return an error, not panic
    let result: Result<Response, ErrorReason> = graph.find_ancestor(empty_pattern);

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
    let EnvAb { graph, .. } = EnvAb::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Create an empty pattern
    let empty_pattern: Pattern = Pattern::default();

    // Should return an error, not panic
    let result: Result<Token, ErrorState> = graph.insert(empty_pattern);

    assert!(result.is_err(), "Expected error for empty pattern insert");
    assert_eq!(result.unwrap_err().reason, ErrorReason::EmptyPatterns);
}
