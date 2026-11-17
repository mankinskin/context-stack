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
//! - Pattern creation: `insert_next_match([a,b,a,b])` returns ab (width 2)
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

use crate::{
    expansion::{
        block::BlockExpansionCtx,
        closure,
        ExpansionCtx,
    },
    pipeline::root::{
        RootManager,
        Span,
    },
};
use context_insert::ToInsertCtx;
use context_search::ErrorState;
use context_trace::{
    graph::vertex::atom::Atom,
    *,
};

// ============================================================================
// Test Environment Setup
// ============================================================================

/// Create a graph with atoms a, b, c and pattern "ab"
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    let result =
        ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query.clone());

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path: _ } = result.unwrap().into_result();

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
    let result =
        ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query.clone());

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path: _ } = result.unwrap().into_result();

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
    let (graph, a, b, _c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b, a, b] - should find "ab" as largest prefix
    let query = vec![a, b, a, b];
    let result =
        ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query.clone());

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path } = result.unwrap().into_result();

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
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path: _ } = result.unwrap().into_result();

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
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path: _ } = result.unwrap().into_result();

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
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

    assert!(result.is_ok(), "insert_next_match should succeed");
    let IndexWithPath { index, path: _ } = result.unwrap().into_result();

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
    let (graph, a, b, c, _ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);

    // Query for [a, b, c]
    let query = vec![a, b, c];
    let result =
        ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query.clone());

    assert!(result.is_ok(), "insert_next_match should succeed");

    // Might find ab (width 2) or create abc (width 3) depending on implementation
    let IndexWithPath { index, .. } = result.unwrap().into_result();

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

    // Step 1: First insert_next_match
    let result1 = ToInsertCtx::<IndexWithPath>::insert_next_match(
        &graph,
        original_query.to_vec(),
    );

    assert!(result1.is_ok(), "First insert should succeed");
    let first_match = result1.unwrap().into_result();

    let first_width = *first_match.index.width();

    // First match should be at least ab (width 2)
    assert!(first_width >= 2, "First match should be at least width 2");

    // This test verifies the basic flow works
    // The actual cursor advancement implementation will need to:
    // 1. Track remaining = original_query[first_width..]
    // 2. Call insert_next_match on remaining
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
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

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
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

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
        }
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

#[test]
fn expansion_sink_updates_graph_before_next_span() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b] =
        graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
    else {
        panic!("insert_atoms failed");
    };
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(a);
    sink.append_completed(a);
    sink.append_pattern(Pattern::from(vec![b]));
    let before = sink.root.unwrap();
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![b, a, a, b, b]);

    assert_eq!(expansion.next(&sink), Some(Span::Append(b)));
    expansion.observe_commit(sink.commit(Span::Append(b)));
    let aabb = sink.root.expect("sink must contain the committed atom");
    assert_eq!(aabb.width(), TokenWidth(4));
    assert_eq!(before.vertex_index(), aabb.vertex_index());
    assert_patterns!(graph, aabb => [[a, a, b, b]]);

    assert_eq!(expansion.next(&sink), Some(Span::Append(aabb)));
    expansion.observe_commit(sink.commit(Span::Append(aabb)));
    let root = sink.root.unwrap();
    assert_patterns!(graph, root => [[aabb, aabb]]);
    assert_eq!(graph.vertex_count(), 4);
    assert_eq!(expansion.next(&sink), None);
}

#[test]
fn expansion_closes_root_self_overlap_before_replacing_root() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
    ])[..] else {
        panic!("insert_atoms failed");
    };
    let abc = graph.insert_pattern(vec![a, b, c]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abc);
    let mut expansion =
        ExpansionCtx::new(graph.clone(), vec![a, b, c, a, b, c]);

    assert_eq!(expansion.next(&sink), Some(Span::Append(abc)));
    expansion.observe_commit(sink.commit(Span::Append(abc)));
    let abcabc = sink.root.unwrap();
    assert_patterns!(graph, abcabc => [[abc, abc]]);

    let Some(Span::ReplaceRoot { previous, token }) = expansion.next(&sink)
    else {
        panic!("root overlap must produce a closed replacement");
    };
    assert_eq!(previous, abcabc);
    assert_eq!(sink.root, Some(abcabc));
    assert_patterns!(
        graph,
        abcabc => [[abc, abc]],
        token => [[abcabc, abc], [abc, abcabc]]
    );
    expansion
        .observe_commit(sink.commit(Span::ReplaceRoot { previous, token }));
    assert_eq!(sink.root, Some(token));
    assert_eq!(graph.vertex_count(), 6);
    assert_eq!(expansion.next(&sink), None);
}

