//! Corpus fixtures, label expectations, and parse smoke tests.
//!
//! This module is also compiled outside tests: `TestCorpus` is used as the
//! production labelling context's image/source wrapper. Only test functions and
//! explicitly gated modules are test-only.

pub(crate) mod count;
#[cfg(test)]
mod composition;
#[cfg(test)]
mod hash_order;
#[cfg(test)]
mod output;
#[cfg(test)]
mod retention;
#[cfg(test)]
mod selection;

use std::path::Path;

use crate::{
    cancellation::Cancellation,
    graph::{
        labelling::{
            frequency,
            LabellingCtx,
            LabellingImage,
        },
        vocabulary::{
            entry::HasVertexEntries,
            ProcessStatus,
            Vocabulary,
        },
        Corpus,
        StatusHandle,
    },
};
use context_trace_legacy::{
    graph::vertex::key::VertexKey,
    HashSet,
};
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use itertools::Itertools;
use ngram::NGram;
use pretty_assertions::assert_eq;

/// Repetition-rich German text fixture used by counting and manual label checks.
pub(crate) const OTTOS_MOPS_CORPUS: [&str; 4] = [
    "ottos mops trotzt",
    "otto: fort mops fort",
    "ottos mops hopst fort",
    "otto: soso",
];
/// Join the second column of tab-delimited records, panicking on missing/bad data.
fn read_corpus(file_path: impl AsRef<Path>) -> String {
    //let corpus: String = String::from("fldfjdlsjflskdjflsdfaädüwwrivfokl");
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path(file_path)
        .expect("Corpus file not found.");
    csv.records().map(|r| r.unwrap()[1].to_string()).join(" ")
}

/// Source corpus and image, plus independently derived expected roots and atoms.
///
/// Despite its name, this wrapper is also used during normal corpus parsing.
#[derive(Debug)]
pub(crate) struct TestCorpus {
    /// Vocabulary and current shared label set.
    pub(crate) image: LabellingImage,
    /// Original named inputs.
    pub(crate) corpus: Corpus,
    /// Distinct full input strings expected as containment roots.
    pub(crate) roots_test: HashSet<String>,
    /// Distinct individual characters expected as leaves.
    pub(crate) leaves_test: HashSet<String>,
}
impl TestCorpus {
    /// Pair an image with its source and derive root/leaf expectations from texts.
    pub(crate) fn new(
        image: LabellingImage,
        corpus: Corpus,
    ) -> Self {
        let roots_test: HashSet<_> =
            corpus.texts.iter().map(ToString::to_string).collect();
        let leaves_test: HashSet<_> = corpus
            .texts
            .iter()
            .flat_map(|s| {
                s.chars().ngrams(1).map(String::from_iter).collect_vec()
            })
            .collect();
        Self {
            image,
            corpus,
            roots_test,
            leaves_test,
        }
    }
    /// Return sorted expected roots for stable assertion diagnostics.
    pub(crate) fn get_roots_test(&self) -> Vec<String> {
        self.roots_test.iter().cloned().sorted().collect()
    }
    /// Return sorted expected atoms for stable assertion diagnostics.
    pub(crate) fn get_leaves_test(&self) -> Vec<String> {
        self.leaves_test.iter().cloned().sorted().collect()
    }
    /// Assert root/leaf content and exact concatenation of elementary patterns.
    ///
    /// This does not compare the selected output graph or traversal schedules.
    pub(crate) fn test_containment(&self) {
        let Self {
            image: LabellingImage { vocab, .. },
            corpus,
            leaves_test,
            roots_test,
        } = self;
        assert_eq!(
            vocab
                .leaves
                .iter()
                .map(|vi| { vocab.get_vertex(vi).unwrap().ngram.clone() })
                .collect::<HashSet<_>>(),
            *leaves_test,
        );
        assert_eq!(
            vocab
                .roots
                .iter()
                .map(|vi| { vocab.get_vertex(vi).unwrap().ngram.clone() })
                .collect::<HashSet<_>>(),
            *roots_test,
        );
        for (k, e) in &vocab.entries {
            let patterns = vocab.containment.expect_child_patterns(k);
            assert!([0, 1, 2].contains(&patterns.len()));
            for (pid, p) in patterns.iter() {
                assert_eq!(
                    p.iter()
                        .map(|i| {
                            vocab
                                .entries
                                .get(&vocab.containment.expect_key_for_index(i))
                                .unwrap()
                                .ngram
                                .clone()
                        })
                        .join(""),
                    e.ngram,
                );
            }
        }
    }
}
/// Expected additional labels by stage, excluding mandatory roots and atoms.
#[derive(Debug, new)]
pub(crate) struct LabelTest {
    /// Tokens expected to be introduced by frequency selection.
    frequency: HashSet<String>,
    /// Tokens expected to be introduced by overlap wrapping.
    wrapper: HashSet<String>,
    /// Tokens expected to be introduced during partition gap discovery.
    partition: HashSet<String>,
}
impl LabelTest {
    /// Assert that stage-specific additions are pairwise disjoint.
    pub(crate) fn validate(&self) {
        for (a, b) in [&self.frequency, &self.wrapper, &self.partition]
            .into_iter()
            .combinations(2)
            .map(|v| (v[0], v[1]))
        {
            assert_eq!(
                a.intersection(b).cloned().collect_vec(),
                Vec::<String>::default(),
            );
        }
    }
}
/// Build owned expectation sets and reject overlap between stage categories.
macro_rules! test_labels {
    ($freq:expr, $wrap:expr,$part:expr$(,)?) => {{
        let frequency = $freq.into_iter().map(ToString::to_string).collect();
        let wrapper: HashSet<String> =
            $wrap.into_iter().map(ToString::to_string).collect();
        let partition: HashSet<String> =
            $part.into_iter().map(ToString::to_string).collect();
        let s = LabelTest {
            frequency,
            wrapper,
            partition,
        };
        s.validate();
        s
    }};
}

