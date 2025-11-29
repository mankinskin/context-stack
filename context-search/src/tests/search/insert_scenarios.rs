/// Tests that replicate graph scenarios from context-insert tests
/// to verify search algorithm correctness independently
#[cfg(test)]
use {
    crate::{
        cursor::{
            checkpointed::Checkpointed,
            PatternCursor,
        },
        search::context::AncestorSearchTraversal,
        state::{
            end::{
                postfix::PostfixEnd,
                prefix::PrefixEnd,
                range::RangeEnd,
                PathCoverage,
            },
            matched::{
                CheckpointedCursor,
                MatchResult,
            },
        },
        Find,
        Response,
        Searchable,
    },
    context_trace::trace::cache::key::directed::{
        down::{
            DownKey,
            DownPosition,
        },
        DirectedKey,
    },
    context_trace::*,
    pretty_assertions::assert_eq,
};

#[test]
fn prefix1() {
    let mut graph = Hypergraph::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_id) => [l, d],
        (heldld, heldld_id) => [h, e, ld, ld]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);
    let res = Searchable::<AncestorSearchTraversal>::search(
        vec![h, e, l, l],
        graph.into(),
    );
    assert_eq!(
        res,
        Ok(Response {
            end: MatchResult {
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            vec![h, e, l, l],
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: Default::default()
                    }
                )),
                path: PathCoverage::Prefix(PrefixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(heldld, heldld_id),
                        RolePath::new(
                            2,
                            vec![ChildLocation::new(ld, ld_id, 0)]
                        )
                    ),
                    target: DownKey {
                        index: l,
                        pos: DownPosition(1.into()),
                    },
                    root_pos: 1.into(),
                    end_pos: 3.into(),
                }),
            },
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
        })
    );
}
#[test]
fn search_pattern1_by_z() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        ab => [[a, b]],
        by => [[b, y]],
        yz => [[y, z]],
        xa => [[x, a]],
    );
    insert_patterns!(graph,
        xab => [[x, ab], [xa, b]],
    );
    insert_patterns!(graph,
        (xaby, xaby_ids) => [[xab, y], [xa, by]],
    );
    insert_patterns!(graph,
        (xabyz, xabyz_ids) => [[xaby, z], [xab, yz]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xa_by_id = xaby_ids[1]; // [xa, by] pattern
    let xaby_z_id = xabyz_ids[0]; // [xaby, z] pattern

    let query = vec![by, z];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    root_pos: 2.into(),
                    path: RootedRolePath::new(
                        PatternLocation::new(xabyz, xaby_z_id),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xaby, xa_by_id, 1)],
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
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(xaby, 2),
                                        SubLocation::new(xaby_z_id, 0),
                                    )]),
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
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(by, 2),
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
fn search_pattern1_ab_y() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        ab => [[a, b]],
        by => [[b, y]],
    );
    insert_patterns!(graph,
        yz => [[y, z]],
        xa => [[x, a]],
    );
    insert_patterns!(graph,
        (xab, xab_ids) => [[x, ab], [xa, b]],
        (xaby, xaby_ids) => [[xab, y], [xa, by]],
    );
    insert_patterns!(graph,
        _xabyz => [[xaby, z], [xab, yz]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xab_y_id = xaby_ids[0]; // [xab, y]

    // Search for [ab, y] - should find xaby as ancestor
    let query = vec![ab, y];
    let response = graph.find_ancestor(&query).unwrap();

    let xab_pat_id = xab_ids[0];
    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        ab.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: ab,
                        }
                    ),
                    (
                        xab.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(ab, 2),
                                        SubLocation::new(xab_pat_id, 1),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: xab,
                        }
                    ),
                    (
                        xaby.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(xab, 2),
                                        SubLocation::new(xab_y_id, 0),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: xaby,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(xaby, xab_y_id),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xab, xab_pat_id, 1)],
                        ),
                    ),
                    root_pos: 2.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
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
        }
    );
}

