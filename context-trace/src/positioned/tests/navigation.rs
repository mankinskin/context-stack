//! `read_interval` and the navigation primitives (`start_at`,
//! `prefix_child`, `parent_occurrences`, `GraphRead::atom`) on every
//! fixture; end paths are tested in context-search. Vertices are
//! addressed through the name map; expectations come from brute-force string
//! matching over the fixture keys.

use pretty_assertions::assert_eq;

use crate::{
    ToToken,
    positioned::{
        GraphRead,
        Navigate,
    },
};
use crate::{
    AtomPosition,
    Token,
    positioned::{
        OccurrenceLocation,
        PositionedGraph,
        fixtures::fixture_graphs,
    },
};

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[test]
fn read_interval_equals_fixture_substrings() {
    for fixture in fixture_graphs() {
        for (key, token) in &fixture.names {
            let content = chars(key);
            for start in 0..=content.len() {
                for end in start..=content.len() {
                    let vertex = fixture.graph.vertex(token).unwrap();
                    let view = vertex.interval(start.into()..end.into()).unwrap();
                    assert_eq!(
                        vertex.read_interval(view.range).as_deref(),
                        Some(&content[start..end]),
                        "{}: {key:?}[{start}..{end}]",
                        fixture.name
                    );
                }
            }
            let beyond = AtomPosition::from(content.len() + 1);
            let vertex = fixture.graph.vertex(token).unwrap();
            assert!(vertex.interval(beyond..beyond).is_none());
        }
    }
}

/// Checks that `steps` is a stored chain from `root` to `target` and
/// returns the offset of `target` in `root`.
fn chain_offset(
    graph: &PositionedGraph,
    steps: &[OccurrenceLocation],
    root: Token,
    target: Token,
    context: &str,
) -> AtomPosition {
    assert!(steps.windows(2).all(|w| w[0].child == w[1].parent), "{context}");
    assert!(graph.vertex(root).unwrap().contains_path(steps), "{context}");
    assert_eq!(steps.first().map(|s| s.parent), Some(root.index), "{context}");
    assert_eq!(steps.last().map(|s| s.child), Some(target.index), "{context}");
    steps
        .iter()
        .fold(AtomPosition::default(), |sum, step| sum + step.offset.0)
}

/// Longest fixture key other than `owner` starting at `position` of
/// `owner`'s string.
fn longest_key_at<'a>(
    names: &'a std::collections::BTreeMap<String, Token>,
    owner: &str,
    position: usize,
) -> Option<(&'a str, Token)> {
    let rest = &chars(owner)[position..];
    names
        .iter()
        .filter(|(key, _)| key.as_str() != owner && rest.starts_with(&chars(key)))
        .max_by_key(|(key, _)| key.chars().count())
        .map(|(key, token)| (key.as_str(), *token))
}

#[test]
fn start_at_is_longest_key_starting_at_position() {
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let width = key.chars().count();
            for p in 0..width {
                let context = format!("{}: {key:?} @ {p}", fixture.name);
                let found = graph.vertex(*token).and_then(|v| v.start_at(p));
                let Some((_, expected)) = longest_key_at(&fixture.names, key, p) else {
                    assert_eq!(width, 1, "{context}: composite without key at position");
                    assert_eq!(found, None, "{context}");
                    continue;
                };
                let (found, steps) = found.unwrap_or_else(|| panic!("{context}: None"));
                assert_eq!(found, expected, "{context}");
                assert_eq!(
                    chain_offset(graph, &steps, *token, expected, &context),
                    AtomPosition::from(p),
                    "{context}"
                );
            }
            for p in [width, width + 1] {
                let found = graph.vertex(*token).unwrap().start_at(p);
                assert_eq!(found, None, "{}: {key:?} @ {p}", fixture.name);
            }
        }
    }
}

#[test]
fn prefix_child_chain_is_all_prefix_keys_longest_first() {
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let content = chars(key);
            let mut expected: Vec<(usize, Token)> = fixture
                .names
                .iter()
                .filter(|(other, _)| *other != key && content.starts_with(&chars(other)))
                .map(|(other, token)| (other.chars().count(), *token))
                .collect();
            expected.sort_by(|a, b| b.0.cmp(&a.0));
            let expected: Vec<Token> = expected.into_iter().map(|(_, t)| t).collect();
            let chain: Vec<Token> =
                std::iter::successors(graph.vertex(*token).unwrap().prefix_child(), |o| {
                    graph.vertex(o.child).unwrap().prefix_child()
                })
                    .inspect(|o| assert_eq!(o.offset, AtomPosition::from(0)))
                    .map(|o| o.child)
                    .collect();
            assert_eq!(chain, expected, "{}: {key:?}", fixture.name);
            let mut parent = *token;
            let located: Vec<Token> = graph
                .vertex(*token)
                .unwrap()
                .prefixes()
                .map(|(location, prefix)| {
                    assert_eq!(
                        (location.parent, location.offset, location.child),
                        (parent.index, AtomPosition::from(0), prefix.index),
                        "{}: {key:?}",
                        fixture.name
                    );
                    parent = prefix;
                    prefix
                })
                .collect();
            assert_eq!(located, expected, "{}: prefixes of {key:?}", fixture.name);
            if let Some(composition) = graph.vertex(token.index).unwrap().composition() {
                let prefix = graph.vertex(*token).unwrap().prefix_child();
                assert_eq!(Some(*composition.prefix()), prefix);
            }
        }
    }
}

