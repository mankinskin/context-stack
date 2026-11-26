//! Validated (de)serialization of [`PositionedGraph`] (feature `serde`).

use pretty_assertions::assert_eq;
use serde_json::{
    Value,
    json,
};

use crate::{
    BaseGraphKind,
    positioned::PositionedGraph,
};

type Graph = PositionedGraph<BaseGraphKind>;

/// Same slots, nodes, widths, parents, labels, atoms and generation (keys
/// are fresh on load).
fn assert_identical(
    expected: &Graph,
    actual: &Graph,
    name: &str,
) {
    assert_eq!(actual.generation, expected.generation, "{name}");
    assert_eq!(actual.vertex_count(), expected.vertex_count(), "{name}");
    for (e, a) in expected.vertices.iter().zip(&actual.vertices) {
        assert_eq!(a.index, e.index, "{name}");
        assert_eq!(a.width, e.width, "{name}: {e}");
        assert_eq!(a.node, e.node, "{name}: {e}");
        assert_eq!(a.parents, e.parents, "{name}: {e}");
        #[cfg(debug_assertions)]
        assert_eq!(a.label, e.label, "{name}: {e}");
    }
    assert_eq!(actual.atoms, expected.atoms, "{name}");
    #[cfg(feature = "test-api")]
    {
        let difference =
            crate::positioned::diff(&expected.to_interchange(), &actual.to_interchange());
        assert!(difference.is_empty(), "{name}: {difference}");
    }
    actual.validate().unwrap_or_else(|error| panic!("{name}: {error}"));
}

fn round_trip(
    graph: &Graph,
    name: &str,
) {
    let json = serde_json::to_string(graph).unwrap();
    let from_json: Graph = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("{name}: {error}\n{json}"));
    assert_identical(graph, &from_json, name);
    let bytes = bincode::serialize(graph).unwrap();
    let from_bincode: Graph = bincode::deserialize(&bytes)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    assert_identical(graph, &from_bincode, name);
}

#[cfg(feature = "test-api")]
#[test]
fn every_fixture_round_trips_index_identically() {
    let fixtures = crate::positioned::fixtures::fixture_graphs();
    assert!(!fixtures.is_empty());
    for fixture in fixtures {
        round_trip(&fixture.graph, &fixture.name);
    }
}

/// `ababa` with `aba` at offsets 0 and 2: slots `a`, `b`, `aba`, `ababa`.
fn ababa() -> Value {
    json!({
        "vertices": [
            {"Atom": "a"},
            {"Atom": "b"},
            {"Composite": [[0, 0], [1, 1], [2, 0]]},
            {"Composite": [[0, 2], [2, 2]]},
        ],
        "generation": 3,
    })
}

fn load(value: Value) -> Result<Graph, String> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn with(
    pointer: &str,
    value: Value,
) -> Value {
    let mut doc = ababa();
    *doc.pointer_mut(pointer).unwrap() = value;
    doc
}

fn assert_rejected(
    doc: Value,
    expected: &str,
) {
    match load(doc.clone()) {
        Ok(graph) => panic!("accepted {doc}: {graph:?}"),
        Err(error) => assert!(
            error.contains(expected),
            "error {error:?} for {doc} lacks {expected:?}"
        ),
    }
}

#[test]
fn stored_form_is_nodes_and_generation() {
    let graph = load(ababa()).unwrap();
    assert_eq!(graph.generation, 3);
    assert_eq!(serde_json::to_value(&graph).unwrap(), ababa());
    assert_eq!(graph.vertices[3].width.0, 5);
    #[cfg(debug_assertions)]
    assert_eq!(graph.vertices[3].label, "ababa");
    round_trip(&graph, "ababa");
}

#[test]
fn empty_graph_loads() {
    let graph = load(json!({"vertices": [], "generation": 0})).unwrap();
    assert_eq!(graph.vertex_count(), 0);
    round_trip(&graph, "empty");
}

/// Children with higher indices than their parents (atoms read late).
#[test]
fn children_may_follow_their_parents() {
    let graph = load(json!({
        "vertices": [
            {"Composite": [[0, 3], [1, 2]]},
            {"Composite": [[0, 0], [2, 2]]},
            {"Atom": "c"},
            {"Atom": "a"},
        ],
        "generation": 7,
    }))
    .unwrap();
    assert_eq!(graph.vertices[1].width.0, 3);
    #[cfg(debug_assertions)]
    assert_eq!(graph.vertices[1].label, "acc");
    round_trip(&graph, "late atoms");
}

#[test]
fn corrupted_tables_are_rejected() {
    // Missing child index in an occurrence.
    assert_rejected(with("/vertices/3/Composite/1", json!([2])), "invalid length");
    assert_rejected(with("/vertices/3/Composite/1", json!([2, 9])), "unknown child 9");
    // Offsets: a gap, and an end that overflows.
    assert_rejected(with("/vertices/3/Composite/1", json!([4, 2])), "not covered");
    assert_rejected(
        with("/vertices/3/Composite/1", json!([u64::MAX, 2])),
        "offset out of range",
    );
    assert_rejected(with("/vertices/3/Composite", json!([[0, 2]])), "at least two");
    assert_rejected(with("/vertices/3/Composite", json!([])), "at least two");
    assert_rejected(
        with("/vertices/2/Composite", json!([[1, 1], [0, 0], [2, 0]])),
        "not sorted",
    );
    assert_rejected(
        with("/vertices/3/Composite", json!([[0, 2], [1, 1], [2, 2]])),
        "contained",
    );
    assert_rejected(
        with("/vertices/3/Composite", json!([[0, 2], [0, 2], [2, 2]])),
        "duplicate occurrence",
    );
    let mut duplicate_atom = ababa();
    duplicate_atom["vertices"].as_array_mut().unwrap().push(json!({"Atom": "a"}));
    assert_rejected(duplicate_atom, "duplicate atom a");
    // A vertex that is its own descendant, directly and through another.
    assert_rejected(with("/vertices/2/Composite", json!([[0, 2], [1, 1]])), "cycle");
    assert_rejected(with("/vertices/2/Composite", json!([[0, 3], [1, 1]])), "cycle");
    assert_rejected(with("/vertices/1", json!({"Bogus": 1})), "unknown variant");
}

#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn content_violations_are_rejected() {
    // Two vertices with content `ab`.
    let mut duplicate = ababa();
    let vertices = duplicate["vertices"].as_array_mut().unwrap();
    vertices.push(json!({"Composite": [[0, 0], [1, 1]]}));
    vertices.push(json!({"Composite": [[0, 0], [1, 1]]}));
    assert_rejected(duplicate, "content");
    // `aba@0` and `aba@1` disagree on their overlap (`ba` vs `ab`).
    assert_rejected(with("/vertices/3/Composite", json!([[0, 2], [1, 2]])), "overlap");
}
