//! Positioned child covers (v3 oracle) against direct substring enumeration.
//!
//! The reference enumerator below deliberately shares nothing with the
//! production recurrence: it lists every proper span of an owner by atom
//! indices, filters by retained content, and prunes contained spans pairwise.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    sync::{
        atomic::AtomicBool,
        Arc,
    },
    time::Instant,
};

use context_trace_legacy::{
    graph::vertex::key::VertexKey,
    HashSet,
};
use itertools::Itertools;
use pretty_assertions::assert_eq;

use crate::{
    cancellation::Cancellation,
    graph::{
        composition::{
            compose,
            compose_with_order,
            TieOrder,
        },
        labelling::LabellingCtx,
        traversal::pass::CancelReason,
        vocabulary::{
            ProcessStatus,
            Vocabulary,
        },
        Corpus,
        StatusHandle,
    },
};

type Table = BTreeMap<String, Vec<(usize, String)>>;

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(ToString::to_string).collect()
}

fn entries(items: &[(usize, &str)]) -> Vec<(usize, String)> {
    items.iter().map(|&(o, c)| (o, c.to_string())).collect()
}

/// Fresh frequency selection followed by composition.
fn selected_table(
    texts: &[&str],
    order: TieOrder,
) -> (BTreeSet<String>, Table) {
    let ctx = LabellingCtx::select_frequency_fresh(
        Corpus::new("composition", texts),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap();
    let retained = ctx
        .labels()
        .iter()
        .map(|k| ctx.vocab().entries[k].ngram.clone())
        .collect();
    let covers = compose_with_order(
        ctx.vocab(),
        ctx.labels(),
        &ctx.status,
        &ctx.cancellation,
        order,
    )
    .unwrap();
    (retained, covers.by_content(ctx.vocab()))
}

fn vocab_of(texts: &[&str]) -> (Vocabulary, StatusHandle) {
    let mut status = StatusHandle::default();
    let vocab =
        Vocabulary::from_corpus(&Corpus::new("composition", texts), &mut status)
            .unwrap();
    (vocab, status)
}

/// Compose with a retained set supplied by content, bypassing selection.
fn supplied_table(
    texts: &[&str],
    retained: &BTreeSet<String>,
    order: TieOrder,
) -> Table {
    let (vocab, status) = vocab_of(texts);
    let keys: HashSet<VertexKey> =
        retained.iter().map(|c| vocab.ids[c.as_str()].key).collect();
    compose_with_order(&vocab, &keys, &status, &Cancellation::None, order)
        .unwrap()
        .by_content(&vocab)
}

/// Maximal proper retained occurrences of `owner`, by direct enumeration.
pub(super) fn reference_cover(
    owner: &str,
    retained: &BTreeSet<String>,
) -> Vec<(usize, String)> {
    let chars = owner.chars().collect_vec();
    let n = chars.len();
    let spans = (0..n)
        .flat_map(|s| (s + 1..=n).map(move |e| (s, e)))
        .filter(|&(s, e)| e - s < n)
        .filter(|&(s, e)| {
            retained.contains(&chars[s..e].iter().collect::<String>())
        })
        .collect_vec();
    spans
        .iter()
        .filter(|&&(s, e)| {
            !spans
                .iter()
                .any(|&(os, oe)| (os, oe) != (s, e) && os <= s && e <= oe)
        })
        .sorted_by_key(|&&(s, e)| (s, e - s))
        .map(|&(s, e)| (s, chars[s..e].iter().collect()))
        .collect()
}

fn reference_table(retained: &BTreeSet<String>) -> Table {
    retained
        .iter()
        .map(|owner| (owner.clone(), reference_cover(owner, retained)))
        .collect()
}

/// Paint children into owner coordinates; require exact, complete coverage.
fn check_reconstruction(table: &Table) -> Result<(), String> {
    for (owner, cover) in table {
        let chars = owner.chars().collect_vec();
        if chars.len() == 1 {
            if !cover.is_empty() {
                return Err(format!("atom {owner:?} has children {cover:?}"));
            }
            continue;
        }
        let mut painted: Vec<Option<char>> = vec![None; chars.len()];
        for (offset, child) in cover {
            let width = child.chars().count();
            if width >= chars.len() || offset + width > chars.len() {
                return Err(format!("{owner:?}: bad entry {child:?}@{offset}"));
            }
            for (i, c) in child.chars().enumerate() {
                if chars[offset + i] != c {
                    return Err(format!(
                        "{owner:?}: {child:?}@{offset} mismatches content"
                    ));
                }
                painted[offset + i] = Some(c);
            }
        }
        if let Some(pos) = painted.iter().position(Option::is_none) {
            return Err(format!("{owner:?}: position {pos} uncovered"));
        }
    }
    Ok(())
}

/// Deterministic order-independent membership hash for supplied subsets.
fn keep(
    seed: u64,
    content: &str,
) -> bool {
    let mut h: u64 = 0xcbf29ce484222325 ^ seed.wrapping_mul(0x9e3779b97f4a7c15);
    for b in content.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    (h >> 32) % 2 == 0
}

/// Atoms, input texts, and a seeded half of the remaining substrings.
fn supplied_retained(
    texts: &[&str],
    seed: u64,
) -> BTreeSet<String> {
    texts
        .iter()
        .flat_map(|t| {
            let chars = t.chars().collect_vec();
            let n = chars.len();
            (0..n)
                .flat_map(move |s| (s + 1..=n).map(move |e| (s, e)))
                .map(move |(s, e)| chars[s..e].iter().collect::<String>())
        })
        .filter(|c| {
            c.chars().count() == 1 || texts.contains(&c.as_str()) || keep(seed, c)
        })
        .collect()
}

fn compare(
    what: &str,
    actual: &Table,
    expected: &Table,
) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        let diff = actual
            .keys()
            .chain(expected.keys())
            .unique()
            .filter(|k| actual.get(*k) != expected.get(*k))
            .map(|k| {
                format!(
                    "  {k:?}: got {:?}, expected {:?}",
                    actual.get(k),
                    expected.get(k)
                )
            })
            .join("\n");
        Err(format!("{what}:\n{diff}"))
    }
}

