//! `read_interval` and the search navigation primitives
//! (`start_at`, `end_at`, `prefix_child`, `parent_occurrences`, `atom_token`,
//! `advance_end_path`) on every fixture. Vertices are
//! addressed through the name map; expectations come from brute-force string
//! matching over the fixture keys.

use pretty_assertions::assert_eq;

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
                    let view = fixture
                        .graph
                        .interval(token.index, start.into()..end.into())
                        .unwrap();
                    assert_eq!(
                        fixture.graph.read_interval(&view).as_deref(),
                        Some(&content[start..end]),
                        "{}: {key:?}[{start}..{end}]",
                        fixture.name
                    );
                }
            }
            let beyond = AtomPosition::from(content.len() + 1);
            assert!(fixture.graph.interval(token.index, beyond..beyond).is_none());
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
    assert!(graph.contains_path(steps), "{context}");
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
                let found = graph.start_at(*token, p);
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
                assert_eq!(graph.start_at(*token, p), None, "{}: {key:?} @ {p}", fixture.name);
            }
        }
    }
}

#[test]
fn end_at_is_longest_key_ending_at_position() {
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let content = chars(key);
            let width = content.len();
            assert_eq!(graph.end_at(*token, 0), None);
            assert_eq!(graph.end_at(*token, width + 1), None);
            for e in 1..=width {
                let context = format!("{}: {key:?} ending @ {e}", fixture.name);
                let found = graph.end_at(*token, e);
                let expected = fixture
                    .names
                    .iter()
                    .filter(|(other, _)| *other != key && content[..e].ends_with(&chars(other)))
                    .max_by_key(|(other, _)| other.chars().count())
                    .map(|(_, token)| *token);
                let Some(expected) = expected else {
                    assert_eq!(width, 1, "{context}: composite without key at position");
                    assert_eq!(found, None, "{context}");
                    continue;
                };
                let (found, steps) = found.unwrap_or_else(|| panic!("{context}: None"));
                assert_eq!(found, expected, "{context}");
                assert_eq!(
                    chain_offset(graph, &steps, *token, expected, &context) + expected.width.0,
                    AtomPosition::from(e),
                    "{context}"
                );
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
                std::iter::successors(graph.prefix_child(*token), |o| graph.prefix_child(o.child))
                    .inspect(|o| assert_eq!(o.offset, AtomPosition::from(0)))
                    .map(|o| o.child)
                    .collect();
            assert_eq!(chain, expected, "{}: {key:?}", fixture.name);
            if let Some(composition) = graph.vertex(token.index).unwrap().composition() {
                assert_eq!(Some(*composition.prefix()), graph.prefix_child(*token));
            }
        }
    }
}

#[test]
fn covering_is_last_occurrence_starting_at_or_before() {
    for fixture in fixture_graphs() {
        for vertex in fixture.graph.vertices() {
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
            let found: Vec<_> = graph.parent_occurrences(*token).collect();
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
                assert_eq!(fixture.graph.atom_token(&atom), Some(*token));
            }
        }
        assert_eq!(fixture.graph.atom_token(&'\u{0}'), None);
    }
}

#[test]
fn advance_end_path_equals_start_at() {
    let mut names = std::collections::BTreeSet::new();
    let mut corrected = 0;
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let width = key.chars().count();
            for p0 in 0..width {
                let Some((_, old)) = graph.start_at(*token, p0) else {
                    continue;
                };
                for p in p0 + 1..=width + 1 {
                    let context = format!("{}: {key:?} {p0} → {p}", fixture.name);
                    let expected = graph.start_at(*token, p);
                    assert_eq!(
                        graph.advance_end_path(*token, old.clone(), p),
                        expected,
                        "{context}"
                    );
                    if expected.is_some() && naive_pop(graph, &old, p) != expected {
                        corrected += 1;
                        names.insert(fixture.name.clone());
                    }
                }
            }
        }
    }
    // The overlap fixtures exercise the corrected pop rule.
    assert!(corrected > 0);
    for name in ["seam_bcd", "complex_overlap"] {
        assert!(names.contains(name), "{name} not exercised: {names:?}");
    }
}

/// Legacy-style pop: keep steps while their child covers `p`, then descend
/// with `start_at`. Wrong on overlaps (search plan, "End-path update").
fn naive_pop(
    graph: &crate::positioned::PositionedGraph,
    old: &[OccurrenceLocation],
    p: usize,
) -> Option<(Token, Vec<OccurrenceLocation>)> {
    let mut base = 0;
    let mut kept = Vec::new();
    for step in old {
        let start = base + step.offset.0;
        let width = graph.vertex(step.child).unwrap().width().0;
        if !(start < p && p < start + width) {
            break;
        }
        kept.push(*step);
        base = start;
    }
    let parent = kept.last().map_or(old[0].parent, |s| s.child);
    let (token, steps) = graph.start_at(parent, p - base)?;
    kept.extend(steps);
    Some((token, kept))
}
