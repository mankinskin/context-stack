//! `search` views, `--fail-if-missing` and occurrence completeness.

mod common;

use common::Store;
use context_api::{
    CommandResult,
    types::{
        SearchOutcome,
        SearchResult,
    },
};
use predicates::prelude::*;

const CORPUS: [&str; 3] = [
    "the quick brown fox jumps over the lazy dog",
    "a brown fox and a grey fox",
    "the brown fox is fast",
];

fn corpus_store() -> Store {
    let store = Store::new();
    store
        .cmd()
        .arg("create")
        .arg("corpus")
        .args(CORPUS)
        .assert()
        .success();
    store
}

fn search(
    store: &Store,
    args: &[&str],
) -> SearchResult {
    let mut full = vec!["search", "corpus"];
    full.extend_from_slice(args);
    match store.ok(&full) {
        CommandResult::Search(result) => result,
        other => panic!("{other:?}"),
    }
}

#[test]
fn search_views() {
    let store = corpus_store();
    for view in ["summary", "path", "contexts", "occurrences", "parents"] {
        store
            .cmd()
            .args(["search", "corpus", "brown fox", "--view", view])
            .assert()
            .success()
            .stdout(predicate::str::contains("brown fox"));
        let result = search(&store, &["brown fox", "--view", view]);
        assert_eq!(result.query, "brown fox");
        assert!(!matches!(result.outcome, SearchOutcome::NotFound { .. }));
        match view {
            "path" => assert!(result.path.is_some()),
            "contexts" => assert!(result.contexts.is_some()),
            "occurrences" =>
                assert_eq!(result.occurrences.unwrap().total, Some(3)),
            "parents" => assert!(result.parents.is_some()),
            _ => {},
        }
    }
    store
        .cmd()
        .args([
            "search",
            "corpus",
            "brown fox",
            "--view",
            "occurrences",
            "--width",
            "5",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("3 occurrences in 3 inputs")
                .and(predicate::str::contains("[brown fox]")),
        );

    // Missing and partial queries: exit 0 by default, 1 with --fail-if-missing.
    for query in ["xyz", "fox jumps over the lazy cat"] {
        store
            .cmd()
            .args(["search", "corpus", query])
            .assert()
            .success();
        store
            .cmd()
            .args(["search", "corpus", query, "--fail-if-missing", "-q"])
            .assert()
            .code(1);
    }
    store
        .cmd()
        .args(["search", "corpus", "brown fox", "--fail-if-missing"])
        .assert()
        .success();
    store
        .cmd()
        .args(["search", "corpus", "XYZ"])
        .assert()
        .stdout(predicate::str::starts_with("not found"));
    store
        .cmd()
        .args(["search", "corpus", "xyz"])
        .assert()
        .stdout(predicate::str::starts_with("partial"));
}

/// Overlapping occurrences of `query` in `text`.
fn brute_force(
    text: &str,
    query: &str,
) -> usize {
    text.char_indices()
        .filter(|(i, _)| text[*i..].starts_with(query))
        .count()
}

#[test]
fn search_matches_grep() {
    let store = corpus_store();
    let mut queries: Vec<String> = [
        "o",
        "the",
        "fox",
        "brown fox",
        "a ",
        "aa",
        "dog",
        "fox is",
        "xyz",
        " ",
    ]
    .map(String::from)
    .to_vec();
    // Every substring of length 2..=4 of the first input.
    let chars: Vec<char> = CORPUS[0].chars().collect();
    for len in 2..=4 {
        for window in chars.windows(len) {
            queries.push(window.iter().collect());
        }
    }
    queries.sort();
    queries.dedup();
    for query in queries {
        let expected: usize =
            CORPUS.iter().map(|text| brute_force(text, &query)).sum();
        let result = search(
            &store,
            &[&query, "--view", "occurrences", "--limit", "100000"],
        );
        let found = result.occurrences.map(|o| o.total).unwrap_or(Some(0));
        assert_eq!(found, Some(expected), "query {query:?}");
        assert_eq!(
            matches!(
                result.outcome,
                SearchOutcome::NotFound { .. } | SearchOutcome::Partial { .. }
            ),
            expected == 0,
            "query {query:?}: {:?}",
            result.outcome
        );
    }
}
