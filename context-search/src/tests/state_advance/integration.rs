//! Integration tests for StateAdvance trait behavior
//!
//! These tests focus on:
//! - End-to-end state advancement flows
//! - Integration between different state types
//! - Edge cases and boundary conditions
//! - State invariants during advancement

#[cfg(test)]
use {
    crate::{
        compare::parent::ParentCompareState,
        cursor::{
            Checkpointed,
            PatternCursor,
        },
    },
    context_trace::{
        path::accessors::path_accessor::HasTargetOffset,
        *,
    },
    std::marker::PhantomData,
};

#[test]
fn test_advancement_chain_through_multiple_states() {
    // Test advancing through a sequence of states
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abcd, abcd_id) => [a, b, c, d]
    );
    let _tracing = init_test_tracing!(&graph);

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
            cursor: Checkpointed {
                checkpoint: cursor.clone(),
                candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
            },
        };

        // Should advance successfully for indices 0, 1, 2
        let result = parent_compare_state.advance_state(&graph);
        assert!(
            result.is_ok(),
            "Should advance successfully from index {}",
            start_index
        );

        let advanced = result.unwrap();
        tracing::trace!(?advanced);
        tracing::info!("Advanced successfully from index {}", start_index);

        // Verify the child state is properly set up
        assert!(
            advanced.candidate.child.current().child_state.path.role_root_child_index::<
                context_trace::path::accessors::role::End,
            >() > start_index,
            "Child state should point to index after parent"
        );
    }
}

#[test]
fn test_advancement_preserves_atom_positions() {
    // Verify that advancement preserves atom positions correctly
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );
    let _tracing = init_test_tracing!(&graph);

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
            cursor: Checkpointed {
                checkpoint: cursor.clone(),
                candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
            },
        };

        let result = parent_compare_state.advance_state(&graph);
        assert!(result.is_ok(), "Should advance for position {}", pos);

        let advanced = result.unwrap();

        // Verify all positions are preserved
        assert_eq!(
            usize::from(advanced.candidate.query.current().atom_position),
            pos,
            "Cursor position should be preserved"
        );
        assert_eq!(
            usize::from(
                *advanced
                    .candidate
                    .child
                    .current()
                    .child_state
                    .target_offset()
            ),
            pos,
            "Child cursor target_offset should be preserved"
        );
        assert_eq!(
            usize::from(advanced.candidate.query.checkpoint().atom_position),
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
    // Test advancement with patterns of various sizes
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, e, f});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (abc, abc_id) => [a, b, c],
        (abcdef, abcdef_id) => [a, b, c, d, e, f]
    );
    let _tracing = init_test_tracing!(&graph);

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
            cursor: Checkpointed {
                checkpoint: cursor.clone(),
                candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
            },
        };

        let result = parent_compare_state.advance_state(&graph);
        assert!(result.is_ok(), "Should advance for {}", description);

        tracing::info!("Successfully advanced {}", description);
    }
}

#[test]
fn test_advancement_fails_at_boundaries() {
    // Test that advancement properly fails at pattern boundaries
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );
    let _tracing = init_test_tracing!(&graph);

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
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
        },
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
        usize::from(returned.cursor.current().atom_position),
        usize::from(cursor.atom_position),
        "Cursor should be unchanged on failure"
    );
}

#[test]
fn test_advancement_with_nested_patterns() {
    // Test advancement through nested pattern structures
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        ab => [a, b],
        cd => [c, d],
    );
    insert_patterns!(graph,
        (abcd, abcd_id) => [ab, cd]
    );
    let _tracing = init_test_tracing!(&graph);

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
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
        },
    };

    let result = parent_compare_state.advance_state(&graph);
    assert!(result.is_ok(), "Should advance in nested pattern");

    let advanced = result.unwrap();
    tracing::info!(?advanced, "Advanced through nested pattern");

    // Verify the child state points to the range starting at cd
    let child_root_index = advanced
        .candidate
        .child
        .current()
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
fn test_state_advance_idempotency_on_error() {
    // Test that failed advances don't modify state
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );
    let _tracing = init_test_tracing!(&graph);

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
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            candidate: Some(cursor.as_candidate()),
                _state: PhantomData,
        },
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
        usize::from(returned_state.cursor.current().atom_position),
        usize::from(original_state.cursor.current().atom_position),
        "Cursor position should be unchanged"
    );

    tracing::info!("State correctly preserved on failed advance");
}
