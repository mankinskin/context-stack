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

#[test]
fn name_map_addresses_every_fixture_vertex() {
    for fixture in super::super::fixtures::v3_graphs() {
        let name = &fixture.name;
        assert_eq!(fixture.names.len(), fixture.graph.vertex_count(), "{name}");
        for (key, token) in &fixture.names {
            assert_eq!(token.width.0, key.chars().count(), "{name}: {key}");
            assert_eq!(
                fixture.graph.vertex(token.index).map(|v| v.token()),
                Some(*token),
                "{name}: {key}"
            );
        }
        for text in &fixture.corpus {
            assert!(fixture.names.contains_key(text), "{name}: {text}");
        }
    }
}

#[cfg(debug_assertions)]
#[test]
fn debug_labels_equal_fixture_keys_and_resolve_names() {
    use crate::graph::vertex::name_resolver::{
        clear_token_name_resolver,
        resolve_token_name,
    };
    for fixture in super::super::fixtures::v3_graphs() {
        fixture.graph.register_name_resolver();
        for (key, token) in &fixture.names {
            let vertex = fixture.graph.vertex(token.index).unwrap();
            assert_eq!(vertex.label(), key, "{}", fixture.name);
            assert_eq!(resolve_token_name(token.index.0).as_deref(), Some(key.as_str()));
            assert_eq!(
                vertex.to_string(),
                format!("#{}(w{}) {key:?}", token.index.0, token.width.0)
            );
        }
        clear_token_name_resolver();
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn release_display_shows_index_and_width_only() {
    for fixture in super::super::fixtures::v3_graphs() {
        for token in fixture.names.values() {
            let vertex = fixture.graph.vertex(token.index).unwrap();
            assert_eq!(vertex.to_string(), format!("#{}(w{})", token.index.0, token.width.0));
        }
    }
}
