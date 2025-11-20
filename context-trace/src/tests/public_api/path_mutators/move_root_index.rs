//! Tests for MoveRootIndex trait - moving the root entry index within a pattern
//!
//! MoveRootIndex is used to advance or retract the root child index of a rooted path,
//! allowing traversal along the root pattern's children.

use crate::*;
use std::ops::ControlFlow;

#[test]
fn move_root_index_right_advances_through_pattern() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    // Create a RootedRolePath starting at index 0 (pointing to 'a')
    let loc = ChildLocation::new(abcd, abcd_id, 0);
    let mut path = IndexEndPath::new_location(loc);

    // Move right should advance to index 1 ('b')
    let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 1);

    // Move right again should advance to index 2 ('c')
    let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 2);
}

#[test]
fn move_root_index_right_breaks_at_pattern_end() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Start at the last valid index (2, pointing to 'c')
    let loc = ChildLocation::new(abc, abc_id, 2);
    let mut path = IndexEndPath::new_location(loc);

    // Move right should break (no more elements)
    let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Break(()));
    assert_eq!(path.root_entry, 2); // Index unchanged
}

#[test]
fn move_root_index_left_retracts_through_pattern() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    // Start at index 3 (pointing to 'd')
    let loc = ChildLocation::new(abcd, abcd_id, 3);
    let mut path = IndexEndPath::new_location(loc);

    // Move left should retract to index 2 ('c')
    let result = MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 2);

    // Move left again should retract to index 1 ('b')
    let result = MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 1);
}

#[test]
fn move_root_index_left_breaks_at_pattern_start() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Start at the first index (0, pointing to 'a')
    let loc = ChildLocation::new(abc, abc_id, 0);
    let mut path = IndexEndPath::new_location(loc);

    // Move left should break (no previous element)
    let result = MoveRootIndex::<Left>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Break(()));
    assert_eq!(path.root_entry, 0); // Index unchanged
}

#[test]
fn move_root_index_works_with_compound_patterns() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        ab => [a, b],
        cd => [c, d]
    );
    insert_patterns!(graph,
        (abcd, abcd_id) => [ab, cd]
    );

    // Create a path pointing to the first child (ab)
    let loc = ChildLocation::new(abcd, abcd_id, 0);
    let mut path = IndexEndPath::new_location(loc);

    // Move right should advance to the second child (cd)
    let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 1);

    // Move right again should break (at end)
    let result = MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(result, ControlFlow::Break(()));
}