/// On every fixture graph, `postfixes(v)` is every vertex whose string is a
/// proper suffix of `v`'s, largest first, each located at the end of its
/// predecessor on the chain (moved from the reader's `expand::postfixes`).
#[test]
fn postfixes_are_all_postfix_tokens() {
    for fixture in fixture_graphs() {
        let keys: std::collections::BTreeMap<_, _> = fixture
            .names
            .iter()
            .map(|(key, token)| (token.index, key.as_str()))
            .collect();
        for (key, token) in &fixture.names {
            let mut expected: Vec<&str> = fixture
                .names
                .keys()
                .filter(|other| other.len() < key.len() && key.ends_with(other.as_str()))
                .map(String::as_str)
                .collect();
            expected.sort_by_key(|other| std::cmp::Reverse(other.len()));
            let mut parent = *token;
            let found: Vec<&str> = fixture
                .graph
                .vertex(*token)
                .unwrap()
                .postfixes()
                .map(|(location, postfix)| {
                    assert_eq!(
                        (location.parent, location.offset.0 + postfix.width.0, location.child),
                        (parent.index, parent.width.0, postfix.index),
                        "{}: {key:?}",
                        fixture.name
                    );
                    parent = postfix;
                    keys[&postfix.index]
                })
                .collect();
            assert_eq!(found, expected, "{}: postfixes of {}", fixture.name, key);
        }
    }
}

#[test]
fn covering_is_last_occurrence_starting_at_or_before() {
    for fixture in fixture_graphs() {
        for vertex in fixture.graph.vertices.iter() {
            let Some(composition) = vertex.composition() else {
                continue;
            };
            let width = vertex.width().0;
            for p in 0..width {
                let expected = composition
                    .occurrences()
                    .iter()
                    .rfind(|o| o.offset.0 <= p)
                    .unwrap();
                let found = composition.covering(p).unwrap();
                assert_eq!(found, expected, "{}: {vertex} @ {p}", fixture.name);
                assert!(found.offset.0 <= p && p < found.end().0);
            }
            assert_eq!(composition.covering(width), None);
        }
    }
}

/// Start offsets of `needle` in `haystack`.
fn find_all(
    haystack: &[char],
    needle: &[char],
) -> impl Iterator<Item = usize> {
    (0..(haystack.len() + 1).saturating_sub(needle.len()))
        .filter(move |&x| haystack[x..x + needle.len()] == *needle)
}

#[test]
fn parent_occurrences_are_maximal_key_occurrences() {
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let content = chars(key);
            // `key` at `x` in `parent` is a composition occurrence iff no other
            // proper key of `parent` occurring there contains it.
            let mut expected: Vec<(Token, AtomPosition)> = Vec::new();
            for (parent_key, parent_token) in &fixture.names {
                let parent = chars(parent_key);
                if parent.len() <= content.len() {
                    continue;
                }
                for x in find_all(&parent, &content) {
                    let contained = fixture.names.keys().any(|other| {
                        let other = chars(other);
                        other.len() > content.len()
                            && other.len() < parent.len()
                            && find_all(&parent, &other)
                                .any(|y| y <= x && x + content.len() <= y + other.len())
                    });
                    if !contained {
                        expected.push((*parent_token, AtomPosition::from(x)));
                    }
                }
            }
            expected.sort_by_key(|(parent, offset)| (parent.width, parent.index, *offset));
            let found: Vec<_> = graph.vertex(*token).unwrap().parent_occurrences().collect();
            assert_eq!(found, expected, "{}: {key:?}", fixture.name);
        }
    }
}

#[test]
fn atom_token_finds_atoms_only() {
    for fixture in fixture_graphs() {
        for (key, token) in &fixture.names {
            let mut atoms = key.chars();
            if let (Some(atom), None) = (atoms.next(), atoms.next()) {
                assert_eq!(fixture.graph.atom(&atom).map(|v| v.to_token()), Some(*token));
            }
        }
        assert_eq!(fixture.graph.atom(&'\u{0}').map(|v| v.to_token()), None);
    }
}
