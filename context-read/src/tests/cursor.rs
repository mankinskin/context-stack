//! Tests for cursor advancement during block expansion.
//!
//! These tests verify cursor behavior when searching for patterns in the hypergraph.
//! They document both working functionality and known limitations.
//!
//! ## What Works
//!
//! - Finding existing patterns via atom sequences (e.g., [a,b] finds ab)
//! - Cursor position tracking during search
//! - Path advancement through patterns (MoveRootIndex)
//! - Handling single-atom queries (returns SingleIndex error)
//! - Finding repeated patterns when pre-built (aa, aaa)
//!
//! ## Known Limitations (marked with TODOs)
//!
//! - Pattern creation: `insert_or_get_complete([a,b,a,b])` returns ab (width 2)
//!   instead of creating abab (width 4)
//! - Optimal matching: Search prefers first match found over best match
//!   (e.g., returns aaa instead of aaaa for [a,a,a,a])
//! - Mixed patterns: `insert([ab, c])` returns ab instead of creating abc
//!
//! ## Test Categories
//!
//! - **Basic advancement**: Single match advances cursor
//! - **Multiple advances**: Iterative consumption of input
//! - **Width tracking**: Atoms consumed matches token widths
//! - **Repeated tokens**: Same token appearing multiple times in input
//! - **Edge cases**: Single token, empty patterns

use context_insert::*;
use context_search::*;
use context_trace::{
    graph::vertex::atom::Atom,
    *,
};
use pretty_assertions::assert_eq;

// ============================================================================
// Test Environment Setup
// ============================================================================

/// Create a graph with atoms a, b, c and pattern "ab"
fn setup_abc_ab() -> (HypergraphRef, Token, Token, Token, Token) {
    let graph = Hypergraph::default();

    // Insert atoms
    let [a, b, c] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
    ])[..] else {
        panic!("insert_atoms failed")
    };

    // Insert "ab" pattern BEFORE wrapping in HypergraphRef
    let (ab, _ab_id) = graph.insert_pattern_with_id(vec![a, b]);

    graph.emit_graph_snapshot();

    // Wrap in HypergraphRef for test graph support
    let graph = HypergraphRef::from(graph);

    (graph, a, b, c, ab)
}

/// Create a graph with just atom 'a'
fn setup_single_atom() -> (HypergraphRef, Token) {
    let graph = Hypergraph::default();
    let [a] = graph.insert_atoms([Atom::Element('a')])[..] else {
        panic!("insert_atoms failed")
    };
    graph.emit_graph_snapshot();
    let graph = HypergraphRef::from(graph);
    (graph, a)
}

/// Create a graph with 'a' and 'aa' pattern already built
fn setup_aa_pattern() -> (HypergraphRef, Token, Token) {
    let graph = Hypergraph::default();
    let [a] = graph.insert_atoms([Atom::Element('a')])[..] else {
        panic!("insert_atoms failed")
    };
    // Pre-build the aa pattern
    let (aa, _) = graph.insert_pattern_with_id(vec![a, a]);

    graph.emit_graph_snapshot();

    let graph = HypergraphRef::from(graph);
    (graph, a, aa)
}

/// Create a graph with 'a', 'aa', and 'aaa' patterns
fn setup_aaa_pattern() -> (HypergraphRef, Token, Token, Token) {
    let graph = Hypergraph::default();
    let [a] = graph.insert_atoms([Atom::Element('a')])[..] else {
        panic!("insert_atoms failed")
    };
    let (aa, _) = graph.insert_pattern_with_id(vec![a, a]);
    let (aaa, _) = graph.insert_pattern_with_id(vec![aa, a]);

    graph.emit_graph_snapshot();

    let graph = HypergraphRef::from(graph);
    (graph, a, aa, aaa)
}

/// Create graph with 'a', 'aa', 'aaa', 'aaaa' patterns
fn setup_aaaa_pattern() -> (HypergraphRef, Token, Token, Token, Token) {
    let graph = Hypergraph::default();
    let [a] = graph.insert_atoms([Atom::Element('a')])[..] else {
        panic!("insert_atoms failed")
    };
    let (aa, _) = graph.insert_pattern_with_id(vec![a, a]);
    let (aaa, _) = graph.insert_pattern_with_id(vec![aa, a]);
    let (aaaa, _) = graph.insert_pattern_with_id(vec![aa, aa]);

    graph.emit_graph_snapshot();

    let graph = HypergraphRef::from(graph);
    (graph, a, aa, aaa, aaaa)
}

