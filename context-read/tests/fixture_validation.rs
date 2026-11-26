//! Deliberately malformed data tests the fixture validator, not reader semantics.
//! Each lane is checked against its own manifest (the frozen v2 lane is
//! checked in `context-read-legacy`): the positioned lane also for being in
//! sync with the current `ngrams` oracle, the insert lane (`v3/insert`) also for its manifest being the
//! enumeration of the current positioned fixtures, the read lane (`v3/read`)
//! also for its manifest being derived from the positioned manifest.
#[allow(dead_code)]
mod support;

use ngrams::{
    graph::{parse_corpus, Corpus, Status, StatusHandle},
    Cancellation,
};
use std::collections::BTreeSet;
use support::{
    fixture_names,
    positioned::{self, load_positioned, positioned_dir, PositionedGraphSnapshot},
};

#[test]
fn positioned_fixtures_match_manifest_exactly() {
    let mut expected = BTreeSet::new();
    for scenario in positioned::scenarios() {
        assert!(expected.insert(scenario.name), "duplicate scenario");
    }
    assert_eq!(
        fixture_names(&positioned_dir()),
        expected,
        "positioned fixtures must be exactly one per scenario"
    );
}

#[test]
fn positioned_fixtures_are_valid_and_match_the_oracle() {
    for scenario in positioned::scenarios() {
        let fixture = load_positioned(&scenario.name);
        assert_eq!(
            fixture.corpus, scenario.inputs,
            "{}: positioned corpus must equal the scenario inputs",
            scenario.name
        );
        let parsed = parse_corpus(
            Corpus::new(&scenario.name, &scenario.inputs),
            StatusHandle::from(Status::new(&scenario.inputs)),
            Cancellation::None,
        )
        .unwrap_or_else(|error| panic!("{}: oracle failed: {error:?}", scenario.name));
        assert_eq!(
            PositionedGraphSnapshot::new(scenario.inputs, parsed.graph),
            fixture,
            "{}: positioned fixture is out of sync with the oracle; regenerate it",
            scenario.name
        );
    }
}

#[test]
fn insert_fixtures_match_manifest_and_enumeration() {
    use context_insert::{
        fixtures::{
            insert_dir,
            insert_manifest,
        },
    };
    use context_trace::positioned::fixtures::manifest;
    let entries = manifest(&insert_dir());
    let mut expected = BTreeSet::new();
    for entry in &entries {
        assert!(expected.insert(entry.name.clone()), "duplicate insert case");
    }
    assert_eq!(
        fixture_names(&insert_dir()),
        expected,
        "insert fixtures must be exactly one per manifest entry"
    );
    let bases: std::collections::BTreeMap<String, Vec<String>> = positioned::scenarios()
        .into_iter()
        .map(|scenario| (scenario.name, scenario.inputs))
        .collect();
    for entry in &entries {
        let path = insert_dir().join(format!("{}.json", entry.name));
        let fixture: PositionedGraphSnapshot =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        fixture
            .validate()
            .unwrap_or_else(|error| panic!("{}: invalid insert fixture: {error}", entry.name));
        assert_eq!(fixture.corpus, entry.inputs, "{}: corpus must equal the inputs", entry.name);
        let (scenario, _) = entry.name.rsplit_once("__").unwrap();
        let base = &bases[scenario];
        assert!(
            entry.inputs.len() == base.len() + 1 && entry.inputs.starts_with(base),
            "{}: inputs must be the corpus of {scenario} plus the query",
            entry.name
        );
    }
    assert!(
        entries == insert_manifest(),
        "insert manifest differs from the enumeration of the v3 fixtures; regenerate the insert lane"
    );
}

#[test]
fn read_fixtures_match_manifest_and_derivation() {
    use context_trace::positioned::fixtures::manifest;
    use support::{
        read_lane::{
            read_cases,
            read_dir,
            read_manifest,
        },
    };
    let entries = manifest(&read_dir());
    let mut expected = BTreeSet::new();
    for entry in &entries {
        assert!(expected.insert(entry.name.clone()), "duplicate read case");
    }
    assert_eq!(
        fixture_names(&read_dir()),
        expected,
        "read fixtures must be exactly one per manifest entry"
    );
    assert!(
        entries == read_manifest(),
        "read manifest differs from the v3 manifest; regenerate the read lane"
    );
    // Loads every case: corpus = inputs, base = the previous case's graph.
    assert_eq!(read_cases().len(), entries.len());
}

