//! End-path navigation ([`PathNavigate`]) on every fixture: `end_at` is
//! the longest key ending at a position, `advance_end_path` equals
//! `start_at`. Expectations come from brute-force string matching over the
//! fixture keys.

use pretty_assertions::assert_eq;

use crate::path::PathNavigate as _;
use context_trace::{
    AtomPosition,
    Token,
    Wide,
    positioned::{
        GraphRead,
        Navigate,
        OccurrenceLocation,
        PositionedGraph,
        fixtures::fixture_graphs,
    },
};

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
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

#[test]
fn end_at_is_longest_key_ending_at_position() {
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let content = chars(key);
            let width = content.len();
            assert_eq!(graph.vertex(*token).and_then(|v| v.end_at(0)), None);
            assert_eq!(graph.vertex(*token).and_then(|v| v.end_at(width + 1)), None);
            for e in 1..=width {
                let context = format!("{}: {key:?} ending @ {e}", fixture.name);
                let found = graph.vertex(*token).and_then(|v| v.end_at(e));
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
fn advance_end_path_equals_start_at() {
    let mut names = std::collections::BTreeSet::new();
    let mut corrected = 0;
    for fixture in fixture_graphs() {
        let graph = &fixture.graph;
        for (key, token) in &fixture.names {
            let width = key.chars().count();
            for p0 in 0..width {
                let Some((_, old)) = graph.vertex(*token).and_then(|v| v.start_at(p0)) else {
                    continue;
                };
                for p in p0 + 1..=width + 1 {
                    let context = format!("{}: {key:?} {p0} → {p}", fixture.name);
                    let expected = graph.vertex(*token).and_then(|v| v.start_at(p));
                    assert_eq!(
                        graph.vertex(*token).and_then(|v| v.advance_end_path(old.clone(), p)),
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
    graph: &PositionedGraph,
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
    let (token, steps) = graph.vertex(parent).and_then(|v| v.start_at(p - base))?;
    kept.extend(steps);
    Some((token, kept))
}
