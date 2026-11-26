//! Model-only rules: complete compositions on create and reciprocal links.
//! Structural cover rules are tested on the interchange validator; the edit
//! layer in context-insert. Graphs are built as tables (interchange import).

use std::collections::BTreeMap;

use crate::{
    AtomPosition,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        Composition,
        GraphRead,
        Navigate,
        Occurrence,
        OccurrenceLocation,
        PositionedError,
        PositionedGraph,
        VertexNode,
        interchange::PositionedGraphData,
    },
};

/// A graph of the given composites as a table (atoms are added for every
/// character), and the name → token map.
pub(super) fn named(
    composites: &[(&str, &[(usize, &str)])]
) -> (PositionedGraph, BTreeMap<String, Token>) {
    let mut vertices: BTreeMap<String, Vec<(usize, String)>> = composites
        .iter()
        .map(|(content, cover)| {
            let cover = cover.iter().map(|&(offset, child)| (offset, child.to_string()));
            (content.to_string(), cover.collect())
        })
        .collect();
    for atom in composites.iter().flat_map(|(content, _)| content.chars()) {
        vertices.entry(atom.to_string()).or_default();
    }
    PositionedGraph::from_interchange_named(&PositionedGraphData { vertices }).unwrap()
}

/// `ababa = [aba@0, aba@2]`, `aba = [a@0, b@1, a@2]`, built as a table, and
/// the name → token map.
fn ababa_named() -> (PositionedGraph, BTreeMap<String, Token>) {
    named(&[
        ("aba", &[(0, "a"), (1, "b"), (2, "a")]),
        ("ababa", &[(0, "aba"), (2, "aba")]),
    ])
}

/// [`ababa_named`] with the tokens of `aba` and `ababa`.
fn ababa() -> (PositionedGraph, Token, Token) {
    let (graph, names) = ababa_named();
    (graph, names["aba"], names["ababa"])
}

#[test]
fn validate_rejects_broken_reciprocal_links() {
    let (graph, aba, ababa) = ababa();
    let mut missing = graph.clone();
    missing.vertices[aba.index.0]
        .parents
        .get_mut(&ababa.index)
        .unwrap()
        .remove(&AtomPosition::from(2));
    assert_eq!(
        missing.validate(),
        Err(PositionedError::ParentLink {
            parent: ababa.index,
            child: aba.index,
            offset: AtomPosition::from(2),
        })
    );
    let mut stale = graph;
    stale.vertices[aba.index.0]
        .parents
        .get_mut(&ababa.index)
        .unwrap()
        .insert(AtomPosition::from(1));
    assert_eq!(
        stale.validate(),
        Err(PositionedError::ParentLink {
            parent: ababa.index,
            child: aba.index,
            offset: AtomPosition::from(1),
        })
    );
}

#[test]
fn occurrence_paths_and_intervals_are_checked_against_the_graph() {
    let (graph, names) = ababa_named();
    let (b, aba, ababa) = (names["b"], names["aba"], names["ababa"]);
    let step = |parent: Token, offset: usize, child: Token| OccurrenceLocation {
        parent: parent.index,
        offset: offset.into(),
        child: child.index,
    };
    assert!(graph.vertex(ababa).unwrap().contains_path(&[step(ababa, 2, aba), step(aba, 1, b)]));
    assert!(!graph.vertex(ababa).unwrap().contains_path(&[step(ababa, 1, aba)]));

    let range = |start: usize, end: usize| AtomPosition::from(start)..AtomPosition::from(end);
    assert!(graph.vertex(ababa.index).and_then(|v| v.interval(range(1, 4))).is_some());
    assert!(graph.vertex(ababa.index).and_then(|v| v.interval(range(3, 6))).is_none());
}

/// A second `ab` added by field access is rejected by `validate` (the
/// `create` half is in context-insert).
#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn duplicate_content_is_rejected_by_validate() {
    let (mut graph, names) = named(&[
        ("aba", &[(0, "a"), (1, "b"), (2, "a")]),
        ("ababa", &[(0, "aba"), (2, "aba")]),
        ("ab", &[(0, "a"), (1, "b")]),
    ]);
    let [a, b, ab] = ["a", "b", "ab"].map(|name| names[name]);
    graph.validate().unwrap();
    let again = VertexIndex(graph.vertex_count());
    let mut copy = graph.vertices[ab.index.0].clone();
    copy.index = again;
    graph.vertices.push(copy);
    for child in [a, b] {
        let parents = &mut graph.vertices[child.index.0].parents;
        parents.insert(again, parents[&ab.index].clone());
    }
    assert_eq!(
        graph.validate(),
        Err(PositionedError::DuplicateContent {
            first: ab.index,
            second: again,
        })
    );
}

/// Overlapping occurrences that disagree are rejected by `validate` (the
/// `create` half is in context-insert).
#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn overlap_disagreement_is_rejected_by_validate() {
    let (graph, names) = named(&[
        ("aba", &[(0, "a"), (1, "b"), (2, "a")]),
        ("ababa", &[(0, "aba"), (2, "aba")]),
        ("ab", &[(0, "a"), (1, "b")]),
        ("ca", &[(0, "c"), (1, "a")]),
        ("abc", &[(0, "a"), (1, "b"), (2, "c")]),
    ]);
    let [a, b, c, ab, ca, abc] = ["a", "b", "c", "ab", "ca", "abc"].map(|name| names[name]);
    graph.validate().unwrap();
    // Corrupt `abc = [a@0, b@1, c@2]` into `[ab@0, ca@1]` by field access.
    let left = Occurrence::new(0, ab);
    let right = Occurrence::new(1, ca);
    let mut corrupt = graph.clone();
    corrupt.vertices[abc.index.0].node =
        VertexNode::Composite(Composition::new(vec![left, right]).unwrap());
    for child in [a, b, c] {
        corrupt.vertices[child.index.0].parents.remove(&abc.index);
    }
    corrupt.vertices[ab.index.0].parents.insert(abc.index, [0.into()].into());
    corrupt.vertices[ca.index.0].parents.insert(abc.index, [1.into()].into());
    assert_eq!(
        corrupt.validate(),
        Err(PositionedError::OverlapMismatch {
            owner: abc.index,
            left,
            right,
        })
    );
}
