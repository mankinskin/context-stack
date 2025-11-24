#[cfg(test)]
use {
    crate::{
        cursor::PatternCursor,
        search::Find,
        state::end::{
            postfix::PostfixEnd,
            PathCoverage,
        },
        state::matched::MatchResult,
        state::result::Response,
    },
    context_trace::*,
    itertools::*,
    pretty_assertions::{
        assert_eq,
        assert_matches,
    },
};

// Test: Single token bc should return error (not a pattern)
#[test]
fn find_ancestor1_single_token() {
    let Env1 { graph, bc, .. } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(bc, 2)];
    assert_eq!(
        graph.find_ancestor(&query),
        Err(ErrorReason::SingleIndex(Box::new(IndexWithPath {
            index: *bc,
            path: query.into()
        }))),
        "bc"
    );
}

// Test: Pattern [b, c] should match token bc (Complete)
#[test]
fn find_ancestor1_b_c() {
    let Env1 {
        graph, b, c, bc, ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(b, 1), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_eq!(response.query_exhausted(), true);
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *bc
    );
}

// Test: Pattern [a, bc] should match token abc (Complete)
#[test]
fn find_ancestor1_a_bc() {
    let Env1 {
        graph, a, bc, abc, ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(a, 1), Token::new(bc, 2)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);
}

// Test: Pattern [ab, c] should match token abc (Complete)
#[test]
fn find_ancestor1_ab_c() {
    let Env1 {
        graph, ab, c, abc, ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(ab, 2), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);
}

// Test: Pattern [a, bc, d] should match token abcd (Complete)
#[test]
fn find_ancestor1_a_bc_d() {
    let Env1 {
        graph,
        a,
        bc,
        d,
        abcd,
        ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(a, 1), Token::new(bc, 2), Token::new(d, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abcd
    );
    assert_eq!(response.query_exhausted(), true);
}

// Test: Pattern [a, b, c] should match token abc (Complete)
#[test]
fn find_ancestor1_a_b_c() {
    let Env1 {
        graph,
        a,
        b,
        c,
        abc,
        ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query = vec![Token::new(a, 1), Token::new(b, 1), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);
}
// Test: Pattern [a, b, c, c] should partially match token abc - only first 3 tokens match
//#[test]
//fn find_ancestor1_a_b_c_c() {
//    let Env1 {
//        graph,
//        a,
//        b,
//        c,
//        abc,
//        ..
//    } = &*Env1::get_expected();
//    let _tracing = init_test_tracing!(graph);
//
//    let query = vec![
//        Token::new(a, 1),
//        Token::new(b, 1),
//        Token::new(c, 1),
//        Token::new(c, 1),
//    ];
//    let response = graph.find_ancestor(&query).unwrap();
//    assert_matches!(
//        response.end.path,
//        PathCoverage::EntireRoot(ref path)
//            if path.path_root().pattern_location().parent == *abc
//    );
//    assert_eq!(response.query_exhausted(), false);
//}

// Test: Long pattern [a,b,a,b,a,b,a,b,c,d,e,f,g,h,i] should match ababababcdefghi (Complete)
#[test]
fn find_ancestor1_long_pattern() {
    let Env1 {
        graph,
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        h,
        i,
        ababababcdefghi,
        ..
    } = &*Env1::get_expected();
    let _tracing = init_test_tracing!(graph);

    let query: Vec<_> = [a, b, a, b, a, b, a, b, c, d, e, f, g, h, i]
        .into_iter()
        .cloned()
        .collect();
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *ababababcdefghi
    );
    assert_eq!(response.query_exhausted(), true);
}

#[test]
fn find_ancestor2() {
    use context_trace::*;

    let mut graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        ab => [a, b],
        by => [b, y],
        yz => [y, z],
        xa => [x, a],
    );
    insert_patterns!(graph,
        xab => [[x, ab],[xa, b]],
    );
    insert_patterns!(graph,
        (xaby, xaby_ids) => [[xa, by],[xab,y]],
        (xabyz, xabyz_ids) => [[xaby, z],[xab,yz]],
    );
    let _tracing = init_test_tracing!(&graph);
    let xa_by_id = xaby_ids[0];
    let xaby_z_id = xabyz_ids[0];
    //assert_eq!(xaby_z_id, 8);
    let graph = HypergraphRef::from(graph);
    let query = vec![by, z];
    let byz_found = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        byz_found.end.path().clone(),
        PathCoverage::Postfix(PostfixEnd {
            root_pos: 2.into(),
            path: RootedRolePath::new(
                PatternLocation::new(xabyz, xaby_z_id,),
                RolePath::new(0, vec![ChildLocation::new(xaby, xa_by_id, 1,)],),
            )
        })
    );
    assert_eq!(
        byz_found.end.cursor().clone(),
        PatternCursor {
            path: RootedRangePath::new(
                query.clone(),
                RolePath::new_empty(0),
                RolePath::new_empty(1),
            ),
            atom_position: 3.into(),
            _state: std::marker::PhantomData,
        }
    );
    assert_eq!(
        byz_found,
        Response {
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    root_pos: 2.into(),
                    path: RootedRolePath::new(
                        PatternLocation::new(xabyz, xaby_z_id,),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xaby, xa_by_id, 1,)],
                        ),
                    )
                }),
                cursor: PatternCursor {
                    path: RootedRangePath::new(
                        query.clone(),
                        RolePath::new_empty(0),
                        RolePath::new_empty(1),
                    ),
                    atom_position: 3.into(),
                    _state: std::marker::PhantomData,
                },
            },

            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        xabyz.index,
                        VertexCache {
                            index: xabyz,
                            top_down: FromIterator::from_iter([]),
                            bottom_up: FromIterator::from_iter([(
                                2.into(), // width of by
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(xaby, 2), // width of by
                                        SubLocation::new(xaby_z_id, 0),
                                    ),]),
                                )
                            )]),
                        }
                    ),
                    (
                        xaby.index,
                        VertexCache {
                            index: xaby,
                            top_down: FromIterator::from_iter([]),
                            bottom_up: FromIterator::from_iter([(
                                2.into(), // width of by
                                PositionCache::new(
                                    HashSet::from_iter([]),
                                    HashMap::from_iter([(
                                        DirectedKey::up(by, 2), // width of by
                                        SubLocation::new(xa_by_id, 1)
                                    )]),
                                )
                            )]),
                        }
                    ),
                    (
                        by.index,
                        VertexCache {
                            index: by,
                            top_down: FromIterator::from_iter([]),
                            bottom_up: FromIterator::from_iter([]),
                        }
                    ),
                ]),
            },
        }
    );
}

