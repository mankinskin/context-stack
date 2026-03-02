#[cfg(test)]
use {
    crate::search::Find,
    crate::state::end::PathCoverage,
    crate::tests::search::event_helpers::*,
    context_trace::{
        graph::{
            getters::{
                ErrorReason,
                IndexWithPath,
            },
            vertex::token::Token,
            visualization::Transition,
        },
        init_test_tracing,
        tests::{
            env::Env1,
            test_case::TestEnv,
        },
        GraphRoot,
    },
    pretty_assertions::assert_eq,
};

#[test]
fn find_parent1() {
    let Env1 {
        graph,
        a,
        b,
        c,
        ab,
        bc,
        abc,
        aba,
        bcd,
        abab,
        ababcd,
        abcdef,
        ababab,
        ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();
    //let a_bc_pattern = [Token::new(a, 1), Token::new(bc, 2)];
    let ab_c_pattern = [Token::new(ab, 2), Token::new(c, 1)];
    //let a_bc_d_pattern =
    //    [Token::new(a, 1), Token::new(bc, 2), Token::new(d, 1)];
    let b_c_pattern = vec![Token::new(b, 1), Token::new(c, 1)];
    let bc_pattern = [Token::new(bc, 2)];
    //let a_b_c_pattern = [Token::new(a, 1), Token::new(b, 1), Token::new(c, 1)];

    let query = bc_pattern;
    assert_eq!(
        graph.find_parent(&query[..]),
        Err(ErrorReason::SingleIndex(Box::new(IndexWithPath {
            index: *bc,
            path: query.into()
        }))),
        "bc"
    );
    let query = b_c_pattern;
    let response = graph.find_parent(&query).unwrap();
    assert!(response.query_exhausted(), "Query should be complete");
    match &response.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *bc, "Should match bc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }

    // Exact expected event sequence for b_c (7 events)
    assert_events(&response.events, &[
        start(b),                                         // 0
        explore(b, &[bc, ab, bcd, abab]),                 // 1
        up(b, bc),                                        // 2
        down(bc, c, false),                               // 3
        matched(c, 2),                                    // 4
        root_match(bc),                                   // 5
        done_ok(bc),                                      // 6
    ]);

    let query = ab_c_pattern;
    let response = graph.find_parent(query).unwrap();
    assert!(response.query_exhausted(), "Query should be complete");
    match &response.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *abc, "Should match abc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }

    // Exact expected event sequence for ab_c (11 events)
    assert_events(&response.events, &[
        start(ab),                                                                         // 0
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]), // 1
        up(ab, aba),                                                                       // 2
        down(aba, a, false),                                                               // 3
        mismatched(a, 3, c, a),                                                            // 4
        skip(aba, 8, true),                                                                // 5
        up(ab, abc),                                                                       // 6
        down(abc, c, false),                                                               // 7
        matched(c, 3),                                                                     // 8
        root_match(abc),                                                                   // 9
        done_ok(abc),                                                                      // 10
    ]);
    // enable when bfs for parent-token batches is implemented
    //let query = a_bc_pattern;
    //assert_matches!(
    //    graph.find_parent(&query),
    //    Ok(Response {
    //        kind: ResponseKind::Complete(x),
    //        ..
    //    }) if x == *abc,
    //    "a_bc"
    //);
    //let query = a_bc_d_pattern;
    //assert_matches!(
    //    graph.find_parent(&query),
    //    Ok(Response {
    //        kind: ResponseKind::Complete(x),
    //        ..
    //    }) if x == *abc,
    //    "a_bc_d"
    //);
    //let query = a_b_c_pattern.clone();
    //assert_matches!(
    //    graph.find_parent(&query),
    //    Ok(Response {
    //        kind: ResponseKind::Complete(x),
    //        ..
    //    }) if x == *abc,
    //    "a_b_c"
    //);
    //let query = [&a_b_c_pattern[..], &[Token::new(c, 1)]].concat();
    //assert_matches!(
    //    graph.find_parent(&query),
    //    Ok(Response {
    //        kind: ResponseKind::Complete(x),
    //        ..
    //    }) if x == *abc,
    //    "a_b_c_c"
    //);
}
