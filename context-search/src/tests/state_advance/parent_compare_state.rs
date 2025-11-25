//! Tests for ParentCompareState advancing to CompareRootState

#[cfg(test)]
use {
    crate::{
        compare::parent::{
            CompareRootState,
            ParentCompareState,
        },
        cursor::{
            Checkpointed,
            PatternCursor,
        },
    },
    context_trace::{
        *,
        path::accessors::path_accessor::HasTargetOffset,
    },
    std::marker::PhantomData,
};

#[test]
fn test_parent_compare_state_advance_success() {
    // Create a graph with pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create a ParentState at index 0
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let parent_path: IndexStartPath = rooted_path!(Start: root, 0);
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    // Create a PatternCursor
    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![a, b, c]),
        start: 0,
        end: 0
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            current: cursor.as_candidate(),
        },
    };

    tracing::info!(?parent_compare_state, "Initial ParentCompareState");

    // Advance should succeed
    let result = parent_compare_state.clone().advance_state(&graph);

    assert!(
        result.is_ok(),
        "ParentCompareState should advance successfully"
    );

    let compare_root_state: CompareRootState = result.unwrap();
    tracing::info!(?compare_root_state, "Advanced to CompareRootState");

    // Verify root_parent is preserved
    assert_eq!(
        compare_root_state.root_parent.path.path_root(),
        parent_compare_state.parent_state.path.path_root()
    );

    // Verify cursors are created properly
    assert_eq!(
        compare_root_state.candidate.query.current().atom_position,
        parent_compare_state.cursor.current().atom_position
    );
    assert_eq!(
        *compare_root_state.candidate.child.current().child_state.target_offset(),
        parent_compare_state.cursor.current().atom_position
    );

    // Verify checkpoint is preserved
    assert_eq!(
        compare_root_state
            .candidate
            .query
            .checkpoint()
            .atom_position,
        parent_compare_state.cursor.current().atom_position
    );
}

#[test]
fn test_parent_compare_state_advance_at_last_index() {
    // Create a graph with pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create a ParentState at the last index (2)
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 2).into_pattern_location(),
    );
    let parent_path: IndexStartPath = rooted_path!(Start: root, 2);
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    // Create a PatternCursor
    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![a, b, c]),
        start: 0,
        end: 2
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: 2.into(),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            current: cursor.as_candidate(),
        },
    };

    tracing::info!(?parent_compare_state, "ParentCompareState at last index");

    // Advance should fail because parent is at last index
    let result = parent_compare_state.clone().advance_state(&graph);

    assert!(
        result.is_err(),
        "ParentCompareState at last index should fail to advance"
    );

    let returned_state = result.unwrap_err();
    assert_eq!(
        returned_state.parent_state.path.path_root(),
        parent_compare_state.parent_state.path.path_root(),
        "Failed advance should return original state"
    );
}

#[test]
fn test_parent_compare_state_advance_with_nested_pattern() {
    // Create a graph with nested patterns: [a, b] and [ab, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, _ab_id) => [a, b],
        (abc, abc_id) => [ab, c]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create a ParentState at index 0 of 'abc' pattern
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let parent_path: IndexStartPath = rooted_path!(Start: root.clone(), 0);
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    // Create a PatternCursor with the nested pattern
    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![ab, c]),
        start: 0,
        end: 1
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            current: cursor.as_candidate(),
        },
    };

    tracing::info!(
        ?parent_compare_state,
        "ParentCompareState with nested pattern"
    );

    // Advance should succeed
    let result = parent_compare_state.clone().advance_state(&graph);

    assert!(
        result.is_ok(),
        "ParentCompareState with nested pattern should advance"
    );

    let compare_root_state = result.unwrap();
    tracing::info!(?compare_root_state, "Advanced with nested pattern");

    // Verify the child cursor has the correct root
    assert_eq!(
        compare_root_state
            .candidate
            .child
            .current()
            .child_state
            .path
            .path_root(),
        root
    );
}

#[test]
fn test_parent_compare_state_cursor_conversion() {
    // Test cursor type conversions during advancement
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );
    let _tracing = init_test_tracing!(&graph);

    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let parent_path: IndexStartPath = rooted_path!(Start: root, 0);
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    // Create PatternCursor with specific atom position
    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![a, b, c]),
        start: 0,
        end: 2
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(5), // Non-zero position
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            current: cursor.as_candidate(),
        },
    };

    let result = parent_compare_state.clone().advance_state(&graph);
    assert!(result.is_ok(), "Should advance successfully");

    let compare_root_state = result.unwrap();

    // Verify PatternRangePath was converted to PatternPrefixPath
    tracing::info!(
        cursor_path = ?compare_root_state.candidate.query.current().path,
        "Cursor path after conversion"
    );

    // Verify atom_position was preserved
    assert_eq!(
        compare_root_state.candidate.query.current().atom_position,
        AtomPosition::from(5),
        "Cursor atom_position should be preserved"
    );
    assert_eq!(
        *compare_root_state.candidate.child.current().child_state.target_offset(),
        AtomPosition::from(0),
        "Child cursor target_offset should match parent_state root_pos (not query cursor position)"
    );
    assert_eq!(
        compare_root_state
            .candidate
            .query
            .checkpoint()
            .atom_position,
        AtomPosition::from(5),
        "Checkpoint atom_position should be preserved"
    );
}

#[test]
fn test_state_advance_error_propagation() {
    // Test that errors from underlying ParentState are properly propagated
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, { a, b });
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create ParentState that cannot advance (at last index)
    let root = IndexRoot::from(
        ChildLocation::new(ab, ab_id, 1).into_pattern_location(),
    );
    let parent_path: IndexStartPath = rooted_path!(Start: root, 1);
    let parent_state = ParentState {
        path: parent_path,
        prev_pos: AtomPosition::from(0),
        root_pos: AtomPosition::from(0),
    };

    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![a, b]),
        start: 0,
        end: 0
    );
    let cursor = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let parent_compare_state = ParentCompareState {
        parent_state,
        cursor: Checkpointed {
            checkpoint: cursor.clone(),
            current: cursor.as_candidate(),
        },
    };

    tracing::info!(
        ?parent_compare_state,
        "ParentCompareState that cannot advance"
    );

    let result = parent_compare_state.clone().advance_state(&graph);

    assert!(
        result.is_err(),
        "Should fail when underlying ParentState cannot advance"
    );

    let returned_state = result.unwrap_err();
    assert_eq!(
        returned_state.cursor.current().atom_position,
        parent_compare_state.cursor.current().atom_position,
        "Original cursor should be returned on error"
    );
}
