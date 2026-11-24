//! Model-only rules: complete compositions on create and reciprocal links.
//! Structural cover rules are tested on the interchange validator; the edit
//! layer in `edit.rs`.

use crate::{
    AtomPosition,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        Composition,
        CompositionError,
        GraphRead,
        Navigate,
        Occurrence,
        OccurrenceLocation,
        PositionedError,
        PositionedGraph,
        VertexNode,
    },
};

/// `ababa = [aba@0, aba@2]`, `aba = [a@0, b@1, a@2]`.
fn ababa() -> (PositionedGraph, Token, Token) {
    let mut graph = PositionedGraph::default();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let aba = edit
        .create(vec![
            Occurrence::new(2, a),
            Occurrence::new(0, a),
            Occurrence::new(1, b),
        ])
        .unwrap();
    let ababa = edit
        .create(vec![Occurrence::new(0, aba), Occurrence::new(2, aba)])
        .unwrap();
    edit.finish();
    graph.validate().unwrap();
    (graph, aba, ababa)
}

#[test]
fn create_links_parents_and_rejects_incomplete_compositions() {
    let (mut graph, aba, ababa) = ababa();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    assert_eq!(a.index, VertexIndex(0), "atoms are reused");
    assert_eq!(ababa.width.0, 5);
    let offsets: Vec<_> = edit
        .graph()
        .vertex_data(aba.index).unwrap().parents()[&ababa.index]
        .iter()
        .map(|offset| offset.0)
        .collect();
    assert_eq!(offsets, [0, 2]);

    let composition = |error| PositionedError::Composition { owner: None, error };
    assert_eq!(
        edit.create(vec![Occurrence::new(0, aba)]),
        Err(composition(CompositionError::TooFew))
    );
    assert_eq!(
        edit.create(vec![Occurrence::new(0, aba), Occurrence::new(4, a)]),
        Err(composition(CompositionError::Gap(AtomPosition::from(3))))
    );
    assert!(matches!(
        edit.create(vec![Occurrence::new(0, aba), Occurrence::new(1, a), Occurrence::new(3, a)]),
        Err(PositionedError::Composition {
            error: CompositionError::Contained { .. },
            ..
        })
    ));
    assert_eq!(
        edit.create(vec![Occurrence::new(0, a), Occurrence::new(1, Token::new(VertexIndex(99), 1))]),
        Err(PositionedError::UnknownVertex(VertexIndex(99)))
    );
    edit.finish();
    graph.validate().unwrap();
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
    let (mut graph, aba, ababa) = ababa();
    let b = graph.edit().atom('b');
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

/// A second `ab` is rejected by `create`; added by field access, it is
/// rejected by `validate`.
#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn duplicate_content_is_rejected_by_create_and_validate() {
    let (mut graph, _, _) = ababa();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let ab = edit.create(vec![Occurrence::new(0, a), Occurrence::new(1, b)]).unwrap();
    let again = VertexIndex(edit.graph().vertex_count());
    assert_eq!(
        edit.create(vec![Occurrence::new(0, a), Occurrence::new(1, b)]),
        Err(PositionedError::DuplicateContent {
            first: ab.index,
            second: again,
        })
    );
    edit.finish();
    graph.validate().unwrap();
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

#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn overlap_disagreement_is_rejected_by_create_and_validate() {
    let (mut graph, _, _) = ababa();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let c = edit.atom('c');
    let ab = edit.create(vec![Occurrence::new(0, a), Occurrence::new(1, b)]).unwrap();
    let ca = edit.create(vec![Occurrence::new(0, c), Occurrence::new(1, a)]).unwrap();
    let abc = edit
        .create(vec![Occurrence::new(0, a), Occurrence::new(1, b), Occurrence::new(2, c)])
        .unwrap();
    // Structurally complete: `ab@0` and `ca@1` overlap at position 1 (`b` ≠ `c`).
    let left = Occurrence::new(0, ab);
    let right = Occurrence::new(1, ca);
    let owner = VertexIndex(edit.graph().vertex_count());
    assert_eq!(
        edit.create(vec![left, right]),
        Err(PositionedError::OverlapMismatch { owner, left, right })
    );
    edit.finish();
    graph.validate().unwrap();
    // Corrupt `abc = [a@0, b@1, c@2]` into `[ab@0, ca@1]` by field access.
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