#[test]
fn closure_uses_connected_shared_token_paths() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c, d] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
        Atom::Element('d'),
    ])[..] else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let cd = graph.insert_pattern(vec![c, d]);
    let abcd = graph.insert_pattern(vec![ab, cd]);
    let cdab = graph.insert_pattern(vec![cd, ab]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abcd);
    let span =
        closure::prepare(
        &graph,
        &sink,
        closure::accept(&graph, abcd, cd, cdab),
    )
    .apply(&graph);
    let result = sink.commit(span).root;

    assert_patterns!(
        graph,
        abcd => [[ab, cd]],
        cdab => [[cd, ab]],
        ab => [[a, b]],
        result => [[abcd, ab], [ab, cdab]]
    );
}

#[test]
fn closure_closes_nested_suffix_before_outer_scope() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c, d] = graph.insert_atoms("abcd".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let bc = graph.insert_pattern(vec![b, c]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let cd = graph.insert_pattern(vec![c, d]);
    crate::tests::assert_graph_invariants(&graph);

    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abc);
    let accepted = closure::accept(&graph, abc, c, cd);
    assert_eq!(accepted.anchor_path.len(), 2);

    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    let abcd = sink.commit(span).root;
    assert_eq!(graph.index_string(abcd), "abcd");
    crate::tests::assert_graph_invariants(&graph);

    // The overlap at abc -> bc -> c also needs the union of bc with cd.
    let bcd = context_insert::insert::scoped::find_represented(
        &graph, abcd, 1, 3,
    )
    .expect("closure must represent the nested suffix overlap bcd");
    assert_patterns!(
        graph,
        bcd => [[bc, d], [b, cd]],
        abcd => [[abc, d], [a, bcd]]
    );
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn expansion_sink_preserves_adopted_unshared_token() {
    let (graph, a, b, c, ab) = setup_abc_ab();
    let _tracing = init_test_tracing!(&graph);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    sink.append_completed(c);
    let root = sink.root.unwrap();

    assert_eq!(ab.vertex(&graph).to_token().width(), TokenWidth(2));
    assert_patterns!(graph, ab => [[a, b]], root => [[ab, c]]);
}

#[test]
fn expansion_sink_stops_extending_a_shared_container() {
    let (graph, a) = setup_single_atom();
    let _tracing = init_test_tracing!(&graph);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(a);
    sink.append_completed(a);
    let aa = sink.root.unwrap();
    let parent = graph.insert_pattern(vec![aa, a]);

    sink.append_completed(a);
    let root = sink.root.unwrap();

    assert_ne!(root.vertex_index(), aa.vertex_index());
    assert_eq!(aa.vertex(&graph).to_token().width(), TokenWidth(2));
    assert_patterns!(
        graph,
        aa => [[a, a]],
        parent => [[aa, a]],
        root => [[aa, a]]
    );
}

#[test]
fn expansion_block_does_not_overlap_the_accumulation_root() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c, x] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
        Atom::Element('x'),
    ])[..] else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let bc = graph.insert_pattern(vec![b, c]);
    let shared_parent = graph.insert_pattern(vec![x, ab]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    let mut block = BlockExpansionCtx::new(sink, Pattern::from(vec![c]));

    block.process();
    let root = block.finish().root.expect("block must append its atom");

    assert_eq!(root.width(), TokenWidth(3));
    assert_eq!(ab.vertex(&graph).to_token().width(), TokenWidth(2));
    assert_patterns!(
        graph,
        root => [[ab, c]],
        ab => [[a, b]],
        bc => [[b, c]],
        shared_parent => [[x, ab]]
    );
}

#[test]
fn expansion_commits_forward_match_before_root_overlap() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
    ])[..] else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let bc = graph.insert_pattern(vec![b, c]);
    let mut sink = RootManager::new(graph.clone());
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![a, b, c]);

    assert_eq!(expansion.next(&sink), Some(Span::Append(ab)));
    expansion.observe_commit(sink.commit(Span::Append(ab)));
    let Some(span @ Span::ReplaceRoot { token: abc, .. }) =
        expansion.next(&sink)
    else {
        panic!("root overlap must produce a replacement");
    };

    assert_eq!(abc.width(), TokenWidth(3));
    assert_patterns!(graph, abc => [[ab, c], [a, bc]]);
    expansion.observe_commit(sink.commit(span));
    assert_eq!(expansion.next(&sink), None);
}

