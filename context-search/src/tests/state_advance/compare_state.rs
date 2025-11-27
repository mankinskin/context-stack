//! Tests for CompareState<Candidate, Candidate> and CompareState<Matched, Matched> advancement

use crate::{
    compare::state::CompareState,
    cursor::{
        Candidate,
        Matched,
        PathCursor,
        PatternCursor,
    },
};
use context_trace::{path::accessors::path_accessor::HasTargetOffset, *};
use std::marker::PhantomData;

#[test]
fn test_compare_state_candidate_advance() {
    // Create a graph with pattern: [a, b, c]
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (_abc, _abc_id) => [ab, c]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create a CompareState<Candidate, Candidate>
    let root = IndexRoot::from(
        ChildLocation::new(ab, ab_id, 0).into_pattern_location(),
    );
    let child_state = context_trace::ChildState {
        entry_pos: AtomPosition::from(0),
        start_pos: AtomPosition::from(0),
        path: rooted_path!(Range: root, start: 0, end: 0),
    };

    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![ab, c]),
        start: 0,
        end: 1
    );
    let cursor = PathCursor {
        path: pattern_path.clone(),
        atom_position: AtomPosition::from(0),
        _state: PhantomData::<Candidate>,
    };

    let checkpoint = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let compare_state = CompareState {
        child: crate::cursor::Checkpointed {
            candidate: Some(crate::cursor::ChildCursor {
                child_state: child_state.clone(),
                _state: PhantomData,
            }),
            checkpoint: crate::cursor::ChildCursor {
                child_state,
                _state: PhantomData,
            },
            _state: PhantomData,
        },
        query: crate::cursor::Checkpointed {
            candidate: Some(cursor),
            checkpoint,
            _state: PhantomData,
        },
        mode: crate::compare::state::PathPairMode::GraphMajor,
        target: context_trace::trace::cache::key::directed::down::DownKey::new(
            ab,
            context_trace::trace::cache::key::directed::down::DownPosition(
                AtomPosition::from(0),
            ),
        ),
    };

    tracing::info!(
        ?compare_state,
        "Initial CompareState<Candidate, Candidate>"
    );

    // Advance the state
    let result = compare_state.clone().advance_state(&graph);

    // CompareState<Candidate, Candidate> always returns Ok (wraps the result)
    assert!(
        result.is_ok(),
        "CompareState<Candidate> advance should return Ok"
    );

    let advanced_state = result.unwrap();
    tracing::info!(?advanced_state, "Advanced CompareState");

    // Verify cursors and checkpoint are preserved
    assert_eq!(
        advanced_state.query.checkpoint().atom_position,
        compare_state.query.checkpoint().atom_position
    );
}

#[test]
fn test_compare_state_matched_advance() {
    // Create a graph
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, _ab_id) => [a, b],
        (abc, abc_id) => [ab, c]
    );
    let _tracing = init_test_tracing!(&graph);

    // Create a CompareState<Matched, Matched>
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let child_state = context_trace::ChildState {
        entry_pos: AtomPosition::from(0),
        start_pos: AtomPosition::from(0),
        path: rooted_path!(Range: root, start: 0, end: 0),
    };

    let pattern_path: PatternRangePath = rooted_path!(
        Range: Pattern::from(vec![ab, c]),
        start: 0,
        end: 1
    );
    let cursor: PathCursor<PatternRangePath, Matched> = PathCursor {
        path: pattern_path.clone(),
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let checkpoint = PatternCursor {
        path: pattern_path,
        atom_position: AtomPosition::from(0),
        _state: PhantomData,
    };

    let compare_state = CompareState {
        child: crate::cursor::Checkpointed {
            candidate: Some(crate::cursor::ChildCursor {
                child_state: child_state.clone(),
                _state: PhantomData,
            }),
            checkpoint: crate::cursor::ChildCursor {
                child_state,
                _state: PhantomData,
            },
            _state: PhantomData,
        },
        query: crate::cursor::Checkpointed {
            candidate: Some(cursor),
            checkpoint,
            _state: PhantomData,
        },
        mode: crate::compare::state::PathPairMode::GraphMajor,
        target: context_trace::trace::cache::key::directed::down::DownKey::new(
            ab,
            context_trace::trace::cache::key::directed::down::DownPosition(
                AtomPosition::from(0),
            ),
        ),
    };

    tracing::info!(?compare_state, "Initial CompareState<Matched, Matched>");

    // Advance the state
    let result = compare_state.clone().advance_state(&graph);

    // CompareState<Matched, Matched> always returns Ok
    assert!(
        result.is_ok(),
        "CompareState<Matched> advance should return Ok"
    );

    let advanced_state = result.unwrap();
    tracing::info!(?advanced_state, "Advanced CompareState<Matched>");

    // Verify all fields are preserved correctly
    assert_eq!(
        advanced_state.query.current().atom_position,
        compare_state.query.current().atom_position
    );
    assert_eq!(
        *advanced_state.child.current().child_state.target_offset(),
        *compare_state.child.current().child_state.target_offset()
    );
    assert_eq!(
        advanced_state.query.checkpoint().atom_position,
        compare_state.query.checkpoint().atom_position
    );
    assert_eq!(advanced_state.mode, compare_state.mode);
}
