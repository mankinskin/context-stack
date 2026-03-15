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
        ab,
        abc,
        gh,
        ghi,
        efgh,
        efghi,
        aba,
        abcd,
        abab,
        ababcd,
        abcdef,
        ababab,
        abcdefghi,
        ababcdefghi,
        ababababcdefghi,
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

    // Exact expected event sequence for first search (37 events)
    assert_events(
        &fin1.events,
        &[
            start(g),                          // 0
            explore(g, &[gh]),                 // 1
            up(g, gh),                         // 2
            down(gh, h, false),                // 3
            matched(h, 2),                     // 4
            root_match(gh),                    // 5
            explore(gh, &[ghi, efgh]),         // 6
            up(gh, ghi),                       // 7
            down(ghi, i, false),               // 8
            matched(i, 3),                     // 9
            root_match(ghi),                   // 10
            explore(ghi, &[efghi, abcdefghi]), // 11
            up(ghi, efghi),                    // 12
            explore(
                efghi,
                &[abcdefghi, abcdefghi, ababcdefghi, ababababcdefghi],
            ), // 13
            up(efghi, abcdefghi),              // 14
            explore(
                abcdefghi,
                &[
                    abcdefghi,
                    ababcdefghi,
                    ababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 15
            up(abcdefghi, abcdefghi),          // 16
            explore(
                abcdefghi,
                &[
                    ababcdefghi,
                    ababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababcdefghi,
                    ababababcdefghi,
                ],
            ), // 17
            up(abcdefghi, ababcdefghi),        // 18
            explore(
                ababcdefghi,
                &[
                    ababcdefghi,
                    ababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 19
            up(ababcdefghi, ababcdefghi),      // 20
            explore(
                ababcdefghi,
                &[
                    ababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 21
            up(ababcdefghi, ababcdefghi),      // 22
            explore(
                ababcdefghi,
                &[
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 23
            up(ababcdefghi, ababababcdefghi),  // 24
            explore(
                ababababcdefghi,
                &[
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 25
            up(ababababcdefghi, ababababcdefghi), // 26
            explore(
                ababababcdefghi,
                &[
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                    ababababcdefghi,
                ],
            ), // 27
            up(ababababcdefghi, ababababcdefghi), // 28
            explore(
                ababababcdefghi,
                &[ababababcdefghi, ababababcdefghi, ababababcdefghi],
            ), // 29
            up(ababababcdefghi, ababababcdefghi), // 30
            explore(ababababcdefghi, &[ababababcdefghi, ababababcdefghi]), // 31
            up(ababababcdefghi, ababababcdefghi), // 32
            explore(ababababcdefghi, &[ababababcdefghi]), // 33
            up(ababababcdefghi, ababababcdefghi), // 34
            explore(ababababcdefghi, &[]),     // 35
            done_ok(ghi),                      // 36
        ],
    );

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

    // Exact expected event sequence for second search (16 events)
    assert_events(
        &fin2.events,
        &[
            start(a),                          // 0
            explore(a, &[ab, abc, aba, abcd]), // 1
            up(a, ab),                         // 2
            down(ab, b, false),                // 3
            matched(b, 5),                     // 4
            root_match(ab),                    // 5
            explore(
                ab,
                &[
                    aba,
                    abc,
                    abab,
                    abab,
                    ababcd,
                    abcdef,
                    ababab,
                    ababab,
                    ababcdefghi,
                ],
            ), // 6
            up(ab, aba),                       // 7
            down(aba, a, false),               // 8
            mismatched(a, 6, c, a),            // 9
            skip(aba, 8, true),                // 10
            up(ab, abc),                       // 11
            down(abc, c, false),               // 12
            matched(c, 6),                     // 13
            root_match(abc),                   // 14
            done_ok(abc),                      // 15
        ],
    );
}
