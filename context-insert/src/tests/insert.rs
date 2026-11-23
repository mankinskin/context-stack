//! `insert_match` (task 3.4): the insert gate over every insert case, the
//! §9 tables, unchanged graphs for `EntireRoot` responses and stale
//! responses.

use std::{
    collections::BTreeSet,
    panic::{
        AssertUnwindSafe,
        catch_unwind,
    },
};

use context_search::Find;
use context_trace::positioned::{
    PositionedGraphRef,
    Publication,
    diff,
    fixtures::{
        InsertCase,
        insert_cases,
    },
};
use pretty_assertions::assert_eq;

use super::{
    changed_vertices,
    content,
    insert_case,
    labels,
    named,
    search_case,
};
use crate::{
    InsertError,
    InsertOutcome,
    insert,
    insert_match,
};

/// Insert the case's query and check the result against the expected
/// oracle graph; returns the publication.
fn check_case(case: InsertCase) -> Publication {
    let name = case.name.clone();
    let before = case.base.graph.to_interchange();
    let expected = case.expected.graph.to_interchange();
    let response = search_case(&case, &case.base.graph);
    let base_labels = labels(&case.base);
    let graph = PositionedGraphRef::new(case.base.graph);
    let outcome = insert_match(&graph, response.clone())
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let InsertOutcome::Created {
        token,
        response: returned,
        publication,
    } = outcome
    else {
        panic!("{name}: expected Created, got {outcome:?}");
    };
    assert_eq!(returned, response, "{name}");
    let g = graph.read();
    g.validate().unwrap_or_else(|error| panic!("{name}: {error}"));
    let actual = g.to_interchange();
    let difference = diff(&expected, &actual);
    assert!(difference.is_empty(), "{name}: {difference}");

    assert_eq!(publication.created.last().map(|t| t.index), Some(token.index), "{name}: N last");
    assert_eq!(content(&g, token), case.query, "{name}: N");
    for created in &publication.created {
        let s = content(&g, *created);
        assert!(
            case.query.starts_with(&s) || case.query.ends_with(&s),
            "{name}: new token {s:?} is not a prefix or postfix of N (C3)"
        );
    }
    let edited: BTreeSet<String> = publication
        .edits
        .iter()
        .map(|edit| base_labels[&edit.vertex].clone())
        .collect();
    assert_eq!(edited, changed_vertices(&before, &expected), "{name}: edits");
    publication
}

/// Insert gate (§12 3.4, §13): every insert case equals its oracle graph.
#[test]
fn insert_gate() {
    let cases = insert_cases();
    let total = cases.len();
    let mut failed = Vec::new();
    let mut created = 0;
    for case in cases {
        let name = case.name.clone();
        match catch_unwind(AssertUnwindSafe(|| check_case(case))) {
            Ok(publication) => created += publication.created.len(),
            Err(_) => failed.push(name),
        }
    }
    println!(
        "insert gate: {} / {total} cases pass, {created} tokens created",
        total - failed.len()
    );
    assert!(failed.is_empty(), "{} of {total} cases fail: {failed:?}", failed.len());
}

fn occurrences_of(
    graph: &PositionedGraphRef,
    key: &str,
    names: &std::collections::BTreeMap<String, context_trace::Token>,
) -> Vec<(String, usize)> {
    let g = graph.read();
    let composition = g.vertex(names[key].index).unwrap().composition().unwrap().clone();
    named(&g, composition.occurrences())
}

fn strings(
    graph: &PositionedGraphRef,
    tokens: &[context_trace::Token],
) -> Vec<String> {
    let g = graph.read();
    tokens.iter().map(|t| content(&g, *t)).collect()
}

fn occ(list: &[(&str, usize)]) -> Vec<(String, usize)> {
    list.iter().map(|(s, o)| (s.to_string(), *o)).collect()
}

/// Insert `query` into the base graph of `scenario`; the graph and the
/// base name map.
fn insert_named(
    scenario: &str,
    query: &str,
) -> (
    PositionedGraphRef,
    std::collections::BTreeMap<String, context_trace::Token>,
    Publication,
) {
    let case = insert_case(scenario, query);
    let publication = check_case(insert_case(scenario, query));
    let response = search_case(&case, &case.base.graph);
    let graph = PositionedGraphRef::new(case.base.graph);
    let names = case.base.names;
    let InsertOutcome::Created { publication: again, .. } = insert_match(&graph, response).unwrap()
    else {
        panic!("expected Created");
    };
    assert_eq!(again, publication, "deterministic");
    (graph, names, publication)
}

/// §9.3 `insert_small_periodic`.
#[test]
fn insert_small_periodic() {
    let (graph, names, publication) = insert_named("insert_small_periodic", "baba");
    assert_eq!(strings(&graph, &publication.created), ["ba", "aba", "bab", "baba"]);
    assert_eq!(occurrences_of(&graph, "abab", &names), occ(&[("aba", 0), ("bab", 1)]));
    assert_eq!(
        occurrences_of(&graph, "ababab", &names),
        occ(&[("abab", 0), ("baba", 1), ("abab", 2)])
    );
    let g = graph.read();
    let edits: Vec<_> = publication
        .edits
        .iter()
        .map(|edit| {
            (
                content(&g, g.vertex(edit.vertex).unwrap().token()),
                named(&g, &edit.added),
                named(&g, &edit.removed),
            )
        })
        .collect();
    assert_eq!(
        edits,
        vec![
            ("abab".to_string(), occ(&[("aba", 0), ("bab", 1)]), occ(&[("ab", 0), ("ab", 2)])),
            ("ababab".to_string(), occ(&[("baba", 1)]), occ(&[])),
        ]
    );
}