/// Full oracle checks for one corpus: selection, references, permutations.
fn verify(
    texts: &[&str],
    seeds: std::ops::Range<u64>,
) -> Result<(), String> {
    let (retained, table) = selected_table(texts, TieOrder::Forward);
    compare("selected vs reference", &table, &reference_table(&retained))?;
    check_reconstruction(&table)?;
    let (_, reversed) = selected_table(texts, TieOrder::Reversed);
    compare("reversed tie order", &reversed, &table)?;
    let supplied = seeds
        .map(|seed| supplied_retained(texts, seed))
        .collect_vec();
    for r in &supplied {
        let t = supplied_table(texts, r, TieOrder::Forward);
        compare(&format!("supplied {r:?}"), &t, &reference_table(r))?;
        check_reconstruction(&t)?;
    }
    for perm in texts.iter().copied().permutations(texts.len()).unique() {
        let (r, t) = selected_table(&perm, TieOrder::Forward);
        if r != retained {
            return Err(format!(
                "retained differs for {perm:?}: {r:?} vs {retained:?}"
            ));
        }
        compare(&format!("permutation {perm:?}"), &t, &table)?;
        for r in &supplied {
            compare(
                &format!("supplied permutation {perm:?}"),
                &supplied_table(&perm, r, TieOrder::Reversed),
                &reference_table(r),
            )?;
        }
    }
    Ok(())
}

fn strings_over(
    alphabet: &[char],
    widths: std::ops::RangeInclusive<usize>,
) -> Vec<String> {
    widths
        .flat_map(|w| {
            (0..w)
                .map(|_| alphabet.iter().copied())
                .multi_cartesian_product()
                .map(|cs| cs.into_iter().collect::<String>())
        })
        .collect()
}

fn run_all<'a>(
    corpora: impl IntoIterator<Item = Vec<&'a str>>,
    seeds: std::ops::Range<u64>,
) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut failures = Vec::new();
    for texts in corpora {
        count += 1;
        if let Err(e) = verify(&texts, seeds.clone()) {
            failures.push(format!("{texts:?}: {e}"));
        }
    }
    (count, failures)
}