/// Labelling context paired with expected cumulative stage selections.
#[derive(Debug, new, Deref, DerefMut)]
pub(crate) struct TestCase {
    /// Mutable pipeline state used to execute the scenario.
    #[deref]
    #[deref_mut]
    ctx: LabellingCtx,
    /// Expected additions grouped by their intended stage.
    labels: LabelTest,
}
impl TestCase {
    /// Run stages and compare labels only when the status equals that stage.
    ///
    /// Partition construction currently ends at `Finished`, so its final
    /// `Partitions` equality guard does not execute the partition-label assertion.
    pub(crate) fn execute(&mut self) {
        // graph of all containment edges between n and n+1
        self.corpus.test_containment();
        self.label_freq().unwrap();

        if *self.status.pass() == ProcessStatus::Frequency {
            let ctx = LabelTestCtx::new(self.labels(), self);
            ctx.test_roots();
            ctx.test_leaves();

            ctx.test_freq();
        }

        self.label_wrap().unwrap();

        if *self.status.pass() == ProcessStatus::Wrappers {
            let ctx = LabelTestCtx::new(self.labels(), self);
            ctx.test_wrap();
        }

        self.label_part().unwrap();

        if *self.status.pass() == ProcessStatus::Partitions {
            let ctx = LabelTestCtx::new(self.labels(), self);
            ctx.test_part();
        }
    }
}
/// Borrowed view for comparing current selection against one scenario's expectations.
#[derive(Debug, new)]
pub(crate) struct LabelTestCtx<'a> {
    /// Current label identities to resolve through the vocabulary.
    labels: &'a HashSet<VertexKey>,
    /// Source, vocabulary, and expected stage additions.
    test: &'a TestCase,
}
impl<'a> LabelTestCtx<'a> {
    /// Assert every expected full input is present in the selected content.
    pub(crate) fn test_roots(&self) {
        let label_strings = self.label_strings_set();
        let roots_test = self.test.corpus.get_roots_test();
        assert_eq!(
            label_strings
                .intersection(&roots_test.iter().cloned().collect())
                .cloned()
                .sorted()
                .collect_vec(),
            roots_test,
        );
    }
    /// Assert every expected atom is present in the selected content.
    pub(crate) fn test_leaves(&self) {
        let label_strings = self.label_strings_set();
        let leaves_test = self.test.corpus.get_leaves_test();
        assert_eq!(
            label_strings
                .intersection(&leaves_test.iter().cloned().collect())
                .cloned()
                .sorted()
                .collect_vec(),
            leaves_test,
        );
    }
    /// Combine sorted roots, atoms, and frequency additions without deduplicating.
    pub(crate) fn get_frequency_test(&self) -> Vec<String> {
        self.test
            .corpus
            .get_leaves_test()
            .iter()
            .chain(self.test.corpus.get_roots_test().iter())
            .chain(self.test.labels.frequency.iter())
            .sorted()
            .cloned()
            .collect()
    }
    /// Extend frequency expectations with wrapper additions.
    pub(crate) fn get_wrapper_test(&self) -> Vec<String> {
        self.get_frequency_test()
            .iter()
            .chain(self.test.labels.wrapper.iter())
            .sorted()
            .cloned()
            .collect()
    }
    /// Extend wrapper expectations with partition additions.
    pub(crate) fn get_partition_test(&self) -> Vec<String> {
        self.get_wrapper_test()
            .iter()
            .chain(self.test.labels.partition.iter())
            .sorted()
            .cloned()
            .collect()
    }
    /// Resolve selected identities to a content set for order-insensitive checks.
    pub(crate) fn label_strings_set(&self) -> HashSet<String> {
        self.labels
            .iter()
            .map(|vi| self.test.vocab().get_vertex(vi).unwrap().ngram.clone())
            .collect()
    }
    /// Assert exact cumulative frequency-stage content.
    pub(crate) fn test_freq(&self) {
        let label_strings = self.label_strings_set();
        let frequency_test = self.get_frequency_test();

        assert_eq!(
            label_strings.iter().cloned().sorted().collect_vec(),
            frequency_test,
        );
    }
    /// Assert exact cumulative wrapper-stage content.
    pub(crate) fn test_wrap(&self) {
        let label_strings = self.label_strings_set();
        let wrapper_test = self.get_wrapper_test();

        assert_eq!(
            label_strings.iter().cloned().sorted().collect_vec(),
            wrapper_test,
        );
    }
    /// Assert exact cumulative partition-stage content, not decomposition patterns.
    pub(crate) fn test_part(&self) {
        let label_strings = self.label_strings_set();
        let partition_test = self.get_partition_test();

        assert_eq!(
            label_strings.iter().cloned().sorted().collect_vec(),
            partition_test,
        );
    }
}

