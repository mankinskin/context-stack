use crate::{
    graph::{
        Hypergraph,
        kind::BaseGraphKind,
    },
    init_test_tracing,
    insert_atoms,
    insert_patterns,
};
use std::fs;

#[test]
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