#[test]
fn abcabca_keeps_both_overlapping_occurrences() {
    let (retained, table) = selected_table(&["abcabca"], TieOrder::Forward);
    assert_eq!(retained, set(&["a", "b", "c", "abca", "abcabca"]));
    for absent in ["abc", "bca", "bc", "abcabc", "bcabca"] {
        assert!(!table.contains_key(absent), "{absent}");
    }
    assert_eq!(table["abca"], entries(&[(0, "a"), (1, "b"), (2, "c"), (3, "a")]));
    assert_eq!(table["abcabca"], entries(&[(0, "abca"), (3, "abca")]));
    assert_eq!(table["a"], vec![]);
}

#[test]
fn ababa_keeps_both_overlapping_occurrences() {
    let (retained, table) = selected_table(&["ababa"], TieOrder::Forward);
    assert_eq!(retained, set(&["a", "b", "aba", "ababa"]));
    assert_eq!(table["aba"], entries(&[(0, "a"), (1, "b"), (2, "a")]));
    assert_eq!(table["ababa"], entries(&[(0, "aba"), (2, "aba")]));
}

#[test]
fn fixed_retention_keeps_redundant_middle_occurrence() {
    let retained = set(&["a", "aa", "aaaa"]);
    for order in [TieOrder::Forward, TieOrder::Reversed] {
        let table = supplied_table(&["aaaa"], &retained, order);
        assert_eq!(table["aaaa"], entries(&[(0, "aa"), (1, "aa"), (2, "aa")]));
        assert_eq!(table["aa"], entries(&[(0, "a"), (1, "a")]));
        assert_eq!(table, reference_table(&retained));
    }
}

#[test]
fn substring_root_is_composed_in_both_orders() {
    for texts in [["abc", "xabc"], ["xabc", "abc"]] {
        let (retained, table) = selected_table(&texts, TieOrder::Forward);
        // Every substring of `abc` occurs exactly twice, once per input, so
        // no ancestor with strictly fewer occurrences justifies `ab`/`bc`;
        // `x`-prefixed windows occur once and are below the roots only.
        assert_eq!(retained, set(&["a", "b", "c", "x", "abc", "xabc"]));
        assert_eq!(table["abc"], entries(&[(0, "a"), (1, "b"), (2, "c")]));
        assert_eq!(table["xabc"], entries(&[(0, "x"), (1, "abc")]));
    }
}

#[test]
fn subdivision_visualization_is_order_independent() {
    let forward = ["subdivision", "visualization"];
    let backward = ["visualization", "subdivision"];
    let (retained, table) = selected_table(&forward, TieOrder::Forward);
    assert!(retained.contains("vis"));
    assert!(!retained.contains("vi"));
    assert_eq!(table["vis"], entries(&[(0, "v"), (1, "i"), (2, "s")]));
    // Same retained content and covers under reversed corpus order...
    let (r, t) = selected_table(&backward, TieOrder::Forward);
    assert_eq!(r, retained);
    assert_eq!(t, table);
    // ...and under reversed same-width processing with selection fixed.
    for texts in [forward, backward] {
        let (r, t) = selected_table(&texts, TieOrder::Reversed);
        assert_eq!(r, retained);
        assert_eq!(t, table);
        assert_eq!(supplied_table(&texts, &retained, TieOrder::Reversed), table);
    }
    assert_eq!(table, reference_table(&retained));
}

#[test]
fn covers_reconstruct_every_owner() {
    let corpora: [&[&str]; 6] = [
        &["abcabca"],
        &["ababa"],
        &["subdivision", "visualization"],
        &["é🙂é", "🙂é🙂"],
        &["aabbaabbaa"],
        &["abab", "abcabc", "babc", "ab"],
    ];
    for texts in corpora {
        let (retained, table) = selected_table(texts, TieOrder::Forward);
        assert_eq!(table.keys().cloned().collect::<BTreeSet<_>>(), retained);
        check_reconstruction(&table).unwrap();
        assert_eq!(table, reference_table(&retained), "{texts:?}");
    }
}

#[test]
fn multibyte_offsets_count_atoms() {
    let retained = set(&["é", "🙂", "é🙂", "é🙂é"]);
    let table = supplied_table(&["é🙂é"], &retained, TieOrder::Forward);
    assert_eq!(table["é🙂é"], entries(&[(0, "é🙂"), (2, "é")]));
}