/// Check frequency selections for overlapping repeated ASCII substrings.
#[test]
pub(crate) fn test_graph1() {
    let corpus = ["abab", "abcabc", "babc"];
    let texts = corpus.into_iter().map(ToString::to_string).collect_vec();
    TestCase {
        ctx: LabellingCtx::from_corpus(
            Corpus::new("abab_corpus".to_owned(), texts),
            Cancellation::None,
        )
        .unwrap(),
        labels: test_labels! {
            [
                "ab",
                "abc",
                "bab",
            ],
            [] as [&str; 0],
            [] as [&str; 0],
        },
    }
    .execute();
}

/// Slow manual scenario, not registered as a test; wrapper expectations are tentative.
#[allow(unused)]
pub(crate) fn test_graph2() {
    let corpus = OTTOS_MOPS_CORPUS;
    let texts = corpus.into_iter().map(ToString::to_string).collect_vec();

    TestCase {
        ctx: LabellingCtx::from_corpus(
            Corpus::new("ottos_mops".to_owned(), texts),
            Cancellation::None,
        )
        .unwrap(),
        labels: test_labels! {
            [
                "ot",
                "s ",
                "so",
                "os",
                "t ",
                "ops",
                "otto",
                " mops ",
                "otto: ",
                " fort",
                "ottos mops ",
            ],
            [
                // Todo: check for correctness
                " fort ",
                " fort mops ",
                " fort mops fort",
                " mops fort",
                "ops ",
                "ops fort",
                "os ",
                "os mops ",
                "oso",
                "oso",
                "otto: fort",
                "otto: fort ",
                "otto: fort mops ",
                "ottos",
                "ottos ",
                "s fort",
                "s mops ",
                "sos",
                "soso",
                "t fort",
                "t mops ",
                "t mops fort",
            ],
            [] as [&str; 0],
        },
    }
    .execute();
}

