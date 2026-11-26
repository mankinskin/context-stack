//! Interchange decoding and structural validation.

use serde_json::{
    Value,
    json,
};

use crate::positioned::interchange::PositionedGraphSnapshot;

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

/// The old interchange import through one `GraphEdit` (replaced by table
/// construction, crate ownership plan P0.2); kept as the reference.
fn build_with_edit(
    data: &crate::positioned::interchange::PositionedGraphData
) -> (
    crate::positioned::PositionedGraph,
    std::collections::BTreeMap<String, crate::Token>,
) {
    use crate::positioned::{
        Occurrence,
        PositionedGraph,
    };
    let (atoms, mut composites): (Vec<_>, Vec<_>) =
        data.vertices.iter().partition(|(_, cover)| cover.is_empty());
    composites.sort_by_key(|(content, _)| (content.chars().count(), *content));
    let mut graph = PositionedGraph::default();
    let mut tokens = std::collections::BTreeMap::new();
    let mut edit = graph.edit();
    for (content, _) in atoms {
        tokens.insert(content.clone(), edit.atom(content.chars().next().unwrap()));
    }
    for (content, cover) in composites {
        let occurrences = cover
            .iter()
            .map(|(offset, child)| Occurrence::new(*offset, tokens[child]))
            .collect();
        tokens.insert(content.clone(), edit.create(occurrences).unwrap());
    }
    edit.finish();
    (graph, tokens)
}

/// Table construction reproduces the `GraphEdit` import exactly: slots,
/// nodes, widths, parent links, labels, atom map, generation and names, on
/// every graph of the v3, insert and read lanes.
#[test]
fn table_import_equals_edit_import() {
    use crate::positioned::{
        PositionedGraph,
        fixtures::{
            fixture_snapshots,
            insert_dir,
            manifest,
            read_dir,
        },
    };
    let lane = |dir: std::path::PathBuf| {
        manifest(&dir).into_iter().map(move |scenario| {
            let path = dir.join(format!("{}.json", scenario.name));
            let snapshot: PositionedGraphSnapshot =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (scenario.name, snapshot)
        })
    };
    let graphs: Vec<_> = fixture_snapshots()
        .into_iter()
        .chain(lane(insert_dir()))
        .chain(lane(read_dir()))
        .collect();
    assert!(graphs.len() > 1000, "{} graphs", graphs.len());
    for (name, snapshot) in graphs {
        let (expected, expected_names) = build_with_edit(&snapshot.graph);
        let (actual, actual_names) =
            PositionedGraph::from_interchange_named(&snapshot.graph).unwrap();
        assert_eq!(actual.generation, expected.generation, "{name}: generation");
        // Vertex keys are random per vertex on both paths.
        let fields = |graph: &PositionedGraph| {
            graph
                .vertices
                .iter()
                .map(|v| {
                    #[cfg(debug_assertions)]
                    let label = v.label.clone();
                    #[cfg(not(debug_assertions))]
                    let label = ();
                    format!("{:?}", (v.index, v.width, &v.node, &v.parents, label))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(fields(&actual), fields(&expected), "{name}: vertices");
        let sorted = |graph: &PositionedGraph| {
            graph.atoms.iter().map(|(a, i)| (*a, *i)).collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(sorted(&actual), sorted(&expected), "{name}: atoms");
        assert_eq!(actual_names, expected_names, "{name}: names");
    }
}
