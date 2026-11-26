//! The insert lane of the reader fixtures (`test-api`):
//! `context-read/tests/fixtures/v3/insert` holds one oracle graph per insert
//! case (insert plan rev 3, task 3.1.1); its manifest is enumerated from the
//! v3 fixtures by [`insert_manifest`].

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    path::PathBuf,
};

use context_trace::positioned::{
    fixtures::{
        FixtureGraph,
        Scenario,
        fixture_snapshots,
        fixtures_dir,
        manifest,
        snapshot,
    },
    interchange::{
        PositionedGraphData,
        PositionedGraphSnapshot,
    },
};

/// Directory of the insert lane (one oracle graph per insert case).
pub fn insert_dir() -> PathBuf {
    fixtures_dir().join("insert")
}

/// Every substring of at least two atoms of a vertex string of `graph` that
/// is not itself a vertex, sorted by `(width, string)`: the insert queries
/// of one fixture.
///
/// Asserts that each has exactly one minimal host occurrence (G5): an
/// occurrence `(v, x)` that lies in no child occurrence of `v`.
pub fn insert_queries(graph: &PositionedGraphData) -> Vec<String> {
    let vertices: BTreeMap<&str, Vec<char>> = graph
        .vertices
        .keys()
        .map(|key| (key.as_str(), key.chars().collect()))
        .collect();
    let mut queries = BTreeSet::new();
    for chars in vertices.values() {
        for start in 0..chars.len() {
            for end in start + 2..=chars.len() {
                let query: String = chars[start..end].iter().collect();
                if !vertices.contains_key(query.as_str()) {
                    queries.insert(query);
                }
            }
        }
    }
    let mut queries: Vec<String> = queries.into_iter().collect();
    queries.sort_by_key(|query| (query.chars().count(), query.clone()));
    for query in &queries {
        let query_chars: Vec<char> = query.chars().collect();
        let width = query_chars.len();
        let hosts: Vec<(&str, usize)> = vertices
            .iter()
            .filter(|(_, chars)| chars.len() > width)
            .flat_map(|(&key, chars)| {
                let cover = &graph.vertices[key];
                (0..=chars.len() - width)
                    .filter(|&x| chars[x..x + width] == query_chars[..])
                    .filter(|&x| {
                        !cover.iter().any(|(offset, child)| {
                            *offset <= x && x + width <= offset + child.chars().count()
                        })
                    })
                    .map(move |x| (key, x))
            })
            .collect();
        assert_eq!(hosts.len(), 1, "{query:?}: minimal hosts {hosts:?} (G5)");
    }
    queries
}

/// The insert lane manifest, enumerated from the current v3 fixtures: for
/// every scenario and each of its [`insert_queries`] (numbered from 1), the
/// entry `<scenario>__<k>` with inputs `corpus + [query]`.
pub fn insert_manifest() -> Vec<Scenario> {
    fixture_snapshots()
        .into_iter()
        .flat_map(|(name, fixture)| {
            insert_queries(&fixture.graph)
                .into_iter()
                .enumerate()
                .map(move |(k, query)| {
                    let mut inputs = fixture.corpus.clone();
                    inputs.push(query);
                    Scenario {
                        name: format!("{name}__{}", k + 1),
                        inputs,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// One insert case: the base graph, the query to insert and the expected
/// oracle graph of the base corpus plus the query.
pub struct InsertCase {
    /// `<scenario>__<k>`.
    pub name: String,
    pub base: FixtureGraph,
    pub query: String,
    pub expected: FixtureGraph,
}

/// Every entry of the insert lane manifest, with its base v3 fixture.
pub fn insert_cases() -> Vec<InsertCase> {
    let mut bases: BTreeMap<String, PositionedGraphSnapshot> = BTreeMap::new();
    manifest(&insert_dir())
        .into_iter()
        .map(|Scenario { name, mut inputs }| {
            let (scenario, _) = name
                .rsplit_once("__")
                .unwrap_or_else(|| panic!("{name}: not <scenario>__<k>"));
            let base = bases
                .entry(scenario.to_string())
                .or_insert_with(|| snapshot(&fixtures_dir(), scenario))
                .clone();
            let query = inputs.pop().unwrap_or_else(|| panic!("{name}: no inputs"));
            assert_eq!(inputs, base.corpus, "{name}: inputs must extend the base corpus");
            let expected = snapshot(&insert_dir(), &name);
            InsertCase {
                base: FixtureGraph::new(scenario.to_string(), base),
                query,
                expected: FixtureGraph::new(name.clone(), expected),
                name,
            }
        })
        .collect()
}
