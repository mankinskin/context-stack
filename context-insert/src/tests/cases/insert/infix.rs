use crate::{
    insert::ToInsertCtx,
    tests::env::{
        EnvInsertInfix1,
        EnvInsertInfix2,
    },
};
use context_search::*;
use context_trace::{
    tests::{
        macros::string_repr::{
            assert_all_vertices_unique,
            assert_token_string_repr,
        },
        test_case::TestEnv,
    },
    trace::has_graph::HasGraph,
    *,
};
use pretty_assertions::{
    assert_eq,
    assert_matches,
};

#[test]
fn insert_infix1() {
    // Create independent test environment
    let EnvInsertInfix1 {
        graph,
        a,
        b,
        w,
        x,
        y,
        z,
        yz,
        xxabyzw,
    } = EnvInsertInfix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    let aby: Token = graph.insert(vec![a, b, y]).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(g, aby, "aby");
        assert_all_vertices_unique(g);
    }

    let ab = graph
        .find_ancestor(vec![a, b])
        .unwrap()
        .expect_complete("ab")
        .root_parent();
    let g = graph.graph();
    let aby_vertex = g.expect_vertex_data(aby);
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
    let abyz_vertex = g.expect_vertex_data(abyz);
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
    let xxabyzw_vertex = g.expect_vertex_data(xxabyzw);
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
fn insert_infix2() {
    // Create independent test environment
    let EnvInsertInfix2 {
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
    } = EnvInsertInfix2::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    let abcd: Token = graph.insert(vec![a, b, c, d]).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(g, abcd, "abcd");
        assert_all_vertices_unique(g);
    }

    let g = graph.graph();
    let abcd_vertex = g.expect_vertex_data(abcd);
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
    let abcdx_vertex = g.expect_vertex_data(abcdx);
    assert_eq!(
        abcdx_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![abcd, x]),]),
        "abcx"
    );
}
