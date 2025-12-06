use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
    tests::{
        cases::pattern1::{
            Pattern1Aby,
            Pattern1Byz,
        },
        env::{
            EnvIndexInfix1,
            EnvIndexInfix2,
            EnvIndexPattern1,
            EnvIndexPattern2,
        },
        test_case::InsertTestCase,
    },
};
use context_search::{
    tests::env::{
        EnvIndexPostfix1,
        EnvIndexPrefix1,
    },
    *,
};
use context_trace::{
    tests::{
        macros::string_repr::{
            assert_all_vertices_unique,
            assert_token_string_repr,
        },
        test_case::{
            TestCase,
            TestEnv,
        },
    },
    trace::has_graph::HasGraph,
    *,
};
use pretty_assertions::{
    assert_eq,
    assert_matches,
};

#[test]
fn index_pattern1() {
    // Test case 1: Insert "byz"
    let case = Pattern1Byz;
    let env = case.environment();
    let _tracing = context_trace::init_test_tracing!(env.graph());

    // Verify all vertices have unique string representations before insertion
    {
        let g = env.graph.graph();
        assert_all_vertices_unique(&*g);
    }

    let query = case.input_tokens();
    let result_token: Token =
        env.graph.insert(query.clone()).expect("Indexing failed");

    // Assert the token has the expected string representation
    {
        let g = env.graph.graph();
        assert_token_string_repr(&*g, result_token, case.expected_string());
        assert_all_vertices_unique(&*g);
    }
    assert_eq!(
        result_token.width(),
        case.expected_token().width(),
        "byz should have expected width"
    );

    let found = env.graph.find_ancestor(&query);
    assert_matches!(
        found,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == result_token,
        "byz"
    );

    // Test case 2: Insert "aby"
    let case2 = Pattern1Aby;
    let query2 = case2.input_tokens();
    let result_token2: Token =
        env.graph.insert(query2.clone()).expect("Indexing failed");

    // Assert aby has the expected string representation
    {
        let g = env.graph.graph();
        assert_token_string_repr(&*g, result_token2, case2.expected_string());
        assert_all_vertices_unique(&*g);
    }

    let found2 = env.graph.find_parent(&query2);
    assert_matches!(
        found2,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == result_token2,
        "aby"
    );
}

#[test]
fn index_pattern2() {
    // Create independent test environment
    let EnvIndexPattern2 {
        graph,
        a,
        b,
        x,
        y,
        z,
        yz: _yz,
        xab: _xab,
        xyz: _xyz,
        xabz: _xabz,
        xabyz: _xabyz,
    } = EnvIndexPattern2::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Verify all vertices have unique string representations before insertion
    {
        let g = graph.graph();
        assert_all_vertices_unique(&*g);
    }

    let query = vec![a, b, y, x];
    let aby: Token = graph.insert(query.clone()).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(&*g, aby, "aby");
        assert_all_vertices_unique(&*g);
    }
    assert_eq!(aby.width(), 3);

    let ab = graph
        .find_ancestor("ab".chars())
        .unwrap()
        .expect_complete("ab")
        .root_parent();
    let g = graph.graph();
    let aby_vertex = g.expect_vertex(aby);
    assert_eq!(aby_vertex.parents().len(), 1, "aby");
    assert_eq!(
        aby_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![ab, y]),])
    );
    drop(g);
    let query = vec![a, b, y];
    let aby_found = graph.find_ancestor(&query);
    assert_matches!(
        aby_found,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == aby,
        "aby"
    );
}

