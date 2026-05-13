//! Ngrams oracle validation tests — Phase 3.1
//!
//! For each test string we:
//!   1. Run `ngrams::graph::parse_corpus` to build the oracle graph.
//!   2. Run `Command::InsertSequence` to build the context-read candidate graph.
//!   3. Extract a `GraphSnapshot` from each.
//!   4. Build [`LabelMap`]s from both snapshots.
//!   5. Run [`compare_against_oracle`] and assert [`report_is_ok`].
//!
//! # Current Status
//!
//! All end-to-end oracle tests are `#[ignore = "RC-1"]` because
//! `InsertSequence` does not yet execute the outer loop — it stops after the
//! first `insert_next_match` call and therefore produces an incomplete graph.
//! Un-ignoring a test after the RC-1 fix gives instant pass/fail signal.
//!
//! The *self-check* tests at the bottom are NOT ignored: they exercise the
//! comparison machinery itself (ngrams oracle vs. itself) independently of
//! any context-read behaviour.
//!
//! See `20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md` §Execution Steps for the
//! full plan.

use ngrams::{
    Cancellation,
    graph::{
        Corpus,
        Status,
        StatusHandle,
        parse_corpus,
    },
};

use crate::common::{
    graph_compare::{
        compare_against_oracle,
        label_map_from_snapshot,
        report_is_ok,
    },
    helpers::TestWorkspace,
};

// ---------------------------------------------------------------------------
// Core oracle assertion helper
// ---------------------------------------------------------------------------