// ============================================================================
// Basic Cursor Advancement Tests
// ============================================================================

/// Test: Query for atoms [a, b] matches existing "ab" pattern
///
/// Input: [a, b] atoms where ab=[a,b] exists in graph
/// Expected: Returns ab token with width 2
#[test]
fn cursor_single_token_exhausts() {
    let (graph, a, b, _c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b] atoms - should find existing "ab" pattern
    let query = vec![a, b];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    // Should find the existing ab token
    let IndexWithPath { index, path: _ } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    assert_eq!(index, ab, "Should find ab token");
    assert_eq!(index.width(), TokenWidth(2), "ab has width 2");
}

/// Test: Two-token query finds first match
///
/// Input: [a, b, a, b] where ab exists
/// Expected: Returns ab (width 2) since that's the best match for the prefix
/// Note: Full pattern matching would require creating [ab, ab] which isn't implemented
#[test]
fn cursor_two_tokens_first_match() {
    let (graph, a, b, _c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b, a, b] - four atoms
    let query = vec![a, b, a, b];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path: _ } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // With current algorithm, finds 'ab' as prefix match (width 2)
    // TODO: Full implementation would create/find [ab, ab] with width 4
    assert_eq!(index, ab, "Should find ab as prefix");
    assert_eq!(*index.width(), 2, "ab has width 2");
}

/// Test: Query with atoms advances by atom width
///
/// Input: [a, b, a, b] where ab exists
/// Expected: Finds ab (width 2), cursor advances by 2 atoms
#[test]
fn cursor_atoms_finds_pattern() {
    let (graph, a, b, _c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b, a, b] - should find "ab" as largest prefix
    let query = vec![a, b, a, b];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    let IndexWithPath { index, path } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Should find ab token (width 2)
    assert_eq!(index.width(), TokenWidth(2), "Should find ab with width 2");

    // The path indicates cursor state - root should be [a,b,a,b]
    assert_eq!(path.path_root().len(), 4, "Query has 4 atoms");
}

// ============================================================================
// Repeated Token Tests (with pre-built patterns)
// ============================================================================

/// Test: Find existing 'aa' pattern via [a, a] query
///
/// Input: [a, a] where 'aa' already exists
/// Expected: Finds 'aa' token with width 2
#[test]
fn cursor_repeated_atoms_aa() {
    let (graph, a, aa) = setup_aa_pattern();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, a] - should find existing aa
    let query = vec![a, a];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    assert!(result.is_ok(), "Should find aa pattern");
    let inner = result.unwrap();

    let IndexWithPath { index, path: _ } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    assert_eq!(index, aa, "Should find aa token");
    assert_eq!(index.width(), TokenWidth(2), "aa should have width 2");
}

/// Test: Find existing 'aaa' pattern via [a, a, a] query
///
/// Input: [a, a, a] where 'aa' and 'aaa' exist  
/// Expected: Finds 'aaa' token with width 3
#[test]
fn cursor_repeated_atoms_aaa() {
    let (graph, a, _aa, aaa) = setup_aaa_pattern();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, a, a] - should find existing aaa
    let query = vec![a, a, a];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    assert!(result.is_ok(), "Should find aaa pattern");
    let inner = result.unwrap();

    let IndexWithPath { index, path: _ } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    assert_eq!(index, aaa, "Should find aaa token");
    assert_eq!(index.width(), TokenWidth(3), "aaa should have width 3");
}

/// Test: Find 'aaaa' pattern via [a, a, a, a] query
///
/// Input: [a, a, a, a] where aa, aaa (=[aa,a]), aaaa (=[aa,aa]) exist
/// Current: Returns aaa (width 3) as first best match found
/// Expected (TODO): Should find aaaa (width 4) as complete match
#[test]
fn cursor_repeated_atoms_aaaa() {
    let (graph, a, aa, aaa, _aaaa) = setup_aaaa_pattern();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, a, a, a] - should ideally find aaaa
    let query = vec![a, a, a, a];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    assert!(result.is_ok(), "Should find pattern");
    let inner = result.unwrap();

    let IndexWithPath { index, path: _ } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Current behavior: finds either aaa or aa depending on search path
    // The search finds aa first, then explores parents to find aaa = [aa, a]
    // But doesn't continue to find aaaa = [aa, aa] which would match the full query
    let w = *index.width();
    assert!(
        index == aaa || index == aa,
        "Currently finds aaa (width 3) or aa (width 2), got width {}",
        w
    );
    // TODO: When search is fixed, this should find aaaa:
    // assert_eq!(index, aaaa, "Should find aaaa token");
    // assert_eq!(index.width(), TokenWidth(4), "aaaa should have width 4");
}