#[test]
fn index_infix1() {
    // Create independent test environment
    let EnvIndexInfix1 {
        graph,
        a,
        b,
        w,
        x,
        y,
        z,
        yz,
        xxabyzw,
    } = EnvIndexInfix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Verify all vertices have unique string representations before insertion
    {
        let g = graph.graph();
        assert_all_vertices_unique(&*g);
    }

    let aby: Token = graph.insert(vec![a, b, y]).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(&*g, aby, "aby");
        assert_all_vertices_unique(&*g);
    }

    let ab = graph
        .find_ancestor(vec![a, b])
        .unwrap()
        .expect_complete("ab")
        .root_parent();
    let g = graph.graph();
    let aby_vertex = g.expect_vertex(aby);
    assert_eq!(aby.width(), 3, "aby");
    assert_eq!(aby_vertex.parents().len(), 1, "aby");
    assert_eq!(aby_vertex.child_patterns().len(), 1, "aby");
    assert_eq!(
        aby_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![ab, y])]),
        "aby"
    );
    drop(g);
    let query = vec![a, b, y];
    let aby_found = graph.find_ancestor(&query);
    assert_matches!(
        aby_found,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == aby,
        "aby"
    );
    let abyz = graph
        .find_ancestor(vec![ab, yz])
        .unwrap()
        .expect_complete("abyz")
        .root_parent();
    let g = graph.graph();
    let abyz_vertex = g.expect_vertex(abyz);
    assert_eq!(
        abyz_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([
            Pattern::from(vec![aby, z]),
            Pattern::from(vec![ab, yz])
        ]),
        "abyz"
    );
    let xxabyzw_vertex = g.expect_vertex(xxabyzw);
    assert_eq!(
        xxabyzw_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![x, x, abyz, w])]),
        "xxabyzw"
    );
}

#[test]
fn index_infix2() {
    // Create independent test environment
    let EnvIndexInfix2 {
        graph,
        a,
        b,
        c,
        d,
        x,
        y: _y,
        yy: _yy,
        xx: _xx,
        xy: _xy,
        abcdx,
        yabcdx: _yabcdx,
        abcdxx: _abcdxx,
        xxy: _xxy,
        xxyyabcdxxyy: _xxyyabcdxxyy,
    } = EnvIndexInfix2::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Verify all vertices have unique string representations before insertion
    {
        let g = graph.graph();
        assert_all_vertices_unique(&*g);
    }

    let abcd: Token = graph.insert(vec![a, b, c, d]).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(&*g, abcd, "abcd");
        assert_all_vertices_unique(&*g);
    }

    let g = graph.graph();
    let abcd_vertex = g.expect_vertex(abcd);
    assert_eq!(abcd.width(), 4, "abcd");
    assert_eq!(abcd_vertex.parents().len(), 1, "abcd");
    assert_eq!(abcd_vertex.child_patterns().len(), 1, "abcd");
    assert_eq!(
        abcd_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![a, b, c, d])]),
        "abc"
    );
    let abcdx_vertex = g.expect_vertex(abcdx);
    assert_eq!(
        abcdx_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![abcd, x]),]),
        "abcx"
    );
}

#[test]
fn index_prefix1() {
    // Create independent test environment
    let EnvIndexPrefix1 {
        graph,
        h,
        e,
        l,
        d,
        ld,
        ld_id,
        heldld,
        heldld_id,
    } = EnvIndexPrefix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Expected InitInterval from search for [h, e, l, l]
    let expected_init = InitInterval {
        root: heldld,
        cache: build_trace_cache!(
            heldld => (
                BU {},
                TD { 2 => ld -> (heldld_id, 2) },
            ),
            ld => (
                BU {},
                TD { 2 => l -> (ld_id, 0) },
            ),
            h => (
                BU {},
                TD {},
            ),
            l => (
                BU {},
                TD { 2 },
            ),
        ),
        end_bound: 3.into(),
    };

    let hel: Token = graph.insert_init((), expected_init);
    assert_indices!(graph, he, held);
    assert_patterns! {
        graph,
        he => [[h, e]],
        hel => [[he, l]],
        held => [[hel, d], [he, ld]],
        heldld => [[held, ld]]
    };
}

#[test]
fn index_postfix1() {
    // Create independent test environment
    let EnvIndexPostfix1 {
        graph,
        a,
        b,
        c,
        d,
        ab,
        ab_id,
        ababcd,
        ababcd_id,
    } = EnvIndexPostfix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Expected InitInterval from search for [b, c, d, d]
    let expected_init = InitInterval {
        root: ababcd,
        cache: build_trace_cache!(
            ababcd => (
                BU { 1 => ab -> (ababcd_id, 1) },
                TD {},
            ),
            ab => (
                BU { 1 => b -> (ab_id, 1) },
                TD {},
            ),
            b => (
                BU {},
                TD {},
            ),
        ),
        end_bound: 3.into(),
    };

    let bcd: Token = graph.insert_init((), expected_init);
    assert_indices!(graph, cd, abcd);
    assert_patterns! {
        graph,
        cd => [[c, d]],
        bcd => [[b, cd]],
        abcd => [[a, bcd], [ab, cd]],
        ababcd => [[ab, abcd]]
    };
}
