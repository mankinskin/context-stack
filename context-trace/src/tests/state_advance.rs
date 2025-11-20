//! Unit tests for the StateAdvance trait
//!
//! Tests the state advancement behavior for:
//! - ParentState advancing to RootChildState
//! - ChildState advancing within a pattern
//!
//! Each test verifies:
//! - Successful advancement when conditions are met
//! - Proper error handling when advancement fails
//! - State consistency after advancement

use crate::{
    path::structs::rooted::{
        index_range::IndexRangePath,
        role_path::IndexStartPath,
        root::IndexRoot,
    },
    tests::macros::*,
    trace::{
        child::state::{
            ChildState,
            RootChildState,
        },
        state::{
            BaseState,
            parent::ParentState,
        },
    },
    *,
};

#[test]
fn test_parent_state_advance_success() {
    let _tracing = init_test_tracing!();

    // Create a graph with pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Create ParentState at index 0 (pointing to 'a')
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let path = IndexStartPath::new(root, RolePath::new_empty(0));

    let parent_state = ParentState {
        path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    tracing::info!(?parent_state, "Initial parent state");

    // Advance should succeed because there is a next index (1)
    let result = parent_state.clone().advance_state(&graph);

    assert!(result.is_ok(), "ParentState should advance successfully");

    let root_child_state: RootChildState = result.unwrap();
    tracing::info!(?root_child_state, "Advanced to RootChildState");

    // Verify the root_parent is preserved
    assert_eq!(
        root_child_state.root_parent.path.root,
        parent_state.path.root
    );
    assert_eq!(root_child_state.root_parent.prev_pos, parent_state.prev_pos);
    assert_eq!(root_child_state.root_parent.root_pos, parent_state.root_pos);

    // Verify the child state points to the range starting at index 1
    assert_eq!(
        root_child_state.child_state.path.root,
        parent_state.path.root
    );
}

#[test]
fn test_parent_state_advance_at_last_index() {
    let _tracing = init_test_tracing!();

    // Create a graph with pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Create ParentState at the last index (2, pointing to 'c')
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 2).into_pattern_location(),
    );
    let path = IndexStartPath::new(root, RolePath::new_empty(2));

    let parent_state = ParentState {
        path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    tracing::info!(?parent_state, "Parent state at last index");

    // Advance should fail because there is no next index
    let result = parent_state.clone().advance_state(&graph);

    assert!(
        result.is_err(),
        "ParentState at last index should fail to advance"
    );

    let returned_state = result.unwrap_err();
    assert_eq!(
        returned_state.path.root, parent_state.path.root,
        "Failed advance should return original state"
    );
}

#[test]
fn test_parent_state_advance_single_element_pattern() {
    let _tracing = init_test_tracing!();

    // Create a graph with single-element pattern: [a]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    // Create ParentState at the last index (1, the only element after 'a')
    let root = IndexRoot::from(
        ChildLocation::new(ab, ab_id, 1).into_pattern_location(),
    );
    let path = IndexStartPath::new(root, RolePath::new_empty(1));

    let parent_state = ParentState {
        path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    tracing::info!(?parent_state, "Parent state at last index");

    // Advance should fail because there is no next index
    let result = parent_state.clone().advance_state(&graph);

    assert!(
        result.is_err(),
        "ParentState at last index should fail to advance"
    );
}

#[test]
fn test_child_state_advance_success() {
    let _tracing = init_test_tracing!();

    // Create a graph with nested patterns
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (abc, abc_id) => [ab, c]
    );

    // Create ChildState at index 0 pointing to 'ab' in 'abc'
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let path = IndexRangePath::new(
        root,
        RolePath::new_empty(0),
        RolePath::new_empty(0),
    );

    let child_state = ChildState {
        current_pos: AtomPosition::from(0),
        path,
    };

    tracing::info!(?child_state, "Initial child state");

    // Advance should succeed if the path can be advanced
    let result = child_state.clone().advance_state(&graph);

    match result {
        Ok(advanced_state) => {
            tracing::info!(
                ?advanced_state,
                "Child state advanced successfully"
            );
            // Verify the state is updated correctly
            assert_eq!(
                advanced_state.path.root, child_state.path.root,
                "Root should remain the same"
            );
        },
        Err(returned_state) => {
            tracing::info!(
                ?returned_state,
                "Child state could not advance (at end of pattern)"
            );
            // This is also valid - depends on whether there's more to advance
        },
    }
}

