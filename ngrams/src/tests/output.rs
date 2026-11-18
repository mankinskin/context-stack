//! v3 positioned snapshot encoding, decoding, and structural validation.

use std::collections::BTreeSet;

use pretty_assertions::assert_eq;
use serde_json::{
    json,
    Value,
};

use super::composition::reference_cover;
use crate::{
    cancellation::Cancellation,
    graph::{
        parse_corpus,
        Corpus,
        StatusHandle,
    },
    output::{
        PositionedGraphData,
        PositionedGraphSnapshot,
    },
};

fn oracle(texts: &[&str]) -> PositionedGraphSnapshot {
    let graph = parse_corpus(
        Corpus::new("output", texts),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap()
    .graph;
    PositionedGraphSnapshot::new(
        texts.iter().map(ToString::to_string).collect(),
        graph,
    )
}

/// Decode and structurally validate, flattening both failure kinds to text.
fn decode(json: &str) -> Result<PositionedGraphSnapshot, String> {
    let snapshot: PositionedGraphSnapshot =
        serde_json::from_str(json).map_err(|e| e.to_string())?;
    snapshot.validate()?;
    Ok(snapshot)
}

/// Valid `ababa` snapshot with `aba` at two overlapping offsets.
fn ababa() -> Value {
    json!({
        "format_version": 3,
        "corpus": ["ababa"],
        "vertices": {
            "a": [],
            "b": [],
            "aba": [[0, "a"], [1, "b"], [2, "a"]],
            "ababa": [[0, "aba"], [2, "aba"]]
        }
    })
}

fn with(
    pointer: &str,
    value: Value,
) -> String {
    let mut doc = ababa();
    *doc.pointer_mut(pointer).unwrap() = value;
    doc.to_string()
}

fn assert_rejected(
    json: &str,
    expected: &str,
) {
    match decode(json) {
        Ok(snapshot) => panic!("accepted {json}: {snapshot:?}"),
        Err(error) => assert!(
            error.contains(expected),
            "error {error:?} for {json} lacks {expected:?}"
        ),
    }
}

#[test]
fn overlapping_snapshot_round_trips() {
    let snapshot = oracle(&["ababa"]);
    assert_eq!(decode(&ababa().to_string()).unwrap(), snapshot);
    let bytes = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        bytes,
        r#"{"format_version":3,"corpus":["ababa"],"vertices":{"a":[],"aba":[[0,"a"],[1,"b"],[2,"a"]],"ababa":[[0,"aba"],[2,"aba"]],"b":[]}}"#
    );
    assert_eq!(decode(&bytes).unwrap(), snapshot);
}

#[test]
fn identical_inputs_serialize_identically() {
    let texts = ["subdivision", "visualization", "subdivision"];
    let first = serde_json::to_vec_pretty(&oracle(&texts)).unwrap();
    let second = serde_json::to_vec_pretty(&oracle(&texts)).unwrap();
    assert_eq!(first, second);
}

#[test]
fn rejects_wrong_version_and_shape() {
    assert_rejected(&with("/format_version", json!(2)), "format version");
    // A v2 row array is not a positioned entry.
    assert_rejected(&with("/vertices/aba", json!([["a", "b", "a"]])), "invalid type");
    let mut unknown = ababa();
    unknown["unexpected"] = json!(true);
    assert_rejected(&unknown.to_string(), "unknown field");
    let mut missing = ababa();
    missing.as_object_mut().unwrap().remove("corpus");
    assert_rejected(&missing.to_string(), "corpus");
}

#[test]
fn rejects_duplicate_vertex_keys() {
    let json = r#"{"format_version":3,"corpus":["ab"],"vertices":{"a":[],"b":[],"ab":[[0,"a"],[1,"b"]],"a":[]}}"#;
    assert_rejected(json, "duplicate vertex \"a\"");
}

#[test]
fn rejects_malformed_entries() {
    let cases = [
        (
            "/vertices/ababa",
            json!([[0, "aba"], [0, "aba"], [2, "aba"]]),
            "\"ababa\" entry 1 (\"aba\"@0): duplicate entry",
        ),
        (
            "/vertices/ababa",
            json!([[2, "aba"], [0, "aba"]]),
            "\"ababa\" entry 1 (\"aba\"@0): not ordered",
        ),
        (
            "/vertices/ababa",
            json!([[0, "ab"], [2, "aba"]]),
            "\"ababa\" entry 0 (\"ab\"@0): unknown child",
        ),
        (
            "/vertices/ababa",
            json!([[0, "ababa"]]),
            "not narrower",
        ),
        (
            "/vertices/ababa",
            json!([[0, "aba"], [3, "aba"]]),
            "\"ababa\" entry 1 (\"aba\"@3): span out of range",
        ),
        (
            "/vertices/ababa",
            json!([[0, "aba"], [2, "aba"], [usize::MAX, "a"]]),
            "span out of range",
        ),
        (
            "/vertices/aba",
            json!([[0, "b"], [1, "b"], [2, "a"]]),
            "\"aba\" entry 0 (\"b\"@0): span does not match",
        ),
        (
            "/vertices/ababa",
            json!([[0, "aba"]]),
            "\"ababa\": position 3 is not covered",
        ),
        (
            "/vertices/ababa",
            json!([[0, "aba"], [1, "b"], [2, "aba"]]),
            "\"ababa\" entry 1 (\"b\"@1): contained occurrence",
        ),
        (
            "/vertices/ababa",
            json!([[0, "a"], [0, "aba"], [2, "aba"]]),
            "\"ababa\" entry 1 (\"aba\"@0): contained occurrence",
        ),
        ("/vertices/a", json!([[0, "a"]]), "atom \"a\""),
        ("/vertices/aba", json!([]), "\"aba\" has an empty cover"),
        ("/corpus", json!(["ababa", "ab"]), "missing corpus root 1"),
        ("/corpus", json!([]), "corpus must contain"),
        ("/corpus", json!(["ababa", ""]), "corpus text 1 is empty"),
    ];
    for (pointer, value, expected) in cases {
        assert_rejected(&with(pointer, value), expected);
    }
    let mut empty_key = ababa();
    empty_key["vertices"][""] = json!([]);
    assert_rejected(&empty_key.to_string(), "empty vertex content");
}

/// A complete, well-formed cover can still omit a maximal occurrence; only
/// semantic verification against the retained set detects it.
#[test]
fn structural_validation_cannot_detect_omitted_occurrence() {
    let omitted =
        with("/vertices/ababa", json!([[0, "aba"], [3, "b"], [4, "a"]]));
    let snapshot = decode(&omitted).expect("structurally valid");
    let retained: BTreeSet<String> =
        snapshot.graph.vertices.keys().cloned().collect();
    let expected = reference_cover("ababa", &retained);
    assert!(expected.contains(&(2, "aba".to_string())));
    assert!(!snapshot.graph.vertices["ababa"].contains(&(2, "aba".into())));
    assert_ne!(snapshot.graph.vertices["ababa"], expected);
    assert_ne!(snapshot.graph, oracle(&["ababa"]).graph);
}

#[test]
fn oracle_graph_validates_and_keeps_retained_set_only() {
    let snapshot = oracle(&["abcabca"]);
    snapshot.validate().unwrap();
    let PositionedGraphData { vertices } = snapshot.graph;
    assert_eq!(
        vertices.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "abca", "abcabca", "b", "c"]
    );
}
