//! Integration tests for StateAdvance trait behavior
//!
//! These tests focus on:
//! - End-to-end state advancement flows
//! - Integration between different state types
//! - Edge cases and boundary conditions
//! - State invariants during advancement

use crate::{
    compare::{
        parent::{
            CompareRootState,
            ParentCompareState,
        },
        state::CompareState,
    },
    cursor::{
        Candidate,
        PatternCursor,
    },
    tests::macros::*,
};
use context_trace::{
    graph::vertex::location::child::ChildLocation,
    path::{
        mutators::adapters::StateAdvance,
        structs::rooted::{
            pattern_range::PatternRangePath,
            role_path::{
                IndexStartPath,
                RolePath,
            },
            root::IndexRoot,
        },
    },
    trace::state::{
        parent::ParentState,
        BaseState,
    },
    *,
};
use std::marker::PhantomData;

#[test]
fn test_advancement_chain_through_multiple_states() {
    let _tracing = init_test_tracing!();

    // Test advancing through a sequence of states
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );

    tracing::info!("Testing advancement chain through multiple indices");

    for start_index in 0..3 {
        tracing::info!(start_index, "Testing from start_index");

        let root = IndexRoot::from(
            ChildLocation::new(abcd, abcd_id, start_index)
                .into_pattern_location(),
        );
        let parent_path =
            IndexStartPath::new(root, RolePath::new_empty(start_index));
        let parent_state = ParentState {
            path: parent_path,
            prev_pos: 0.into(),
            root_pos: 0.into(),
        };

        let pattern_path = PatternRangePath::new(
            Pattern::from(vec![a, b, c, d]),
            RolePath::new_empty(0),
            RolePath::new_empty(3),
        );
        let cursor = PatternCursor {
            path: pattern_path,
            atom_position: start_index.into(),
            _state: PhantomData,
        };

        let parent_compare_state = ParentCompareState {
            parent_state,
            cursor,
        };

        // Should advance successfully for indices 0, 1, 2
        let result = parent_compare_state.advance_state(&graph);
        assert!(
            result.is_ok(),
            "Should advance successfully from index {}",
            start_index
        );

        let advanced = result.unwrap();
        tracing::info!(
            ?advanced,
            "Advanced successfully from index {}",
            start_index
        );

        // Verify the child state is properly set up
        assert!(
            advanced.token.child_state.path.role_root_child_index::<
                context_trace::path::accessors::role::End,
            >() > start_index,
            "Child state should point to index after parent"
        );
    }
}

#[test]
fn test_advancement_preserves_atom_positions() {
    let _tracing = init_test_tracing!();

    // Verify that advancement preserves atom positions correctly
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    let test_positions = vec![0, 5, 10, 100];

    for pos in test_positions {
        tracing::info!(atom_position = pos, "Testing position preservation");

        let root = IndexRoot::from(
            ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
        );
        let parent_path = IndexStartPath::new(root, RolePath::new_empty(0));
        let parent_state = ParentState {
            path: parent_path,
            prev_pos: pos.into(),
            root_pos: pos.into(),
        };

        let pattern_path = PatternRangePath::new(
            Pattern::from(vec![a, b, c]),
            RolePath::new_empty(0),
            RolePath::new_empty(2),
        );
        let cursor = PatternCursor {
            path: pattern_path,
            atom_position: pos.into(),
            _state: PhantomData,
        };

        let parent_compare_state = ParentCompareState {
            parent_state,
            cursor,
        };

        let result = parent_compare_state.advance_state(&graph);
        assert!(result.is_ok(), "Should advance for position {}", pos);

        let advanced = result.unwrap();

        // Verify all positions are preserved
        assert_eq!(
            usize::from(advanced.token.cursor.atom_position),
            pos,
            "Cursor position should be preserved"
        );
        assert_eq!(
            usize::from(*advanced.token.child_cursor.child_state.target_pos()),
            pos,
            "Child cursor target_pos should be preserved"
        );
        assert_eq!(
            usize::from(advanced.token.checkpoint.atom_position),
            pos,
            "Checkpoint position should be preserved"
        );
        assert_eq!(
            usize::from(advanced.root_parent.prev_pos),
            pos,
            "Parent prev_pos should be preserved"
        );
        assert_eq!(
            usize::from(advanced.root_parent.root_pos),
            pos,
            "Parent root_pos should be preserved"
        );
    }
}

