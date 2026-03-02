use context_trace::graph::visualization::Transition;
use crate::{
    cursor::Checkpointed,
    state::matched::CheckpointedCursor,
};
#[cfg(test)]
use {
    crate::{
        search::Find,
        state::end::PathCoverage,
    },
    context_trace::*,
    pretty_assertions::assert_matches,
};

#[test]
fn find_consecutive1() {
    let Env1 {
        graph,
        a,
        b,
        c,
        g,
        h,
        i,
        abc,
        ghi,
        ..
    } = &*Env1::get();
    let _tracing = context_trace::init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    //let a_bc_pattern = [Token::new(a, 1), Token::new(bc, 2)];
    //let ab_c_pattern = [Token::new(ab, 2), Token::new(c, 1)];
    let g_h_i_a_b_c_pattern = vec![
        Token::new(g, 1),
        Token::new(h, 1),
        Token::new(i, 1),
        Token::new(a, 1),
        Token::new(b, 1),
        Token::new(c, 1),
    ];

    let query = PatternPrefixPath::from(Pattern::from(g_h_i_a_b_c_pattern));
    let fin1 = graph.find_ancestor(&query).unwrap();

    // Verify cursor state after first search
    let cursor = fin1.end.cursor();
    let checkpoint_pos = *cursor.atom_position.as_ref();

    // Verify cursor path range
    let start_index = cursor.path.role_root_child_index::<Start>();
    let end_index = cursor.path.role_root_child_index::<End>();

    tracing::debug!(%checkpoint_pos, %start_index, %end_index, "After first search");
    tracing::debug!(%cursor.path, "Cursor path");

    // Check the internal Checkpointed structure
    tracing::debug!(?fin1.end.cursor, "Full Checkpointed cursor state");

    // Test checkpoint state
    let checkpoint = fin1.end.cursor.checkpoint();
    let checkpoint_end = checkpoint.path.role_root_child_index::<End>();
    assert_eq!(
        *checkpoint.atom_position.as_ref(),
        3,
        "Checkpoint atom_position should be 3 after matching ghi"
    );
    assert_eq!(
        checkpoint_end, 2,
        "Checkpoint end_index should be 2 (last matched token 'i')"
    );

    // Test candidate state - THIS IS THE KEY ASSERTION
    assert!(
        matches!(fin1.end.cursor, CheckpointedCursor::HasCandidate(_)),
        "Cursor should have a candidate (advanced position) after parent exploration"
    );

    let candidate = fin1.end.cursor.cursor();
    let candidate_end = candidate.path.role_root_child_index::<End>();
    assert_eq!(
        *candidate.atom_position.as_ref(),
        4,
        "Candidate atom_position should be 4 (advanced beyond checkpoint)"
    );
    assert_eq!(
        candidate_end, 3,
        "Candidate end_index should be 3 (pointing to first unmatched token 'a')"
    );
    assert!(
        !fin1.query_exhausted(),
        "Query should not be exhausted after matching only ghi"
    );
    match &fin1.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *ghi, "Should match ghi root");
        },
        _ => panic!("Expected EntireRoot path"),
    }

    // Validate events for first search
    let t1 = fin1.transitions();
    assert_matches!(t1.first(), Some(Transition::StartNode { .. }));
    assert_matches!(t1.last(), Some(Transition::Done { .. }));
    assert!(t1.iter().any(|t| matches!(t, Transition::VisitChild { .. })), "Expected VisitChild");
    let steps1: Vec<usize> = fin1.events.iter().map(|e| e.step).collect();
    assert_eq!(steps1, (0..steps1.len()).collect::<Vec<_>>(), "Steps should be sequential");

    // Extract the cursor from the response and use it for the next search
    let query = fin1.end.cursor().clone();
    // second search
    let fin2 = graph.find_ancestor(&query).unwrap();
    // Verify cursor state after second search
    let cursor = fin2.end.cursor();
    let checkpoint_pos = *cursor.atom_position.as_ref();

    // Verify cursor path range
    let start_index = cursor.path.role_root_child_index::<Start>();
    let end_index = cursor.path.role_root_child_index::<End>();

    tracing::debug!(%checkpoint_pos, %start_index, %end_index, "After second search");
    tracing::debug!(%cursor.path, "Cursor path");

    assert_eq!(
        checkpoint_pos, 6,
        "Checkpoint position should be 6 after matching ghi and abc"
    );
    assert_eq!(start_index, 0, "Start index should be 0");
    assert_eq!(
        end_index, 5,
        "End index should be 5 (pointing at last matched token 'c', query exhausted)"
    );
    assert!(
        fin2.query_exhausted(),
        "Query should be exhausted after matching abc"
    );
    match &fin2.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *abc, "Should match abc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }

    // Validate events for second search
    let t2 = fin2.transitions();
    assert_matches!(t2.first(), Some(Transition::StartNode { .. }));
    assert_matches!(t2.last(), Some(Transition::Done { .. }));
    let steps2: Vec<usize> = fin2.events.iter().map(|e| e.step).collect();
    assert_eq!(steps2, (0..steps2.len()).collect::<Vec<_>>(), "Steps should be sequential");
}
