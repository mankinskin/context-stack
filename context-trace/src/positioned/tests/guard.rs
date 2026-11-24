//! Guard for the binding rule "no content-keyed index": the stored fields
//! of `PositionedGraph` and `PositionedVertex` must match an allowlist of
//! structural fields. Adding a field fails this test, forcing a review
//! against the rule; only `#[cfg(debug_assertions)]` fields may hold content.

const SOURCE: &str = include_str!("../graph.rs");

/// `(name, type, debug_only)` of every field of `struct name` in `SOURCE`.
fn fields(name: &str) -> Vec<(String, String, bool)> {
    let header = format!("pub struct {name}<");
    let start = SOURCE.find(&header).unwrap_or_else(|| panic!("{name} not found"));
    let body = &SOURCE[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
    let mut fields = Vec::new();
    let mut debug_only = false;
    for line in body.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[") {
            debug_only |= line == "#[cfg(debug_assertions)]";
            continue;
        }
        let (field, ty) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("{name}: unparsed line {line:?}"));
        let ty = ty
            .trim()
            .strip_suffix(',')
            .unwrap_or_else(|| panic!("{name}: keep fields on one line: {line:?}"));
        let field = field.trim().rsplit(' ').next().unwrap();
        fields.push((field.to_string(), ty.to_string(), debug_only));
        debug_only = false;
    }
    fields
}

fn assert_structural(
    name: &str,
    allowed: &[(&str, &str)],
    debug_allowed: &[&str],
) {
    let fields = fields(name);
    let stored: Vec<(&str, &str)> = fields
        .iter()
        .filter(|(_, _, debug)| !debug)
        .map(|(field, ty, _)| (field.as_str(), ty.as_str()))
        .collect();
    for (field, ty) in &stored {
        for content in ["String", "str", "Vec<char>", "Vec<AtomOf", "Vec<A>"] {
            assert!(!ty.contains(content), "{name}.{field}: {ty} stores content");
        }
    }
    assert_eq!(stored, allowed, "{name}: review new fields against the binding rules");
    let debug: Vec<&str> = fields
        .iter()
        .filter(|(_, _, debug)| *debug)
        .map(|(field, _, _)| field.as_str())
        .collect();
    assert_eq!(debug, debug_allowed, "{name}: debug-only fields");
}

#[test]
fn positioned_graph_has_no_content_keyed_field() {
    assert_structural(
        "PositionedGraph",
        &[
            ("vertices", "Vec<PositionedVertex<G>>"),
            ("atoms", "HashMap<AtomOf<G>, VertexIndex>"),
            ("generation", "u64"),
            ("stage", "Option<StageId>"),
        ],
        &[],
    );
    assert_structural(
        "PositionedVertex",
        &[
            ("index", "VertexIndex"),
            ("key", "VertexKey"),
            ("width", "TokenWidth"),
            ("state", "VertexState"),
            ("node", "VertexNode<AtomOf<G>>"),
            ("parents", "BTreeMap<VertexIndex, BTreeSet<AtomPosition>>"),
        ],
        &["label"],
    );
}

/// Only `context-insert` and `context-read` may enable `context-trace/edit`
/// (insert plan D8): neither through a dependency's feature list nor
/// through one of their own features.
#[test]
fn only_insertion_and_the_reader_enable_the_edit_feature() {
    const ALLOWED: [&str; 2] = ["context-insert", "context-read"];
    let output = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    assert!(packages.iter().any(|p| p["name"] == "context-trace"));
    let mut users = std::collections::BTreeSet::new();
    for package in packages {
        let name = package["name"].as_str().unwrap();
        let by_dependency = package["dependencies"].as_array().unwrap().iter().any(|dependency| {
            dependency["name"] == "context-trace"
                && dependency["features"].as_array().unwrap().iter().any(|f| f == "edit")
        });
        let by_feature = package["features"].as_object().unwrap().values().any(|enables| {
            enables.as_array().unwrap().iter().any(|f| {
                f == "context-trace/edit" || f == "context-trace?/edit"
            })
        });
        if by_dependency || by_feature {
            users.insert(name);
        }
    }
    for user in &users {
        assert!(ALLOWED.contains(user), "{user} must not enable context-trace/edit");
    }
}