#[test]
fn test_advancement_with_different_pattern_sizes() {
    let _tracing = init_test_tracing!();

    // Test advancement with patterns of various sizes
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, e, f});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (abc, abc_id) => [a, b, c],
        (abcdef, abcdef_id) => [a, b, c, d, e, f]
    );

    let test_cases = vec![
        (ab, ab_id, vec![a, b], "two-element pattern"),
        (abc, abc_id, vec![a, b, c], "three-element pattern"),
        (
            abcdef,
            abcdef_id,
            vec![a, b, c, d, e, f],
            "six-element pattern",
        ),
    ];

    for (parent, parent_id, pattern_tokens, description) in test_cases {
        tracing::info!(description, "Testing pattern");

        let root = IndexRoot::from(
            ChildLocation::new(parent, parent_id, 0).into_pattern_location(),
        );
        let parent_path = IndexStartPath::new(root, RolePath::new_empty(0));
        let parent_state = ParentState {
            path: parent_path,
            prev_pos: 0.into(),
            root_pos: 0.into(),
        };

        let pattern_path = PatternRangePath::new(
            Pattern::from(pattern_tokens.clone()),
            RolePath::new_empty(0),
            RolePath::new_empty(pattern_tokens.len() - 1),
        );
        let cursor = PatternCursor {
            path: pattern_path,
            atom_position: 0.into(),
            _state: PhantomData,
        };

        let parent_compare_state = ParentCompareState {
            parent_state,
            cursor,
        };

        let result = parent_compare_state.advance_state(&graph);
        assert!(result.is_ok(), "Should advance for {}", description);

        tracing::info!("Successfully advanced {}", description);
    }
}

#[test]
fn test_advancement_fails_at_boundaries() {
    let _tracing = init_test_tracing!();

    // Test that advancement properly fails at pattern boundaries
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Test at the last index
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 2).into_pattern_location(),
    );
    let parent_path = IndexStartPath::new(root, RolePath::new_empty(2));
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: 0.into(),
        root_pos: 0.into(),
    };

    let pattern_path = PatternRangePath::new(
        Pattern::from(vec![a, b, c]),
        RolePath::new_empty(0),
        RolePath::new_empty(2),
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: 2.into(),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state: parent_state.clone(),
        cursor: cursor.clone(),
    };

    tracing::info!(?parent_compare_state, "Testing at boundary (last index)");

    let result = parent_compare_state.advance_state(&graph);

    assert!(result.is_err(), "Should fail to advance at last index");

    let returned = result.unwrap_err();
    assert_eq!(
        returned.parent_state.path.path_root(),
        parent_state.path.path_root(),
        "Original state should be returned on failure"
    );
    assert_eq!(
        usize::from(returned.cursor.atom_position),
        usize::from(cursor.atom_position),
        "Cursor should be unchanged on failure"
    );
}

