//! Round trip of every v3 reader fixture through [`PositionedGraph`].

use pretty_assertions::assert_eq;

use crate::positioned::{
    PositionedGraph,
    diff,
    fixtures::v3_fixtures as fixtures,
};

#[test]
fn v3_fixtures_round_trip_through_positioned_graph() {
    for (name, fixture) in fixtures() {
        let graph = PositionedGraph::<crate::BaseGraphKind>::from_interchange(&fixture.graph)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        graph
            .validate()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(graph.vertex_count(), fixture.graph.vertices.len(), "{name}");
        let actual = graph.to_interchange();
        let difference = diff(&fixture.graph, &actual);
        assert!(difference.is_empty(), "{name}: {difference}");
        assert_eq!(actual, fixture.graph, "{name}");
    }
}
