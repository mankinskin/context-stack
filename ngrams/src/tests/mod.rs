//! Corpus fixtures, label expectations, and parse smoke tests.
//!
//! This module is also compiled outside tests: `TestCorpus` is used as the
//! production labelling context's image/source wrapper. Only test functions and
//! explicitly gated modules are test-only.

pub(crate) mod count;
#[cfg(test)]
mod hash_order;

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
use context_trace::{
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

/// Smoke-test nonempty results and string reconstruction for both returned graphs.
#[test]
pub(crate) fn test_parse_corpus() {
    use crate::graph::{
        parse_corpus,
        Status,
    };

    let corpus = ["hello", "world", "hello world"];
    let texts = corpus.into_iter().map(ToString::to_string).collect_vec();
    let corpus_name = "test_parse_corpus".to_owned();

    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(result.is_ok(), "parse_corpus should succeed");

    let parse_result = result.unwrap();

    // Verify that the result contains expected data
    assert!(
        !parse_result.labels.is_empty(),
        "Should have some labels after parsing"
    );

    // Verify the graph has vertices
    assert!(
        parse_result.graph.vertex_count() > 0,
        "Graph should have vertices"
    );

    // Verify the containment graph has vertices
    assert!(
        parse_result.containment.vertex_count() > 0,
        "Containment graph should have vertices"
    );

    // Validate ALL vertices in the partition graph can produce string representations
    let graph_keys: Vec<_> = parse_result.graph.vertex_keys().collect();
    for key in &graph_keys {
        let s = parse_result.graph.vertex_key_string(key);
        assert!(
            !s.is_empty(),
            "Vertex {:?} should have non-empty string representation",
            key
        );
    }

    // Validate ALL vertices in the containment graph can produce string representations
    let containment_keys: Vec<_> =
        parse_result.containment.vertex_keys().collect();
    for key in &containment_keys {
        let s = parse_result.containment.vertex_key_string(key);
        assert!(!s.is_empty(), "Containment vertex {:?} should have non-empty string representation", key);
    }
}

/// Exercise a repeated-block input without asserting exact selected token content.
#[test]
pub(crate) fn test_parse_corpus_aabbaabbaa() {
    use crate::graph::{
        parse_corpus,
        Status,
    };

    let texts = vec!["aabbaabbaa".to_string()];
    let corpus_name = "test_aabbaabbaa".to_owned();

    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(result.is_ok(), "parse_corpus should succeed for aabbaabbaa");

    let parse_result = result.unwrap();

    // Verify basic structure
    assert!(
        parse_result.graph.vertex_count() > 0,
        "Graph should have vertices"
    );
    assert!(
        parse_result.containment.vertex_count() > 0,
        "Containment should have vertices"
    );

    // This is a nonempty-selection check, not an assertion of particular tokens.
    assert!(
        !parse_result.labels.is_empty(),
        "Should have frequency labels"
    );

    // Validate ALL vertices in the partition graph
    let graph_keys: Vec<_> = parse_result.graph.vertex_keys().collect();
    for key in &graph_keys {
        let s = parse_result.graph.vertex_key_string(key);
        assert!(
            !s.is_empty(),
            "Vertex {:?} should have non-empty string representation",
            key
        );
    }

    // Validate ALL vertices in the containment graph
    let containment_keys: Vec<_> =
        parse_result.containment.vertex_keys().collect();
    for key in &containment_keys {
        let s = parse_result.containment.vertex_key_string(key);
        assert!(!s.is_empty(), "Containment vertex {:?} should have non-empty string representation", key);
    }
}

/// Exercise a repeated single-character alphabet using the four-character text `aaaa`.
#[test]
pub(crate) fn test_parse_corpus_single_char() {
    use crate::graph::{
        parse_corpus,
        Status,
    };

    let texts = vec!["aaaa".to_string()];
    let corpus_name = "test_single_char".to_owned();

    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(result.is_ok(), "parse_corpus should succeed for aaaa");

    let parse_result = result.unwrap();
    assert!(
        parse_result.graph.vertex_count() > 0,
        "Graph should have vertices"
    );

    // Selection is checked for nonemptiness, not specifically for the token "aa".
    assert!(
        !parse_result.labels.is_empty(),
        "Should have labels for repeated aa"
    );

    // Validate ALL vertices in the partition graph
    let graph_keys: Vec<_> = parse_result.graph.vertex_keys().collect();
    for key in &graph_keys {
        let s = parse_result.graph.vertex_key_string(key);
        assert!(
            !s.is_empty(),
            "Vertex {:?} should have non-empty string representation",
            key
        );
    }

    // Validate ALL vertices in the containment graph
    let containment_keys: Vec<_> =
        parse_result.containment.vertex_keys().collect();
    for key in &containment_keys {
        let s = parse_result.containment.vertex_key_string(key);
        assert!(!s.is_empty(), "Containment vertex {:?} should have non-empty string representation", key);
    }
}

/// Smoke-test two texts sharing `bc`, without asserting its exact decomposition.
#[test]
pub(crate) fn test_parse_corpus_two_texts() {
    use crate::graph::{
        parse_corpus,
        Status,
    };

    let texts = vec!["abc".to_string(), "bcd".to_string()];
    let corpus_name = "test_two_texts".to_owned();

    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(result.is_ok(), "parse_corpus should succeed for two texts");

    let parse_result = result.unwrap();
    assert!(
        parse_result.graph.vertex_count() > 0,
        "Graph should have vertices"
    );

    // This does not independently assert that the shared token "bc" was selected.
    assert!(
        !parse_result.labels.is_empty(),
        "Should have labels for shared bc"
    );

    // Validate ALL vertices in the partition graph
    let graph_keys: Vec<_> = parse_result.graph.vertex_keys().collect();
    for key in &graph_keys {
        let s = parse_result.graph.vertex_key_string(key);
        assert!(
            !s.is_empty(),
            "Vertex {:?} should have non-empty string representation",
            key
        );
    }

    // Validate ALL vertices in the containment graph
    let containment_keys: Vec<_> =
        parse_result.containment.vertex_keys().collect();
    for key in &containment_keys {
        let s = parse_result.containment.vertex_key_string(key);
        assert!(!s.is_empty(), "Containment vertex {:?} should have non-empty string representation", key);
    }
}

/// Check successful parsing and string reconstruction when the input has no repeats.
#[test]
pub(crate) fn test_parse_corpus_empty_result() {
    use crate::graph::{
        parse_corpus,
        Status,
    };

    // A corpus with no repeated n-grams
    let texts = vec!["xyz".to_string()];
    let corpus_name = "test_no_repeats".to_owned();

    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    assert!(
        result.is_ok(),
        "parse_corpus should succeed even with no repeats"
    );

    let parse_result = result.unwrap();
    // Should still have the basic structure
    assert!(
        parse_result.containment.vertex_count() > 0,
        "Containment should have vertices"
    );

    // Validate ALL vertices in the partition graph (may be empty or minimal)
    let graph_keys: Vec<_> = parse_result.graph.vertex_keys().collect();
    for key in &graph_keys {
        let s = parse_result.graph.vertex_key_string(key);
        assert!(
            !s.is_empty(),
            "Vertex {:?} should have non-empty string representation",
            key
        );
    }

    // Validate ALL vertices in the containment graph
    let containment_keys: Vec<_> =
        parse_result.containment.vertex_keys().collect();
    for key in &containment_keys {
        let s = parse_result.containment.vertex_key_string(key);
        assert!(!s.is_empty(), "Containment vertex {:?} should have non-empty string representation", key);
    }
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
