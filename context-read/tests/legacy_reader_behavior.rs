//! Black-box tests of the legacy (row-model) reader: only the public
//! `context_read::legacy` API and offline v2 oracle data.
//!
//! Known legacy failures (final corpus graph differs from the v2 oracle
//! fixture), marked `#[ignore = "legacy known failure: ..."]` below so that the
//! legacy gate is "all non-ignored tests pass". This list is fixed and checked
//! by review (see `agents/plans/20261116_PLAN_OVERLAP_PROVIDER_MIGRATION.md`):
//! `complex_overlap`, `infix`, `multiple_overlaps`, `nested_complements`,
//! `flat_complement`, `accepted_materialization`.
#[allow(dead_code)]
mod support;

use context_trace::legacy::{HypergraphRef, Wide};
use support::legacy::{load, GraphData};

fn assert_scenario(name: &str) {
    let fixture = load(name);
    let graph = HypergraphRef::default();
    for (index, input) in fixture.corpus.iter().enumerate() {
        let root = context_read::legacy::read(&graph, input.chars()).expect("nonempty input");
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
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        const SCENARIO_NAMES: &[&str] = &[$(stringify!($name)),*];
        $(
        #[test]
        $(#[$attr])*
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
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    complex_overlap,
    text,
    text_extension,
    sequence,
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    infix,
    infix_short,
    loose,
    repeating_known,
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    multiple_overlaps,
    cursor_seed,
    connected_closure,
    nested_closure,
    successive_closures,
    owned_tail,
    whole_tail,
    direct_tail,
    canonical_closure,
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    nested_complements,
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    flat_complement,
    self_overlap,
    #[ignore = "legacy known failure: final graph differs from v2 oracle"]
    accepted_materialization,
    tail_replacement,
    closure_forward_seed,
    closure_nested_seed,
    closure_tail_seed,
);

#[test]
fn empty_input_does_not_change_graph() {
    let graph = HypergraphRef::default();
    assert!(context_read::legacy::read(&graph, "".chars()).is_none());
    assert_eq!(graph.vertex_count(), 0);
}

#[test]
fn single_atom_is_reused() {
    let graph = HypergraphRef::default();
    let first = context_read::legacy::read(&graph, "x".chars()).unwrap();
    assert_eq!(graph.index_string(first), "x");
    assert_eq!(first.width().0, 1);
    assert_eq!(context_read::legacy::read(&graph, "x".chars()), Some(first));
    assert_eq!(graph.vertex_count(), 1);
}

/// Every frozen v2 manifest entry has a test here and round-trips through a
/// row graph. File completeness is checked in `fixture_validation`.
#[test]
fn fixture_manifest_is_covered_and_round_trips() {
    let mut names = std::collections::BTreeSet::new();
    for scenario in support::legacy::scenarios() {
        assert!(names.insert(scenario.name.clone()), "duplicate scenario");
        let fixture = load(&scenario.name);
        assert_eq!(
            GraphData::from_graph(&support::legacy::to_graph(&fixture.graph)).unwrap(),
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
}
