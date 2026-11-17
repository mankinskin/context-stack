use context_trace::{graph::vertex::atom::Atom, *};
pub use ngrams::output::{GraphData, GraphSnapshot as Fixture, Scenario};
use std::collections::BTreeMap;

pub fn to_graph(data: &GraphData) -> HypergraphRef {
    data.validate().expect("invalid oracle fixture");
    let graph = HypergraphRef::default();
    let mut tokens = BTreeMap::new();
    let mut vertices: Vec<_> = data.vertices.iter().collect();
    vertices.sort_by_key(|(text, _)| (text.chars().count(), *text));
    for (text, patterns) in vertices {
        let token = if patterns.is_empty() {
            graph.insert_atoms([Atom::Element(text.chars().next().unwrap())])[0]
        } else {
            graph.insert_patterns(
                patterns
                    .iter()
                    .map(|pattern| {
                        pattern
                            .iter()
                            .map(|child| tokens[child])
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            )
        };
        tokens.insert(text, token);
    }
    assert_eq!(GraphData::from_graph(&graph).unwrap(), *data);
    graph
}

pub fn scenarios() -> Vec<Scenario> {
    serde_json::from_str(include_str!("../fixtures/scenarios.json"))
        .expect("invalid oracle scenario manifest")
}

pub fn load(name: &str) -> Fixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("{name}.json"));
    let fixture: Fixture = serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    fixture.validate().expect("invalid oracle fixture");
    fixture
}

pub fn token(graph: &HypergraphRef, text: &str) -> Token {
    let matches: Vec<_> = graph
        .vertex_keys()
        .filter(|key| graph.vertex_key_string(key) == text)
        .map(|key| graph.expect_vertex_data(key).to_token())
        .collect();
    assert_eq!(matches.len(), 1, "expected one token for {text:?}");
    matches[0]
}

pub fn seed(name: &str) -> HypergraphRef {
    to_graph(&load(name).graph)
}
