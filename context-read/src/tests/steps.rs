//! Per-step oracle lane (dissolution plan §5.3, cheap stabilization plan
//! §5.1–§5.2): every scenario of the v3 manifest (and every small input
//! over `{a, b}`) is read input by input into one graph; after every settled
//! and stabilized step the stage must be a valid store, satisfy invariant
//! (S), equal the oracle graph of the earlier inputs plus the consumed
//! prefix, follow Rule S′ (R) up to its fixpoint (F), and every stable
//! string must satisfy the conclusion of Theorem S′ on the strings (T).

use std::collections::{
    BTreeMap,
    BTreeSet,
};

use context_trace::{
    positioned::{
        diff,
        fixtures::{
            fixtures_dir,
            manifest,
        },
        GraphRead,
        HasGraph,
        interchange::{
            PositionedGraphData,
            store_interchange,
        },
        PositionedGraphRef,
        string_repr,
        validate_store,
        VertexRead,
    },
    StageIndex,
};
use context_insert::edit::Stage;
use ngrams::{
    Cancellation,
    graph::{
        Corpus,
        Status,
        StatusHandle,
        parse_corpus,
    },
};

use crate::read::{
    StepView,
    read_steps,
};

/// The `ngrams` oracle graph of `inputs`.
fn oracle(
    name: &str,
    inputs: &[String],
) -> PositionedGraphData {
    parse_corpus(
        Corpus::new(name, inputs),
        StatusHandle::from(Status::new(inputs)),
        Cancellation::None,
    )
    .unwrap_or_else(|error| panic!("{}: oracle failed for {:?}: {:?}", name, inputs, error))
    .graph
}

/// A context of an occurrence in a corpus: the neighbouring atom, or the
/// start (left) or end (right) of text `k` (input boundaries are distinct
/// from every atom and from each other).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Context {
    Atom(char),
    Boundary(usize),
}

/// The left contexts of `s` over all its occurrences in `corpus`, and the
/// right contexts over its non-open occurrences (all except the one ending
/// at the end of the last text, the cursor).
fn contexts(
    corpus: &[Vec<char>],
    s: &[char],
) -> (BTreeSet<Context>, BTreeSet<Context>) {
    let last = corpus.len() - 1;
    let (mut left, mut right) = (BTreeSet::new(), BTreeSet::new());
    for (k, text) in corpus.iter().enumerate() {
        for x in 0..(text.len() + 1).saturating_sub(s.len()) {
            if text[x..x + s.len()] != *s {
                continue;
            }
            left.insert(match x {
                0 => Context::Boundary(k),
                _ => Context::Atom(text[x - 1]),
            });
            let end = x + s.len();
            if end < text.len() {
                right.insert(Context::Atom(text[end]));
            } else if k != last {
                right.insert(Context::Boundary(k));
            }
        }
    }
    (left, right)
}

/// The fixed parent occurrences of `vertex` (Rule S′): ending strictly
/// inside the parent, or in a committed or stable parent.
fn fixed_parent_occurrences<'a, V: VertexRead<'a, Index = StageIndex>>(
    stage: &impl Stage,
    vertex: V,
) -> usize {
    let width = vertex.width().0;
    vertex
        .parents()
        .filter(|(parent, offset)| {
            usize::from(*offset) + width < parent.width.0
                || !parent.index.is_staged()
                || stage.is_stable(parent.index)
        })
        .count()
}

/// Counters of the lane.
#[derive(Default)]
struct Totals {
    reads: usize,
    steps: usize,
    dissolved: usize,
    discarded: usize,
    stabilized: usize,
    /// Steps from the first appearance of a staged string to its
    /// stabilization.
    latencies: Vec<usize>,
    /// Staged composites other than the root after the last step of a read
    /// (they survive to the commit), and how many of them are stable.
    survivors: usize,
    stable_at_end: usize,
}

impl Totals {
    fn report(
        &self,
        lane: &str,
    ) {
        let latency = &self.latencies;
        eprintln!(
            "{}: {} reads, {} steps checked, {} dissolved, {} discarded, {} stabilized \
             (latency mean {:.2}, max {}), {} unstable at end, coverage {}/{} = {:.1} %",
            lane,
            self.reads,
            self.steps,
            self.dissolved,
            self.discarded,
            self.stabilized,
            latency.iter().sum::<usize>() as f64 / latency.len().max(1) as f64,
            latency.iter().max().copied().unwrap_or(0),
            self.survivors - self.stable_at_end,
            self.stable_at_end,
            self.survivors,
            100.0 * self.stable_at_end as f64 / self.survivors.max(1) as f64,
        );
    }
}

