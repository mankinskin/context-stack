#[cfg(test)]
use {
    crate::search::Find,
    crate::state::end::PathCoverage,
    context_trace::tests::env::Env1,
    context_trace::GraphRoot,
    context_trace::Pattern,
    context_trace::PatternPrefixPath,
    context_trace::{
        graph::vertex::token::Token,

        tests::env::TestEnv,
    },
};

#[test]
fn find_consecutive1() {
    let _tracing = context_trace::init_test_tracing!();

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
    } = &*Env1::get_expected();
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
    let fin = graph.find_ancestor(&query).unwrap();
    assert!(fin.query_exhausted(), "Query should be complete");
    match &fin.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *ghi, "Should match ghi root");
        },
        _ => panic!("Expected EntireRoot path"),
    }

    // Extract the cursor from the response and use it for the next search
    let query = fin.end.cursor().clone();
    let response = graph.find_ancestor(&query).unwrap();
    assert!(response.query_exhausted(), "Query should be complete");
    match &response.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *abc, "Should match abc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }
}
