#[cfg(test)]
use {
    crate::{
        cursor::{
            checkpointed::Checkpointed,
            PatternCursor,
        },
        search::Find,
        state::{
            end::{
                postfix::PostfixEnd,
                PathCoverage,
            },
            matched::{
                CheckpointedCursor,
                MatchResult,
            },
            response::Response,
        },
        tests::search::event_helpers::*,
    },
    context_trace::{
        *,
        graph::visualization::Transition,
    },
    itertools::*,
    pretty_assertions::{
        assert_eq,
        assert_matches,
    },
};

// Test: Single token bc should return error (not a pattern)
#[test]
fn find_ancestor1_single_token() {
    let Env1 { graph, bc, .. } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

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
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let Env1 { ab, bcd, abab, .. } = &*Env1::get();
    let query = vec![Token::new(b, 1), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_eq!(response.query_exhausted(), true);
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *bc
    );

    // Exact expected event sequence
    assert_events(&response.events, &[
        start(b),
        explore(b, &[bc, ab, bcd, abab]),
        up(b, bc),
        down(bc, c, false),
        matched(c, 2),
        root_match(bc),
        done_ok(bc),
    ]);
}

// Test: Pattern [a, bc] should match token abc (Complete)
#[test]
fn find_ancestor1_a_bc() {
    let Env1 {
        graph, a, b, c, ab, bc, abc, aba, abcd,
        abab, ababcd, abcdef, ababab, ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query = vec![Token::new(a, 1), Token::new(bc, 2)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);

    // Exact expected event sequence (18 events)
    assert_events(&response.events, &[
        start(a),                                                                  // 0
        explore(a, &[ab, abc, aba, abcd]),                                         // 1
        up(a, ab),                                                                 // 2
        down(ab, b, false),                                                        // 3
        explore(ab, &[aba, abcd, abc]),                                            // 4
        down(ab, b, false),                                                        // 5
        matched(b, 2),                                                             // 6
        root_match(ab),                                                            // 7
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]), // 8
        up(ab, aba),                                                               // 9
        down(aba, a, false),                                                       // 10
        mismatched(a, 3, c, a),                                                    // 11
        skip(aba, 8, true),                                                        // 12
        up(a, abc),                                                                // 13
        down(abc, c, false),                                                       // 14
        matched(c, 3),                                                             // 15
        root_match(abc),                                                           // 16
        done_ok(abc),                                                              // 17
    ]);
}

