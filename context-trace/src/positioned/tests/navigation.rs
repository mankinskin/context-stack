//! `read_interval` and `occurrences_in` on every v3 fixture. Vertices are
//! addressed through the name map; expectations come from brute-force string
//! matching over the fixture keys.

use pretty_assertions::assert_eq;

use crate::{
    AtomPosition,
    positioned::{
        Occurrence,
        fixtures::v3_graphs,
    },
};

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[test]
fn read_interval_equals_fixture_substrings() {
    for fixture in v3_graphs() {
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

#[test]
fn occurrences_in_equals_all_fixture_key_occurrences() {
    for fixture in v3_graphs() {
        for (key, token) in &fixture.names {
            let content = chars(key);
            for start in 0..=content.len() {
                for end in start..=content.len() {
                    let interval = &content[start..end];
                    let mut expected: Vec<Occurrence> = fixture
                        .names
                        .iter()
                        .filter(|(other, _)| *other != key)
                        .flat_map(|(other, other_token)| {
                            let other = chars(other);
                            (0..interval.len().saturating_sub(other.len() - 1))
                                .filter(move |&p| interval[p..p + other.len()] == other[..])
                                .map(move |p| Occurrence::new(start + p, *other_token))
                        })
                        .collect();
                    expected.sort_by_key(|o| (o.offset, o.child.width));
                    let view = fixture
                        .graph
                        .interval(token.index, start.into()..end.into())
                        .unwrap();
                    assert_eq!(
                        fixture.graph.occurrences_in(&view),
                        Some(expected),
                        "{}: {key:?}[{start}..{end}]",
                        fixture.name
                    );
                }
            }
        }
    }
}
