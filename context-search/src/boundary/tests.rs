//! Boundary search on every fixture. Vertices are addressed through the
//! name map; expectations are the fixture keys starting (or ending) at the
//! position by brute-force string matching.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use context_trace::{
    AtomPosition,
    Token,
    positioned::fixtures::fixture_graphs,
};

use super::{
    Boundary,
    BoundaryOccurrence,
    boundary_occurrences,
    postfixes,
    prefixes,
};

/// Keys other than `owner` touching `position` of `owner` at `boundary`,
/// longest first.
fn expected(
    names: &BTreeMap<String, Token>,
    owner: &str,
    position: usize,
    boundary: Boundary,
) -> Vec<Token> {
    let content: Vec<char> = owner.chars().collect();
    let mut tokens: Vec<Token> = names
        .iter()
        .filter(|(key, _)| key.as_str() != owner)
        .filter(|(key, _)| {
            let key: Vec<char> = key.chars().collect();
            match boundary {
                Boundary::Start => content[position..].starts_with(&key),
                Boundary::End => content[..position].ends_with(&key),
            }
        })
        .map(|(_, token)| *token)
        .collect();
    tokens.sort_by(|a, b| b.width.cmp(&a.width).then(a.index.cmp(&b.index)));
    tokens
}

fn check_paths(
    graph: &context_trace::positioned::PositionedGraph,
    owner: Token,
    found: &[BoundaryOccurrence],
    position: usize,
    boundary: Boundary,
    context: &str,
) {
    for occurrence in found {
        let path = &occurrence.path;
        assert!(graph.contains_path(path), "{context}: {path:?}");
        assert_eq!(path.root(), owner.index, "{context}");
        assert_eq!(path.target(), occurrence.token.index, "{context}");
        let touching = match boundary {
            Boundary::Start => occurrence.offset(),
            Boundary::End => occurrence.end(),
        };
        assert_eq!(touching, AtomPosition::from(position), "{context}");
    }
}

#[test]
fn boundary_occurrences_equal_fixture_keys_at_every_position() {
    for fixture in fixture_graphs() {
        for (key, &owner) in &fixture.names {
            for position in 0..=owner.width.0 {
                for boundary in [Boundary::Start, Boundary::End] {
                    let context = format!("{}: {key:?}@{position} {boundary:?}", fixture.name);
                    let found =
                        boundary_occurrences(&fixture.graph, owner.index, position.into(), boundary)
                            .unwrap();
                    let tokens: Vec<Token> = found.iter().map(|o| o.token).collect();
                    assert_eq!(
                        tokens,
                        expected(&fixture.names, key, position, boundary),
                        "{context}"
                    );
                    check_paths(&fixture.graph, owner, &found, position, boundary, &context);
                }
            }
            let beyond = AtomPosition::from(owner.width.0 + 1);
            assert!(
                boundary_occurrences(&fixture.graph, owner.index, beyond, Boundary::Start)
                    .is_none()
            );
        }
    }
}

#[test]
fn prefixes_and_postfixes_equal_fixture_keys() {
    for fixture in fixture_graphs() {
        for (key, &owner) in &fixture.names {
            let width = owner.width.0;
            for (found, position, boundary) in [
                (prefixes(&fixture.graph, owner.index).unwrap(), 0, Boundary::Start),
                (postfixes(&fixture.graph, owner.index).unwrap(), width, Boundary::End),
            ] {
                let context = format!("{}: {key:?} {boundary:?}", fixture.name);
                let tokens: Vec<Token> = found.iter().map(|o| o.token).collect();
                assert_eq!(
                    tokens,
                    expected(&fixture.names, key, position, boundary),
                    "{context}"
                );
                check_paths(&fixture.graph, owner, &found, position, boundary, &context);
            }
        }
    }
}