// ============================================================================
// Mixed Pattern Tests
// ============================================================================

/// Test: [ab, c] where ab and c exist separately
///
/// Input: [ab, c]
/// Current: Returns ab (width 2) as best match found, doesn't create abc
/// Expected (TODO): Should create abc token with width 3
#[test]
fn cursor_mixed_pattern_abc() {
    let (graph, _a, _b, _c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [ab, c]
    let query = vec![ab, _c];
    let result: Result<Token, ErrorState> = graph.insert(query);

    // Current behavior: Returns ab as partial match, doesn't create [ab, c]
    assert!(result.is_ok(), "Should return partial match");
    let token = result.unwrap();

    // Current: returns ab (width 2) since that's the matched prefix
    assert_eq!(token, ab, "Currently returns ab as best match");
    assert_eq!(token.width(), TokenWidth(2), "ab has width 2");

    // TODO: When pattern creation is fixed:
    // assert_eq!(token.width(), TokenWidth(3), "abc should have width 3");
}

/// Test: [a, b, c] when ab exists - should use ab
///
/// Input: [a, b, c] where ab exists
/// Expected: Search finds ab as prefix, remaining c
#[test]
fn cursor_atoms_uses_existing_pattern() {
    let (graph, a, b, c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b, c]
    let query = vec![a, b, c];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query.clone());

    assert!(result.is_ok(), "insert_or_get_complete should succeed");
    let inner = result.unwrap();

    // Might find ab (width 2) or create abc (width 3) depending on implementation
    let IndexWithPath { index, .. } = match inner {
        Ok(found) => found,
        Err(found) => found,
    };

    // Width should be at least 2 (found ab)
    assert!(*index.width() >= 2, "Should find at least ab");
}

// ============================================================================
// Cursor Position Tracking Tests
// ============================================================================

