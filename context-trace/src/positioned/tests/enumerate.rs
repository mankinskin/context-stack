//! Occurrence enumeration on every v3 fixture, against brute-force substring
//! search over the retained set.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use crate::{
    Token,
    positioned::{
        Occurrence,
        PositionedGraph,
        fixtures::v3_fixtures,
    },
};

type Graph = PositionedGraph<crate::BaseGraphKind>;

fn fixture(name: &str) -> Graph {
    let (_, fixture) = v3_fixtures().into_iter().find(|(n, _)| n == name).unwrap();
    Graph::from_interchange(&fixture.graph).unwrap()
}

fn token(
    graph: &Graph,
    content: &str,
) -> Token {
    graph.find(&content.chars().collect::<Vec<_>>()).unwrap()
}

fn occurrences(
    graph: &Graph,
    owner: &str,
    start: usize,
    end: usize,
) -> Vec<(usize, String)> {
    let view = graph
        .interval(token(graph, owner).index, start.into()..end.into())
        .unwrap();
    graph
        .occurrences_in(&view)
        .unwrap()
        .into_iter()
        .map(|o| (o.offset.0, graph.content(o.child.index).unwrap().into_iter().collect()))
        .collect()
}

/// Expected: every retained vertex (except the owner) whose content occurs
/// in the owner's content key wholly inside the interval, by string
/// comparison, sorted by `(offset, width)`.
#[test]
fn occurrences_in_equals_brute_force_for_every_interval() {
    let mut checked = 0;
    for (name, fixture) in v3_fixtures() {
        let graph = Graph::from_interchange(&fixture.graph).unwrap();
        let retained: Vec<(Vec<char>, Token)> = fixture
            .graph
            .vertices
            .keys()
            .map(|content| {
                let atoms: Vec<char> = content.chars().collect();
                let token = graph.find(&atoms).unwrap();
                (atoms, token)
            })
            .collect();
        for (owner_atoms, owner) in &retained {
            let width = owner_atoms.len();
            for start in 0..=width {
                for end in start..=width {
                    let mut expected: Vec<Occurrence> = Vec::new();
                    for (atoms, token) in &retained {
                        if token == owner || atoms.len() > end - start {
                            continue;
                        }
                        for offset in start..=end - atoms.len() {
                            if owner_atoms[offset..offset + atoms.len()] == atoms[..] {
                                expected.push(Occurrence::new(offset, *token));
                            }
                        }
                    }
                    expected.sort_by_key(|o| (o.offset, o.child.width));
                    let view = graph.interval(owner.index, start.into()..end.into()).unwrap();
                    assert_eq!(
                        graph.occurrences_in(&view).unwrap(),
                        expected,
                        "{name}: {:?}[{start}..{end}]",
                        owner_atoms.iter().collect::<String>()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 1000, "{checked}");
}

/// `abcde = [abc@0, bcd@1, cde@2]`: `1..3` cuts through `abc` and `bcd`
/// (no direct child lies inside), yet `bc` nested in both is found.
#[test]
fn interval_cutting_through_children_finds_nested_occurrences() {
    let graph = fixture("seam_bcd");
    let owner = graph.vertex(token(&graph, "abcde").index).unwrap();
    let cut = |o: &Occurrence| o.offset.0 < 3 && o.end().0 > 1;
    assert!(
        owner
            .composition()
            .unwrap()
            .occurrences()
            .iter()
            .filter(|o| cut(o))
            .all(|o| o.offset.0 < 1 || o.end().0 > 3),
        "every intersecting child crosses the boundary"
    );
    assert_eq!(
        occurrences(&graph, "abcde", 1, 3),
        [(1, "b".into()), (1, "bc".into()), (2, "c".into())]
    );
}

/// Number of occurrence paths from `owner` to each `(token, offset)`,
/// counted without deduplication.
fn path_counts(
    graph: &Graph,
    owner: Token,
) -> BTreeMap<(usize, Token), usize> {
    let mut counts = BTreeMap::new();
    let mut stack = vec![(owner, 0)];
    while let Some((vertex, base)) = stack.pop() {
        let Some(composition) = graph.vertex(vertex.index).unwrap().composition() else {
            continue;
        };
        for o in composition.occurrences() {
            let offset = base + o.offset.0;
            *counts.entry((offset, o.child)).or_default() += 1;
            stack.push((o.child, offset));
        }
    }
    counts
}

#[test]
fn occurrence_reached_through_two_paths_is_reported_once() {
    let graph = fixture("seam_bcd");
    let abcde = token(&graph, "abcde");
    let bc = token(&graph, "bc");
    // via abc@0 → bc@1 and bcd@1 → bc@0
    assert_eq!(path_counts(&graph, abcde)[&(1, bc)], 2);
    let whole = occurrences(&graph, "abcde", 0, 5);
    assert_eq!(whole.iter().filter(|o| **o == (1, "bc".to_string())).count(), 1);

    let mut shared = 0;
    for (name, fixture) in v3_fixtures() {
        let graph = Graph::from_interchange(&fixture.graph).unwrap();
        for vertex in graph.vertices() {
            let counts = path_counts(&graph, vertex.token());
            let view = graph.interval(vertex.index(), 0.into()..vertex.width().0.into()).unwrap();
            let found: Vec<(usize, Token)> = graph
                .occurrences_in(&view)
                .unwrap()
                .into_iter()
                .map(|o| (o.offset.0, o.child))
                .collect();
            let mut keys: Vec<_> = counts.keys().copied().collect();
            keys.sort_by_key(|(offset, token)| (*offset, token.width));
            assert_eq!(found, keys, "{name}: vertex {}", vertex.index().0);
            shared += counts.values().filter(|&&n| n > 1).count();
        }
    }
    assert!(shared > 0, "some fixture reaches an occurrence through two paths");
}
