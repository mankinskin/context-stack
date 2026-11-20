//! Shared helpers: fixtures through the name map and brute-force
//! expectations from the fixture strings.

use std::collections::BTreeMap;

use context_trace::{
    positioned::{
        fixtures::{
            fixture_graphs,
            FixtureGraph,
        },
        OccurrenceLocation,
    },
    AtomPosition,
    Token,
};

pub(super) fn fixture(name: &str) -> FixtureGraph {
    fixture_graphs()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("fixture {name}"))
}

pub(super) fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

pub(super) fn pos(p: usize) -> AtomPosition {
    AtomPosition::from(p)
}

pub(super) fn span(
    start: usize,
    end: usize,
) -> std::ops::Range<AtomPosition> {
    pos(start)..pos(end)
}

pub(super) fn step(
    parent: Token,
    offset: usize,
    child: Token,
) -> OccurrenceLocation {
    OccurrenceLocation {
        parent: parent.index,
        offset: pos(offset),
        child: child.index,
    }
}

/// Start offsets of `needle` in `haystack`.
pub(super) fn find_all(
    haystack: &[char],
    needle: &[char],
) -> Vec<usize> {
    (0..(haystack.len() + 1).saturating_sub(needle.len()))
        .filter(|&x| haystack[x..x + needle.len()] == *needle)
        .collect()
}

/// Brute-force expectation for an atom query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Expected {
    /// Length of the longest query prefix occurring in a corpus text.
    pub(super) k: usize,
    /// Minimal-width fixture key containing that prefix.
    pub(super) root: String,
    /// Offset of the prefix in `root`.
    pub(super) offset: usize,
}

/// `k` = longest prefix of `query` in a corpus text, `root` = the unique
/// minimal-width fixture key containing `query[..k]`, at the unique offset.
/// Panics on ambiguity (an oracle or model issue).
pub(super) fn expected(
    corpus: &[String],
    names: &BTreeMap<String, Token>,
    query: &str,
) -> Expected {
    let q = chars(query);
    let k = (1..=q.len())
        .rev()
        .find(|&k| {
            corpus
                .iter()
                .any(|text| !find_all(&chars(text), &q[..k]).is_empty())
        })
        .unwrap_or_else(|| panic!("{query}: first atom not in the corpus"));
    let containing: Vec<(&String, Vec<usize>)> = names
        .keys()
        .map(|key| (key, find_all(&chars(key), &q[..k])))
        .filter(|(_, offsets)| !offsets.is_empty())
        .collect();
    let width = containing
        .iter()
        .map(|(key, _)| key.chars().count())
        .min()
        .unwrap();
    let minimal: Vec<_> = containing
        .into_iter()
        .filter(|(key, _)| key.chars().count() == width)
        .collect();
    assert_eq!(minimal.len(), 1, "{query}: ambiguous root {minimal:?}");
    let (root, offsets) = &minimal[0];
    assert_eq!(offsets.len(), 1, "{query}: ambiguous offset in {root}");
    Expected {
        k,
        root: (*root).clone(),
        offset: offsets[0],
    }
}