/// Run the v3 pipeline and return its table with retained-set invariants checked.
#[cfg(test)]
fn parse_table(
    texts: &[&str]
) -> std::collections::BTreeMap<String, Vec<(usize, String)>> {
    use crate::graph::{
        parse_corpus,
        Status,
    };
    let status = StatusHandle::from(Status::new(texts));
    let result = parse_corpus(
        Corpus::new("test_parse_corpus", texts),
        status.clone(),
        Cancellation::None,
    )
    .unwrap_or_else(|e| panic!("parse_corpus failed for {texts:?}: {e:?}"));
    assert_eq!(*status.pass(), ProcessStatus::Finished);
    let corpus = texts.iter().map(ToString::to_string).collect_vec();
    result.graph.validate_for_corpus(&corpus).unwrap();
    result.graph.vertices
}

/// Build an expected v3 table from `(owner, [(offset, child)])` rows.
#[cfg(test)]
fn table(
    rows: &[(&str, &[(usize, &str)])]
) -> std::collections::BTreeMap<String, Vec<(usize, String)>> {
    rows.iter()
        .map(|(owner, cover)| {
            (
                owner.to_string(),
                cover.iter().map(|&(o, c)| (o, c.to_string())).collect(),
            )
        })
        .collect()
}

/// Separate texts keep their roots; the shared root is covered by both.
#[test]
pub(crate) fn test_parse_corpus() {
    let atoms = [" ", "d", "e", "h", "l", "o", "r", "w"].map(|a| (a, &[][..]));
    let mut expected = table(&atoms);
    expected.extend(table(&[
        (
            "hello",
            &[(0, "h"), (1, "e"), (2, "l"), (3, "l"), (4, "o")],
        ),
        (
            "world",
            &[(0, "w"), (1, "o"), (2, "r"), (3, "l"), (4, "d")],
        ),
        ("hello world", &[(0, "hello"), (5, " "), (6, "world")]),
    ]));
    assert_eq!(parse_table(&["hello", "world", "hello world"]), expected);
}

/// Repeated blocks overlap at offset 4 instead of being re-chunked.
#[test]
pub(crate) fn test_parse_corpus_aabbaabbaa() {
    assert_eq!(
        parse_table(&["aabbaabbaa"]),
        table(&[
            ("a", &[]),
            ("b", &[]),
            ("aa", &[(0, "a"), (1, "a")]),
            ("aabbaa", &[(0, "aa"), (2, "b"), (3, "b"), (4, "aa")]),
            ("aabbaabbaa", &[(0, "aabbaa"), (4, "aabbaa")]),
        ])
    );
}

/// Each width of a unary text is covered by two overlapping narrower children.
#[test]
pub(crate) fn test_parse_corpus_single_char() {
    assert_eq!(
        parse_table(&["aaaa"]),
        table(&[
            ("a", &[]),
            ("aa", &[(0, "a"), (1, "a")]),
            ("aaa", &[(0, "aa"), (1, "aa")]),
            ("aaaa", &[(0, "aaa"), (1, "aaa")]),
        ])
    );
}

/// The shared `bc` is selected and used by both roots at their own offsets.
#[test]
pub(crate) fn test_parse_corpus_two_texts() {
    assert_eq!(
        parse_table(&["abc", "bcd"]),
        table(&[
            ("a", &[]),
            ("b", &[]),
            ("c", &[]),
            ("d", &[]),
            ("bc", &[(0, "b"), (1, "c")]),
            ("abc", &[(0, "a"), (1, "bc")]),
            ("bcd", &[(0, "bc"), (2, "d")]),
        ])
    );
}

