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
    context_trace::{
        End,
        HasRootChildIndex,
        Start,
    },
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
    } = &*Env1::get_expected();
    let _tracing = context_trace::init_test_tracing!(graph);

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
    let start_index =
        HasRootChildIndex::<Start>::root_child_index(&cursor.path);
    let end_index = HasRootChildIndex::<End>::root_child_index(&cursor.path);

    tracing::debug!(%checkpoint_pos, %start_index, %end_index, "After first search");
    tracing::debug!(%cursor.path, "Cursor path");

    assert_eq!(
        checkpoint_pos, 3,
        "Checkpoint position should be 3 after matching ghi"
    );
    assert_eq!(start_index, 0, "Start index should be 0");
    assert_eq!(
        end_index, 3,
        "End index should be 3 (pointing to first unmatched token 'a')"
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

    // Extract the cursor from the response and use it for the next search
    let query = fin1.end.cursor().clone();
    // second search
    let fin2 = graph.find_ancestor(&query).unwrap();
    // Verify cursor state after second search
    let cursor = fin2.end.cursor();
    let checkpoint_pos = *cursor.atom_position.as_ref();

    // Verify cursor path range
    let start_index =
        HasRootChildIndex::<Start>::root_child_index(&cursor.path);
    let end_index = HasRootChildIndex::<End>::root_child_index(&cursor.path);

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
}