#[test]
fn search_pattern2_a_b_y() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        (yz, yz_ids) => [[y, z]],
        (xab, xab_ids) => [[x, a, b]],
    );
    insert_patterns!(graph,
        _xyz => [[x, yz]],
        _xabz => [[xab, z]],
    );
    insert_patterns!(graph,
        (xabyz, xabyz_ids) => [[xab, yz]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xab_yz_id = xabyz_ids[0]; // [xab, yz]
    let xab_pat_id = xab_ids[0];
    let yz_pat_id = yz_ids[0];

    // Search for [a, b, y] - should match in xabyz
    let query = vec![a, b, y];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        xab.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(a, 1),
                                        SubLocation::new(xab_pat_id, 1),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: xab,
                        }
                    ),
                    (
                        xabyz.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(xab, 2),
                                        SubLocation::new(xab_yz_id, 0),
                                    )]),
                                )
                            )]),
                            top_down: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(yz, 2),
                                        SubLocation::new(xab_yz_id, 1),
                                    )]),
                                )
                            )]),
                            index: xabyz,
                        }
                    ),
                    (
                        y.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    Default::default(),
                                )
                            )]),
                            index: y,
                        }
                    ),
                    (
                        yz.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                2.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(y, 2),
                                        SubLocation::new(yz_pat_id, 0),
                                    )]),
                                )
                            )]),
                            index: yz,
                        }
                    ),
                    (
                        a.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: a,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Range(RangeEnd {
                    path: RootedRangePath::new(
                        PatternLocation::new(xabyz, xab_yz_id),
                        RolePath::new(
                            0,
                            vec![ChildLocation::new(xab, xab_pat_id, 1)],
                        ),
                        RolePath::new(
                            1,
                            vec![ChildLocation::new(yz, yz_pat_id, 0)],
                        ),
                    ),
                    target: DownKey {
                        index: y,
                        pos: DownPosition(2.into()),
                    },
                    root_pos: 2.into(),
                    end_pos: 3.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_pattern2_a_b() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        yz => [[y, z]],
    );
    insert_patterns!(graph,
        (xab, xab_ids) => [[x, a, b]],
    );
    insert_patterns!(graph,
        _xyz => [[x, yz]],
        _xabz => [[xab, z]],
        _xabyz => [[xab, yz]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xab_pat_id = xab_ids[0];

    // Search for [a, b] - should find partial match in xab
    let query = vec![a, b];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        xab.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(a, 1),
                                        SubLocation::new(xab_pat_id, 1),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: xab,
                        }
                    ),
                    (
                        a.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: a,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(xab, xab_pat_id),
                        RolePath::new_empty(1),
                    ),
                    root_pos: 1.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 2.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_infix1_a_b_y() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, w, x, y, z});
    insert_patterns!(graph,
        (yz, yz_ids) => [[y, z]],
        (xxabyzw, xxabyzw_ids) => [[x, x, a, b, yz, w]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xxabyzw_pat_id = xxabyzw_ids[0];
    let yz_pat_id = yz_ids[0];

    // Search for [a, b, y] - should find infix match in xxabyzw
    let query = vec![a, b, y];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        a.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: a,
                        }
                    ),
                    (
                        yz.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(y, 1),
                                        SubLocation::new(yz_pat_id, 0),
                                    )]),
                                )
                            )]),
                            index: yz,
                        }
                    ),
                    (
                        xxabyzw.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(a, 1),
                                        SubLocation::new(xxabyzw_pat_id, 2),
                                    )]),
                                )
                            )]),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(yz, 1),
                                        SubLocation::new(xxabyzw_pat_id, 4),
                                    )]),
                                )
                            )]),
                            index: xxabyzw,
                        }
                    ),
                    (
                        y.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::default()
                            )]),
                            index: y,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Range(RangeEnd {
                    path: RootedRangePath::new(
                        PatternLocation::new(xxabyzw, xxabyzw_pat_id),
                        RolePath::new_empty(2),
                        RolePath::new(
                            4,
                            vec![ChildLocation::new(yz, yz_pat_id, 0)],
                        ),
                    ),
                    target: DownKey {
                        index: y,
                        pos: DownPosition(1.into()),
                    },
                    root_pos: 1.into(),
                    end_pos: 3.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_infix1_a_b() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, w, x, y, z});
    insert_patterns!(graph,
        yz => [[y, z]],
    );
    insert_patterns!(graph,
        (xxabyzw, xxabyzw_ids) => [[x, x, a, b, yz, w]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let xxabyzw_pat_id = xxabyzw_ids[0];

    // Search for [a, b] - appears as infix in xxabyzw
    let query = vec![a, b];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        a.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: a,
                        }
                    ),
                    (
                        b.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    Default::default(),
                                )
                            )]),
                            index: b,
                        }
                    ),
                    (
                        xxabyzw.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(a, 1),
                                        SubLocation::new(xxabyzw_pat_id, 2),
                                    )]),
                                )
                            )]),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(b, 1),
                                        SubLocation::new(xxabyzw_pat_id, 3),
                                    )]),
                                )
                            )]),
                            index: xxabyzw,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Range(RangeEnd {
                    path: RootedRangePath::new(
                        PatternLocation::new(xxabyzw, xxabyzw_pat_id),
                        RolePath::new_empty(2),
                        RolePath::new_empty(3),
                    ),
                    target: DownKey {
                        index: b,
                        pos: DownPosition(1.into()),
                    },
                    root_pos: 1.into(),
                    end_pos: 2.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 2.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_infix2_a_b_c_d() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, x, y});
    insert_patterns!(graph,
        yy => [y, y],
        xx => [x, x],
        xy => [x, y],
    );
    insert_patterns!(graph,
        (abcdx, abcdx_ids) => [a, b, c, d, x],
    );
    insert_patterns!(graph,
        yabcdx => [y, abcdx],
        abcdxx => [abcdx, x]
    );
    insert_patterns!(graph,
        xxy => [[xx, y], [x, xy]],
        _xxyyabcdxxyy => [[xx, yy, abcdxx, yy], [xxy, yabcdx, xy, y]]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let abcdx_pat_id = abcdx_ids;

    // Search for [a, b, c, d] - prefix of abcdx and abcdxx
    let query = vec![a, b, c, d];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        abcdx.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(d, 1),
                                        SubLocation::new(abcdx_pat_id, 3),
                                    )]),
                                )
                            )]),
                            index: abcdx,
                        }
                    ),
                    (
                        d.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    Default::default(),
                                )
                            )]),
                            index: d,
                        }
                    ),
                    (
                        a.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: a,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Prefix(PrefixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(abcdx, abcdx_pat_id),
                        RolePath::new_empty(3),
                    ),
                    target: DownKey {
                        index: d,
                        pos: DownPosition(1.into()),
                    },
                    root_pos: 1.into(),
                    end_pos: 4.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(3),
                        ),
                        atom_position: 4.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_prefix1_h_e_l_l() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_ids) => [l, d],
        (heldld, heldld_ids) => [h, e, ld, ld]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let ld_pat_id = ld_ids;
    let heldld_pat_id = heldld_ids;

    // Search for [h, e, l, l] - prefix match in heldld
    // heldld = [h, e, ld, ld] = [h, e, [l,d], [l,d]]
    // [h, e, l, l] matches [h, e, l...] then gets stuck
    let query = vec![h, e, l, l];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        ld.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(l, 1),
                                        SubLocation::new(ld_pat_id, 0),
                                    )]),
                                )
                            )]),
                            index: ld,
                        }
                    ),
                    (
                        heldld.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::down(ld, 1),
                                        SubLocation::new(heldld_pat_id, 2),
                                    )]),
                                )
                            )]),
                            index: heldld,
                        }
                    ),
                    (
                        h.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: h,
                        }
                    ),
                    (
                        l.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    Default::default(),
                                )
                            )]),
                            index: l,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Prefix(PrefixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(heldld, heldld_pat_id),
                        RolePath::new(
                            2,
                            vec![ChildLocation::new(ld, ld_pat_id, 0)],
                        ),
                    ),
                    target: DownKey {
                        index: l,
                        pos: DownPosition(1.into()),
                    },
                    root_pos: 1.into(),
                    end_pos: 3.into(),
                }),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_postfix1_b_c_d_d() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (ab, ab_ids) => [a, b],
        (ababcd, ababcd_ids) => [ab, ab, c, d]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let ab_pat_id = ab_ids;
    let ababcd_pat_id = ababcd_ids;

    // Search for [b, c, d, d] - postfix match in ababcd
    // ababcd = [ab, ab, c, d] = [[a,b], [a,b], c, d]
    // [b, c, d, d] matches [...b, c, d...] starting from position 1
    let query = vec![b, c, d, d];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([
                    (
                        ababcd.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(ab, 1),
                                        SubLocation::new(ababcd_pat_id, 1),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: ababcd,
                        }
                    ),
                    (
                        b.index,
                        VertexCache {
                            bottom_up: Default::default(),
                            top_down: Default::default(),
                            index: b,
                        }
                    ),
                    (
                        ab.index,
                        VertexCache {
                            bottom_up: FromIterator::from_iter([(
                                1.into(),
                                PositionCache::new(
                                    Default::default(),
                                    HashMap::from_iter([(
                                        DirectedKey::up(b, 1),
                                        SubLocation::new(ab_pat_id, 1),
                                    )]),
                                )
                            )]),
                            top_down: Default::default(),
                            index: ab,
                        }
                    ),
                ]),
            },
            end: MatchResult {
                path: PathCoverage::Postfix(PostfixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(ababcd, ababcd_pat_id),
                        RolePath::new(
                            1,
                            vec![ChildLocation::new(ab, ab_pat_id, 1)],
                        ),
                    ),
                    root_pos: 1.into(),
                }),
                cursor: CheckpointedCursor::HasCandidate(Checkpointed {
                    checkpoint: PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: std::marker::PhantomData,
                    },
                    candidate: PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(3),
                        ),
                        atom_position: 4.into(),
                        _state: std::marker::PhantomData,
                    },
                    _state: std::marker::PhantomData,
                }),
            },
        }
    );
}