/// Retention invariant G6 (dissolution plan §2.1): every vertex that is
/// neither an atom nor a corpus input has at least two parent occurrences.
fn retention_violations(fixture: &PositionedGraphSnapshot) -> Vec<String> {
    let mut occurrences = std::collections::BTreeMap::<&str, usize>::new();
    for cover in fixture.graph.vertices.values() {
        for (_, child) in cover {
            *occurrences.entry(child.as_str()).or_default() += 1;
        }
    }
    fixture
        .graph
        .vertices
        .iter()
        .filter(|(key, cover)| !cover.is_empty() && !fixture.corpus.contains(key))
        .filter(|(key, _)| occurrences.get(key.as_str()).copied().unwrap_or(0) < 2)
        .map(|(key, _)| key.clone())
        .collect()
}

/// Every oracle graph of all positioned lanes (v3, insert, read) by name.
fn oracle_graphs() -> Vec<(String, PositionedGraphSnapshot)> {
    use context_trace::{
        positioned::{
            fixtures::{
                fixture_snapshots,
                manifest,
            },
        },
    };
    use context_insert::fixtures::insert_dir;
    use support::read_lane::read_cases;
    let mut graphs: Vec<(String, PositionedGraphSnapshot)> = fixture_snapshots();
    graphs.extend(manifest(&insert_dir()).into_iter().map(|entry| {
        let path = insert_dir().join(format!("{}.json", entry.name));
        let fixture = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        (entry.name, fixture)
    }));
    graphs.extend(read_cases().into_iter().map(|case| (case.name, case.expected)));
    graphs
}

/// G6 holds on every oracle graph of all positioned lanes (v3, insert,
/// read).
#[test]
fn every_oracle_graph_satisfies_retention() {
    let graphs = oracle_graphs();
    let violations: Vec<String> = graphs
        .iter()
        .filter_map(|(name, fixture)| {
            let violating = retention_violations(fixture);
            (!violating.is_empty()).then(|| format!("{name}: {violating:?}"))
        })
        .collect();
    assert!(graphs.len() > 1000, "{} oracle graphs", graphs.len());
    assert!(violations.is_empty(), "G6 violated:\n{}", violations.join("\n"));
}

/// A left or right context of an occurrence (dissolution plan C.0): an
/// atom, or the start / end of text `k`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Context {
    Atom(char),
    Boundary(usize),
}

/// The atoms, inputs and maximal repeats of `corpus`, by brute force: every
/// substring of ≥ 2 atoms with ≥ 2 distinct left and ≥ 2 distinct right
/// contexts over its occurrences (C.0).
fn maximal_repeat_vertices(corpus: &[String]) -> BTreeSet<String> {
    use std::collections::BTreeMap;
    let texts: Vec<Vec<char>> = corpus.iter().map(|text| text.chars().collect()).collect();
    let mut contexts = BTreeMap::<&[char], (BTreeSet<Context>, BTreeSet<Context>)>::new();
    for (k, text) in texts.iter().enumerate() {
        for start in 0..text.len() {
            for end in start + 2..=text.len() {
                let (left, right) = contexts.entry(&text[start..end]).or_default();
                left.insert(match start {
                    0 => Context::Boundary(k),
                    _ => Context::Atom(text[start - 1]),
                });
                right.insert(match text.get(end) {
                    Some(&next) => Context::Atom(next),
                    None => Context::Boundary(k),
                });
            }
        }
    }
    let mut vertices: BTreeSet<String> = corpus.iter().cloned().collect();
    vertices.extend(texts.iter().flatten().map(char::to_string));
    vertices.extend(
        contexts
            .into_iter()
            .filter(|(_, (left, right))| left.len() >= 2 && right.len() >= 2)
            .map(|(s, _)| s.iter().collect()),
    );
    vertices
}

/// M1 (dissolution plan C.1): the vertex strings of every oracle graph are
/// exactly the atoms, inputs and maximal repeats of its corpus.
#[test]
fn oracle_vertex_sets_are_maximal_repeats() {
    let graphs = oracle_graphs();
    let failures: Vec<String> = graphs
        .iter()
        .filter_map(|(name, fixture)| {
            let expected = maximal_repeat_vertices(&fixture.corpus);
            let actual: BTreeSet<String> = fixture.graph.vertices.keys().cloned().collect();
            (actual != expected).then(|| {
                format!(
                    "{name}: missing {:?}, extra {:?}",
                    expected.difference(&actual).collect::<Vec<_>>(),
                    actual.difference(&expected).collect::<Vec<_>>()
                )
            })
        })
        .collect();
    assert!(graphs.len() > 1000, "{} oracle graphs", graphs.len());
    assert!(failures.is_empty(), "M1 violated:\n{}", failures.join("\n"));
}
