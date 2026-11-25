//! Per-step oracle lane (dissolution plan §5.3): every scenario of the v3
//! manifest is read input by input into one graph; after every settled
//! step the stage must be a valid store, satisfy invariant (S) and equal
//! the oracle graph of the earlier inputs plus the consumed prefix.

use context_trace::{
    StageIndex,
    positioned::{
        GraphRead,
        HasGraph,
        PositionedGraphRef,
        Stage,
        VertexRead,
        diff,
        fixtures::{
            Scenario,
            fixtures_dir,
            manifest,
        },
        interchange::{
            PositionedGraphData,
            store_interchange,
        },
        validate_store,
    },
};
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

/// The checks of §5.3 on the stage after one step; `Err` describes the
/// first failing check.
fn check_step(
    name: &str,
    earlier: &[String],
    input: &str,
    view: &StepView<'_, '_>,
) -> Result<(), String> {
    let root = view.root.expect("a step leaves a root");
    let prefix: String = input.chars().take(root.width.0).collect();
    let context = format!(
        "{name}: earlier inputs {earlier:?}, consumed prefix {prefix:?} of {input:?} ({:?})",
        view.step
    );
    let stage = view.handle.graph();
    validate_store(&stage).map_err(|error| format!("{context}: invalid store: {error}"))?;
    for staged in stage.staged() {
        let index = StageIndex::from(staged);
        let vertex = stage.vertex(index).expect("staged vertex");
        let count = vertex.parent_count();
        if index != root.index && vertex.node().composite().is_some() && count < 2 {
            return Err(format!(
                "{context}: invariant (S) violated: staged {index} has {count} parent occurrences"
            ));
        }
    }
    let mut corpus = earlier.to_vec();
    corpus.push(prefix);
    let expected = oracle(name, &corpus);
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

#[derive(Default)]
struct Totals {
    steps: usize,
    dissolved: usize,
    discarded: usize,
}

/// Read the inputs of `scenario` in order into one graph, checking every
/// step; the first failure.
fn check_scenario(
    scenario: &Scenario,
    totals: &mut Totals,
) -> Result<(), String> {
    let graph = PositionedGraphRef::default();
    for (k, input) in scenario.inputs.iter().enumerate() {
        let earlier = &scenario.inputs[..k];
        let mut failure = None;
        read_steps(&graph, input.chars(), |view| {
            totals.steps += 1;
            totals.dissolved += view.step.dissolved;
            totals.discarded += view.step.discarded;
            if failure.is_none() {
                failure = check_step(&scenario.name, earlier, input, view).err();
            }
        })
        .expect("nonempty input");
        if let Some(failure) = failure {
            return Err(failure);
        }
    }
    Ok(())
}

/// Every step of every scenario equals its oracle graph (M4, checked
/// empirically).
#[test]
fn every_step_equals_its_oracle_graph() {
    let scenarios = manifest(&fixtures_dir());
    let mut totals = Totals::default();
    let failures: Vec<String> = scenarios
        .iter()
        .filter_map(|scenario| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check_scenario(scenario, &mut totals)
            }))
            .unwrap_or_else(|_| Err(format!("{}: panicked", scenario.name)))
            .err()
        })
        .collect();
    eprintln!(
        "per-step lane: {} scenarios, {} steps checked, {} dissolved, {} discarded",
        scenarios.len(),
        totals.steps,
        totals.dissolved,
        totals.discarded
    );
    assert!(totals.steps > 0);
    assert!(
        failures.is_empty(),
        "{} of {} scenarios fail:\n{}",
        failures.len(),
        scenarios.len(),
        failures.join("\n")
    );
}
