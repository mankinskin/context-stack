//! `GraphEdit` operations: create, add occurrences, extend, generation and
//! their rejections.

use pretty_assertions::assert_eq;

use crate::edit::{
    EditExt as _,
    Publication,
    VertexEdit,
};
use context_trace::{
    AtomPosition,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        CompositionError,
        GraphRead,
        Occurrence,
        PositionedError,
        PositionedGraph,
        interchange::PositionedGraphData,
    },
};

fn occ(
    offset: usize,
    token: Token,
) -> Occurrence {
    Occurrence::new(offset, token)
}

/// The parent links of `child` as `(parent, offset)` pairs.
fn links(
    graph: &PositionedGraph,
    child: Token,
) -> Vec<(VertexIndex, usize)> {
    graph
        .vertex_data(child.index)
        .unwrap()
        .parents()
        .iter()
        .flat_map(|(&parent, offsets)| {
            offsets.iter().map(move |o| (parent, o.0))
        })
        .collect()
}

struct Ababab {
    graph: PositionedGraph,
    a: Token,
    b: Token,
    ab: Token,
    abab: Token,
    ababab: Token,
}

/// Oracle graph of `ababab`: `ab = a@0, b@1`, `abab = ab@0, ab@2`,
/// `ababab = abab@0, abab@2`.
fn ababab() -> Ababab {
    let mut graph: PositionedGraph = PositionedGraph::default();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let abab = edit.create(vec![occ(0, ab), occ(2, ab)]).unwrap();
    let ababab = edit.create(vec![occ(0, abab), occ(2, abab)]).unwrap();
    edit.finish();
    graph.validate().unwrap();
    Ababab {
        graph,
        a,
        b,
        ab,
        abab,
        ababab,
    }
}

/// Insertion of `baba` into `ababab` (insert plan §9.3) by hand.
#[test]
fn create_and_add_occurrences_keep_the_graph_valid_and_maintain_links() {
    let Ababab {
        mut graph,
        a,
        b,
        ab,
        abab,
        ababab,
    } = ababab();
    let mut edit = graph.edit();
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let aba = edit.create(vec![occ(0, ab), occ(1, ba)]).unwrap();
    let bab = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
    edit.graph().validate().unwrap();
    edit.add_occurrences(abab.index, vec![occ(0, aba), occ(1, bab)])
        .unwrap();
    edit.graph().validate().unwrap();
    let baba = edit.create(vec![occ(0, bab), occ(1, aba)]).unwrap();
    edit.add_occurrences(ababab.index, vec![occ(1, baba)])
        .unwrap();
    let publication = edit.finish();
    graph.validate().unwrap();

    assert_eq!(
        publication,
        Publication {
            created: vec![ba, aba, bab, baba],
            edits: vec![
                VertexEdit {
                    vertex: abab.index,
                    added: vec![occ(0, aba), occ(1, bab)],
                    removed: vec![occ(0, ab), occ(2, ab)],
                },
                VertexEdit {
                    vertex: ababab.index,
                    added: vec![occ(1, baba)],
                    removed: vec![],
                },
            ],
        }
    );
    let composition = |token: Token| {
        graph
            .vertex(token.index)
            .unwrap()
            .composition()
            .unwrap()
            .occurrences()
            .to_vec()
    };
    assert_eq!(composition(abab), [occ(0, aba), occ(1, bab)]);
    assert_eq!(
        composition(ababab),
        [occ(0, abab), occ(1, baba), occ(2, abab)]
    );
    assert_eq!(links(&graph, ab), [(aba.index, 0), (bab.index, 1)]);
    assert_eq!(links(&graph, aba), [(abab.index, 0), (baba.index, 1)]);
    assert_eq!(links(&graph, bab), [(abab.index, 1), (baba.index, 0)]);
    assert_eq!(links(&graph, baba), [(ababab.index, 1)]);
    #[cfg(debug_assertions)]
    assert_eq!(graph.vertex_data(baba.index).unwrap().label(), "baba");
}

#[test]
fn add_occurrences_and_extend_reject_inputs_outside_their_preconditions() {
    let Ababab {
        mut graph,
        a,
        ab,
        abab,
        ababab,
        ..
    } = ababab();
    let before = graph.clone();
    let mut edit = graph.edit();
    assert_eq!(
        edit.add_occurrences(abab.index, vec![occ(4, a)]),
        Err(PositionedError::OutsideVertex {
            vertex: abab.index,
            occurrence: occ(4, a),
        })
    );
    assert_eq!(
        edit.add_occurrences(abab.index, vec![occ(3, ab)]),
        Err(PositionedError::WidthChange { vertex: abab.index })
    );
    // Committed content is immutable: `extend` is always rejected on the
    // committed store (staged cases: `tests/overlay.rs`).
    assert_eq!(
        edit.extend(abab.index, occ(4, ab)),
        Err(PositionedError::NotStaged(abab.index))
    );
    assert_eq!(
        edit.extend(ababab.index, occ(4, ab)),
        Err(PositionedError::NotStaged(ababab.index))
    );
    assert_eq!(
        edit.add_occurrences(a.index, vec![occ(0, a)]),
        Err(PositionedError::Atom(a.index))
    );
    assert_eq!(
        edit.extend(a.index, occ(1, a)),
        Err(PositionedError::Atom(a.index))
    );
    assert_eq!(edit.finish(), Publication::default());
    assert!(
        context_trace::positioned::diff(
            &before.to_interchange(),
            &graph.to_interchange()
        )
        .is_empty()
    );
    assert_eq!(
        graph.generation(),
        before.generation(),
        "rejections do not mutate"
    );
}

