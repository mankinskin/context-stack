//! `GraphEdit` operations, generation, `maximal` and `Composition::crossing`.

use pretty_assertions::assert_eq;

use crate::{
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        Occurrence,
        PositionedError,
        PositionedGraph,
        Publication,
        VertexEdit,
        fixtures::fixture_graphs,
        maximal,
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
        .vertex(child.index)
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
    assert_eq!(graph.vertex(baba.index).unwrap().label(), "baba");
}

#[test]
fn extend_grows_a_parentless_vertex() {
    let mut graph: PositionedGraph = PositionedGraph::default();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let c = edit.atom('c');
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let bc = edit.create(vec![occ(0, b), occ(1, c)]).unwrap();
    let root = edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
    edit.finish();

    let mut edit = graph.edit();
    let abcab = edit.extend(root.index, occ(3, ab)).unwrap();
    assert_eq!(abcab, Token::new(root.index, 5));
    edit.graph().validate().unwrap();
    let abcabc = edit.extend(root.index, occ(4, bc)).unwrap();
    assert_eq!(abcabc, Token::new(root.index, 6));
    let publication = edit.finish();
    graph.validate().unwrap();

    assert_eq!(
        publication.edits,
        [VertexEdit {
            vertex: root.index,
            added: vec![occ(3, ab), occ(4, bc)],
            removed: vec![],
        }]
    );
    assert_eq!(links(&graph, ab), [(root.index, 0), (root.index, 3)]);
    assert_eq!(links(&graph, bc), [(root.index, 4)]);
    assert_eq!(graph.vertex(root.index).unwrap().token(), abcabc);
    #[cfg(debug_assertions)]
    assert_eq!(graph.vertex(root.index).unwrap().label(), "abcabc");
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
    assert_eq!(
        edit.extend(abab.index, occ(4, ab)),
        Err(PositionedError::HasParents(abab.index))
    );
    assert_eq!(
        edit.extend(ababab.index, occ(7, ab)),
        Err(PositionedError::OutsideVertex {
            vertex: ababab.index,
            occurrence: occ(7, ab),
        })
    );
    assert_eq!(
        edit.extend(ababab.index, occ(4, ab)),
        Err(PositionedError::OutsideVertex {
            vertex: ababab.index,
            occurrence: occ(4, ab),
        })
    );
    assert_eq!(edit.finish(), Publication::default());
    assert!(
        crate::positioned::diff(
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

#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn validate_rejects_an_overlap_that_is_not_a_vertex() {
    let mut graph: PositionedGraph = PositionedGraph::default();
    let mut edit = graph.edit();
    let [a, b, c, d] = ['a', 'b', 'c', 'd'].map(|atom| edit.atom(atom));
    let abc = edit.create(vec![occ(0, a), occ(1, b), occ(2, c)]).unwrap();
    let bcd = edit.create(vec![occ(0, b), occ(1, c), occ(2, d)]).unwrap();
    let abcd = edit.create(vec![occ(0, abc), occ(3, d)]).unwrap();
    edit.finish();
    graph.validate().unwrap();
    // Corrupt `abcd` into `[abc@0, bcd@1]`: content agrees, but `bc` is no vertex.
    let composition =
        crate::positioned::Composition::new(vec![occ(0, abc), occ(1, bcd)])
            .unwrap();
    graph.vertices[abcd.index.0].node =
        crate::positioned::VertexNode::Composite(composition);
    graph.vertices[d.index.0].parents.remove(&abcd.index);
    graph.vertices[bcd.index.0]
        .parents
        .insert(abcd.index, [1.into()].into());
    assert_eq!(
        graph.validate(),
        Err(PositionedError::OverlapNotVertex {
            owner: abcd.index,
            left: occ(0, abc),
            right: occ(1, bcd),
        })
    );
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
    let start = graph.generation();
    assert!(start > 0, "building the graph was an edit");

    let mut edit = graph.edit();
    assert_eq!(edit.atom('a'), a);
    assert_eq!(edit.atom('b'), b);
    assert!(edit.create(vec![occ(0, ab)]).is_err());
    assert_eq!(edit.finish(), Publication::default());
    assert_eq!(graph.generation(), start, "existing atoms and rejections");

    let mut edit = graph.edit();
    let c = edit.atom('c');
    edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
    edit.finish();
    assert_eq!(graph.generation(), start + 1, "once per edit");

    {
        let mut edit = graph.edit();
        edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    }
    assert_eq!(graph.generation(), start + 2, "also for a dropped edit");
}

#[test]
fn maximal_drops_duplicates_and_contained_occurrences() {
    let a = Token::new(VertexIndex(0), 1);
    let ab = Token::new(VertexIndex(1), 2);
    let abc = Token::new(VertexIndex(2), 3);
    let bc = Token::new(VertexIndex(3), 2);
    assert_eq!(
        maximal(vec![
            occ(1, bc),
            occ(0, a),
            occ(0, abc),
            occ(1, bc),
            occ(0, ab),
            occ(2, a)
        ]),
        [occ(0, abc)]
    );
    assert_eq!(
        maximal(vec![
            occ(2, bc),
            occ(1, bc),
            occ(0, ab),
            occ(1, a),
            occ(1, bc)
        ]),
        [occ(0, ab), occ(1, bc), occ(2, bc)],
        "partial overlaps are kept"
    );
    assert_eq!(maximal(vec![]), []);
}

#[test]
fn crossing_equals_the_brute_force_filter_on_every_fixture_composition() {
    for fixture in fixture_graphs() {
        for vertex in fixture.graph.vertices() {
            let Some(composition) = vertex.composition() else {
                continue;
            };
            for p in 0..=vertex.width().0 + 1 {
                let expected: Vec<Occurrence> = composition
                    .occurrences()
                    .iter()
                    .filter(|o| o.offset.0 < p && p < o.end().0)
                    .copied()
                    .collect();
                assert_eq!(
                    composition.crossing(p),
                    expected.as_slice(),
                    "{}: {vertex} at {p}",
                    fixture.name
                );
            }
        }
    }
}

#[test]
fn validate_passes_on_every_fixture() {
    for fixture in fixture_graphs() {
        fixture
            .graph
            .validate()
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.name));
    }
}