#[test]
fn test_advancement_with_nested_patterns() {
    let _tracing = init_test_tracing!();

    // Test advancement through nested pattern structures
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (cd, cd_id) => [c, d],
        (abcd, abcd_id) => [ab, cd]
    );

    tracing::info!("Testing nested pattern advancement");

    // Advance from first child (ab) to second child (cd)
    let root = IndexRoot::from(
        ChildLocation::new(abcd, abcd_id, 0).into_pattern_location(),
    );
    let parent_path = IndexStartPath::new(root, RolePath::new_empty(0));
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: 0.into(),
        root_pos: 0.into(),
    };

    let pattern_path = PatternRangePath::new(
        Pattern::from(vec![ab, cd]),
        RolePath::new_empty(0),
        RolePath::new_empty(1),
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: 0.into(),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor,
    };

    let result = parent_compare_state.advance_state(&graph);
    assert!(result.is_ok(), "Should advance in nested pattern");

    let advanced = result.unwrap();
    tracing::info!(?advanced, "Advanced through nested pattern");

    // Verify the child state points to the range starting at cd
    let child_root_index = advanced
        .token
        .child_state
        .path
        .role_root_child_index::<context_trace::path::accessors::role::End>(
    );
    assert_eq!(
        child_root_index, 1,
        "Child should point to second element after advancement"
    );
}

#[test]
fn test_compare_state_advancement_consistency() {
    let _tracing = init_test_tracing!();

    // Test that CompareState advancement maintains consistency
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // First get to CompareRootState
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let parent_path = IndexStartPath::new(root, RolePath::new_empty(0));
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: 0.into(),
        root_pos: 0.into(),
    };

    let pattern_path = PatternRangePath::new(
        Pattern::from(vec![a, b, c]),
        RolePath::new_empty(0),
        RolePath::new_empty(2),
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: 0.into(),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor,
    };

    let compare_root_result = parent_compare_state.advance_state(&graph);
    assert!(compare_root_result.is_ok(), "Should get CompareRootState");

    let compare_root = compare_root_result.unwrap();
    let compare_state: CompareState<Candidate, Candidate> = compare_root.token;

    tracing::info!(?compare_state, "Testing CompareState advancement");

    // CompareState<Candidate, Candidate> always returns Ok
    let advance_result = compare_state.clone().advance_state(&graph);
    assert!(
        advance_result.is_ok(),
        "CompareState<Candidate> should always return Ok"
    );

    let advanced_compare = advance_result.unwrap();

    // Verify checkpoint is preserved (it's not advanced by CompareState)
    assert_eq!(
        usize::from(advanced_compare.checkpoint.atom_position),
        usize::from(compare_state.checkpoint.atom_position),
        "Checkpoint should remain unchanged"
    );
}

#[test]
fn test_state_advance_idempotency_on_error() {
    let _tracing = init_test_tracing!();

    // Test that failed advances don't modify state
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    let root = IndexRoot::from(
        ChildLocation::new(ab, ab_id, 1).into_pattern_location(),
    );
    let parent_path = IndexStartPath::new(root, RolePath::new_empty(1));
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: 42.into(),
        root_pos: 100.into(),
    };

    let pattern_path = PatternRangePath::new(
        Pattern::from(vec![a, b]),
        RolePath::new_empty(0),
        RolePath::new_empty(1),
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: 7.into(),
        _state: PhantomData,
    };

    let original_state = ParentCompareState {
        parent_state: parent_state.clone(),
        cursor: cursor.clone(),
    };

    tracing::info!(?original_state, "Original state before failed advance");

    let result = original_state.clone().advance_state(&graph);
    assert!(result.is_err(), "Should fail at boundary");

    let returned_state = result.unwrap_err();

    // Verify all fields match original
    assert_eq!(
        returned_state.parent_state.path.path_root(),
        original_state.parent_state.path.path_root(),
        "Root should be unchanged"
    );
    assert_eq!(
        usize::from(returned_state.parent_state.prev_pos),
        usize::from(original_state.parent_state.prev_pos),
        "prev_pos should be unchanged"
    );
    assert_eq!(
        usize::from(returned_state.parent_state.root_pos),
        usize::from(original_state.parent_state.root_pos),
        "root_pos should be unchanged"
    );
    assert_eq!(
        usize::from(returned_state.cursor.atom_position),
        usize::from(original_state.cursor.atom_position),
        "Cursor position should be unchanged"
    );

    tracing::info!("State correctly preserved on failed advance");
}
