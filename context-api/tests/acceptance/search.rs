//! Plan 1.8 test 3, search completeness: the `occurrences` view equals a
//! brute-force scan of the input texts, and the outcome is consistent with
//! it (not found / partial exactly when no input contains the query).

use std::collections::{
    BTreeMap,
    BTreeSet,
};

use context_api::{
    types::*,
    Session,
};

use crate::{
    common::*,
    corpora::search_corpora,
};

/// Chars of context per snippet side requested by these tests.
const CONTEXT: usize = 2;

#[test]
fn occurrences_equal_brute_force_for_every_substring_and_non_substring() {
    let env = Env::with(|config| config.keep_open = true);
    let mut failures = Failures::default();
    let mut checked = 0;
    for (i, corpus) in search_corpora().iter().enumerate() {
        let graph = format!("c{i}");
        create(&env.session, &graph, corpus);
        let case = Case::new(&env.session, &graph, corpus);
        for query in queries(corpus) {
            for error in case.check(&query) {
                failures.check(false, || format!("{corpus:?} / {query:?}: {error}"));
            }
            checked += 1;
        }
    }
    assert!(checked > 10_000, "only {checked} queries");
    failures.assert_none("search completeness");
}

#[test]
fn occurrences_report_lines_and_columns_in_chars() {
    let env = Env::new();
    let corpus = strings(&["ab\ncab\n\nba", "x\ny ab\nä ab", "ab"]);
    create(&env.session, "lines", &corpus);
    let case = Case::new(&env.session, "lines", &corpus);
    let mut failures = Failures::default();
    for query in ["ab", "b\n", "\n", "\n\n", "a", "b", "y ab", " ab", "ä", "\nä a", "abz"] {
        for error in case.check(query) {
            failures.check(false, || format!("{query:?}: {error}"));
        }
    }
    failures.assert_none("line/column");
}

#[test]
fn empty_query_is_not_found_without_unknown_chars() {
    let env = Env::new();
    create(&env.session, "g", &strings(&["abc"]));
    let result = search(&env.session, "g", "", SearchView::Summary, Limits::default());
    assert_eq!(
        result.outcome,
        SearchOutcome::NotFound {
            unknown_chars: vec![]
        }
    );
}

/// Every distinct substring, every right extension of one by a char of
/// `{a, b, c}` that is not a substring, every string over `{a, b, c}` up to
/// length 3, and queries with the unknown char `z`.
fn queries(corpus: &[String]) -> BTreeSet<String> {
    let subs = substrings(corpus);
    let mut queries = subs.clone();
    for sub in &subs {
        for c in ['a', 'b', 'c'] {
            queries.insert(format!("{sub}{c}"));
        }
    }
    queries.extend(crate::corpora::strings(&['a', 'b', 'c'], 3));
    for q in ["z", "zz", "az", "za", "aza"] {
        queries.insert(q.to_string());
    }
    if let Some(longest) = subs.iter().max_by_key(|s| s.len()) {
        queries.insert(format!("{longest}z"));
    }
    queries
}

/// A graph read from `corpus`, with its input log and vertex set.
struct Case<'a> {
    session: &'a Session,
    graph: &'a str,
    texts: Vec<Vec<char>>,
    /// Input id → position in the corpus.
    positions: BTreeMap<u32, usize>,
    records: Vec<InputRecord>,
    vertices: BTreeSet<String>,
}

impl<'a> Case<'a> {
    fn new(
        session: &'a Session,
        graph: &'a str,
        corpus: &[String],
    ) -> Self {
        let records = inputs(session, graph);
        assert_eq!(records.len(), corpus.len(), "{graph}: one input per text");
        let positions = records.iter().enumerate().map(|(pos, r)| (r.id, pos)).collect();
        Self {
            session,
            graph,
            texts: corpus.iter().map(|t| t.chars().collect()).collect(),
            positions,
            records,
            vertices: export(session, graph).graph.vertices.into_keys().collect(),
        }
    }

    fn occurs(
        &self,
        query: &str,
    ) -> bool {
        self.texts
            .iter()
            .any(|t| !brute_offsets(&t.iter().collect::<String>(), query).is_empty())
    }

