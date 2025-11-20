//! Tests for MoveLeaf trait - moving leaf positions within patterns
//!
//! MoveLeaf is used to advance or retract the sub_index of a ChildLocation,
//! allowing traversal through the children of a pattern.

use crate::{
    path::mutators::move_path::leaf::MoveLeaf,
    *,
};
use std::ops::ControlFlow;

#[test]
fn move_leaf_right_advances_through_pattern() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    // Create a ChildLocation at index 0 (pointing to 'a')
    let mut loc = ChildLocation::new(abcd, abcd_id, 0);

    // Move right should advance to index 1 ('b')
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(loc.sub_index, 1);

    // Move right again to index 2 ('c')
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(loc.sub_index, 2);
}

#[test]
fn move_leaf_right_breaks_at_pattern_end() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Start at last valid index (2, pointing to 'c')
    let mut loc = ChildLocation::new(abc, abc_id, 2);

    // Move right should break (no more elements)
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Break(()));
    assert_eq!(loc.sub_index, 2); // Index unchanged
}

#[test]
fn move_leaf_left_retracts_through_pattern() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    // Start at index 3 (pointing to 'd')
    let mut loc = ChildLocation::new(abcd, abcd_id, 3);

    // Move left should retract to index 2 ('c')
    let result = MoveLeaf::<Left>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(loc.sub_index, 2);

    // Move left again to index 1 ('b')
    let result = MoveLeaf::<Left>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(loc.sub_index, 1);
}

#[test]
fn move_leaf_left_breaks_at_pattern_start() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Start at first index (0, pointing to 'a')
    let mut loc = ChildLocation::new(abc, abc_id, 0);

    // Move left should break (no previous element)
    let result = MoveLeaf::<Left>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Break(()));
    assert_eq!(loc.sub_index, 0); // Index unchanged
}

#[test]
fn move_leaf_works_with_compound_children() {
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

    // Create a location at the first compound child ('ab')
    let mut loc = ChildLocation::new(abcd, abcd_id, 0);

    // Move right should advance to the second compound child ('cd')
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Continue(()));
    assert_eq!(loc.sub_index, 1);

    // Move right again should break (at end)
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Break(()));
}

#[test]
fn move_leaf_sequential_movements() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, e});
    insert_patterns!(graph,
        (abcde, abcde_id) => [a, b, c, d, e]
    );

    // Start at beginning and move through entire pattern
    let mut loc = ChildLocation::new(abcde, abcde_id, 0);

    for expected_idx in 1..=4 {
        let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
        assert_eq!(result, ControlFlow::Continue(()));
        assert_eq!(loc.sub_index, expected_idx);
    }

    // Next move should break
    let result = MoveLeaf::<Right>::move_leaf(&mut loc, &graph);
    assert_eq!(result, ControlFlow::Break(()));
}