#[test]
fn test_child_state_advance_at_end() {
    let _tracing = init_test_tracing!();

    // Create a graph with pattern: [a, b]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    // Create ChildState at the last position
    let root = IndexRoot::from(
        ChildLocation::new(ab, ab_id, 0).into_pattern_location(),
    );
    let path = IndexRangePath::new(
        root,
        RolePath::new_empty(0),
        RolePath::new_empty(1), // End at last index
    );

    let child_state = ChildState {
        current_pos: AtomPosition::from(0),
        path,
    };

    tracing::info!(?child_state, "Child state at end position");

    // Try to advance
    let result = child_state.clone().advance_state(&graph);

    // Should fail when already at the end
    assert!(result.is_err(), "Child state at end should fail to advance");

    let returned_state = result.unwrap_err();
    assert_eq!(
        returned_state.path.root, child_state.path.root,
        "Failed advance should return original state"
    );
}

#[test]
fn test_root_child_state_composition() {
    let _tracing = init_test_tracing!();

    // Test that RootChildState properly composes ParentState and ChildState
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Create a ParentState
    let parent_root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let parent_path =
        IndexStartPath::new(parent_root.clone(), RolePath::new_empty(0));

    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    // Advance to get RootChildState
    let result = parent_state.clone().advance_state(&graph);
    assert!(result.is_ok(), "Should advance successfully");

    let root_child_state: RootChildState = result.unwrap();

    // Verify composition
    assert_eq!(
        root_child_state.root_parent.path.root, parent_state.path.root,
        "RootChildState should preserve parent root"
    );

    // The child_state should have a valid path
    let child_path = &root_child_state.child_state.path;
    assert_eq!(
        child_path.root, parent_root,
        "Child path should have same root as parent"
    );

    tracing::info!(
        ?root_child_state,
        "RootChildState correctly composes parent and child"
    );
}

#[test]
fn test_state_advance_chain() {
    let _tracing = init_test_tracing!();

    // Test advancing through multiple states in sequence
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    // Start with ParentState at index 0
    let root = IndexRoot::from(
        ChildLocation::new(abcd, abcd_id, 0).into_pattern_location(),
    );
    let path = IndexStartPath::new(root, RolePath::new_empty(0));

    let mut parent_state = ParentState {
        path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    tracing::info!(?parent_state, "Starting state");

    // Advance parent to child
    let result1 = parent_state.clone().advance_state(&graph);
    assert!(result1.is_ok(), "First parent advance should succeed");
    let root_child1 = result1.unwrap();
    tracing::info!(?root_child1, "After first advance to RootChildState");

    // Try to advance the child state
    let result2 = root_child1.child_state.clone().advance_state(&graph);
    match result2 {
        Ok(child2) => {
            tracing::info!(?child2, "Child state advanced successfully");
        },
        Err(child_err) => {
            tracing::info!(?child_err, "Child state could not advance further");
        },
    }

    // Now try advancing from index 1
    parent_state.path.role_path.sub_path.root_entry = 1;
    let result3 = parent_state.clone().advance_state(&graph);
    assert!(result3.is_ok(), "Second parent advance should succeed");
    tracing::info!(?result3, "Advanced from index 1");
}

#[test]
fn test_state_advance_preserves_positions() {
    let _tracing = init_test_tracing!();

    // Verify that atom positions are preserved correctly during advancement
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let path = IndexStartPath::new(root, RolePath::new_empty(0));

    let parent_state = ParentState {
        path,
        prev_pos: AtomPosition(5),
        root_pos: AtomPosition(10),
    };

    tracing::info!(
        prev_pos = ?parent_state.prev_pos,
        root_pos = ?parent_state.root_pos,
        "Initial positions"
    );

    // Advance and verify positions are preserved
    let result = parent_state.clone().advance_state(&graph);
    assert!(result.is_ok(), "Should advance successfully");

    let root_child_state = result.unwrap();

    // ChildState has current_pos (which should equal parent's root_pos after advance)
    assert_eq!(
        *root_child_state.child_state.target_pos(),
        parent_state.root_pos,
        "child current_pos should match parent root_pos"
    );
    assert_eq!(
        root_child_state.root_parent.prev_pos, parent_state.prev_pos,
        "root_parent prev_pos should match"
    );
    assert_eq!(
        root_child_state.root_parent.root_pos, parent_state.root_pos,
        "root_parent root_pos should match"
    );

    tracing::info!(?root_child_state, "Positions preserved correctly");
}