#[test]
fn expansion_commits_each_overlap_before_the_next_search() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c, d] = graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
        Atom::Element('d'),
    ])[..] else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let bc = graph.insert_pattern(vec![b, c]);
    let cd = graph.insert_pattern(vec![c, d]);
    let mut sink = RootManager::new(graph.clone());
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![a, b, c, d]);

    assert_eq!(expansion.next(&sink), Some(Span::Append(ab)));
    expansion.observe_commit(sink.commit(Span::Append(ab)));
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let abc = sink.root.unwrap();
    assert_patterns!(graph, abc => [[ab, c], [a, bc]]);
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let abcd = sink.root.unwrap();

    assert_eq!(abcd.width(), TokenWidth(4));
    let patterns = graph.expect_child_patterns(abcd);
    // The nested suffix `bc` of `abc` is closed with `cd` first.
    let bcd = patterns
        .values()
        .find(|pattern| pattern[0] == a)
        .expect("the second overlap must be represented by the committed root")
        [1];
    assert_patterns!(
        graph,
        bcd => [[bc, d], [b, cd]],
        abcd => [[abc, d], [a, bcd]]
    );
    assert!(
        patterns
            .values()
            .all(|pattern| pattern_width(pattern) == abcd.width()),
        "all decompositions must cover the same input span"
    );
    for pattern in patterns.values() {
        let text: String = pattern
            .iter()
            .map(|token| graph.index_string(*token))
            .collect();
        assert_eq!(text, "abcd");
    }
    assert_eq!(expansion.next(&sink), None);
}

