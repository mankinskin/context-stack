//! Black-box tests of the positioned reader: only the public
//! `context_read::read` API and the authoritative positioned oracle fixtures
//! (`tests/fixtures/v3`). Each scenario reads all inputs in order into one
//! graph and compares the final graph with the fixture via
//! `context_trace::positioned::diff`.
//!
//! The read lane (`tests/fixtures/v3/read`, one oracle graph per read of a
//! multi-input scenario) is checked by `read_gate`; the worked examples of
//! the reader plan (§2) also check their step traces. After every read the
//! committed graph is validated, including retention invariant G6 with the
//! inputs read so far (dissolution plan §3.1).
#[allow(dead_code)]
mod support;

use context_read::{Commit, Step, StepKind};
use context_trace::positioned::{
    diff,
    fixtures::{read_cases, ReadCase},
    PositionedGraph, PositionedGraphRef,
};
use support::positioned::load_positioned;

fn assert_scenario(name: &str) {
    let fixture = load_positioned(name);
    let graph = PositionedGraphRef::default();
    let mut inputs = Vec::new();
    for input in &fixture.corpus {
        let root = context_read::read(&graph, input.chars()).expect("nonempty input");
        inputs.push(root.index);
        let committed = graph.read();
        committed
            .validate()
            .unwrap_or_else(|error| panic!("{}: invalid graph after {:?}: {}", name, input, error));
        committed
            .validate_retention(inputs.iter().copied())
            .unwrap_or_else(|error| panic!("{}: G6 violated after {:?}: {}", name, input, error));
    }
    let graph = graph.read();
    let difference = diff(&fixture.graph, &graph.to_interchange());
    assert!(
        difference.is_empty(),
        "{}, final corpus graph: {}",
        name,
        difference
    );
}

/// The scenario list, passed to macro `$m` (names and tests).
macro_rules! with_scenarios {
    ($m:ident) => {
        $m!(
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
            deep_chains,
            deep_prefix_chain,
            deep_suffix_chain,
            deep_overlap_chain,
            deep_rotations,
            deep_fibonacci,
            deep_thue_morse,
            deep_nested_repeat,
            insert_small_periodic,
            insert_shared_vertex,
            insert_periodic,
            insert_deep_borders,
            seam_overlap_append,
            period_a_x5,
            period_a_x6,
            period_a_x7,
            period_a_x8,
            period_ab_x4,
            period_ab_x5,
            period_ab_x6,
            period_abc_x4,
            period_abc_x5,
            period_abc_x6,
            period_abcd_x2,
            period_abcd_x3,
            period_abcd_x4,
            period_abcd_x5,
            period_abcdef_x2,
            period_abcdef_x3,
            period_abcdef_x4,
            period_ab_x3_a,
            period_abc_x3_a,
            period_abc_x3_ab,
            period_abcd_x2_a,
            period_abcd_x3_ab,
            period_abcd_x2_abc,
        );
    };
}

macro_rules! scenario_names {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        const SCENARIO_NAMES: &[&str] = &[$(stringify!($name)),*];
    };
}
with_scenarios!(scenario_names);

/// One `#[test]` per scenario.
macro_rules! scenarios {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {$(
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)] // `<name>__<n>` prefix fixtures
        fn $name() {
            assert_scenario(stringify!($name));
        }
    )*};
}
with_scenarios!(scenarios);

#[test]
fn read_gate() {
    assert_read_gate();
}

/// Every positioned manifest entry has exactly one scenario test above.
#[test]
fn manifest_is_covered() {
    let mut names = std::collections::BTreeSet::new();
    for scenario in support::positioned::scenarios() {
        assert!(names.insert(scenario.name), "duplicate scenario");
    }
    let tests: std::collections::BTreeSet<String> =
        SCENARIO_NAMES.iter().map(ToString::to_string).collect();
    assert_eq!(tests.len(), SCENARIO_NAMES.len(), "duplicate scenario test");
    assert_eq!(names, tests, "positioned manifest and reader tests differ");
}

