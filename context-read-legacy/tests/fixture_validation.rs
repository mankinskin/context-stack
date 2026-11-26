//! Fixture validator tests of the frozen v2 row lane: deliberately
//! malformed data tests the validator, not reader semantics.
#[allow(dead_code)]
mod support;

use std::collections::BTreeSet;
use support::{
    fixture_names,
    legacy::{self, load, seed, token, v2_dir, GraphData},
};

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn canonicalization_rejects_duplicate_content_instead_of_overwriting() {
    let graph = seed("repeat_aaa");
    graph.insert_pattern(vec![token(&graph, "aa"), token(&graph, "a")]);
    assert!(GraphData::from_graph(&graph)
        .unwrap_err()
        .contains("duplicate token content"));
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
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

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn v2_fixtures_match_frozen_manifest_exactly() {
    let mut expected = BTreeSet::new();
    for scenario in legacy::scenarios() {
        assert!(expected.insert(scenario.name.clone()), "duplicate scenario");
        assert_eq!(
            load(&scenario.name).corpus,
            scenario.inputs,
            "stale v2 fixture: {}",
            scenario.name
        );
    }
    assert_eq!(fixture_names(&v2_dir()), expected, "missing or orphaned v2 fixtures");
}