/// Without repeats only the root and its atoms are retained.
#[test]
pub(crate) fn test_parse_corpus_empty_result() {
    assert_eq!(
        parse_table(&["xyz"]),
        table(&[
            ("x", &[]),
            ("y", &[]),
            ("z", &[]),
            ("xyz", &[(0, "x"), (1, "y"), (2, "z")]),
        ])
    );
}

/// Corpus order and multiplicity affect neither retention nor covers.
#[test]
pub(crate) fn test_parse_corpus_order_and_multiplicity() {
    assert_eq!(parse_table(&["a", "b", "a"]), table(&[("a", &[]), ("b", &[])]));
    for texts in [["abc", "xabc"], ["xabc", "abc"]] {
        let t = parse_table(&texts);
        assert_eq!(t, table(&[
            ("a", &[]),
            ("b", &[]),
            ("c", &[]),
            ("x", &[]),
            ("abc", &[(0, "a"), (1, "b"), (2, "c")]),
            ("xabc", &[(0, "x"), (1, "abc")]),
        ]));
    }
    assert_eq!(parse_table(&["ab", "ab"]), parse_table(&["ab"]));
    assert_eq!(
        parse_table(&["é🙂é"]),
        table(&[
            ("é", &[]),
            ("🙂", &[]),
            ("é🙂é", &[(0, "é"), (1, "🙂"), (2, "é")]),
        ])
    );
}

/// A signalled cancellation fails without reaching `Finished`.
#[test]
pub(crate) fn test_parse_corpus_cancelled() {
    use crate::graph::{
        parse_corpus,
        traversal::pass::CancelReason,
    };
    use std::sync::{
        atomic::AtomicBool,
        Arc,
    };
    let status = StatusHandle::default();
    let result = parse_corpus(
        Corpus::new("cancelled", ["abcabca"]),
        status.clone(),
        Cancellation::from(Arc::new(AtomicBool::new(true))),
    );
    assert!(matches!(result, Err(CancelReason::Cancelled)), "{result:?}");
    assert_ne!(*status.pass(), ProcessStatus::Finished);
}

/// Mixed empty and nonempty texts are rejected before enumeration.
#[test]
pub(crate) fn test_parse_corpus_mixed_empty_text() {
    use crate::graph::{
        parse_corpus,
        traversal::pass::CancelReason,
    };
    let result = parse_corpus(
        Corpus::new("mixed", ["ab", ""]),
        StatusHandle::default(),
        Cancellation::None,
    );
    assert!(matches!(result, Err(CancelReason::EmptyText(1))), "{result:?}");
}

/// Require an empty input collection to report `EmptyVocabulary`.
#[test]
pub(crate) fn test_parse_corpus_empty_texts() {
    use crate::graph::{
        parse_corpus,
        traversal::pass::CancelReason,
        Status,
    };

    // Empty corpus (no texts)
    let texts: Vec<String> = vec![];
    let corpus_name = "test_empty_corpus".to_owned();

    let status = StatusHandle::from(Status::new(Vec::<String>::new()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(result.is_err(), "parse_corpus should fail for empty corpus");

    if let Err(CancelReason::EmptyVocabulary) = result {
        // Expected error
    } else {
        panic!("Expected EmptyVocabulary error, got: {:?}", result);
    }
}

/// Require an input collection containing only empty strings to report `EmptyVocabulary`.
#[test]
pub(crate) fn test_parse_corpus_only_empty_strings() {
    use crate::graph::{
        parse_corpus,
        traversal::pass::CancelReason,
        Status,
    };

    // Corpus with only empty strings
    let corpus_name = "test_empty_strings".to_owned();

    let status = StatusHandle::from(Status::new(Vec::<String>::new()));

    let result = parse_corpus(
        Corpus::new(corpus_name, vec!["".to_string(), "".to_string()]),
        status,
        Cancellation::None,
    );

    assert!(
        result.is_err(),
        "parse_corpus should fail for corpus with only empty strings"
    );

    if let Err(CancelReason::EmptyVocabulary) = result {
        // Expected error
    } else {
        panic!("Expected EmptyVocabulary error, got: {:?}", result);
    }
}
