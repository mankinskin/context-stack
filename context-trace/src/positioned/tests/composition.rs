//! Model-only rules on compositions: `maximal`, `Composition::crossing`,
//! and `validate` on every fixture and on a corrupted graph.

use pretty_assertions::assert_eq;

use super::model::named;
use crate::{
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        Composition,
        Occurrence,
        PositionedError,
        VertexNode,
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

#[cfg(any(debug_assertions, feature = "test-api"))]
#[test]
fn validate_rejects_an_overlap_that_is_not_a_vertex() {
    let (mut graph, names) = named(&[
        ("abc", &[(0, "a"), (1, "b"), (2, "c")]),
        ("bcd", &[(0, "b"), (1, "c"), (2, "d")]),
        ("abcd", &[(0, "abc"), (3, "d")]),
    ]);
    let [abc, bcd, abcd, d] = ["abc", "bcd", "abcd", "d"].map(|name| names[name]);
    // Corrupt `abcd` into `[abc@0, bcd@1]`: content agrees, but `bc` is no vertex.
    let composition = Composition::new(vec![occ(0, abc), occ(1, bcd)]).unwrap();
    graph.vertices[abcd.index.0].node = VertexNode::Composite(composition);
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
    assert_eq!(maximal(Vec::<Occurrence>::new()), []);
}

#[test]
fn crossing_equals_the_brute_force_filter_on_every_fixture_composition() {
    for fixture in fixture_graphs() {
        for vertex in fixture.graph.vertices.iter() {
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
