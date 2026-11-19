//! Model-only rules: complete compositions on insert, reciprocal links and
//! one vertex per content. Structural cover rules are tested on the
//! interchange validator.

use crate::{
    AtomPosition,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        CompositionError,
        Occurrence,
        OccurrenceLocation,
        OccurrencePath,
        PositionedError,
        PositionedGraph,
        VertexNode,
    },
};

/// `ababa = [aba@0, aba@2]`, `aba = [a@0, b@1, a@2]`.
fn ababa() -> (PositionedGraph, Token, Token) {
    let mut graph = PositionedGraph::default();
    let a = graph.insert_atom('a');
    let b = graph.insert_atom('b');
    let aba = graph
        .insert(vec![
            Occurrence::new(2, a),
            Occurrence::new(0, a),
            Occurrence::new(1, b),
        ])
        .unwrap();
    let ababa = graph
        .insert(vec![Occurrence::new(0, aba), Occurrence::new(2, aba)])
        .unwrap();
    graph.validate().unwrap();
    (graph, aba, ababa)
}

#[test]
fn insert_links_parents_and_rejects_incomplete_or_inconsistent_compositions() {
    let (mut graph, aba, ababa) = ababa();
    let a = graph.insert_atom('a');
    assert_eq!(a.index, VertexIndex(0), "atoms are reused");
    assert_eq!(ababa.width.0, 5);
    let offsets: Vec<_> = graph.vertex(aba.index).unwrap().parents()[&ababa.index]
        .iter()
        .map(|offset| offset.0)
        .collect();
    assert_eq!(offsets, [0, 2]);

    let composition = |error| PositionedError::Composition { owner: None, error };
    assert_eq!(
        graph.insert(vec![Occurrence::new(0, aba)]),
        Err(composition(CompositionError::TooFew))
    );
    assert_eq!(
        graph.insert(vec![Occurrence::new(0, aba), Occurrence::new(4, a)]),
        Err(composition(CompositionError::Gap(AtomPosition::from(3))))
    );
    assert!(matches!(
        graph.insert(vec![Occurrence::new(0, aba), Occurrence::new(1, a), Occurrence::new(3, a)]),
        Err(PositionedError::Composition {
            error: CompositionError::Contained { .. },
            ..
        })
    ));
    // `aba@1` places `a` over the `b` of `aba@0`.
    assert_eq!(
        graph.insert(vec![Occurrence::new(0, aba), Occurrence::new(1, aba)]),
        Err(PositionedError::ContentMismatch {
            owner: None,
            position: AtomPosition::from(1),
        })
    );
    assert_eq!(
        graph.insert(vec![Occurrence::new(0, a), Occurrence::new(1, Token::new(VertexIndex(99), 1))]),
        Err(PositionedError::UnknownVertex(VertexIndex(99)))
    );
    // Same content as `ababa` through a different (non-maximal) cover.
    let ab = graph.insert(vec![Occurrence::new(0, a), Occurrence::new(1, graph.find(&['b']).unwrap())]).unwrap();
    assert_eq!(
        graph.insert(vec![Occurrence::new(0, ab), Occurrence::new(2, aba)]),
        Err(PositionedError::DuplicateContent {
            existing: ababa.index,
            duplicate: None,
        })
    );
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
fn validate_rejects_duplicate_content() {
    let (mut graph, _, _) = ababa();
    let mut duplicate = graph.vertices[0].clone();
    duplicate.index = VertexIndex(graph.vertices.len());
    duplicate.node = VertexNode::Atom('a');
    duplicate.parents.clear();
    graph.vertices.push(duplicate);
    assert_eq!(
        graph.validate(),
        Err(PositionedError::DuplicateContent {
            existing: VertexIndex(0),
            duplicate: Some(VertexIndex(4)),
        })
    );
}

#[test]
fn occurrence_paths_and_intervals_are_checked_against_the_graph() {
    let (graph, aba, ababa) = ababa();
    let b = graph.find(&['b']).unwrap();
    let step = |parent: Token, offset: usize, child: Token| OccurrenceLocation {
        parent: parent.index,
        offset: offset.into(),
        child: child.index,
    };
    let path = OccurrencePath::new(vec![step(ababa, 2, aba), step(aba, 1, b)]).unwrap();
    assert!(graph.contains_path(&path));
    assert_eq!(path.offset(), AtomPosition::from(3));
    assert_eq!((path.root(), path.target()), (ababa.index, b.index));
    let absent = OccurrencePath::new(vec![step(ababa, 1, aba)]).unwrap();
    assert!(!graph.contains_path(&absent));
    assert!(OccurrencePath::new(vec![step(ababa, 2, aba), step(ababa, 0, aba)]).is_none());

    let range = |start: usize, end: usize| AtomPosition::from(start)..AtomPosition::from(end);
    assert!(graph.interval(ababa.index, range(1, 4)).is_some());
    assert!(graph.interval(ababa.index, range(3, 6)).is_none());
}