/// Read `case.input` into its base graph; the differences from the oracle
/// graph, if any.
fn read_case(case: &ReadCase) -> Result<(), String> {
    let (base, mut inputs) = match &case.base {
        Some(base) => {
            let (graph, names) = PositionedGraph::from_interchange_named(&base.graph)
                .map_err(|error| format!("invalid base: {error}"))?;
            let inputs = base
                .corpus
                .iter()
                .map(|input| {
                    names
                        .get(input)
                        .map(|token| token.index)
                        .ok_or_else(|| format!("base input {input:?} is no vertex"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (graph, inputs)
        },
        None => (PositionedGraph::default(), Vec::new()),
    };
    let graph = PositionedGraphRef::new(base);
    let root = context_read::read(&graph, case.input.chars()).ok_or("no root")?;
    inputs.push(root.index);
    let graph = graph.read();
    graph
        .validate()
        .map_err(|error| format!("invalid graph: {error}"))?;
    graph
        .validate_retention(inputs)
        .map_err(|error| format!("G6 violated: {error}"))?;
    if root.width.0 != case.input.chars().count() {
        return Err(format!("root width {}", root.width.0));
    }
    let difference = diff(&case.expected.graph, &graph.to_interchange());
    if difference.is_empty() {
        Ok(())
    } else {
        Err(difference.to_string())
    }
}

/// Every read of the read lane equals its oracle graph (reader plan §9.1,
/// dissolution plan §5.2).
fn assert_read_gate() {
    let cases = read_cases();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    for case in &cases {
        let result = std::panic::catch_unwind(|| read_case(case))
            .unwrap_or_else(|_| Err("panicked".to_string()));
        if let Err(error) = result {
            failures.push((case.name.clone(), error));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} read cases fail:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .map(|(name, error)| format!("{name}: {error}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// No atom is consumed twice by a forward search (reader plan §7 Q4): every
/// step consumes the next atoms after the root, so the steps' ranges are
/// disjoint and cover the input.
#[test]
fn steps_consume_disjoint_ranges() {
    for case in &read_cases() {
        let base = match &case.base {
            Some(base) => PositionedGraph::from_interchange(&base.graph).unwrap(),
            None => PositionedGraph::default(),
        };
        let graph = PositionedGraphRef::new(base);
        let (_, steps) = context_read::read_traced(&graph, case.input.chars());
        let mut consumed = 0;
        for step in &steps {
            consumed += match step.kind {
                StepKind::Unknown => 1,
                StepKind::Forward(width) => width,
                StepKind::Expansion { postfix, expansion } => {
                    assert!(expansion > postfix, "{}: empty expansion", case.name);
                    expansion - postfix
                },
            };
            assert_eq!(step.root, consumed, "{}: {:?}", case.name, step);
        }
        assert_eq!(consumed, case.input.chars().count(), "{}", case.name);
    }
}

/// Read every input of scenario `name` into one graph; the steps of the
/// last read. The final graph must equal the scenario's oracle graph.
fn trace_scenario(name: &str) -> Vec<Step> {
    let fixture = load_positioned(name);
    let graph = PositionedGraphRef::default();
    let (last, earlier) = fixture.corpus.split_last().expect("nonempty corpus");
    for input in earlier {
        context_read::read(&graph, input.chars()).expect("nonempty input");
    }
    let (_, steps) = context_read::read_traced(&graph, last.chars());
    let graph = graph.read();
    graph.validate().unwrap();
    let difference = diff(&fixture.graph, &graph.to_interchange());
    assert!(difference.is_empty(), "{}: {}", name, difference);
    steps
}

/// A step that dissolves and discards nothing.
fn step(kind: StepKind, commit: Commit, root: usize) -> Step {
    Step {
        kind,
        commit,
        root,
        dissolved: 0,
        discarded: 0,
    }
}

/// `xabcdey` on `abcd, bcde`: append `x`; forward `abcd`; expansion
/// `bcd → bcde`, in place; append `y` in place (reader plan §2).
#[test]
fn seam_overlap_append_steps() {
    use {Commit::*, StepKind::*};
    assert_eq!(
        trace_scenario("seam_overlap_append"),
        vec![
            step(Unknown, Adopt, 1),
            step(Forward(4), NewRoot, 5),
            step(Expansion { postfix: 3, expansion: 4 }, InPlace, 6),
            step(Unknown, InPlace, 7),
        ]
    );
}

/// `abcabca` on `abc`: forward `abc`; forward `abc` (new root); expansion
/// `abc → abca`, inserted into the root and integrated in place.
#[test]
fn accepted_materialization_steps() {
    use {Commit::*, StepKind::*};
    assert_eq!(
        trace_scenario("accepted_materialization"),
        vec![
            step(Forward(3), Adopt, 3),
            step(Forward(3), NewRoot, 6),
            step(Expansion { postfix: 3, expansion: 4 }, InPlace, 7),
        ]
    );
}

/// `aaa`: append `a`; forward `a` (new root `aa`); self-expansion
/// `a → aa` (`E` is the root), a new root.
#[test]
fn repeat_aaa_steps() {
    use {Commit::*, StepKind::*};
    assert_eq!(
        trace_scenario("repeat_aaa"),
        vec![
            step(Unknown, Adopt, 1),
            step(Forward(1), NewRoot, 2),
            step(Expansion { postfix: 1, expansion: 2 }, NewRoot, 3),
        ]
    );
}