// Test: Pattern [ab, c] should match token abc (Complete)
#[test]
fn find_ancestor1_ab_c() {
    let Env1 {
        graph, a, ab, c, abc, aba,
        abab, ababcd, abcdef, ababab, ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query = vec![Token::new(ab, 2), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);

    // Exact expected event sequence (11 events)
    assert_events(&response.events, &[
        start(ab),                                                                         // 0
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]),  // 1
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
}

// Test: Pattern [a, bc, d] should match token abcd (Complete)
#[test]
fn find_ancestor1_a_bc_d() {
    let Env1 {
        graph,
        a, b, c, bc, d,
        ab, abc, abcd, aba,
        abab, ababcd, abcdef, ababab, ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query = vec![Token::new(a, 1), Token::new(bc, 2), Token::new(d, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abcd
    );
    assert_eq!(response.query_exhausted(), true);

    // Exact expected event sequence (23 events)
    assert_events(&response.events, &[
        start(a),                                                                          // 0
        explore(a, &[ab, abc, aba, abcd]),                                                 // 1
        up(a, ab),                                                                         // 2
        down(ab, b, false),                                                                // 3
        explore(ab, &[aba, abcd, abc]),                                                    // 4
        down(ab, b, false),                                                                // 5
        matched(b, 2),                                                                     // 6
        root_match(ab),                                                                    // 7
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]),  // 8
        up(ab, aba),                                                                       // 9
        down(aba, a, false),                                                               // 10
        mismatched(a, 3, c, a),                                                            // 11
        skip(aba, 8, true),                                                                // 12
        up(a, abc),                                                                        // 13
        down(abc, c, false),                                                               // 14
        matched(c, 3),                                                                     // 15
        root_match(abc),                                                                   // 16
        explore(abc, &[abcd, abcdef]),                                                     // 17
        up(abc, abcd),                                                                     // 18
        down(abcd, d, false),                                                              // 19
        matched(d, 4),                                                                     // 20
        root_match(abcd),                                                                  // 21
        done_ok(abcd),                                                                     // 22
    ]);
}

// Test: Pattern [a, b, c] should match token abc (Complete)
#[test]
fn find_ancestor1_a_b_c() {
    let Env1 {
        graph,
        a, b, c,
        ab, abc, aba, abcd,
        abab, ababcd, abcdef, ababab, ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query = vec![Token::new(a, 1), Token::new(b, 1), Token::new(c, 1)];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), true);

    // Exact expected event sequence (16 events)
    assert_events(&response.events, &[
        start(a),                                                                          // 0
        explore(a, &[ab, abc, aba, abcd]),                                                 // 1
        up(a, ab),                                                                         // 2
        down(ab, b, false),                                                                // 3
        matched(b, 2),                                                                     // 4
        root_match(ab),                                                                    // 5
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]),  // 6
        up(ab, aba),                                                                       // 7
        down(aba, a, false),                                                               // 8
        mismatched(a, 3, c, a),                                                            // 9
        skip(aba, 8, true),                                                                // 10
        up(a, abc),                                                                        // 11
        down(abc, c, false),                                                               // 12
        matched(c, 3),                                                                     // 13
        root_match(abc),                                                                   // 14
        done_ok(abc),                                                                      // 15
    ]);
}
// Test: Pattern [a, b, c, c] should partially match token abc - only first 3 tokens match
#[test]
fn find_ancestor1_a_b_c_c() {
    let Env1 {
        graph,
        a, b, c, d,
        ab, abc, abcd, aba,
        abab, ababcd, abcdef, abcdefghi, ababab, ababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query = vec![
        Token::new(a, 1),
        Token::new(b, 1),
        Token::new(c, 1),
        Token::new(c, 1),
    ];
    let response = graph.find_ancestor(&query).unwrap();
    assert_matches!(
        response.end.path,
        PathCoverage::EntireRoot(ref path)
            if path.path_root().pattern_location().parent == *abc
    );
    assert_eq!(response.query_exhausted(), false);

    // Exact expected event sequence (27 events)
    assert_events(&response.events, &[
        start(a),                                                                          // 0
        explore(a, &[ab, abc, aba, abcd]),                                                 // 1
        up(a, ab),                                                                         // 2
        down(ab, b, false),                                                                // 3
        matched(b, 2),                                                                     // 4
        root_match(ab),                                                                    // 5
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]),  // 6
        up(ab, aba),                                                                       // 7
        down(aba, a, false),                                                               // 8
        mismatched(a, 3, c, a),                                                            // 9
        skip(aba, 8, true),                                                                // 10
        up(a, abc),                                                                        // 11
        down(abc, c, false),                                                               // 12
        matched(c, 3),                                                                     // 13
        root_match(abc),                                                                   // 14
        explore(abc, &[abcd, abcdef]),                                                     // 15
        up(abc, abcd),                                                                     // 16
        down(abcd, d, false),                                                              // 17
        mismatched(d, 4, c, d),                                                            // 18
        skip(abcd, 1, true),                                                               // 19
        up(a, abcdef),                                                                     // 20
        down(abcdef, d, false),                                                            // 21
        explore(abcdef, &[]),                                                               // 22
        down(abcdef, d, false),                                                            // 23
        mismatched(d, 4, c, d),                                                            // 24
        skip(abcdef, 0, false),                                                            // 25
        done_ok(abc),                                                                      // 26
    ]);
}

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
        ab,
        abc,
        aba,
        abcd,
        ef,
        efgh,
        abcdef,
        abab,
        ababab,
        ababcd,
        ababababcd,
        ababcdefghi,
        ababababcdefghi,
        ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

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

    // Exact expected event sequence (64 events)
    assert_events(&response.events, &[
        start(a),                                                                          // 0
        explore(a, &[ab, abc, aba, abcd]),                                                 // 1
        up(a, ab),                                                                         // 2
        down(ab, b, false),                                                                // 3
        matched(b, 2),                                                                     // 4
        root_match(ab),                                                                    // 5
        explore(ab, &[aba, abc, abab, abab, ababcd, abcdef, ababab, ababab, ababcdefghi]), // 6
        up(ab, aba),                                                                       // 7
        down(aba, a, false),                                                               // 8
        matched(a, 3),                                                                     // 9
        root_match(aba),                                                                   // 10
        explore(aba, &[abab, ababcd]),                                                     // 11
        up(aba, abab),                                                                     // 12
        down(abab, b, false),                                                              // 13
        matched(b, 4),                                                                     // 14
        root_match(abab),                                                                  // 15
        explore(abab, &[ababab, ababcd, ababab, ababababcd, ababababcdefghi]),              // 16
        up(abab, ababab),                                                                  // 17
        down(ababab, a, false),                                                            // 18
        explore(ababab, &[ababab, ababcd, ababababcdefghi, ababababcd]),                    // 19
        up(a, ababab),                                                                     // 20
        explore(ababab, &[ababcd, ababababcdefghi, ababababcd, ababababcd, ababababcdefghi]), // 21
        up(a, ababcd),                                                                     // 22
        down(ababcd, c, false),                                                            // 23
        explore(ababcd, &[ababababcd, ababababcd, ababababcdefghi, ababababcdefghi]),        // 24
        down(ababab, a, false),                                                            // 25
        matched(a, 5),                                                                     // 26
        root_match(ababab),                                                                // 27
        down_at(ababab, b, true, 1),                                                       // 28
        matched(b, 6),                                                                     // 29
        explore(ababab, &[ababababcd, ababababcdefghi]),                                    // 30
        up(ababab, ababababcd),                                                            // 31
        down(ababababcd, abc, false),                                                      // 32
        down(ababababcd, a, false),                                                        // 33
        explore(ababababcd, &[ababababcdefghi]),                                            // 34
        down(ababababcd, a, false),                                                        // 35
        matched(a, 7),                                                                     // 36
        root_match(ababababcd),                                                            // 37
        down_at(ababababcd, b, true, 1),                                                   // 38
        matched(b, 8),                                                                     // 39
        down_at(ababababcd, c, true, 1),                                                   // 40
        matched(c, 9),                                                                     // 41
        down_at(ababababcd, d, true, 1),                                                   // 42
        matched(d, 10),                                                                    // 43
        explore(ababababcd, &[ababababcdefghi]),                                            // 44
        up(ababababcd, ababababcdefghi),                                                   // 45
        down(ababababcdefghi, efgh, false),                                                // 46
        down(ababababcdefghi, ef, false),                                                  // 47
        explore(ababababcdefghi, &[]),                                                     // 48
        down(ababababcdefghi, ef, false),                                                  // 49
        down(ababababcdefghi, e, false),                                                   // 50
        down(ababababcdefghi, e, false),                                                   // 51
        down(ababababcdefghi, e, false),                                                   // 52
        matched(e, 11),                                                                    // 53
        root_match(ababababcdefghi),                                                       // 54
        down_at(ababababcdefghi, f, true, 1),                                              // 55
        matched(f, 12),                                                                    // 56
        down_at(ababababcdefghi, g, true, 1),                                              // 57
        matched(g, 13),                                                                    // 58
        down_at(ababababcdefghi, h, true, 1),                                              // 59
        matched(h, 14),                                                                    // 60
        down_at(ababababcdefghi, i, true, 1),                                              // 61
        matched(i, 15),                                                                    // 62
        done_ok(ababababcdefghi),                                                          // 63
    ]);
}

