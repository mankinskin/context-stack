//! Content reading on every v3 fixture: every interval of every vertex reads
//! the matching slice of the vertex's content key.

use pretty_assertions::assert_eq;

use crate::{
    AtomPosition,
    positioned::{
        IntervalView,
        PositionedGraph,
        fixtures::v3_fixtures,
    },
};

#[test]
fn read_interval_equals_content_for_every_interval() {
    let mut intervals = 0;
    for (name, fixture) in v3_fixtures() {
        let graph = PositionedGraph::<crate::BaseGraphKind>::from_interchange(&fixture.graph)
            .unwrap();
        for content in fixture.graph.vertices.keys() {
            let atoms: Vec<char> = content.chars().collect();
            let owner = graph.find(&atoms).unwrap().index;
            for start in 0..=atoms.len() {
                for end in start..=atoms.len() {
                    let view = graph.interval(owner, start.into()..end.into()).unwrap();
                    assert_eq!(
                        graph.read_interval(&view).as_deref(),
                        Some(&atoms[start..end]),
                        "{name}: {content:?}[{start}..{end}]"
                    );
                    intervals += 1;
                }
            }
        }
    }
    assert!(intervals > 1000, "{intervals}");
}

#[test]
fn read_interval_rejects_views_outside_the_owner() {
    let (_, fixture) = v3_fixtures()
        .into_iter()
        .find(|(name, _)| name == "repeat_ababa")
        .unwrap();
    let graph =
        PositionedGraph::<crate::BaseGraphKind>::from_interchange(&fixture.graph).unwrap();
    let owner = graph.find(&['a', 'b', 'a']).unwrap().index;
    let view = |start: usize, end: usize| IntervalView {
        owner,
        range: AtomPosition::from(start)..AtomPosition::from(end),
    };
    assert_eq!(graph.read_interval(&view(0, 4)), None);
    assert_eq!(graph.read_interval(&view(2, 1)), None);
    assert_eq!(graph.read_interval(&view(1, 3)), Some(vec!['b', 'a']));
}
