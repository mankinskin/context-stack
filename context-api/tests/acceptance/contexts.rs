//! Plan 1.8 test 4: the `contexts` view (distinct ancestors of the match)
//! and the `parents` view.

use std::collections::{
    BTreeMap,
    BTreeSet,
    HashMap,
};

use context_api::{
    types::*,
    Session,
};

use crate::{
    common::*,
    corpora::random_corpora,
};

const CONTEXT: usize = 2;

fn corpora() -> Vec<Vec<String>> {
    let mut corpora = vec![
        strings(&["abab", "babab", "aabba"]),
        strings(&["abcabc", "bcab", "cabca"]),
        strings(&["aaaa", "aaa"]),
    ];
    corpora.extend(random_corpora(0x5eed_0002, 40));
    corpora
}

/// Contract violations found by the contexts view over all substring queries
/// of `corpora()`: `(soundness, completeness)`.
fn check_contexts() -> (Failures, Failures) {
    let env = Env::with(|config| config.keep_open = true);
    let session = &env.session;
    let mut sound = Failures::default();
    let mut complete = Failures::default();
    for (i, corpus) in corpora().iter().enumerate() {
        let graph = format!("c{i}");
        create(session, &graph, corpus);
        let vertices: Vec<String> = export(session, &graph).graph.vertices.into_keys().collect();
        let mut labels = Labels::new(session, &graph);
        for query in substrings(corpus) {
            let q: Vec<char> = query.chars().collect();
            let limits = Limits {
                context_width: Some(CONTEXT),
                ..unlimited()
            };
            let result = search(session, &graph, &query, SearchView::Contexts, limits);
            let at = |what: String| format!("{corpus:?} / {query:?}: {what}");
            let Some(list) = result.contexts else {
                sound.check(false, || at("no context list".into()));
                continue;
            };
            sound.check(!list.truncated && list.total == Some(list.items.len()), || {
                at(format!("total {:?}, truncated {}", list.total, list.truncated))
            });
            let mut seen = BTreeSet::new();
            let mut reported = BTreeMap::new();
            for item in &list.items {
                let label = labels.get(item.ancestor.index);
                let text: Vec<char> = label.chars().collect();
                sound.check(seen.insert(item.ancestor.index), || {
                    at(format!("duplicate ancestor {:?}", item.ancestor))
                });
                sound.check(item.ancestor.width == text.len(), || {
                    at(format!("ancestor width {:?} vs label {label:?}", item.ancestor))
                });
                sound.check(!item.offsets.is_empty() && item.offsets.is_sorted(), || {
                    at(format!("offsets {:?} empty or not ascending", item.offsets))
                });
                sound.check(
                    item.offsets.windows(2).all(|w| w[0] < w[1]),
                    || at(format!("duplicate offsets {:?}", item.offsets)),
                );
                for &offset in &item.offsets {
                    sound.check(
                        offset + q.len() <= text.len()
                            && text[offset..offset + q.len()] == q[..],
                        || at(format!("{label:?} has no match at {offset}")),
                    );
                }
                if let Some(&first) = item.offsets.first() {
                    if first + q.len() <= text.len() {
                        let expected = snippet(&text, first, q.len(), CONTEXT);
                        sound.check(item.snippet == expected, || {
                            at(format!("snippet {:?}, expected {expected:?}", item.snippet))
                        });
                    }
                }
                reported.insert(label, item.offsets.clone());
            }
            sound.check(
                list.items.windows(2).all(|w| w[0].ancestor.width <= w[1].ancestor.width),
                || at("items not ordered by ancestor width".into()),
            );

            // Every vertex containing the query, except the query itself
            // when it is a vertex, at every offset (plan 1.5: the host
            // assumption; a violation here is a counterexample to it).
            let expected: BTreeMap<String, Vec<usize>> = vertices
                .iter()
                .filter(|v| **v != query)
                .map(|v| (v.clone(), brute_offsets(v, &query)))
                .filter(|(_, offsets)| !offsets.is_empty())
                .collect();
            complete.check(reported == expected, || {
                at(format!("contexts {reported:?}, expected {expected:?}"))
            });
        }
    }
    (sound, complete)
}

#[test]
fn contexts_match_the_query_at_every_offset_without_duplicates() {
    check_contexts().0.assert_none("contexts soundness");
}

#[test]
fn contexts_are_every_vertex_containing_the_query() {
    check_contexts().1.assert_none("contexts completeness");
}

#[test]
fn contexts_work_budget_truncates_and_drops_the_total() {
    let env = Env::new();
    create(&env.session, "g", &strings(&["ab", "ba", "aab", "bab", "abba"]));
    let full = search(&env.session, "g", "a", SearchView::Contexts, unlimited())
        .contexts
        .expect("context list");
    assert!(full.items.len() >= 3, "precondition: many contexts, got {full:?}");
    assert!(!full.truncated);

    let limits = Limits {
        max_visited: Some(1),
        ..unlimited()
    };
    let cut = search(&env.session, "g", "a", SearchView::Contexts, limits)
        .contexts
        .expect("context list");
    assert!(cut.truncated, "{cut:?}");
    assert_eq!(cut.total, None);
    assert!(cut.items.len() < full.items.len());

    let limits = Limits {
        limit: Some(1),
        ..unlimited()
    };
    let one = search(&env.session, "g", "a", SearchView::Contexts, limits)
        .contexts
        .expect("context list");
    assert_eq!(one.items.len(), 1);
    assert_eq!(one.total, Some(full.items.len()), "the item limit does not change the total");
    assert_eq!(
        one.items[0].ancestor.width, full.items[0].ancestor.width,
        "the narrowest context comes first"
    );
}

#[test]
fn parents_are_the_compositions_containing_the_vertex() {
    let env = Env::with(|config| config.keep_open = true);
    let mut failures = Failures::default();
    for (i, corpus) in corpora().iter().enumerate() {
        let graph = format!("c{i}");
        create(&env.session, &graph, corpus);
        let vertices = export(&env.session, &graph).graph.vertices;
        for query in vertices.keys() {
            let mut expected: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for (owner, cover) in &vertices {
                for (offset, child) in cover {
                    if child == query {
                        expected.entry(owner.clone()).or_default().push(*offset);
                    }
                }
            }
            let result = search(&env.session, &graph, query, SearchView::Parents, unlimited());
            let at = |what: String| format!("{corpus:?} / {query:?}: {what}");
            let Some(parents) = result.parents else {
                failures.check(false, || at("no parent list".into()));
                continue;
            };
            let mut reported = BTreeMap::new();
            for parent in &parents {
                failures.check(!parent.parent.label_truncated, || at("truncated label".into()));
                if reported.insert(parent.parent.label.clone(), parent.offsets.clone()).is_some() {
                    failures.check(false, || at(format!("duplicate parent {:?}", parent.parent)));
                }
            }
            failures.check(reported == expected, || {
                at(format!("parents {reported:?}, expected {expected:?}"))
            });
        }
    }
    failures.assert_none("parents");
}

/// Full vertex labels by index, through the `vertex` command.
struct Labels<'a> {
    session: &'a Session,
    graph: &'a str,
    cache: HashMap<usize, String>,
}

impl<'a> Labels<'a> {
    fn new(
        session: &'a Session,
        graph: &'a str,
    ) -> Self {
        Self {
            session,
            graph,
            cache: HashMap::new(),
        }
    }

    fn get(
        &mut self,
        index: usize,
    ) -> String {
        self.cache
            .entry(index)
            .or_insert_with(|| label(self.session, self.graph, index))
            .clone()
    }
}
