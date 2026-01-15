//! Tests for pattern string conversion functionality
//!
//! These tests verify the `to_pattern_strings` method works correctly
//! for verifying inserted patterns in a hypergraph.

use crate::*;
use std::collections::HashSet;

#[test]
fn pattern_strings_single_pattern() {
    let _tracing = init_test_tracing!();

    let mut graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b});

    insert_patterns!(graph,
        ab => [a, b],
    );

    let g = graph.graph();
    let pats_ab: HashSet<_> = HasVertexData::vertex(ab, &g)
        .to_pattern_strings(g)
        .into_iter()
        .collect();
    let expected_ab: HashSet<_> =
        std::iter::once(vec!["a".to_string(), "b".to_string()]).collect();
    assert_eq!(pats_ab, expected_ab);
}

#[test]
fn pattern_strings_multiple_patterns() {
    let _tracing = init_test_tracing!();

    let mut graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c});

    insert_patterns!(graph,
        ab => [a, b],
    );

    insert_patterns!(graph,
        bc => [b, c],
    );

    insert_patterns!(graph,
        abc => [[ab, c], [a, bc]],
    );

    let g = graph.graph();
    let pats_abc: HashSet<_> = HasVertexData::vertex(abc, &g)
        .to_pattern_strings(g)
        .into_iter()
        .collect();
    let expected_abc: HashSet<_> = vec![
        vec!["ab".to_string(), "c".to_string()],
        vec!["a".to_string(), "bc".to_string()],
    ]
    .into_iter()
    .collect();
    assert_eq!(pats_abc, expected_abc);
}

#[test]
fn pattern_strings_complex_decomposition() {
    let _tracing = init_test_tracing!();

    let mut graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c, d});

    insert_patterns!(graph,
        ab => [a, b],
        cd => [c, d],
    );

    insert_patterns!(graph,
        bcd => [b, cd]
    );

    insert_patterns!(graph,
        abcd => [[ab, cd], [a, bcd]],
    );

    let g = graph.graph();
    let pats: HashSet<_> = HasVertexData::vertex(abcd, &g)
        .to_pattern_strings(g)
        .into_iter()
        .collect();
    let expected: HashSet<_> = vec![
        vec!["ab".to_string(), "cd".to_string()],
        vec!["a".to_string(), "bcd".to_string()],
    ]
    .into_iter()
    .collect();
    assert_eq!(pats, expected);
}
