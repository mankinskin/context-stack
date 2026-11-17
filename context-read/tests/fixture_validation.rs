//! Deliberately malformed data tests the fixture validator, not reader semantics.
#[allow(dead_code)]
mod support;

use support::{load, seed, token, GraphData};

#[test]
fn canonicalization_rejects_duplicate_content_instead_of_overwriting() {
    let graph = seed("repeat_aaa");
    graph.insert_pattern(vec![token(&graph, "aa"), token(&graph, "a")]);
    assert!(GraphData::from_graph(&graph)
        .unwrap_err()
        .contains("duplicate token content"));
}

#[test]
fn fixture_validation_rejects_incorrect_content_and_missing_children() {
    let valid = load("linear_abc").graph;
    let mut invalid = valid.clone();
    invalid
        .vertices
        .insert("abc".into(), [vec!["a".into(), "b".into()]].into());
    assert!(invalid
        .validate()
        .unwrap_err()
        .contains("invalid decomposition"));
    let mut invalid = valid;
    invalid.vertices.remove("b");
    assert!(invalid.validate().unwrap_err().contains("invalid child"));
}
