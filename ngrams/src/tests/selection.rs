//! Input validation, containment counts, and deterministic frequency selection.
//!
//! These checks use a fresh vocabulary and selection (no labelling-image cache)
//! and do not run wrapper or partition construction.

use std::collections::BTreeSet;

use context_trace::legacy::{
    graph::vertex::key::VertexKey,
    HashSet,
};
use itertools::Itertools;
use pretty_assertions::assert_eq;

use super::TestCorpus;
use crate::{
    cancellation::Cancellation,
    graph::{
        containment::TextLocation,
        labelling::{
            frequency::FrequencyCtx,
            LabellingCtx,
            LabellingImage,
        },
        traversal::pass::{
            CancelReason,
            RunResult,
        },
        utils::cover::frequency::FrequencyCover,
        vocabulary::{
            entry::HasVertexEntries,
            NGramId,
            ProcessStatus,
            Vocabulary,
        },
        Corpus,
        StatusHandle,
    },
};

fn vocab_of(texts: &[&str]) -> RunResult<(Vocabulary, StatusHandle)> {
    let mut status = StatusHandle::default();
    let vocab =
        Vocabulary::from_corpus(&Corpus::new("selection", texts), &mut status)?;
    Ok((vocab, status))
}

fn contents<'a>(
    vocab: &Vocabulary,
    ids: impl IntoIterator<Item = &'a NGramId>,
) -> BTreeSet<String> {
    ids.into_iter()
        .map(|id| vocab.get_vertex(id).unwrap().ngram.clone())
        .collect()
}

fn occurrences(
    vocab: &Vocabulary,
    content: &str,
) -> BTreeSet<(usize, usize)> {
    vocab
        .get_vertex(&content)
        .unwrap()
        .occurrences
        .iter()
        .map(|l| (l.texti, l.x))
        .collect()
}

/// Content of every vertex selected by the fresh (cache-free) selection.
fn selected(texts: &[&str]) -> BTreeSet<String> {
    let ctx = LabellingCtx::select_frequency_fresh(
        Corpus::new("selection", texts),
        StatusHandle::default(),
        Cancellation::None,
    )
    .unwrap();
    ctx.labels()
        .iter()
        .map(|k| ctx.vocab().get_vertex(k).unwrap().ngram.clone())
        .collect()
}

#[test]
fn roots_include_inputs_first_interned_as_substrings() {
    for texts in [["abc", "xabc"], ["xabc", "abc"]] {
        let (vocab, _) = vocab_of(&texts).unwrap();
        assert_eq!(
            contents(&vocab, &vocab.roots),
            BTreeSet::from(["abc".to_string(), "xabc".to_string()]),
            "{texts:?}"
        );
        let labels = selected(&texts);
        assert!(labels.contains("abc") && labels.contains("xabc"));
    }
}

#[test]
fn repeated_atom_inputs_share_roots() {
    let texts = ["a", "b", "a"];
    let (vocab, status) = vocab_of(&texts).unwrap();
    let atoms = BTreeSet::from(["a".to_string(), "b".to_string()]);
    assert_eq!(contents(&vocab, &vocab.roots), atoms);
    assert_eq!(contents(&vocab, &vocab.leaves), atoms);
    assert_eq!(occurrences(&vocab, "a"), BTreeSet::from([(0, 0), (2, 0)]));
    assert_eq!(*status.steps_total(), 3);
    assert_eq!(selected(&texts), atoms);
}

#[test]
fn atom_only_corpus_runs_every_stage() {
    assert_eq!(selected(&["a"]), BTreeSet::from(["a".to_string()]));
    for texts in [&["a"][..], &["ab", "a"]] {
        let result = crate::graph::parse_corpus(
            Corpus::new("atom_only", texts),
            StatusHandle::default(),
            Cancellation::None,
        );
        assert!(result.is_ok(), "{texts:?}: {result:?}");
    }
}

/// Inputs that are also substrings of other inputs are roots after enumeration,
/// so legacy partitioning must accept atom roots and nested composite roots.
#[test]
fn legacy_partitioning_accepts_atom_and_nested_roots() {
    for (i, texts) in [&["a"][..], &["ab", "a"], &["abc", "ab"], &["ab", "abc"]]
        .into_iter()
        .enumerate()
    {
        let result = crate::graph::parse_corpus_legacy(
            Corpus::new(format!("legacy_roots_{i}"), texts),
            StatusHandle::default(),
            Cancellation::None,
        )
        .unwrap_or_else(|error| panic!("{texts:?}: {error:?}"));
        let data = crate::output::legacy::GraphData::from_graph(&result.graph)
            .unwrap_or_else(|error| panic!("{texts:?}: {error}"));
        for text in texts {
            assert!(data.vertices.contains_key(*text), "{texts:?}: {text:?}");
        }
    }
}

#[test]
fn empty_inputs_are_rejected_before_enumeration() {
    let no_texts: [&str; 0] = [];
    assert!(matches!(vocab_of(&no_texts), Err(CancelReason::EmptyVocabulary)));
    assert!(matches!(
        vocab_of(&["ab", ""]),
        Err(CancelReason::EmptyText(1))
    ));
    assert!(matches!(
        vocab_of(&["", "ab"]),
        Err(CancelReason::EmptyText(0))
    ));
    assert!(matches!(
        LabellingCtx::select_frequency_fresh(
            Corpus::new("selection", ["ab", ""]),
            StatusHandle::default(),
            Cancellation::None,
        ),
        Err(CancelReason::EmptyText(1))
    ));
    assert!(crate::graph::parse_corpus(
        Corpus::new("mixed_empty", ["ab", ""]),
        StatusHandle::default(),
        Cancellation::None,
    )
    .is_err());
}

