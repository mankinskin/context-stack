//! **Legacy v2 rows** (`tests/fixtures/<name>.json`): [`Fixture`] /
//! [`GraphData`] from `ngrams::output::legacy`. Row-dependent reader and
//! cursor tests stay on this frozen lane until the reader produces
//! positioned covers. Loaded by [`load`] and seeded by [`seed`]/[`to_graph`].
use super::{fixtures_dir, read_json};
use context_trace::legacy::{graph::vertex::atom::Atom, *};
pub use ngrams::output::legacy::{GraphData, GraphSnapshot as LegacyFixture};
use std::collections::BTreeMap;

/// Legacy v2 row fixture; alias of [`LegacyFixture`].
pub type Fixture = LegacyFixture;

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

/// Load and validate a frozen legacy v2 row fixture.
pub fn load(name: &str) -> LegacyFixture {
    let path = fixtures_dir().join(format!("{name}.json"));
    let fixture: LegacyFixture = read_json(&path);
    fixture
        .validate()
        .unwrap_or_else(|error| panic!("{}: invalid legacy fixture: {error}", path.display()));
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