/// §9.1 `insert_shared_vertex`.
#[test]
fn insert_shared_vertex() {
    let (graph, names, publication) = insert_named("insert_shared_vertex", "abcabca");
    let created = strings(&graph, &publication.created);
    assert_eq!(created.last().unwrap(), "abcabca");
    assert_eq!(
        created.iter().cloned().collect::<BTreeSet<_>>(),
        ["ca", "ab", "bca", "abc", "abca", "cabca", "abcab", "abcabca"]
            .map(String::from)
            .into_iter()
            .collect()
    );
    let g = graph.read();
    let composition = |s: &str| {
        let token = publication.created[created.iter().position(|c| c == s).unwrap()];
        named(&g, g.vertex(token.index).unwrap().composition().unwrap().occurrences())
    };
    assert_eq!(composition("ca"), occ(&[("c", 0), ("a", 1)]));
    assert_eq!(composition("ab"), occ(&[("a", 0), ("b", 1)]));
    assert_eq!(composition("bca"), occ(&[("bc", 0), ("ca", 1)]));
    assert_eq!(composition("abc"), occ(&[("ab", 0), ("bc", 1)]));
    assert_eq!(composition("abca"), occ(&[("abc", 0), ("bca", 1)]));
    assert_eq!(composition("cabca"), occ(&[("cabc", 0), ("abca", 1)]));
    assert_eq!(composition("abcab"), occ(&[("abca", 0), ("bcab", 1)]));
    assert_eq!(
        composition("abcabca"),
        occ(&[("abcab", 0), ("bcabc", 1), ("cabca", 2)])
    );
    drop(g);
    assert_eq!(occurrences_of(&graph, "cab", &names), occ(&[("ca", 0), ("ab", 1)]));
    assert_eq!(occurrences_of(&graph, "bcab", &names), occ(&[("bca", 0), ("cab", 1)]));
    assert_eq!(occurrences_of(&graph, "cabc", &names), occ(&[("cab", 0), ("abc", 1)]));
    assert_eq!(occurrences_of(&graph, "cabcab", &names), occ(&[("cabca", 0), ("abcab", 1)]));
    assert_eq!(
        occurrences_of(&graph, "acabcabcab", &names),
        occ(&[("a", 0), ("cabcab", 1), ("abcabca", 2), ("cabcab", 4)])
    );
    assert_eq!(publication.edits.len(), 5);
}

/// §9.2: the off-path piece `aba@0` is dropped by `maximal`.
#[test]
fn insert_off_path_position() {
    let (graph, names, publication) = insert_named("complex_overlap", "abababc");
    assert_eq!(strings(&graph, &publication.created), ["abc", "abababc"]);
    assert_eq!(occurrences_of(&graph, "abcaba", &names), occ(&[("abc", 0), ("aba", 3)]));
    assert_eq!(
        occurrences_of(&graph, "abcabababcaba", &names),
        occ(&[("abcaba", 0), ("abababc", 3), ("abcaba", 7)])
    );
    let g = graph.read();
    let n = *publication.created.last().unwrap();
    assert_eq!(
        named(&g, g.vertex(n.index).unwrap().composition().unwrap().occurrences()),
        occ(&[("abab", 0), ("abab", 2), ("abc", 4)])
    );
}

/// `insert_periodic` and `insert_deep_borders` (§12 3.1.1 named scenarios).
#[test]
fn insert_periodic() {
    let (graph, _, publication) = insert_named("insert_periodic", "bababab");
    assert_eq!(strings(&graph, &publication.created).last().unwrap(), "bababab");
}

#[test]
fn insert_deep_borders() {
    let (graph, _, publication) = insert_named("insert_deep_borders", "bbaabbaabb");
    assert_eq!(strings(&graph, &publication.created).last().unwrap(), "bbaabbaabb");
}

/// `EntireRoot` responses leave the graph unchanged.
#[test]
fn entire_root_outcomes() {
    let case = insert_case("insert_small_periodic", "baba");
    let names = case.base.names.clone();
    let graph = PositionedGraphRef::new(case.base.graph);
    let before = graph.read().to_interchange();
    let generation = graph.read().generation();

    let response = graph.find_ancestor("abab".chars()).unwrap();
    let outcome = insert_match(&graph, response).unwrap();
    assert!(
        matches!(outcome, InsertOutcome::Complete { token, .. } if token.index == names["abab"].index),
        "{outcome:?}"
    );

    let response = graph.find_ancestor("abababa".chars()).unwrap();
    let outcome = insert_match(&graph, response).unwrap();
    assert!(
        matches!(outcome, InsertOutcome::NoExpansion { token, .. } if token.index == names["ababab"].index),
        "{outcome:?}"
    );
    assert_eq!(insert(&graph, "ab".chars()).unwrap().index, names["ab"].index);

    assert_eq!(graph.read().generation(), generation);
    assert!(diff(&before, &graph.read().to_interchange()).is_empty());
}

/// A response found before an unrelated edit is rejected, the graph keeps
/// only that edit.
#[test]
fn stale_response() {
    let case = insert_case("insert_small_periodic", "baba");
    let graph = PositionedGraphRef::new(case.base.graph);
    let response = graph.find_ancestor("baba".chars()).unwrap();
    assert_eq!(response.generation(), graph.read().generation());
    {
        let mut g = graph.write();
        let mut edit = g.edit();
        edit.atom('z');
        edit.finish();
    }
    let after_edit = graph.read().to_interchange();
    assert_eq!(insert_match(&graph, response), Err(InsertError::StaleResponse));
    assert!(diff(&after_edit, &graph.read().to_interchange()).is_empty());

    let token = insert(&graph, "baba".chars()).unwrap();
    assert_eq!(content(&graph.read(), token), "baba");
}
