//! Tests for repeated/intermediate token patterns
//!
//! These tests verify context-insert's handling of patterns where:
//! - The same token appears multiple times
//! - Intermediate tokens should be created (e.g., "aaa" -> "aa" + "a")
//! - Pattern overlaps need proper resolution

use crate::{
    insert::ToInsertCtx,
    tests::env::EnvSingleAtom,
};
use context_search::{
    ErrorState,
    Find,
    assert_indices,
};
use context_trace::{
    tests::test_case::TestEnv,
    *,
};

/// Test: Repeated pattern with overlapping contexts
///
/// Tests the scenario where a pattern appears multiple times and
/// context-read needs to find the largest enclosing context.
/// This is related to the missing "aa" token in validate_three_repeated.
///
/// For input "aaa", we should get:
/// - 'a' (atom)
/// - 'aa' (pattern [a, a])
/// - 'aaa' (pattern [aa, a] or [a, aa])
#[test]
fn repeated_pattern_creates_intermediate_tokens() {
    let EnvSingleAtom { graph, a } = EnvSingleAtom::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);
    graph.emit_graph_snapshot();

    // First insert "aa"
    let query_aa = vec![a, a];
    let aa_result: Result<Token, ErrorState> = graph.insert(query_aa);
    assert!(aa_result.is_ok(), "Should be able to insert 'aa'");
    let aa = aa_result.unwrap();
    
    assert_eq!(aa.width(), TokenWidth(2), "aa should have width 2");

    // Then insert "aaa" as [aa, a]
    let query_aaa = vec![aa, a];
    let aaa_result: Result<Token, ErrorState> = graph.insert(query_aaa);
    assert!(aaa_result.is_ok(), "Should be able to insert 'aaa'");
    let aaa = aaa_result.unwrap();
    
    assert_eq!(aaa.width(), TokenWidth(3), "aaa should have width 3");

    // Verify the graph has all expected patterns
    assert_indices!(graph, aa);
}

/// Test: Insert [a, aa] variant
///
/// Verifies that [a, aa] creates a token equivalent to [aa, a]
///
/// **Status**: Fails - alternate decompositions don't create searchable patterns.
/// The graph stores [a, aa] but searching for [aa, a] doesn't find it.
/// This reveals that insert doesn't automatically create all equivalent decompositions.
#[test]
fn repeated_pattern_alternate_decomposition() {
    let EnvSingleAtom { graph, a } = EnvSingleAtom::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);
    graph.emit_graph_snapshot();

    // First insert "aa"
    let query_aa = vec![a, a];
    let aa: Token = graph.insert(query_aa).expect("Should insert aa");

    // Insert "aaa" as [a, aa] - alternate decomposition
    let query_aaa = vec![a, aa];
    let aaa: Token = graph.insert(query_aaa).expect("Should insert aaa");

    assert_eq!(aaa.width(), TokenWidth(3), "aaa should have width 3");
    
    // The token should be findable via either decomposition
    let find_result = graph.find_ancestor(vec![aa, a]);
    assert!(
        find_result.is_ok() && find_result.unwrap().query_exhausted(),
        "Should find aaa via [aa, a] after inserting via [a, aa]"
    );
}

/// Test: Four repeated atoms
///
/// For input "aaaa", test hierarchical decomposition:
/// - 'a' (atom)
/// - 'aa' (pattern [a, a])
/// - 'aaa' (pattern [aa, a] or [a, aa])
/// - 'aaaa' (pattern [aa, aa] or [aaa, a] or [a, aaa])
#[test]
fn four_repeated_atoms_hierarchical() {
    let EnvSingleAtom { graph, a } = EnvSingleAtom::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);
    graph.emit_graph_snapshot();

    // Insert aa
    let aa: Token = graph.insert(vec![a, a]).expect("Should insert aa");
    assert_eq!(aa.width(), TokenWidth(2));

    // Insert aaaa as [aa, aa] - most compact representation
    let aaaa: Token = graph.insert(vec![aa, aa]).expect("Should insert aaaa");
    assert_eq!(aaaa.width(), TokenWidth(4));

    // Should be findable
    let find_result = graph.find_ancestor(vec![aa, aa]);
    assert!(
        find_result.is_ok() && find_result.unwrap().query_exhausted(),
        "Should find aaaa via [aa, aa]"
    );
}
