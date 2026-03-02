#[cfg(test)]
use {
    crate::search::Find,
    crate::state::end::PathCoverage,
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
        b,
        c,
        ab,
        bc,
        abc,
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

    // Validate events: must start with StartNode, end with Done, have sequential steps
    let transitions = response.transitions();
    assert!(
        matches!(transitions.first(), Some(Transition::StartNode { .. })),
        "First event should be StartNode, got {:?}", transitions.first()
    );
    assert!(
        matches!(transitions.last(), Some(Transition::Done { success: true, .. })),
        "Last event should be Done(success: true)"
    );
    let steps: Vec<usize> = response.events.iter().map(|e| e.step).collect();
    assert_eq!(steps, (0..steps.len()).collect::<Vec<_>>(), "Steps should be sequential");

    let query = ab_c_pattern;
    let response = graph.find_parent(query).unwrap();
    assert!(response.query_exhausted(), "Query should be complete");
    match &response.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *abc, "Should match abc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }
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
