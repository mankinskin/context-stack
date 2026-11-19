//! Deliberately malformed data tests the fixture validator, not reader semantics.
//! Each lane is checked against its own manifest: the frozen v2 lane for
//! completeness, the v3 lane also for being in sync with the current `ngrams`
//! oracle.
#[allow(dead_code)]
mod support;

use ngrams::{
    graph::{parse_corpus, Corpus, Status, StatusHandle},
    Cancellation,
};
use std::collections::BTreeSet;
use support::{
    fixture_names,
    legacy::{self, load, seed, token, v2_dir, GraphData},
    positioned::{self, load_v3, v3_dir, PositionedGraphSnapshot},
};

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

#[test]
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

#[test]
fn v3_fixtures_match_manifest_exactly() {
    let mut expected = BTreeSet::new();
    for scenario in positioned::scenarios() {
        assert!(expected.insert(scenario.name), "duplicate scenario");
    }
    assert_eq!(
        fixture_names(&v3_dir()),
        expected,
        "v3 fixtures must be exactly one per scenario"
    );
}

#[test]
fn v3_fixtures_are_valid_and_match_the_oracle() {
    for scenario in positioned::scenarios() {
        let fixture = load_v3(&scenario.name);
        assert_eq!(
            fixture.corpus, scenario.inputs,
            "{}: v3 corpus must equal the scenario inputs",
            scenario.name
        );
        let parsed = parse_corpus(
            Corpus::new(&scenario.name, &scenario.inputs),
            StatusHandle::from(Status::new(&scenario.inputs)),
            Cancellation::None,
        )
        .unwrap_or_else(|error| panic!("{}: oracle failed: {error:?}", scenario.name));
        assert_eq!(
            PositionedGraphSnapshot::new(scenario.inputs, parsed.graph),
            fixture,
            "{}: v3 fixture is out of sync with the oracle; regenerate it",
            scenario.name
        );
    }
}
