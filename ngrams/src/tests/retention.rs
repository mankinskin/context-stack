//! Retention monotonicity of the oracle (migration plan 0.5).
//!
//! Across completed inputs the retained set only grows: the retained set of a
//! corpus prefix is a subset of the retained set of every longer prefix. This
//! is empirical evidence (not a proof) that input boundaries are safe
//! publication points for a streaming reader. Within one input it does not
//! hold (see [`retention_is_not_monotone_within_an_input`]), so tokens derived
//! from a partially read input must stay provisional.

use std::collections::{
    BTreeSet,
    HashMap,
};

use itertools::Itertools;

use crate::{
    cancellation::Cancellation,
    graph::{
        parse_corpus,
        Corpus,
        StatusHandle,
    },
    output::Scenario,
};

fn retained(texts: &[String]) -> BTreeSet<String> {
    parse_corpus(
        Corpus::new("retention", texts),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap_or_else(|error| panic!("{texts:?}: {error:?}"))
    .graph
    .vertices
    .into_keys()
    .collect()
}

/// Memoized retained sets of corpus prefixes.
#[derive(Default)]
struct Prefixes(HashMap<Vec<String>, BTreeSet<String>>);

impl Prefixes {
    fn get(
        &mut self,
        texts: &[String],
    ) -> &BTreeSet<String> {
        self.0
            .entry(texts.to_vec())
            .or_insert_with(|| retained(texts))
    }

    /// Assert monotonicity over every completed-input prefix of `corpus`;
    /// returns the number of checked transitions.
    fn check(
        &mut self,
        corpus: &[String],
    ) -> usize {
        for k in 1..corpus.len() {
            let shorter = self.get(&corpus[..k]).clone();
            let longer = self.get(&corpus[..=k]);
            let lost: Vec<_> = shorter.difference(longer).collect();
            assert!(
                lost.is_empty(),
                "{:?} -> {:?} loses {lost:?}",
                &corpus[..k],
                &corpus[..=k]
            );
        }
        corpus.len().saturating_sub(1)
    }
}

/// All words of width `1..=max_width` over `alphabet`.
fn words(
    alphabet: &str,
    max_width: usize,
) -> Vec<String> {
    (1..=max_width)
        .flat_map(|width| {
            (0..width)
                .map(|_| alphabet.chars())
                .multi_cartesian_product()
                .map(String::from_iter)
        })
        .collect()
}

/// Every `length`-text sequence over `words`, checked prefix by prefix.
fn sweep(
    words: &[String],
    length: usize,
) -> usize {
    let mut prefixes = Prefixes::default();
    (0..length)
        .map(|_| words.iter().cloned())
        .multi_cartesian_product()
        .map(|corpus| prefixes.check(&corpus))
        .sum()
}

#[test]
fn retention_is_monotone_across_inputs_of_reader_scenarios() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../context-read/tests/fixtures/v3/scenarios.json"
    );
    let scenarios: Vec<Scenario> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut prefixes = Prefixes::default();
    let checks: usize = scenarios
        .iter()
        .map(|scenario| prefixes.check(&scenario.inputs))
        .sum();
    assert!(checks > 0);
}

#[test]
fn retention_is_monotone_across_inputs_bounded_sweep() {
    // 14 words, 196 two-text corpora.
    assert_eq!(sweep(&words("ab", 3), 2), 196);
}

/// `cargo test --release -p ngrams --lib retention_is_monotone_across_inputs_sweep -- --ignored`.
#[test]
#[ignore = "slow sweep; run in release"]
fn retention_is_monotone_across_inputs_sweep() {
    // 30 words, 27 000 three-text corpora, 54 000 checks.
    assert_eq!(sweep(&words("ab", 4), 3), 54_000);
    // 39 words, 1 521 two-text corpora over three letters.
    assert_eq!(sweep(&words("abc", 3), 2), 1_521);
}

/// Growing one input can drop a retained token: `ab` is retained for `abcab`
/// but not for `abcabc`, which retains `abc` instead.
#[test]
fn retention_is_not_monotone_within_an_input() {
    let prefix = retained(&["abcab".to_string()]);
    let complete = retained(&["abcabc".to_string()]);
    assert!(prefix.contains("ab"));
    assert!(!complete.contains("ab"));
    assert!(complete.contains("abc"));
}