/// State of one read: when every staged string was first seen, the strings
/// once stable and the counts of the last step.
#[derive(Default)]
struct ReadState {
    step: usize,
    first_seen: BTreeMap<String, usize>,
    stable: BTreeSet<String>,
    survivors: usize,
    stable_now: usize,
}

/// The checks of §5.1 on the stage after one step; `Err` describes the
/// first failing check.
fn check_step(
    name: &str,
    earlier: &[String],
    input: &str,
    view: &StepView<'_, '_>,
    state: &mut ReadState,
    totals: &mut Totals,
) -> Result<(), String> {
    let root = view.root.expect("a step leaves a root");
    let prefix: String = input.chars().take(root.width.0).collect();
    let context = format!(
        "{name}: earlier inputs {earlier:?}, consumed prefix {prefix:?} of {input:?} ({:?})",
        view.step
    );
    let stage = view.handle.graph();
    validate_store(&stage).map_err(|error| format!("{context}: invalid store: {error}"))?;
    let mut corpus: Vec<Vec<char>> =
        earlier.iter().map(|text| text.chars().collect()).collect();
    corpus.push(prefix.chars().collect());
    let mut staged_strings = BTreeSet::new();
    let (mut survivors, mut stable_now) = (0, 0);
    for staged in stage.staged() {
        let index = StageIndex::from(staged);
        let vertex = stage.vertex(index).expect("staged vertex");
        let count = vertex.parent_count();
        let stable = stage.is_stable(index);
        if index == root.index {
            if stable {
                return Err(format!("{context}: the root {index} is stable"));
            }
            continue;
        }
        if vertex.node().composite().is_none() {
            continue;
        }
        if count < 2 {
            return Err(format!(
                "{context}: invariant (S) violated: staged {index} has {count} parent occurrences"
            ));
        }
        survivors += 1;
        let string = string_repr(vertex);
        let fixed = fixed_parent_occurrences(&stage, vertex);
        let first = *state.first_seen.entry(string.clone()).or_insert(state.step);
        if stable {
            stable_now += 1;
            if state.stable.insert(string.clone()) {
                // (R): Rule S′ held when it was stabilized (this step).
                if fixed < 2 {
                    return Err(format!(
                        "{context}: (R) violated: new stable {string:?} has {fixed} fixed parent occurrences"
                    ));
                }
                totals.latencies.push(state.step - first);
            }
            // (T): the conclusion of Theorem S′ on the strings.
            let s: Vec<char> = string.chars().collect();
            let (left, right) = contexts(&corpus, &s);
            let single_atom = matches!(
                right.iter().collect::<Vec<_>>()[..],
                [Context::Atom(_)]
            );
            if left.len() < 2 || single_atom {
                return Err(format!(
                    "{context}: (T) violated: stable {string:?} has left contexts {left:?}, right contexts {right:?}"
                ));
            }
        } else if fixed >= 2 {
            return Err(format!(
                "{context}: (F) violated: unstable {string:?} has {fixed} fixed parent occurrences"
            ));
        }
        staged_strings.insert(string);
    }
    if let Some(lost) = state.stable.iter().find(|s| !staged_strings.contains(*s)) {
        return Err(format!("{context}: stable {lost:?} was removed"));
    }
    state.step += 1;
    state.survivors = survivors;
    state.stable_now = stable_now;
    let expected = oracle(name, &corpus.iter().map(|t| t.iter().collect()).collect::<Vec<String>>());
    let mut actual = store_interchange(&stage);
    // Atoms created by `segments` before they are consumed.
    actual.vertices.retain(|key, cover| {
        !cover.is_empty() || expected.vertices.contains_key(key)
    });
    let difference = diff(&expected, &actual);
    if difference.is_empty() {
        Ok(())
    } else {
        Err(format!("{context}: stage differs from the oracle: {difference}"))
    }
}