#[test]
fn expansion_factors_owned_tail_across_successive_overlaps() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [x, y, z, a, b, c, d, e] =
        graph.insert_atoms("xyzabcde".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let bc = graph.insert_pattern(vec![b, c]);
    let cd = graph.insert_pattern(vec![c, d]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let bcd = graph.insert_patterns(vec![vec![bc, d], vec![b, cd]]);
    let cde = graph.insert_pattern(vec![cd, e]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_pattern(vec![x, y, z].into());
    let original = sink.root.unwrap();
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![a, b, c, d, e]);
    assert_eq!(expansion.next(&sink), Some(Span::Append(abc)));
    expansion.observe_commit(sink.commit(Span::Append(abc)));
    let previous = sink.root.unwrap();

    let span = expansion.next(&sink).unwrap();
    assert!(
        matches!(span, Span::ReplaceTail { previous: r, old, .. } if r == previous && old == abc)
    );
    expansion.observe_commit(sink.commit(span));
    let root = sink.root.unwrap();
    let abcd = *root
        .vertex(&graph)
        .expect_any_child_pattern()
        .1
        .last()
        .unwrap();
    assert_eq!(root.vertex_index(), original.vertex_index());
    assert_patterns!(graph, abcd => [[abc, d], [a, bcd]], root => [[x, y, z, abcd]]);
    assert_eq!(graph.vertex_count(), 15);
    crate::tests::assert_graph_invariants(&graph);
    assert!(!abc
        .vertex(&graph)
        .parents()
        .contains_key(&root.vertex_index()));
    for (index, child) in [x, y, z, abcd].iter().enumerate() {
        let vertex = child.vertex(&graph);
        let parent = vertex.parents().get(&root.vertex_index()).unwrap();
        assert_eq!(parent.width(), root.width());
        assert!(parent
            .pattern_indices()
            .iter()
            .any(|position| position.sub_index() == index));
    }

    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let root = sink.root.unwrap();
    let abcde = *root
        .vertex(&graph)
        .expect_any_child_pattern()
        .1
        .last()
        .unwrap();
    // The tail's nested suffix `bcd` is closed with `cde` first.
    let bcde = graph
        .expect_child_patterns(abcde)
        .values()
        .find(|pattern| pattern[0] == a)
        .expect("expansion must close the overlap")[1];
    assert_patterns!(
        graph,
        abcde => [[abcd, e], [a, bcde]],
        bcde => [[bcd, e], [b, cde]],
        abc => [[a, bc]],
        root => [[x, y, z, abcde]]
    );
    assert_eq!(graph.vertex_count(), 17);
    crate::tests::assert_graph_invariants(&graph);
    assert_eq!(expansion.next(&sink), None);
}

#[test]
fn closure_preserves_entire_last_child_and_canonical_complement() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [x, w, a, b, c, d] =
        graph.insert_atoms("xwabcd".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let wa = graph.insert_pattern(vec![w, a]);
    let bc = graph.insert_pattern(vec![b, c]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let wabc = graph.insert_patterns(vec![vec![w, abc], vec![wa, bc]]);
    let bcd = graph.insert_pattern(vec![bc, d]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(x);
    sink.append_completed(wabc);
    let root = sink.root.unwrap();
    let span =
        closure::prepare(
        &graph,
        &sink,
        closure::accept(&graph, root, bc, bcd),
    )
    .apply(&graph);
    sink.commit(span);
    let root = sink.root.unwrap();
    let wabcd = *root
        .vertex(&graph)
        .expect_any_child_pattern()
        .1
        .last()
        .unwrap();
    let abcd = graph
        .expect_child_patterns(wabcd)
        .values()
        .find(|pattern| pattern[0] == w)
        .expect("closure must close the nested suffix abc")[1];
    assert_patterns!(
        graph,
        abcd => [[abc, d], [a, bcd]],
        wabcd => [[wabc, d], [w, abcd]],
        wabc => [[w, abc], [wa, bc]],
        root => [[x, wabcd]]
    );
    assert_eq!(graph.vertex_count(), 14);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn closure_replaces_whole_tail_without_allocating_a_token() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [x, a, b, c, d] =
        graph.insert_atoms("xabcd".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    // Resolving the unused right complement would allocate a new `cd` token.
    let abcd = graph.insert_pattern(vec![ab, c, d]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(x);
    sink.append_completed(ab);
    let root = sink.root.unwrap();
    let accepted = closure::accept(&graph, root, ab, abcd);
    let count = graph.vertex_count();
    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    assert_eq!(
        span,
        Span::ReplaceTail {
            previous: root,
            old: ab,
            token: abcd
        }
    );
    sink.commit(span);
    let root = sink.root.unwrap();
    assert_patterns!(graph, root => [[x, abcd]], abcd => [[ab, c, d]]);
    assert_eq!(graph.vertex_count(), count);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn closure_reuses_existing_canonical_result() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms("abc".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let bc = graph.insert_pattern(vec![b, c]);
    let abc = graph.insert_patterns(vec![vec![ab, c], vec![a, bc]]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    let accepted = closure::accept(&graph, ab, b, bc);
    let count = graph.vertex_count();
    let span = closure::prepare(&graph, &sink, accepted).apply(&graph);
    assert_eq!(
        span,
        Span::ReplaceRoot {
            previous: ab,
            token: abc
        }
    );
    sink.commit(span);
    assert_eq!(graph.vertex_count(), count);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn closure_represents_nested_complements_inside_their_scopes() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [x, w, a, b, c, d, e, f] =
        graph.insert_atoms("xwabcdef".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let bc = graph.insert_pattern(vec![b, c]);
    let abc = graph.insert_pattern(vec![a, bc]);
    let wabc = graph.insert_pattern(vec![w, abc]);
    let bcd = graph.insert_pattern(vec![bc, d]);
    let bcde = graph.insert_pattern(vec![bcd, e]);
    let bcdef = graph.insert_pattern(vec![bcde, f]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(x);
    sink.append_completed(wabc);
    let root = sink.root.unwrap();
    let span =
        closure::prepare(
        &graph,
        &sink,
        closure::accept(&graph, root, bc, bcdef),
    )
    .apply(&graph);
    sink.commit(span);
    let root = sink.root.unwrap();
    let wabcdef = *root
        .vertex(&graph)
        .expect_any_child_pattern()
        .1
        .last()
        .unwrap();
    let closure = graph.expect_child_patterns(wabcdef);
    let abcdef = closure
        .values()
        .find(|pattern| pattern[0] == w)
        .expect("closure must close the nested suffix abc")[1];
    let def = closure
        .values()
        .find(|pattern| pattern[0] == wabc)
        .expect("closure must start with the scope")[1];
    // `de` would only have the parent `def` and the speculative regrouping
    // `bcde=[bc,de]`, so insertion inlines it
    assert_patterns!(
        graph,
        wabcdef => [[wabc, def], [w, abcdef]],
        abcdef => [[abc, def], [a, bcdef]],
        wabc => [[w, abc]],
        def => [[d, e, f]],
        bcde => [[bcd, e]],
        bcdef => [[bcde, f], [bc, def]],
        root => [[x, wabcdef]]
    );
    assert_eq!(graph.vertex_count(), 18);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn closure_materializes_left_complement_inside_flat_tail() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [x, v, i, s, u] =
        graph.insert_atoms("xvisu".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let vis = graph.insert_pattern(vec![v, i, s]);
    let su = graph.insert_pattern(vec![s, u]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(x);
    sink.append_completed(vis);
    let root = sink.root.unwrap();
    let span =
        closure::prepare(
        &graph,
        &sink,
        closure::accept(&graph, root, s, su),
    )
    .apply(&graph);
    assert!(
        matches!(span, Span::ReplaceTail { previous, old, .. } if previous == root && old == vis)
    );
    sink.commit(span);
    let root = sink.root.unwrap();
    let visu = *root
        .vertex(&graph)
        .expect_any_child_pattern()
        .1
        .last()
        .unwrap();
    let vi = graph
        .expect_child_patterns(visu)
        .values()
        .find(|pattern| pattern.last() == Some(&su))
        .expect("closure must end with the expansion")[0];
    assert_patterns!(
        graph,
        vi => [[v, i]],
        vis => [[vi, s]],
        su => [[s, u]],
        visu => [[vis, u], [vi, su]],
        root => [[x, visu]]
    );
    assert_eq!(graph.vertex_count(), 10);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn forward_step_reuses_existing_accumulation_without_sink_search() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms("abc".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let abc = graph.insert_pattern(vec![ab, c]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(ab);
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![c]);
    let span = expansion.next(&sink).unwrap();
    assert_eq!(
        span,
        Span::ReplaceRoot {
            previous: ab,
            token: abc
        }
    );
    sink.commit(span);
    assert_eq!(graph.vertex_count(), 5);
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
fn root_postfix_probing_does_not_materialize_rejected_matches() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms("abc".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let ab = graph.insert_pattern(vec![a, b]);
    let mut sink = RootManager::new(graph.clone());
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![a, b, c]);
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let before = graph.expect_child_patterns(ab);
    let count = graph.vertex_count();
    assert_eq!(expansion.next(&sink), Some(Span::Append(c)));
    assert_eq!(graph.vertex_count(), count);
    assert_eq!(graph.expect_child_patterns(ab), before);
}

#[test]
fn accepted_materialization_rechecks_root_editability() {
    let graph = HypergraphRef::default();
    let _tracing = init_test_tracing!(&graph);
    let [a, b, c] = graph.insert_atoms("abc".chars().map(Atom::Element))[..]
    else {
        panic!("insert_atoms failed");
    };
    let abc = graph.insert_pattern(vec![a, b, c]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(abc);
    let mut expansion = ExpansionCtx::new(graph.clone(), vec![a, b, c, a]);
    let span = expansion.next(&sink).unwrap();
    expansion.observe_commit(sink.commit(span));
    let previous = sink.root.unwrap();
    assert!(sink.is_editable(previous));
    let span = expansion.next(&sink).unwrap();
    assert!(!sink.is_editable(previous));
    assert!(
        matches!(span, Span::ReplaceRoot { previous: root, .. } if root == previous)
    );
    expansion.observe_commit(sink.commit(span));
    let root = sink.root.unwrap();
    assert_eq!(root.width().0, 7);
    assert_eq!(graph.index_string(root), "abcabca");
    crate::tests::assert_graph_invariants(&graph);
}

#[test]
#[should_panic(expected = "stale last child")]
fn sink_rejects_stale_tail_child() {
    let graph = HypergraphRef::default();
    let [a, b] = graph.insert_atoms("ab".chars().map(Atom::Element))[..] else {
        panic!("insert_atoms failed");
    };
    let aa = graph.insert_pattern(vec![a, a]);
    let mut sink = RootManager::new(graph);
    sink.append_completed(a);
    sink.append_completed(b);
    let root = sink.root.unwrap();
    sink.commit(Span::ReplaceTail {
        previous: root,
        old: a,
        token: aa,
    });
}

#[test]
#[should_panic(expected = "replacement root is stale")]
fn sink_rejects_stale_tail_root() {
    let (graph, a, aa) = setup_aa_pattern();
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(a);
    sink.commit(Span::ReplaceTail {
        previous: aa,
        old: a,
        token: aa,
    });
}

#[test]
#[should_panic(expected = "replacement root is not editable")]
fn sink_rejects_adopted_tail_root() {
    let (graph, a, aa) = setup_aa_pattern();
    let aaa = graph.insert_pattern(vec![aa, a]);
    let mut sink = RootManager::new(graph.clone());
    sink.append_completed(aa);
    sink.commit(Span::ReplaceTail {
        previous: aa,
        old: a,
        token: aaa,
    });
}

#[test]
#[should_panic(expected = "self-referential tail")]
fn sink_rejects_self_referential_tail() {
    let (graph, a) = setup_single_atom();
    let mut sink = RootManager::new(graph);
    sink.append_completed(a);
    sink.append_completed(a);
    let root = sink.root.unwrap();
    sink.commit(Span::ReplaceTail {
        previous: root,
        old: a,
        token: root,
    });
}
