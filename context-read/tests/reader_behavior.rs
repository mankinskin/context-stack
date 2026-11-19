//! Black-box tests of the positioned reader: only the public
//! `context_read::read` API and the authoritative v3 oracle fixtures
//! (`tests/fixtures/v3`). Each scenario reads all inputs in order into one
//! graph and compares the final graph with the fixture via
//! `context_trace::positioned::diff`.
//!
//! All scenario tests are ignored until the positioned reader exists; the
//! manifest coverage test runs now.
#[allow(dead_code)]
mod support;

use context_trace::positioned::{diff, PositionedGraphRef};
use support::positioned::load_v3;

fn assert_scenario(name: &str) {
    let fixture = load_v3(name);
    let graph = PositionedGraphRef::default();
    for (index, input) in fixture.corpus.iter().enumerate() {
        let root = context_read::read(&graph, input.chars()).expect("nonempty input");
        assert_eq!(
            graph.read().content(root.index).map(|atoms| atoms.into_iter().collect::<String>()).as_ref(),
            Some(input),
            "{name}, step {index}: root content"
        );
    }
    let graph = graph.read();
    graph
        .validate()
        .unwrap_or_else(|error| panic!("{}: invalid graph: {}", name, error));
    let difference = diff(&fixture.graph, &graph.to_interchange());
    assert!(
        difference.is_empty(),
        "{}, final corpus graph: {}",
        name,
        difference
    );
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        const SCENARIO_NAMES: &[&str] = &[$(stringify!($name)),*];
        $(
        #[test]
        #[ignore = "positioned reader pending"]
        #[allow(non_snake_case)] // `<name>__<n>` prefix fixtures
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
    repeat_abcabca,
    repeat_ababa,
    seam_bcd,
    late_retention__1,
    late_retention__2,
    duplicate_observation,
    substring_input,
);

/// Every v3 manifest entry has exactly one scenario test above.
#[test]
fn v3_manifest_is_covered() {
    let mut names = std::collections::BTreeSet::new();
    for scenario in support::positioned::scenarios() {
        assert!(names.insert(scenario.name), "duplicate scenario");
    }
    let tests: std::collections::BTreeSet<String> =
        SCENARIO_NAMES.iter().map(ToString::to_string).collect();
    assert_eq!(tests.len(), SCENARIO_NAMES.len(), "duplicate scenario test");
    assert_eq!(names, tests, "v3 manifest and reader tests differ");
}
