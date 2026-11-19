use crate::legacy::{
    graph::{
        kind::BaseGraphKind,
        Hypergraph,
    },
    init_test_tracing,
    insert_atoms,
    insert_patterns,
};
use std::fs;

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn boundary_target_validates_nested_paths_in_both_directions() {
    use crate::legacy::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c, d});
    let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
    let (cd, cd_id) = graph.insert_pattern_with_id(vec![c, d]);
    let (root, root_id) = graph.insert_pattern_with_id(vec![ab, cd]);
    let root_id = root_id.unwrap();
    for (prefix, index, child, child_id, target) in [
        (true, 0, ab, ab_id.unwrap(), a),
        (false, 1, cd, cd_id.unwrap(), d),
    ] {
        let path = [
            ChildLocation::new(root, root_id, index),
            ChildLocation::new(child, child_id, index),
        ];
        assert_eq!(boundary_target(&graph, root, &path, prefix), target);
        assert_eq!(boundary_target(&graph, root, &[], prefix), root);
    }
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
#[should_panic(expected = "disconnected boundary path")]
fn boundary_target_rejects_disconnected_path() {
    use crate::legacy::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c});
    let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
    let root = graph.insert_pattern(vec![ab, c]);
    boundary_target(
        &graph,
        root,
        &[ChildLocation::new(ab, ab_id.unwrap(), 0)],
        true,
    );
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
#[should_panic(expected = "boundary path leaves the scope boundary")]
fn boundary_target_rejects_interior_child() {
    use crate::legacy::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c});
    let (root, root_id) = graph.insert_pattern_with_id(vec![a, b, c]);
    boundary_target(
        &graph,
        root,
        &[ChildLocation::new(root, root_id.unwrap(), 1)],
        true,
    );
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn postfix_chain_keeps_alternative_descendants_connected() {
    use crate::legacy::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c, d, e});
    insert_patterns!(graph,
        de => [[d, e]],
        cde => [[c, de]],
        bc => [[b, c]],
        bcde => [[b, cde], [bc, de]],
        abc => [[a, bc]],
        abcde => [[a, bcde], [abc, de]]
    );
    let steps: Vec<_> = abcde.postfix_iter(&graph).collect();
    assert_eq!(
        steps.iter().map(|(_, token)| *token).collect::<Vec<_>>(),
        vec![bcde, cde, de, e]
    );
    let mut parent = abcde;
    for (location, child) in steps {
        assert_eq!(location.parent, parent);
        assert_eq!(
            graph.expect_pattern_at(location)[location.sub_index],
            child
        );
        parent = child;
    }
    assert_eq!(
        abcde
            .prefix_iter(&graph)
            .map(|(_, token)| token)
            .collect::<Vec<_>>(),
        vec![abc, a]
    );
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn owned_tail_replacement_updates_repeated_child_metadata() {
    use crate::legacy::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b});
    let aa = graph.insert_pattern(vec![a, a]);
    let (root, pattern_id) = graph.insert_pattern_with_id(vec![a, b, a]);
    let pattern_id = pattern_id.unwrap();
    let updated = graph.replace_owned_tail(root, pattern_id, a, aa);
    assert_eq!(updated.vertex_index(), root.vertex_index());
    assert_eq!(updated.width().0, 4);
    assert_eq!(
        graph.expect_pattern_at(PatternLocation::new(updated, pattern_id)),
        vec![a, b, aa].into()
    );
    for (child, position) in [(a, 0), (b, 1), (aa, 2)].iter() {
        let vertex = graph.expect_vertex_data(*child);
        let parent = vertex.parents().get(&root.vertex_index()).unwrap();
        assert_eq!(parent.width(), updated.width());
        assert_eq!(parent.pattern_indices().len(), 1);
        assert!(parent.pattern_indices().contains(
            &crate::legacy::graph::vertex::parent::PatternIndex::new(
                pattern_id, *position
            )
        ));
    }
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn test_graph_snapshot() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph).keep_log();
    insert_atoms!(graph, {a, b, c, d, e});

    insert_patterns!(graph,
        ab => [a, b],
        bc => [b, c],
        cd => [c, d],
        de => [d, e],
    );
    insert_patterns!(graph,
        abc => [[ab, c], [a, bc]],
        bcd => [[bc, d], [b, cd]],
        cde => [[cd, e], [c, de]],
    );
    insert_patterns!(graph,
        _abcd => [[abc, d], [a, bcd]],
        _bcde => [[bcd, e], [b, cde]],
    );

    graph.emit_graph_snapshot();
}

#[test]
#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
fn test_to_petgraph() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, c, d});
    // ab cd
    // abc d
    // a bcd

    insert_patterns!(graph,
        ab => [a, b],
        bc => [b, c],
        cd => [c, d],
    );
    insert_patterns!(graph,
        abc => [[ab, c], [a, bc]],
        bcd => [[bc, d], [b, cd]],
        _abcd => [[abc, d], [a, bcd]]
    );

    // Emit graph snapshot for log-viewer 3D visualization
    graph.emit_graph_snapshot();

    let pg = graph.to_petgraph();

    // Create temporary directory and file
    let temp_dir = std::env::temp_dir().join("context_trace_test");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    let temp_file = temp_dir.join("test_graph1.dot");

    // Write and then delete the file
    pg.write_to_file(&temp_file)
        .expect("Failed to write test graph file!");
    fs::remove_file(&temp_file).expect("Failed to delete test graph file!");
    fs::remove_dir(&temp_dir).expect("Failed to delete temp directory!");
}
