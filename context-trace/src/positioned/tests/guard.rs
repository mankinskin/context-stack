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
        ],
        &[],
    );
    assert_structural(
        "PositionedVertex",
        &[
            ("index", "VertexIndex"),
            ("key", "VertexKey"),
            ("width", "TokenWidth"),
            ("node", "VertexNode<AtomOf<G>>"),
            ("parents", "BTreeMap<VertexIndex, BTreeSet<AtomPosition>>"),
        ],
        &["label"],
    );
}