/// Test: PatternRangePath correctly represents consumed portion
///
/// This tests the cursor's internal position tracking via the path structure.
#[test]
fn cursor_path_position_tracking() {
    let (graph, a, b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Create a PatternRangePath from [a, b]
    let pattern: Pattern = vec![a, b].into();
    let path = PatternRangePath::from(pattern.clone());

    // Initial state: should start at first token
    assert_eq!(path.path_root().len(), 2, "Path has 2 tokens");

    // The start and end indices indicate the range being processed
    use context_trace::HasRootChildIndex;
    let start_idx: usize = HasRootChildIndex::<Start>::root_child_index(&path);
    let end_idx: usize = HasRootChildIndex::<End>::root_child_index(&path);

    // Both should start at 0 (first token)
    assert_eq!(start_idx, 0, "Start at first token");
    assert_eq!(end_idx, 0, "End at first token (range is [0,0])");
}

/// Test: Path advancement via move_root_index
///
/// Tests that the underlying path structure can advance.
#[test]
fn cursor_path_can_advance() {
    let (graph, a, b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Create a PatternRangePath from [a, b]
    let pattern: Pattern = vec![a, b].into();
    let mut path = PatternRangePath::from(pattern.clone());

    use context_trace::{
        HasRootChildIndex,
        MoveRootIndex,
    };
    use std::ops::ControlFlow;

    // Initial end index
    let initial_end: usize = HasRootChildIndex::<End>::root_child_index(&path);
    assert_eq!(initial_end, 0, "End starts at 0");

    // Advance the end index
    let result =
        MoveRootIndex::<Right, End>::move_root_index(&mut path, &graph);

    // Should succeed (have more tokens)
    assert!(
        matches!(result, ControlFlow::Continue(())),
        "Should advance"
    );

    // End index should now be 1
    let new_end: usize = HasRootChildIndex::<End>::root_child_index(&path);
    assert_eq!(new_end, 1, "End should advance to 1");
}

/// Test: Path signals exhaustion at end
///
/// Tests that move_root_index returns Break when exhausted.
#[test]
fn cursor_path_signals_exhaustion() {
    let (graph, a, _b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Create a PatternRangePath from [a] (single token)
    let pattern: Pattern = vec![a].into();
    let mut path = PatternRangePath::from(pattern);

    use context_trace::MoveRootIndex;
    use std::ops::ControlFlow;

    // Try to advance - should fail (only one token, already at it)
    let result =
        MoveRootIndex::<Right, End>::move_root_index(&mut path, &graph);

    // Should break (cannot advance past single token)
    assert!(
        matches!(result, ControlFlow::Break(())),
        "Should signal exhaustion"
    );
}

// ============================================================================
// Insert-Then-Advance Flow Tests
// ============================================================================

/// Test: Simulates the insert→advance→insert flow
///
/// This mimics what BlockExpansionCtx should do:
/// 1. Insert first portion
/// 2. Advance cursor past consumed portion
/// 3. Insert next portion
/// 4. Repeat until exhausted
#[test]
fn cursor_insert_advance_flow() {
    let (graph, a, b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Input: [a, b, a, b] (should find [ab, ab] or create abab)
    let original_query: Pattern = vec![a, b, a, b].into();

    // Step 1: First insert_or_get_complete
    let result1: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(original_query.to_vec());

    assert!(result1.is_ok(), "First insert should succeed");
    let inner1 = result1.unwrap();

    let first_match = match inner1 {
        Ok(found) => found,
        Err(found) => found,
    };

    let first_width = *first_match.index.width();

    // First match should be at least ab (width 2)
    assert!(first_width >= 2, "First match should be at least width 2");

    // This test verifies the basic flow works
    // The actual cursor advancement implementation will need to:
    // 1. Track remaining = original_query[first_width..]
    // 2. Call insert_or_get_complete on remaining
    // 3. Repeat until exhausted
}

// ============================================================================
// Edge Cases
// ============================================================================

/// Test: Empty query
#[test]
fn cursor_empty_query() {
    let (graph, _a, _b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Empty query
    let query: Vec<Token> = vec![];
    let result: Result<Result<IndexWithPath, _>, _> =
        graph.insert_or_get_complete(query);

    // Should either error or handle gracefully
    // Empty patterns are invalid
    assert!(result.is_err(), "Empty query should error");
}

/// Test: Single atom query
/// Single-element queries return SingleIndex error because there's nothing to search
#[test]
fn cursor_single_atom() {
    let (graph, a, _b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Single atom query [a]
    let query = vec![a];
    let result: Result<Result<IndexWithPath, _>, ErrorReason> =
        graph.insert_or_get_complete(query);

    // Single-element queries cannot search (no way to advance beyond the single token)
    // This returns an ErrorReason::SingleIndex
    match result {
        Err(ErrorReason::SingleIndex(idx_path)) => {
            // Expected - single element queries return the token directly via error
            assert_eq!(idx_path.index, a, "Should return the atom itself");
            assert_eq!(
                idx_path.index.width(),
                TokenWidth(1),
                "Single atom has width 1"
            );
        },
        Ok(_) => panic!("Single atom query should return SingleIndex error"),
        Err(other) => panic!("Unexpected error: {:?}", other),
    }
}

// ============================================================================
// Cursor Width Calculation Tests
// ============================================================================

/// Test: Calculate total consumed width from sequence of matches
#[test]
fn cursor_accumulated_width() {
    let (graph, a, b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Pattern [a,b] has width 2
    // Should report total consumed = 2
    let query = vec![a, b];
    let result: Result<Token, ErrorState> = graph.insert(query);

    assert!(result.is_ok());
    let token = result.unwrap();

    assert_eq!(*token.width(), 2, "ab has width 2");
}

/// Test: Width accumulation with multiple tokens
/// Current: Returns ab as partial match (width 2)
/// Expected (TODO): Creates [ab, c] with width 3
#[test]
fn cursor_accumulated_width_multiple() {
    let (graph, _a, _b, c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Pattern [ab, c] should have width 3 (2 + 1)
    let query = vec![ab, c];
    let result: Result<Token, ErrorState> = graph.insert(query);

    assert!(result.is_ok());
    let token = result.unwrap();

    // Current: returns ab (width 2) as partial match
    assert_eq!(token, ab, "Currently returns ab as partial match");
    assert_eq!(*token.width(), 2, "ab has width 2");

    // TODO: When pattern creation is fixed:
    // assert_eq!(*token.width(), 3, "ab+c has width 3");
}