#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn create_rejects_an_overlap_that_is_not_a_vertex() {
    let mut graph: PositionedGraph = PositionedGraph::default();
    let mut edit = graph.edit();
    let [a, b, c, d] = ['a', 'b', 'c', 'd'].map(|atom| edit.atom(atom));
    let abc = edit.create(vec![occ(0, a), occ(1, b), occ(2, c)]).unwrap();
    let bcd = edit.create(vec![occ(0, b), occ(1, c), occ(2, d)]).unwrap();
    let owner = VertexIndex(edit.graph().vertex_count());
    assert_eq!(
        edit.create(vec![occ(0, abc), occ(1, bcd)]),
        Err(PositionedError::OverlapNotVertex {
            owner,
            left: occ(0, abc),
            right: occ(1, bcd),
        })
    );
    edit.finish();
}

#[test]
fn generation_bumps_once_per_mutating_edit() {
    let Ababab {
        mut graph,
        a,
        b,
        ab,
        ..
    } = ababab();
    let start = graph.generation().base;
    assert!(start > 0, "building the graph was an edit");

    let mut edit = graph.edit();
    assert_eq!(edit.atom('a'), a);
    assert_eq!(edit.atom('b'), b);
    assert!(edit.create(vec![occ(0, ab)]).is_err());
    assert_eq!(edit.finish(), Publication::default());
    assert_eq!(
        graph.generation().base,
        start,
        "existing atoms and rejections"
    );

    let mut edit = graph.edit();
    let c = edit.atom('c');
    edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
    edit.finish();
    assert_eq!(graph.generation().base, start + 1, "once per edit");

    {
        let mut edit = graph.edit();
        edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    }
    assert_eq!(
        graph.generation().base,
        start + 2,
        "also for a dropped edit"
    );
}

/// `ababa = [aba@0, aba@2]`, `aba = [a@0, b@1, a@2]`, built as a table.
fn ababa() -> (PositionedGraph, Token, Token) {
    let cover = |children: &[(usize, &str)]| -> Vec<(usize, String)> {
        children.iter().map(|&(offset, child)| (offset, child.to_string())).collect()
    };
    let data = PositionedGraphData {
        vertices: [
            ("a", cover(&[])),
            ("b", cover(&[])),
            ("aba", cover(&[(0, "a"), (1, "b"), (2, "a")])),
            ("ababa", cover(&[(0, "aba"), (2, "aba")])),
        ]
        .into_iter()
        .map(|(content, cover)| (content.to_string(), cover))
        .collect(),
    };
    let (graph, names) = PositionedGraph::from_interchange_named(&data).unwrap();
    (graph, names["aba"], names["ababa"])
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

/// A second `ab` is rejected by `create` (the `validate` half is in
/// context-trace).
#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn duplicate_content_is_rejected_by_create() {
    let (mut graph, _, _) = ababa();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let again = VertexIndex(edit.graph().vertex_count());
    assert_eq!(
        edit.create(vec![occ(0, a), occ(1, b)]),
        Err(PositionedError::DuplicateContent {
            first: ab.index,
            second: again,
        })
    );
    edit.finish();
    graph.validate().unwrap();
}

/// Overlapping occurrences that disagree are rejected by `create` (the
/// `validate` half is in context-trace).
#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn overlap_disagreement_is_rejected_by_create() {
    let (mut graph, _, _) = ababa();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let c = edit.atom('c');
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let ca = edit.create(vec![occ(0, c), occ(1, a)]).unwrap();
    edit.create(vec![occ(0, a), occ(1, b), occ(2, c)]).unwrap();
    // Structurally complete: `ab@0` and `ca@1` overlap at position 1 (`b` ≠ `c`).
    let left = occ(0, ab);
    let right = occ(1, ca);
    let owner = VertexIndex(edit.graph().vertex_count());
    assert_eq!(
        edit.create(vec![left, right]),
        Err(PositionedError::OverlapMismatch { owner, left, right })
    );
    edit.finish();
    graph.validate().unwrap();
}
