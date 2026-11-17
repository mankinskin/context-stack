//! Black-box tests: only the public reader API and offline oracle data.
#[allow(dead_code)]
mod support;

use context_trace::{HypergraphRef, Wide};
use support::{load, GraphData};

fn assert_scenario(name: &str) {
    let fixture = load(name);
    let graph = HypergraphRef::default();
    for (index, input) in fixture.corpus.iter().enumerate() {
        let root = context_read::read(&graph, input.chars()).expect("nonempty input");
        assert_eq!(
            graph.index_string(root),
            *input,
            "{name}, step {index}: root content"
        );
        assert_eq!(
            root.width().0,
            input.chars().count(),
            "{name}, step {index}: root width"
        );
    }
    let actual =
        GraphData::from_graph(&graph).unwrap_or_else(|error| panic!("{}: {}", name, error));
    pretty_assertions::assert_eq!(actual, fixture.graph, "{name}, final corpus graph");
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        const SCENARIO_NAMES: &[&str] = &[$(stringify!($name)),*];
        $(
        #[test]
        fn $name() {
            assert_scenario(stringify!($name));
        }
    )*};
}

scenarios!(
    two_chars,
    linear_abc,
    linear_unique,
    linear_space,
    linear_multiple,
    linear_idempotent,
    linear_digits,
    linear_special,
    linear_qwerty,
    linear_wxyz,
    builder,
    repeat_aa,
    repeat_aaa,
    repeat_aaaa,
    repeat_abab,
    repeat_aba,
    repeat_abba,
    repeat_ababab,
    repeat_aabb,
    repeat_aabbaabb,
    repeat_abcabc,
    repeat_abcabcabc,
    repeat_xyzxyzxyz,
    repeat_separated,
    repeat_ab_separated,
    repeat_hello,
    complex_overlap,
    text,
    text_extension,
    sequence,
    infix,
    infix_short,
    loose,
    repeating_known,
    multiple_overlaps,
    cursor_seed,
    connected_closure,
    nested_closure,
    successive_closures,
    owned_tail,
    whole_tail,
    direct_tail,
    canonical_closure,
    nested_complements,
    flat_complement,
    self_overlap,
    accepted_materialization,
    tail_replacement,
    closure_forward_seed,
    closure_nested_seed,
    closure_tail_seed,
);

#[test]
fn empty_input_does_not_change_graph() {
    let graph = HypergraphRef::default();
    assert!(context_read::read(&graph, "".chars()).is_none());
    assert_eq!(graph.vertex_count(), 0);
}

#[test]
fn single_atom_is_reused() {
    let graph = HypergraphRef::default();
    let first = context_read::read(&graph, "x".chars()).unwrap();
    assert_eq!(graph.index_string(first), "x");
    assert_eq!(first.width().0, 1);
    assert_eq!(context_read::read(&graph, "x".chars()), Some(first));
    assert_eq!(graph.vertex_count(), 1);
}

#[test]
fn fixture_manifest_is_covered_and_round_trips() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names = std::collections::BTreeSet::new();
    for scenario in support::scenarios() {
        assert!(names.insert(scenario.name.clone()), "duplicate scenario");
        let fixture = load(&scenario.name);
        assert_eq!(
            fixture.corpus, scenario.inputs,
            "stale fixture: {}",
            scenario.name
        );
        assert_eq!(
            GraphData::from_graph(&support::to_graph(&fixture.graph)).unwrap(),
            fixture.graph
        );
    }
    assert_eq!(
        names.iter().map(String::as_str).collect::<Vec<_>>(),
        SCENARIO_NAMES
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
        "fixture manifest and behavior tests differ",
    );
    let files: std::collections::BTreeSet<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_owned())
        .filter(|name| name != "scenarios")
        .collect();
    assert_eq!(files, names, "missing or orphaned fixtures");
}
