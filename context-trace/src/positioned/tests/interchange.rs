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