#[test]
fn generated_binary_strings() {
    let strings = strings_over(&['a', 'b'], 1..=8);
    assert_eq!(strings.len(), 510);
    let (count, failures) =
        run_all(strings.iter().map(|s| vec![s.as_str()]), 0..1);
    assert_eq!(count, 510);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Small deterministic multisets of one to three texts, including repeats.
fn bounded_corpora() -> Vec<Vec<&'static str>> {
    vec![
        vec!["ab", "ab"],
        vec!["aba", "ab"],
        vec!["ab", "ba", "aba"],
        vec!["abab", "bab", "abab"],
        vec!["aab", "abb", "ab"],
        vec!["abc", "bca", "cab"],
        vec!["abcab", "cab"],
        vec!["abcabc", "bcabca", "a"],
        vec!["aaa", "aa", "aaa"],
        vec!["acbca", "cbc", "bcb"],
        vec!["abba", "baab"],
        vec!["cabcab", "abcabc", "bcabca"],
    ]
}

#[test]
fn generated_small_corpora() {
    let (_, failures) = run_all(bounded_corpora(), 0..3);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compose_respects_pre_cancelled_handle() {
    let (vocab, status) = vocab_of(&["abcabca"]);
    let retained: HashSet<VertexKey> =
        vocab.roots.iter().chain(&vocab.leaves).map(|id| id.key).collect();
    let cancelled = Cancellation::from(Arc::new(AtomicBool::new(true)));
    assert!(matches!(
        compose(&vocab, &retained, &status, &cancelled),
        Err(CancelReason::Cancelled)
    ));
    assert_eq!(*status.pass(), ProcessStatus::Containment);

    let covers = compose(&vocab, &retained, &status, &Cancellation::None)
        .unwrap();
    assert_eq!(covers.covers.len(), retained.len());
    assert_eq!(*status.pass(), ProcessStatus::Compositions);
    assert_eq!(*status.steps(), *status.steps_total());

    let mut ctx = LabellingCtx::select_frequency_fresh(
        Corpus::new("composition", ["abcabca"]),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap();
    ctx.cancellation = cancelled;
    assert!(matches!(ctx.compose(), Err(CancelReason::Cancelled)));
}

#[test]
fn compose_rejects_unknown_retained_identity() {
    let (vocab, status) = vocab_of(&["ab"]);
    let (other, _) = vocab_of(&["xyz"]);
    // Vertex keys are random UUIDs, so another vocabulary's keys are unknown.
    let retained: HashSet<VertexKey> =
        other.ids.values().map(|id| id.key).collect();
    assert!(matches!(
        compose(&vocab, &retained, &status, &Cancellation::None),
        Err(CancelReason::Error)
    ));
}

/// Larger generated sweep; run with
/// `cargo test --release -p ngrams --lib positioned_corpus_sweep -- --ignored`.
#[test]
#[ignore]
fn positioned_corpus_sweep() {
    let start = Instant::now();
    let binary = strings_over(&['a', 'b'], 1..=4);
    let ternary = strings_over(&['a', 'b', 'c'], 1..=3);
    let mut corpora: Vec<Vec<&str>> = Vec::new();
    // All binary strings up to width 10 and ternary strings up to width 6.
    let long_binary = strings_over(&['a', 'b'], 1..=10);
    let long_ternary = strings_over(&['a', 'b', 'c'], 1..=6);
    corpora.extend(long_binary.iter().map(|s| vec![s.as_str()]));
    corpora.extend(long_ternary.iter().map(|s| vec![s.as_str()]));
    // All multisets of two or three binary texts up to width 4.
    for k in 2..=3 {
        corpora.extend(
            binary
                .iter()
                .map(String::as_str)
                .combinations_with_replacement(k),
        );
    }
    // All multisets of two ternary texts up to width 3.
    corpora.extend(
        ternary
            .iter()
            .map(String::as_str)
            .combinations_with_replacement(2),
    );
    corpora.extend(bounded_corpora());
    let (count, failures) = run_all(corpora, 0..2);
    eprintln!(
        "positioned_corpus_sweep: {count} corpora, {} failures, {:?}",
        failures.len(),
        start.elapsed()
    );
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    assert!(failures.is_empty(), "{} failing corpora", failures.len());
}
