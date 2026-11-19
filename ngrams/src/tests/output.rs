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