#[test]
fn find_ancestor3() {
    let mut graph = Hypergraph::<BaseGraphKind>::default();
    let (a, b, _w, x, y, z) = graph
        .insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('w'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])
        .into_iter()
        .next_tuple()
        .unwrap();
    // 6
    let ab = graph.insert_pattern(vec![a, b]);
    let by = graph.insert_pattern(vec![b, y]);
    let yz = graph.insert_pattern(vec![y, z]);
    let xa = graph.insert_pattern(vec![x, a]);
    // 10
    let (xab, xab_ids) = graph.insert_patterns_with_ids([
        Pattern::from(vec![x, ab]),
        Pattern::from(vec![xa, b]),
    ]);
    let x_ab_id = xab_ids[0];
    //assert_eq!(x_ab_id, 4);
    // 11
    let (xaby, xaby_ids) = graph.insert_patterns_with_ids([
        Pattern::from(vec![xab, y]),
        Pattern::from(vec![xa, by]),
    ]);
    let xab_y_id = xaby_ids[0];
    //assert_eq!(xab_y_id, 6);
    // 12
    let _xabyz = graph.insert_patterns([vec![xaby, z], vec![xab, yz]]);
    let _tracing = init_test_tracing!(&graph);
    let gr = HypergraphRef::from(graph);

    let query = vec![ab, y];
    let aby_found = gr.find_ancestor(&query).unwrap();
    assert_eq!(
        aby_found.clone(),
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        xaby.index,
                        VertexCache {
                            index: xaby,
                            top_down: FromIterator::from_iter([]),
                            bottom_up: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(xab, 2),
                                        SubLocation::new(xab_y_id, 0),
                                    ),]),
                                )
                            )]),
                        }
                    ),
                    (
                        xab.index,
                        VertexCache {
                            index: xab,
                            bottom_up: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(ab, 2),
                                        SubLocation::new(x_ab_id, 1),
                                    ),]),
                                )
                            )]),
                            top_down: FromIterator::from_iter([]),
                        }
                    ),
                    (
                        ab.index,
                        VertexCache {
                            index: ab,
                            top_down: FromIterator::from_iter([]),
                            bottom_up: FromIterator::from_iter([]),
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    root_pos: 2.into(),
                    path: RootedRolePath::new(
                        PatternLocation::new(xaby, xab_y_id),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xab, x_ab_id, 1)],
                        ),
                    )
                }),
                cursor: PatternCursor {
                    path: RootedRangePath::new(
                        query.clone(),
                        RolePath::new_empty(0),
                        RolePath::new_empty(1),
                    ),
                    atom_position: 3.into(),
                    _state: std::marker::PhantomData,
                },
            },
        }
    );
}
