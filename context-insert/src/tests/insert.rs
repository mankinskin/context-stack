use crate::{
    insert::{
        ToInsertCtx,
        context::InsertTraversal,
    },
    interval::init::InitInterval,
};
use context_search::*;
use context_trace::{
    tests::macros::string_repr::{
        assert_all_vertices_unique,
        assert_token_string_repr,
    },
    trace::has_graph::HasGraph,
    *,
};
use pretty_assertions::{
    assert_eq,
    assert_matches,
};
use tracing::debug;

#[test]
fn index_pattern1() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        ab => [[a, b]],
        by => [[b, y]],
        yz => [[y, z]],
        xa => [[x, a]],
        xab => [[x, ab], [xa, b]],
        xaby => [[xab, y], [xa, by]],
        xabyz => [[xaby, z], [xab, yz]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);
    print!("{:#?}", xabyz);
    // todo: split sub patterns not caught by query search

    // Verify all vertices have unique string representations before insertion
    {
        let g = graph.graph();
        assert_all_vertices_unique(&*g);
    }

    let query = vec![by, z];
    let byz: Token = graph.insert(query.clone()).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(&*g, byz, "byz");
        assert_all_vertices_unique(&*g);
    }
    assert_eq!(byz.width(), 3, "byz should have width 3");

    let byz_found = graph.find_ancestor(&query);
    assert_matches!(
        byz_found,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == byz,
        "byz"
    );

    let query = vec![ab, y];
    let aby: Token = graph.insert(query.clone()).expect("Indexing failed");

    // Assert aby has the expected string representation
    {
        let g = graph.graph();
        assert_token_string_repr(&*g, aby, "aby");
        assert_all_vertices_unique(&*g);
    }

    let aby_found = graph.find_parent(&query);
    assert_matches!(
        aby_found,
        Ok(ref response) if response.query_exhausted() && response.is_full_token() && response.root_token() == aby,
        "aby"
    );
}

#[test]
fn index_pattern2() {
    let mut graph = HypergraphRef::default();
    let _tracing = context_trace::init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        yz => [[y, z]],
        xab => [[x, a, b]],
        _xyz => [[x, yz]],
        _xabz => [[xab, z]],
        _xabyz => [[xab, yz]],
    );

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
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, w, x, y, z});
    insert_patterns!(graph,
        yz => [[y, z]],
        xxabyzw => [[x, x, a, b, yz, w]],
    );
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
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, x, y});
    insert_patterns!(graph,
        yy => [y, y],
        xx => [x, x],
        xy => [x, y],
        abcdx => [a, b, c, d, x],
        yabcdx => [y, abcdx],
        abcdxx => [abcdx, x],
    );
    insert_patterns!(graph,
        xxy => [[xx, y], [x, xy]],
        _xxyyabcdxxyy => [[xx, yy, abcdxx, yy], [xxy, yabcdx, xy, y]],
    );
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
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_id) => [l, d],
        (heldld, heldld_id) => [h, e, ld, ld]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);
    let fold_res =
        Searchable::<InsertTraversal>::search(vec![h, e, l, l], graph.clone());
    assert_matches!(fold_res, Ok(ref response) if !response.query_exhausted());
    let state = fold_res.unwrap();
    let init = InitInterval::from(state);

    debug!("end_bound = {:?}", init.end_bound);

    assert_eq!(
        init,
        InitInterval {
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
        }
    );
    let hel: Token = graph.insert_init((), init);
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
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});

    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (ababcd, ababcd_id) => [ab, ab, c, d]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);
    let fold_res =
        Searchable::<InsertTraversal>::search(vec![b, c, d, d], graph.clone());

    assert_matches!(fold_res, Ok(ref response) if !response.query_exhausted());
    let state = fold_res.unwrap();
    let init = InitInterval::from(state);
    assert_eq!(
        init,
        InitInterval {
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
        },
    );
    let bcd: Token = graph.insert_init((), init);
    assert_indices!(graph, cd, abcd);
    assert_patterns! {
        graph,
        cd => [[c, d]],
        bcd => [[b, cd]],
        abcd => [[a, bcd], [ab, cd]],
        ababcd => [[ab, abcd]]
    };
}