    /// Violations of the contract by the search for `query`.
    fn check(
        &self,
        query: &str,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        let limits = Limits {
            context_width: Some(CONTEXT),
            ..unlimited()
        };
        let result = search(self.session, self.graph, query, SearchView::Occurrences, limits);
        let q: Vec<char> = query.chars().collect();
        if result.query != query {
            errors.push(format!("result.query = {:?}", result.query));
        }

        let known: BTreeSet<char> = self.texts.iter().flatten().copied().collect();
        let unknown: BTreeSet<char> = q.iter().copied().filter(|c| !known.contains(c)).collect();
        let mut expected = BTreeSet::new();
        for (pos, text) in self.texts.iter().enumerate() {
            for offset in brute_offsets(&text.iter().collect::<String>(), query) {
                expected.insert((pos, offset));
            }
        }

        match &result.outcome {
            SearchOutcome::NotFound { unknown_chars } => {
                let reported: BTreeSet<char> = unknown_chars.iter().copied().collect();
                if unknown.is_empty() || reported != unknown {
                    errors.push(format!("not_found with {unknown_chars:?}, unknown {unknown:?}"));
                }
            },
            _ if !unknown.is_empty() => {
                errors.push(format!("unknown chars {unknown:?}, outcome {:?}", result.outcome))
            },
            SearchOutcome::Partial {
                host,
                span,
                matched_chars,
                query_chars,
            } => {
                if !expected.is_empty() {
                    errors.push("partial, but the query occurs".into());
                }
                // Plan 1.5: the longest matched query prefix.
                let longest = (1..=q.len())
                    .rev()
                    .find(|&n| self.occurs(&q[..n].iter().collect::<String>()))
                    .unwrap_or(0);
                if *query_chars != q.len() || *matched_chars != longest {
                    errors.push(format!(
                        "partial {matched_chars}/{query_chars}, expected {longest}/{}",
                        q.len()
                    ));
                }
                check_span(&mut errors, host, span, &q[..(*matched_chars).min(q.len())]);
            },
            SearchOutcome::Vertex { token } => {
                if expected.is_empty() {
                    errors.push("vertex, but the query occurs in no input".into());
                }
                if !self.vertices.contains(query) {
                    errors.push("vertex outcome for a non-vertex".into());
                }
                if token.label != query || token.width != q.len() {
                    errors.push(format!("vertex token {token:?}"));
                }
            },
            SearchOutcome::InHost { host, span } => {
                if expected.is_empty() {
                    errors.push("in_host, but the query occurs in no input".into());
                }
                if self.vertices.contains(query) {
                    errors.push("in_host outcome for a vertex".into());
                }
                if host.width <= q.len() {
                    errors.push(format!("host {host:?} not wider than the query"));
                }
                check_span(&mut errors, host, span, &q);
            },
        }

        let items = match &result.occurrences {
            Some(list) => {
                let count = list.items.len();
                if list.total != Some(count) || list.truncated {
                    errors.push(format!(
                        "total {:?}, truncated {}, {count} items",
                        list.total, list.truncated
                    ));
                }
                list.items.as_slice()
            },
            None if expected.is_empty() => &[],
            None => {
                errors.push("no occurrence list".into());
                &[]
            },
        };
        let mut found = BTreeSet::new();
        for item in items {
            let Some(&pos) = self.positions.get(&item.input_id) else {
                errors.push(format!("unknown input id {}", item.input_id));
                continue;
            };
            if !found.insert((pos, item.offset)) {
                errors.push(format!("duplicate occurrence {}@{}", item.input_id, item.offset));
            }
            let text = &self.texts[pos];
            if item.offset + q.len() > text.len() {
                continue;
            }
            let expected_item = OccurrenceItem {
                input_id: item.input_id,
                label: self.records[pos].label.clone(),
                offset: item.offset,
                line: 1 + text[..item.offset].iter().filter(|&&c| c == '\n').count(),
                column: 1 + item.offset
                    - text[..item.offset]
                        .iter()
                        .rposition(|&c| c == '\n')
                        .map_or(0, |newline| newline + 1),
                snippet: snippet(text, item.offset, q.len(), CONTEXT),
            };
            if *item != expected_item {
                errors.push(format!("occurrence {item:?}, expected {expected_item:?}"));
            }
        }
        if found != expected {
            errors.push(format!("occurrences {found:?} (input position, offset), expected {expected:?}"));
        }
        let ordered: Vec<(u32, usize)> = items.iter().map(|i| (i.input_id, i.offset)).collect();
        if !ordered.is_sorted() {
            errors.push("occurrences not ordered by input id, offset".into());
        }
        if let Some(list) = &result.occurrences {
            let with_matches = expected.iter().map(|(pos, _)| pos).collect::<BTreeSet<_>>().len();
            if list.inputs_with_matches != with_matches {
                errors.push(format!(
                    "inputs_with_matches {}, expected {with_matches}",
                    list.inputs_with_matches
                ));
            }
        }
        errors
    }
}

/// `span` lies in `host` and its text is `matched`.
fn check_span(
    errors: &mut Vec<String>,
    host: &TokenInfo,
    span: &Span,
    matched: &[char],
) {
    let label: Vec<char> = host.label.chars().collect();
    if host.label_truncated || label.len() != host.width {
        errors.push(format!("host label not complete: {host:?}"));
        return;
    }
    if span.start > span.end || span.end > host.width || span.end - span.start != matched.len() {
        errors.push(format!("span {span:?} in {host:?} for {} chars", matched.len()));
        return;
    }
    if label[span.start..span.end] != *matched {
        errors.push(format!("span {span:?} of {:?} is not {matched:?}", host.label));
    }
}
