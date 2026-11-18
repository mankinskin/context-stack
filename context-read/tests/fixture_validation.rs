//! Deliberately malformed data tests the fixture validator, not reader semantics.
//! The v3 checks keep the positioned fixture lane complete and in sync with the
//! current `ngrams` oracle.
#[allow(dead_code)]
mod support;

use ngrams::{
    graph::{parse_corpus, Corpus, Status, StatusHandle},
    Cancellation,
};
use std::collections::BTreeSet;
use support::{load, load_v3, scenarios, seed, token, v3_dir, GraphData, PositionedGraphSnapshot};

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
fn v3_fixtures_match_manifest_exactly() {
    let expected: BTreeSet<String> = scenarios()
        .into_iter()
        .map(|scenario| format!("{}.json", scenario.name))
        .collect();
    let actual: BTreeSet<String> = std::fs::read_dir(v3_dir())
        .expect("missing v3 fixture directory")
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(actual, expected, "v3 fixtures must be exactly one per scenario");
}

#[test]
fn v3_fixtures_are_valid_and_match_the_oracle() {
    for scenario in scenarios() {
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