#[test]
fn duplicate_inputs_count_every_occurrence() {
    let (vocab, status) = vocab_of(&["aaaa", "aaaa"]).unwrap();
    let expected: BTreeSet<_> = (0..2)
        .cartesian_product(0..3)
        .collect();
    assert_eq!(occurrences(&vocab, "aa"), expected);
    assert_eq!(contents(&vocab, &vocab.roots).len(), 1);
    assert_eq!(*status.pass(), ProcessStatus::Containment);
    assert_eq!(*status.steps(), 20);
    assert_eq!(*status.steps_total(), 20);
}

#[test]
fn multibyte_atoms_use_char_offsets() {
    let (vocab, status) = vocab_of(&["é🙂é"]).unwrap();
    assert_eq!(*status.steps_total(), 6);
    assert_eq!(
        contents(&vocab, &vocab.leaves),
        BTreeSet::from(["é".to_string(), "🙂".to_string()])
    );
    assert_eq!(occurrences(&vocab, "é"), BTreeSet::from([(0, 0), (0, 2)]));
    assert_eq!(occurrences(&vocab, "🙂"), BTreeSet::from([(0, 1)]));
    assert_eq!(occurrences(&vocab, "é🙂"), BTreeSet::from([(0, 0)]));
    assert_eq!(occurrences(&vocab, "🙂é"), BTreeSet::from([(0, 1)]));
    assert_eq!(
        contents(&vocab, &vocab.roots),
        BTreeSet::from(["é🙂é".to_string()])
    );
    let child_patterns = |content: &str| -> BTreeSet<Vec<String>> {
        let id = vocab.ids[content];
        assert_eq!(id.width, content.chars().count());
        vocab
            .containment
            .expect_child_patterns(&id.key)
            .values()
            .map(|p| {
                p.iter()
                    .map(|t| {
                        let key = vocab.containment.expect_key_for_index(t);
                        vocab.entries[&key].ngram.clone()
                    })
                    .collect()
            })
            .collect()
    };
    let pattern = |p: &[&str]| p.iter().map(ToString::to_string).collect_vec();
    assert_eq!(
        child_patterns("é🙂é"),
        BTreeSet::from([pattern(&["é🙂", "é"]), pattern(&["é", "🙂é"])])
    );
    assert_eq!(
        child_patterns("🙂é"),
        BTreeSet::from([pattern(&["🙂", "é"])])
    );
    assert!(child_patterns("é").is_empty());
    assert!(selected(&["é🙂é"]).contains("é🙂é"));
}

#[test]
fn frequency_selection_is_invariant_under_input_permutation() {
    let corpora: [&[&str]; 5] = [
        &["subdivision", "visualization"],
        &["abcabca"],
        &["ababa"],
        &["aabbaabbaa"],
        &["abab", "abcabc", "babc", "ab"],
    ];
    for texts in corpora {
        let expected = selected(texts);
        for perm in texts.iter().copied().permutations(texts.len()) {
            assert_eq!(selected(&perm), expected, "{perm:?}");
        }
    }
    assert_eq!(
        selected(&["visualization", "subdivision"])
            .into_iter()
            .filter(|s| s.chars().count() > 1)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from_iter(
            ["ion", "su", "vis", "subdivision", "visualization"]
                .map(String::from)
        )
    );
}

/// Pin labels, then report the frequency cover of `ab` and whether it is selected.
fn ab_cover_with_pins(
    texts: &[&str],
    pins: &[&str],
) -> (BTreeSet<String>, bool) {
    let corpus = Corpus::new("tie", texts);
    let (vocab, status) = vocab_of(texts).unwrap();
    let labels: HashSet<VertexKey> = vocab
        .roots
        .iter()
        .chain(&vocab.leaves)
        .chain(pins.iter().map(|p| &vocab.ids[*p]))
        .map(|id| id.key)
        .collect();
    let mut ctx = LabellingCtx::new(
        TestCorpus::new(LabellingImage { vocab, labels }, corpus),
        status,
        Cancellation::None,
    );
    let fctx = FrequencyCtx::new(&mut ctx);
    let entry = fctx.vocab().get_vertex(&"ab").unwrap();
    let cover = FrequencyCover::from_entry(&fctx, &entry).unwrap();
    (
        contents(fctx.vocab(), cover.iter()),
        fctx.entry_is_frequent(&entry).unwrap(),
    )
}

#[test]
fn frequency_cover_ties_follow_content_order() {
    let set = |s: &[&str]| s.iter().map(ToString::to_string).collect();
    for texts in [["xaby", "xabz"], ["xabz", "xaby"]] {
        // `aby` sorts before `xab` and claims one position; it occurs once.
        assert_eq!(
            ab_cover_with_pins(&texts, &["xab", "aby"]),
            (set(&["aby", "xab"]), true),
            "{texts:?}"
        );
    }
    for texts in [["yabx", "zabx"], ["zabx", "yabx"]] {
        // `abx` sorts before `yab` and claims both positions first.
        assert_eq!(
            ab_cover_with_pins(&texts, &["abx", "yab"]),
            (set(&["abx"]), false),
            "{texts:?}"
        );
    }
}

/// A signalled cancellation stops substring enumeration itself, the
/// quadratic pass that dominates long inputs.
#[test]
fn containment_enumeration_observes_cancellation() {
    let cancelled = Cancellation::from(std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(true),
    ));
    let result = Vocabulary::from_corpus_cancellable(
        &Corpus::new("cancelled", ["abcabca"]),
        &mut StatusHandle::default(),
        &cancelled,
    );
    assert!(matches!(result, Err(CancelReason::Cancelled)));
}
