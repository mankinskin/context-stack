//! Tests for path advancement behavior to ensure can_advance and advance are consistent
//!
//! This test suite was created to reproduce a bug found in context-search where
//! can_advance() returned true but advance() returned Break.

use crate::{
    path::mutators::move_path::advance::{
        Advance,
        CanAdvance,
    },
    *,
};

#[test]
fn test_pattern_cursor_at_end_cannot_advance() {
    let _tracing = init_test_tracing!();

    // Create a simple graph with a pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Create a RootedRangePath starting from the root token 'abc'
    // This creates a path with root at pattern location for abc, initially pointing at index 0
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let mut path: IndexRangePath = IndexRangePath::new_empty(root);

    tracing::info!(%path, "Initial path");
    tracing::info!(
        end_index = %path.role_root_child_index::<crate::path::accessors::role::End>(),
        "End index"
    );

    // Advance to position 1
    let result1 = path.advance(&graph);
    tracing::info!(
        ?result1,
        end_index = %path.role_root_child_index::<crate::path::accessors::role::End>(),
        "First advance result"
    );
    assert!(result1.is_continue(), "First advance should succeed");

    // Advance to position 2 (the last element)
    let result2 = path.advance(&graph);
    tracing::info!(
        ?result2,
        end_index = %path.role_root_child_index::<crate::path::accessors::role::End>(),
        "Second advance result"
    );
    assert!(result2.is_continue(), "Second advance should succeed");

    // Now we're at the end (index 2 of a 3-element pattern)
    let end_index =
        path.role_root_child_index::<crate::path::accessors::role::End>();
    tracing::info!(%end_index, "After two advances");

    // Check can_advance
    let can_advance = path.can_advance(&graph);
    tracing::info!(%can_advance, "can_advance result");

    // Try to advance again
    let advance_result = path.clone().advance(&graph);
    tracing::info!(?advance_result, "Third advance result");

    // CRITICAL INVARIANT: can_advance should guarantee advance succeeds!
    if can_advance {
        assert!(
            advance_result.is_continue(),
            "BUG REPRODUCED: can_advance returned true but advance returned Break. \
             This violates the contract that can_advance should guarantee advance succeeds."
        );
    } else {
        assert!(
            advance_result.is_break(),
            "If can_advance is false, advance should return Break"
        );
    }
}

#[test]
fn test_can_advance_advance_consistency() {
    let _tracing = init_test_tracing!();

    // Create a graph with multiple patterns to test various states
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (abc, _abc_id) => [a, b, c],
        (abcd, abcd_id) => [abc, d]
    );

    // Test at various positions
    let root = IndexRoot::from(
        ChildLocation::new(abcd, abcd_id, 0).into_pattern_location(),
    );
    let mut path: IndexRangePath = IndexRangePath::new_empty(root);

    for step in 0..10 {
        let can_advance = path.can_advance(&graph);
        let mut path_clone = path.clone();
        let advance_result = path_clone.advance(&graph);

        tracing::info!(
            "Step {}: can_advance={}, advance={:?}, end_index={}",
            step,
            can_advance,
            advance_result,
            path.role_root_child_index::<crate::path::accessors::role::End>()
        );

        // CRITICAL INVARIANT: can_advance should guarantee advance succeeds
        if can_advance {
            assert!(
                advance_result.is_continue(),
                "Step {}: can_advance returned true but advance returned {:?}. \
                 This is a bug - can_advance should guarantee that advance will succeed.",
                step,
                advance_result
            );
        }

        // If advance fails, we're done
        if advance_result.is_break() {
            break;
        }

        // Apply the advance to continue testing
        let _ = path.advance(&graph);
    }
}