#[test]
fn find_ancestor2() {
    use context_trace::*;

    let graph = Hypergraph::<BaseGraphKind>::default();
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
    graph.emit_graph_snapshot();
    let xa_by_id = xaby_ids[0];
    let xaby_z_id = xabyz_ids[0];
    //assert_eq!(xaby_z_id, 8);
    let graph = HypergraphRef::from(graph);
    let query = vec![by, z];
    let byz_found = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        byz_found,
        Response {
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    entry_pos: 2.into(),
                    path: RootedRolePath::new(
                        PatternLocation::new(xabyz, xaby_z_id,),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xaby, xa_by_id, 1,)],
                        ),
                    )
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::<
                    PatternCursor,
                >::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
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
            events: vec![],
        }
    );

    // Exact expected event sequence (9 events)
    use crate::tests::search::event_helpers::*;
    assert_events(&byz_found.events, &[
        start(&by),                                      // 0
        explore(&by, &[&xaby]),                           // 1
        up(&by, &xaby),                                   // 2
        explore(&xaby, &[&xabyz]),                        // 3
        up(&by, &xabyz),                                  // 4
        down(&xabyz, &z, false),                          // 5
        matched(&z, 3),                                   // 6
        root_match(&xabyz),                               // 7
        done_ok(&xabyz),                                  // 8
    ]);
}

#[test]
fn find_ancestor3() {
    let graph = Hypergraph::<BaseGraphKind>::default();
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
    graph.emit_graph_snapshot();
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
                    entry_pos: 2.into(),
                    path: RootedRolePath::new(
                        PatternLocation::new(xaby, xab_y_id),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xab, x_ab_id, 1)],
                        ),
                    )
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::<
                    PatternCursor,
                >::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
            events: vec![],
        }
    );

    // Exact expected event sequence (9 events)
    use crate::tests::search::event_helpers::*;
    assert_events(&aby_found.events, &[
        start(&ab),                                       // 0
        explore(&ab, &[&xab]),                             // 1
        up(&ab, &xab),                                     // 2
        explore(&xab, &[&xaby, &_xabyz]),                  // 3
        up(&ab, &xaby),                                    // 4
        down(&xaby, &y, false),                            // 5
        matched(&y, 3),                                    // 6
        root_match(&xaby),                                 // 7
        done_ok(&xaby),                                    // 8
    ]);
}