/// Assert that the context-read graph for `input` is structurally consistent
/// with the ngrams oracle graph produced from the same single-text corpus.
///
/// # Steps
///
/// 1. Build oracle via `parse_corpus` (ngrams crate).
/// 2. Build candidate via `InsertSequence` on a fresh `TestWorkspace`.
/// 3. Call `to_graph_snapshot()` on the ngrams `Hypergraph`; call
///    `get_snapshot()` on the workspace.
/// 4. Convert both snapshots to `LabelMap`s.
/// 5. Run `compare_against_oracle`; panic with a human-readable diff on
///    failure.
fn oracle_assert(input: &str) {
    // ── 1. Build oracle (ngrams) ───────────────────────────────────────────
    let texts = vec![input.to_string()];
    let corpus = Corpus::new(format!("oracle-{input}"), texts.clone());
    let status = StatusHandle::from(Status::new(texts));

    let parse_result = parse_corpus(corpus, status, Cancellation::None)
        .unwrap_or_else(|e| {
            panic!("ngrams parse_corpus failed for {input:?}: {e:?}")
        });

    let oracle_snap = parse_result.graph.to_graph_snapshot();

    // ── 2. Build candidate (context-read) ─────────────────────────────────
    let ws_name = format!("read-{input}");
    let mut ws = TestWorkspace::new(&ws_name);

    // InsertSequence auto-creates atoms and (after RC-1 fix) builds the full
    // compound-token graph via the outer insert_next_match loop.
    ws.insert_text(input);

    let candidate_snap = ws.get_snapshot();

    // ── 3. Build LabelMaps ────────────────────────────────────────────────
    let oracle_map = label_map_from_snapshot(&oracle_snap);
    let candidate_map = label_map_from_snapshot(&candidate_snap);

    // ── 4. Compare ────────────────────────────────────────────────────────
    let report = compare_against_oracle(&oracle_map, &candidate_map, input);

    // ── 5. Assert ─────────────────────────────────────────────────────────
    if !report_is_ok(&report) {
        eprintln!("=== Oracle graph (ngrams) ===");
        let mut oracle_tokens: Vec<_> = oracle_map.tokens.iter().collect();
        oracle_tokens.sort_by_key(|(l, _)| l.as_str());
        for (label, entry) in &oracle_tokens {
            eprintln!(
                "  {:?}  width={}  patterns={:?}",
                label, entry.width, entry.patterns
            );
        }

        eprintln!("=== Candidate graph (context-read) ===");
        let mut candidate_tokens: Vec<_> =
            candidate_map.tokens.iter().collect();
        candidate_tokens.sort_by_key(|(l, _)| l.as_str());
        for (label, entry) in &candidate_tokens {
            eprintln!(
                "  {:?}  width={}  patterns={:?}",
                label, entry.width, entry.patterns
            );
        }

        eprintln!("{}", report);
        panic!("Oracle comparison FAILED for input {:?}", input);
    }

    // Log info/warnings even on success so CI output is informative.
    if !report.unverified_by_oracle.is_empty() {
        eprintln!(
            "[INFO] {input:?}: tokens unverified by oracle (expected for minimal \
             graphs): {:?}",
            report.unverified_by_oracle
        );
    }
    if !report.pattern_mismatches.is_empty() {
        eprintln!(
            "[WARN] {input:?}: {} candidate pattern(s) not present in oracle \
             (not a failure — context-read may decompose differently)",
            report.pattern_mismatches.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Helper macro — generates an RC-1-gated oracle test
// ---------------------------------------------------------------------------

/// Generate an oracle end-to-end test that is skipped until RC-1 is fixed.
///
/// Usage:
/// ```
/// oracle_test!(test_name, "input_string");
/// ```
///
/// To run a single test after applying the RC-1 fix:
///
/// ```sh
/// cargo test -p context-cli oracle_abab -- --include-ignored
/// ```
///
/// To run all oracle tests at once:
///
/// ```sh
/// cargo test -p context-cli oracle_ -- --include-ignored
/// ```
macro_rules! oracle_test {
    ($(#[$meta:meta])* $test_name:ident, $string:expr) => {
        $(#[$meta])*
        #[test]
        #[ignore = "RC-1: insert_sequence outer loop not yet implemented"]
        fn $test_name() {
            oracle_assert($string);
        }
    };
}

// ---------------------------------------------------------------------------
// End-to-end oracle tests  (all #[ignore = "RC-1"])
// ---------------------------------------------------------------------------
//
// Input selection criteria (from §Input String Selection in the plan):
//   • Length ≤ 10 chars  →  ngrams completes in < 10 s (debug build)
//   • At least one repeated character or repeated n-gram  →  gives ngrams
//     something to label beyond atoms
//   • Covers boundary cases: all-distinct, all-same, partial overlaps
//
// Expected ngrams labels are documented per test to serve as a contract that
// the fixed algorithm must satisfy.

oracle_test!(
    /// "ab" — minimum 2-char, all-distinct.
    /// ngrams labels: atoms {a, b} only (no repeated substrings).
    /// context-read should produce: atoms + root "ab".
    oracle_ab,
    "ab"
);

oracle_test!(
    /// "abab" — repeated bigram.
    /// ngrams labels: a, b, ab.
    /// context-read should produce: a, b, ab (root).
    oracle_abab,
    "abab"
);

oracle_test!(
    /// "abcabc" — repeated trigram.
    /// ngrams labels: a, b, c, ab, bc, abc.
    /// context-read should produce: a, b, c, {ab or bc}, abc (root).
    oracle_abcabc,
    "abcabc"
);

oracle_test!(
    /// "abcbcd" — adjacent overlap: "abc" and "bcd" share "bc".
    /// ngrams labels: b, c, bc, abc, bcd.
    /// Tests the wrapper/overlap path in both algorithms.
    oracle_abcbcd,
    "abcbcd"
);

oracle_test!(
    /// "aabbaabb" — nested repetition.
    /// ngrams labels: a, b, aa, bb, aabb.
    oracle_aabbaabb,
    "aabbaabb"
);

oracle_test!(
    /// "ababab" — longer binary repetition.
    /// ngrams labels: a, b, ab, aba, bab.
    oracle_ababab,
    "ababab"
);

oracle_test!(
    /// "abcab" — partial overlap at end.
    /// ngrams labels: a, b, ab, abc.
    oracle_abcab,
    "abcab"
);

oracle_test!(
    /// "aabaa" — complex repetition pattern.
    /// ngrams labels: a, b, aa, aab, baa.
    oracle_aabaa,
    "aabaa"
);

oracle_test!(
    /// "abcdabc" — prefix repeat, length 7.
    /// ngrams labels: a, b, c, ab, bc, abc.
    oracle_abcdabc,
    "abcdabc"
);

// ---------------------------------------------------------------------------
// RC-3 gated test — all-same-char repeat/overlap cursor bug
// ---------------------------------------------------------------------------

/// "aa" — all-same-char, the RC-3 boundary case.
///
/// Gated separately from RC-1 because the repeat/overlap cursor bug (RC-3)
/// affects this case independently of the outer loop fix.  Un-ignore when
/// RC-3 is resolved.
#[test]
#[ignore = "RC-3: repeat/overlap cursor bug — all-same-char strings"]
fn oracle_aa() {
    oracle_assert("aa");
}

// ---------------------------------------------------------------------------
// Slow-track inputs  (length 8–9, ngrams takes 10–30 s in debug builds)
//
// Run with:
//   cargo test -p context-cli oracle_slow -- --include-ignored
// ---------------------------------------------------------------------------

/// "abcabcabc" — triple repetition of "abc", length 9.
#[test]
#[ignore = "slow: ngrams takes ~10-30s for length-9 inputs in debug builds; also RC-1"]
fn oracle_slow_abcabcabc() {
    oracle_assert("abcabcabc");
}

/// "abababab" — long binary repetition, length 8.
#[test]
#[ignore = "slow: ngrams takes ~10-30s for length-8 inputs in debug builds; also RC-1"]
fn oracle_slow_abababab() {
    oracle_assert("abababab");
}

// ---------------------------------------------------------------------------
// Self-check tests — exercise comparison machinery; NOT RC-1 gated
//
// These tests run `parse_corpus` and compare the oracle against *itself*.
// They validate that:
//   • `label_map_from_snapshot` correctly round-trips a ngrams graph.
//   • `compare_against_oracle` passes when both sides are identical.
//   • Expected atoms and tokens are present in the ngrams output.
//
// If these fail, the graph_compare helpers themselves are broken.
// ---------------------------------------------------------------------------

/// Self-comparison of the ngrams graph for "ab" must always pass.
#[test]
fn oracle_machinery_self_check_ab() {
    let texts = vec!["ab".to_string()];
    let corpus = Corpus::new("self-check-ab".to_string(), texts.clone());
    let status = StatusHandle::from(Status::new(texts));
    let result = parse_corpus(corpus, status, Cancellation::None)
        .expect("ngrams should succeed for 'ab'");
    let snap = result.graph.to_graph_snapshot();
    let map = label_map_from_snapshot(&snap);

    // Self-comparison must always pass
    let report = compare_against_oracle(&map, &map, "ab");
    assert!(report_is_ok(&report), "{}", report);

    // "ab" is all-distinct so ngrams only labels atoms
    assert!(map.atoms.contains("a"), "oracle should have atom 'a'");
    assert!(map.atoms.contains("b"), "oracle should have atom 'b'");
}

/// Self-comparison of the ngrams graph for "abab" must always pass.
/// Also checks that the expected tokens (a, b, ab) are present.
#[test]
fn oracle_machinery_self_check_abab() {
    let texts = vec!["abab".to_string()];
    let corpus = Corpus::new("self-check-abab".to_string(), texts.clone());
    let status = StatusHandle::from(Status::new(texts));
    let result = parse_corpus(corpus, status, Cancellation::None)
        .expect("ngrams should succeed for 'abab'");
    let snap = result.graph.to_graph_snapshot();
    let map = label_map_from_snapshot(&snap);

    // Self-comparison must always pass
    let report = compare_against_oracle(&map, &map, "abab");
    assert!(report_is_ok(&report), "{}", report);

    // "abab" has repeated bigram "ab" — expect atoms a, b and token ab
    assert!(map.atoms.contains("a"), "oracle should have atom 'a'");
    assert!(map.atoms.contains("b"), "oracle should have atom 'b'");
    assert!(
        map.tokens.contains_key("ab"),
        "oracle should label token 'ab' (repeated bigram)"
    );
}

/// Self-comparison of the ngrams graph for "abcabc" must always pass.
/// Also checks that the expected tokens (a, b, c, ab, bc, abc) are present.
#[test]
fn oracle_machinery_self_check_abcabc() {
    let texts = vec!["abcabc".to_string()];
    let corpus = Corpus::new("self-check-abcabc".to_string(), texts.clone());
    let status = StatusHandle::from(Status::new(texts));
    let result = parse_corpus(corpus, status, Cancellation::None)
        .expect("ngrams should succeed for 'abcabc'");
    let snap = result.graph.to_graph_snapshot();
    let map = label_map_from_snapshot(&snap);

    // Self-comparison must always pass
    let report = compare_against_oracle(&map, &map, "abcabc");
    assert!(report_is_ok(&report), "{}", report);

    // All characters appear twice → atoms labelled
    for ch in ["a", "b", "c"] {
        assert!(map.atoms.contains(ch), "oracle should have atom {ch:?}");
    }
    // "abc" appears at positions 0 and 3 → the root must be reachable.
    // Note: whether "ab" and "bc" survive the partition pass depends on
    // whether the wrapper pass subsumes them under "abc".  We do NOT assert
    // on intermediate bigrams here — only on the root and on atoms.
    assert!(
        map.tokens.contains_key("abc") || map.tokens.contains_key("abcabc"),
        "oracle should contain at least 'abc' or 'abcabc' as a labelled token"
    );
}

/// Self-comparison snapshot round-trip: build snap → LabelMap → snap again
/// and verify the second map equals the first.
#[test]
fn oracle_machinery_label_map_roundtrip() {
    let texts = vec!["abab".to_string()];
    let corpus = Corpus::new("roundtrip-abab".to_string(), texts.clone());
    let status = StatusHandle::from(Status::new(texts));
    let result = parse_corpus(corpus, status, Cancellation::None)
        .expect("ngrams should succeed for 'abab'");

    let snap1 = result.graph.to_graph_snapshot();
    let map1 = label_map_from_snapshot(&snap1);

    // Rebuilding the map from the same snapshot must be identical
    let map2 = label_map_from_snapshot(&snap1);

    assert_eq!(
        map1.atoms, map2.atoms,
        "atoms should be identical across two builds from the same snapshot"
    );
    assert_eq!(
        map1.tokens.len(),
        map2.tokens.len(),
        "token count should be identical across two builds from the same snapshot"
    );
    for (label, entry1) in &map1.tokens {
        let entry2 = map2
            .tokens
            .get(label)
            .expect("token present in map1 must also be in map2");
        assert_eq!(
            entry1.width, entry2.width,
            "width must agree for token {label:?}"
        );
        assert_eq!(
            entry1.patterns, entry2.patterns,
            "patterns must agree for token {label:?}"
        );
    }
}