#[test]
fn search_complete_token_b_c() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (bc, bc_ids) => [b, c],
    );
    insert_patterns!(graph,
        _abc => [a, bc]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let bc_pat_id = bc_ids;

    // Search for [b, c] - should find complete token bc
    let query = vec![b, c];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([(
                    b.index,
                    VertexCache {
                        bottom_up: Default::default(),
                        top_down: Default::default(),
                        index: b,
                    }
                ),]),
            },
            end: MatchResult {
                path: PathCoverage::EntireRoot(RootedRangePath::new(
                    PatternLocation::new(bc, bc_pat_id),
                    RolePath::new_empty(0),
                    RolePath::new_empty(1),
                ),),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            query.clone(),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 2.into(),
                        _state: std::marker::PhantomData,
                    }
                )),
            },
        }
    );
}

#[test]
fn search_complete_token_a_bc() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (bc, bc_ids) => [b, c],
        (abc, abc_ids) => [a, bc]
    );
    let _tracing = context_trace::init_test_tracing!(&graph);

    let abc_pat_id = abc_ids;

    // Search for [a, bc] - should find complete token abc
    let query = vec![a, bc];
    let response = graph.find_ancestor(&query).unwrap();

    assert_eq!(
        response,
        Response {
            cache: TraceCache {
                entries: HashMap::from_iter([(
                    a.index,
                    VertexCache {
                        bottom_up: Default::default(),
                        top_down: Default::default(),
                        index: a,
                    }
                ),]),
            },
            end: MatchResult {
                path: PathCoverage::EntireRoot(RootedRangePath::new(
                    PatternLocation::new(abc, abc_pat_id),
                    RolePath::new_empty(0),
                    RolePath::new_empty(1),
                ),),
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
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
        }
    );
}
