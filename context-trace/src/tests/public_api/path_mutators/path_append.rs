//! Tests for PathAppend trait and SubPath append behavior

use crate::*;

#[test]
fn subpath_append_pushes_childlocation() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    // create a child location pointing to the first child (sub_index 0)
    let child =
        crate::graph::vertex::location::child::ChildLocation::new(ab, ab_id, 0);

    let mut sp = crate::path::structs::sub_path::SubPath::new_empty(0);
    assert_eq!(sp.path.len(), 0);
    sp.path_append(child);
    assert_eq!(sp.path.len(), 1);
    assert_eq!(sp.path[0], child);
}