/// Read `inputs` in order into one graph; every input from `checked` on is
/// checked at every step (the earlier ones are read unchecked). The first
/// failure.
fn check_inputs(
    name: &str,
    inputs: &[String],
    checked: usize,
    totals: &mut Totals,
) -> Result<(), String> {
    let graph = PositionedGraphRef::default();
    for (k, input) in inputs.iter().enumerate() {
        if k < checked {
            crate::read::read(&graph, input.chars()).expect("nonempty input");
            continue;
        }
        let earlier = &inputs[..k];
        let mut failure = None;
        let mut state = ReadState::default();
        totals.reads += 1;
        read_steps(&graph, input.chars(), |view| {
            totals.steps += 1;
            totals.dissolved += view.step.dissolved;
            totals.discarded += view.step.discarded;
            totals.stabilized += view.step.stabilized;
            if failure.is_none() {
                failure = check_step(name, earlier, input, view, &mut state, totals).err();
            }
        })
        .expect("nonempty input");
        if let Some(failure) = failure {
            return Err(failure);
        }
        totals.survivors += state.survivors;
        totals.stable_at_end += state.stable_now;
        let committed = graph.read();
        let strings: BTreeSet<String> = committed.vertices().map(string_repr).collect();
        if let Some(lost) = state.stable.iter().find(|s| !strings.contains(*s)) {
            return Err(format!(
                "{}: input {:?}: stable {:?} is not committed",
                name, input, lost
            ));
        }
    }
    Ok(())
}

/// Run `check` on every case, catching panics; the failures.
fn failures<T>(
    cases: &[T],
    label: impl Fn(&T) -> String,
    mut check: impl FnMut(&T) -> Result<(), String>,
) -> Vec<String> {
    cases
        .iter()
        .filter_map(|case| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(case)))
                .unwrap_or_else(|_| Err(format!("{}: panicked", label(case))))
                .err()
        })
        .collect()
}

/// Every step of every scenario equals its oracle graph (M4, checked
/// empirically) and is stabilized by Rule S′ up to its fixpoint, soundly
/// (Theorem S′ on the strings).
#[test]
fn every_step_equals_its_oracle_graph() {
    let scenarios = manifest(&fixtures_dir());
    let mut totals = Totals::default();
    let failures = failures(
        &scenarios,
        |scenario| scenario.name.clone(),
        |scenario| check_inputs(&scenario.name, &scenario.inputs, 0, &mut totals),
    );
    totals.report(&format!("per-step lane ({} scenarios)", scenarios.len()));
    assert!(totals.steps > 0);
    assert!(
        failures.is_empty(),
        "{} of {} scenarios fail:\n{}",
        failures.len(),
        scenarios.len(),
        failures.join("\n")
    );
}

/// Every string over `{a, b}` of length `1..=max`.
fn strings(max: usize) -> Vec<String> {
    let mut all = Vec::new();
    let mut layer = vec![String::new()];
    for _ in 0..max {
        layer = layer
            .iter()
            .flat_map(|s| ['a', 'b'].iter().map(move |c| format!("{}{}", s, c)))
            .collect();
        all.extend(layer.iter().cloned());
    }
    all
}

/// §5.2: the checks of §5.1 on small inputs over `{a, b}`: every input up
/// to length `input`, after every corpus of up to `corpus` earlier inputs of
/// length up to `earlier` (only the last input is checked).
fn exhaustive(
    input: usize,
    corpus: usize,
    earlier: usize,
) {
    let inputs = strings(input);
    let texts = strings(earlier);
    let mut corpora: Vec<Vec<String>> = vec![Vec::new()];
    let mut layer = corpora.clone();
    for _ in 0..corpus {
        layer = layer
            .iter()
            .flat_map(|c| {
                texts.iter().map(move |t| {
                    let mut c = c.clone();
                    c.push(t.clone());
                    c
                })
            })
            .collect();
        corpora.extend(layer.iter().cloned());
    }
    let cases: Vec<Vec<String>> = corpora
        .iter()
        .flat_map(|c| {
            inputs.iter().map(move |x| {
                let mut case = c.clone();
                case.push(x.clone());
                case
            })
        })
        .collect();
    let mut totals = Totals::default();
    let failures = failures(
        &cases,
        |case| format!("{:?}", case),
        |case| check_inputs(&format!("{:?}", case), case, case.len() - 1, &mut totals),
    );
    totals.report(&format!(
        "exhaustive lane (inputs ≤ {}, corpora of ≤ {} inputs ≤ {}: {} cases)",
        input,
        corpus,
        earlier,
        cases.len()
    ));
    assert!(
        failures.is_empty(),
        "{} of {} cases fail:\n{}",
        failures.len(),
        cases.len(),
        failures.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// §5.2, the default (sampled) size.
#[test]
fn small_inputs_follow_rule_s_prime() {
    exhaustive(6, 1, 4);
}

/// §5.2, the full size (R6): `cargo test ... steps -- --ignored`.
#[test]
#[ignore = "full enumeration (slow); the default run uses a subset"]
fn small_inputs_follow_rule_s_prime_full() {
    exhaustive(6, 2, 4);
}

