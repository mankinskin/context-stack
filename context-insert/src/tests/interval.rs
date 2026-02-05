use std::{
    collections::{
        BTreeMap,
        HashMap,
        HashSet,
        VecDeque,
    },
    iter::FromIterator,
};

use linked_hash_set::LinkedHashSet;
use pretty_assertions::assert_eq;

use crate::{
    join::context::node::merge::{
        PartitionRange,
        RequiredPartitions,
    },
    *,
};
use context_search::{
    tests::search::trace_cache::CdefghiTraceCase,
    *,
};
use context_trace::tests::{
    env::{
        Env1,
        Env2,
    },
    test_case::TestEnv,
};
fn build_split_cache1(env: &Env1) -> SplitCache {
    let Env1 {
        def,
        d_ef_id,
        c_def_id,
        cd_ef_id,
        cdef,
        abcdef,
        abcd_ef_id,
        ab_cdef_id,
        abc_def_id,
        ef,
        e_f_id,
        ..
    } = env;
    build_split_cache!(
        RootMode::Prefix,
        ef => {
            1 => {
                top: [abcdef: 5, def: 2, cdef: 3],
                splits: [e_f_id => (1, None)]
            }
        },
        def => {
            2 => {
                top: [abcdef: 5, cdef: 3],
                splits: [d_ef_id => (1, Some(nz!(1)))]
            }
        },
        cdef => {
            3 => {
                top: [abcdef: 5],
                splits: [
                    c_def_id => (1, Some(nz!(2))),
                    cd_ef_id => (1, Some(nz!(1)))
                ]
            }
        },
        abcdef => {
            5 => {
                top: [],
                splits: [
                    abcd_ef_id => (1, Some(nz!(1))),
                    abc_def_id => (1, Some(nz!(2))),
                    ab_cdef_id => (1, Some(nz!(3))),
                ]
            }
        }
    )
}
#[test]
fn test_split_cache1() {
    let env @ Env1 {
        def,
        d_ef_id,
        c_def_id,
        cd_ef_id,
        cdef,
        abcdef,
        abcd_ef_id,
        ab_cdef_id,
        abc_def_id,
        ef,
        e_f_id,
        ..
    } = &*Env1::get_mut();
    let _tracing = context_trace::init_test_tracing!(&env.graph);
    assert_eq!(
        build_split_cache1(env),
        SplitCache {
            root_mode: RootMode::Prefix,
            entries: HashMap::from_iter([
                (
                    ef.index,
                    SplitVertexCache {
                        positions: BTreeMap::from_iter([(
                            nz!(1),
                            SplitPositionCache {
                                top: HashSet::from_iter([
                                    PosKey {
                                        index: *abcdef,
                                        pos: nz!(5),
                                    },
                                    PosKey {
                                        index: *def,
                                        pos: nz!(2),
                                    },
                                    PosKey {
                                        index: *cdef,
                                        pos: nz!(3),
                                    },
                                ]),
                                pattern_splits: HashMap::from_iter([(
                                    *e_f_id,
                                    TokenTracePos {
                                        inner_offset: None,
                                        sub_index: 1,
                                    }
                                )])
                            }
                        )])
                    }
                ),
                (
                    def.index,
                    SplitVertexCache {
                        positions: BTreeMap::from_iter([(
                            nz!(2),
                            SplitPositionCache {
                                top: HashSet::from_iter([
                                    PosKey {
                                        index: *abcdef,
                                        pos: nz!(5),
                                    },
                                    PosKey {
                                        index: *cdef,
                                        pos: nz!(3),
                                    },
                                ]),
                                pattern_splits: HashMap::from_iter([(
                                    *d_ef_id,
                                    TokenTracePos {
                                        inner_offset: Some(nz!(1)),
                                        sub_index: 1,
                                    }
                                )])
                            }
                        )])
                    }
                ),
                (
                    cdef.index,
                    SplitVertexCache {
                        positions: BTreeMap::from_iter([(
                            nz!(3),
                            SplitPositionCache {
                                top: HashSet::from_iter([PosKey {
                                    index: *abcdef,
                                    pos: nz!(5),
                                },]),
                                pattern_splits: HashMap::from_iter([
                                    (
                                        *c_def_id,
                                        TokenTracePos {
                                            inner_offset: Some(nz!(2)),
                                            sub_index: 1,
                                        },
                                    ),
                                    (
                                        *cd_ef_id,
                                        TokenTracePos {
                                            inner_offset: Some(nz!(1)),
                                            sub_index: 1,
                                        },
                                    )
                                ])
                            }
                        )])
                    }
                ),
                (
                    abcdef.index,
                    SplitVertexCache {
                        positions: BTreeMap::from_iter([(
                            nz!(5),
                            SplitPositionCache {
                                top: HashSet::from_iter([]),
                                pattern_splits: HashMap::from_iter([
                                    (
                                        *abcd_ef_id,
                                        TokenTracePos {
                                            inner_offset: Some(nz!(1)),
                                            sub_index: 1,
                                        }
                                    ),
                                    (
                                        *abc_def_id,
                                        TokenTracePos {
                                            inner_offset: Some(nz!(2)),
                                            sub_index: 1,
                                        }
                                    ),
                                    (
                                        *ab_cdef_id,
                                        TokenTracePos {
                                            inner_offset: Some(nz!(3)),
                                            sub_index: 1,
                                        }
                                    ),
                                ])
                            }
                        )])
                    }
                ),
            ])
        }
    );
}

#[test]
fn interval_graph1() {
    let env = &mut *Env1::get_mut();
    let _tracing = context_trace::init_test_tracing!(&env.graph);
    let graph = &mut env.graph;
    let Env1 {
        a,
        d,
        e,
        bc,
        abcdef,
        ef,
        ..
    } = env;
    let query = vec![*a, *bc, *d, *e];
    let res = graph.find_ancestor(query).unwrap();
    assert!(res.query_exhausted());
    let init = InitInterval::from(res);
    let interval = IntervalGraph::from((&*graph, init));

    // Build expected required partitions
    // For prefix mode with a single target position, we get target_range 0..=0
    // (one partition from start up to the single offset)
    let mut expected_required = RequiredPartitions::new();
    expected_required.add(PartitionRange::from(0..=0));

    assert_eq!(
        interval.clone(),
        IntervalGraph {
            root: *abcdef,
            states: SplitStates {
                leaves: LinkedHashSet::from_iter([PosKey::new(*ef, 1)]).into(),
                queue: VecDeque::default(),
            },
            cache: build_split_cache1(env),
            target_range: PartitionRange::from(0..=0),
            required: expected_required,
        }
    );
}

#[test]
fn interval_graph2() {
    // Use test environment and trace cache test case from context-search
    let trace_test = CdefghiTraceCase::default();
    let _tracing = context_trace::init_test_tracing!(&trace_test.env.graph);

    // Build InitInterval directly from test case expected values (no search needed)
    let init = InitInterval {
        root: trace_test.expected_root,
        cache: trace_test.expected_cache.clone(),
        end_bound: trace_test.expected_end_bound.into(),
    };

    // Extract tokens and pattern IDs from environment
    let Env2 {
        cd,
        hi,
        cdefg,
        efghi,
        cdefghi,
        c_d_id,
        h_i_id,
        cd_efg_id,
        efg_hi_id,
        cdefghi_ids,
        ..
    } = &trace_test.env;

    // Dereference for use in macros and assertions
    let (cd, hi, cdefg, efghi, cdefghi) = (*cd, *hi, *cdefg, *efghi, *cdefghi);
    let (c_d_id, h_i_id, cd_efg_id, efg_hi_id) =
        (*c_d_id, *h_i_id, *cd_efg_id, *efg_hi_id);

    let cdefg_hi_id = cdefghi_ids[0];
    let cd_efghi_id = cdefghi_ids[1];

    // With interior mutability, we only need &graph for IntervalGraph creation
    let interval = IntervalGraph::from((&*trace_test.env.graph, init));

    // Check root and states
    assert_eq!(interval.root, cdefghi);
    assert_eq!(
        interval.states,
        SplitStates {
            leaves: LinkedHashSet::from_iter([
                PosKey::new(cd, 1),
                PosKey::new(hi, 1)
            ])
            .into(),
            queue: VecDeque::default(),
        }
    );

    // Build expected split cache
    let expected_cache = build_split_cache!(
        RootMode::Infix,
        cd => {
            1 => {
                top: [cdefg: 1, cdefghi: 1],
                splits: [c_d_id => (1, None)]
            }
        },
        hi => {
            1 => {
                top: [efghi: 4, cdefghi: 6],
                splits: [h_i_id => (1, None)]
            }
        },
        cdefg => {
            1 => {
                top: [cdefghi: 1],
                splits: [cd_efg_id => (0, Some(nz!(1)))]
            }
        },
        efghi => {
            4 => {
                top: [cdefghi: 6],
                splits: [efg_hi_id => (1, Some(nz!(1)))]
            }
        },
        cdefghi => {
            1 => {
                top: [],
                splits: [
                    cd_efghi_id => (0, Some(nz!(1))),
                    cdefg_hi_id => (0, Some(nz!(1))),
                ]
            },
            2 => {
                top: [],
                splits: [
                    cdefg_hi_id => (0, Some(nz!(2))),
                    cd_efghi_id => (1, None),
                ]
            },
            5 => {
                top: [],
                splits: [
                    cdefg_hi_id => (1, None),
                    cd_efghi_id => (1, Some(nz!(3))),
                ]
            },
            6 => {
                top: [],
                splits: [
                    cdefg_hi_id => (1, Some(nz!(1))),
                    cd_efghi_id => (1, Some(nz!(4))),
                ]
            },
        },
    );

    // Check cache root mode
    assert_eq!(interval.cache.root_mode, expected_cache.root_mode);

    // Verify number of entries
    assert_eq!(
        interval.cache.entries.len(),
        expected_cache.entries.len(),
        "Expected 5 split cache entries: cd, hi, cdefg, efghi, cdefghi"
    );

    // Check each entry individually
    assert_eq!(
        interval.cache.entries.get(&cd.index).unwrap(),
        expected_cache.entries.get(&cd.index).unwrap(),
        "cd entry mismatch"
    );
    assert_eq!(
        interval.cache.entries.get(&hi.index).unwrap(),
        expected_cache.entries.get(&hi.index).unwrap(),
        "hi entry mismatch"
    );
    assert_eq!(
        interval.cache.entries.get(&cdefg.index).unwrap(),
        expected_cache.entries.get(&cdefg.index).unwrap(),
        "cdefg entry mismatch"
    );
    assert_eq!(
        interval.cache.entries.get(&efghi.index).unwrap(),
        expected_cache.entries.get(&efghi.index).unwrap(),
        "efghi entry mismatch"
    );
    assert_eq!(
        interval.cache.entries.get(&cdefghi.index).unwrap(),
        expected_cache.entries.get(&cdefghi.index).unwrap(),
        "cdefghi entry mismatch"
    );
}

/// Test that perfect splits (inner_offset = None) don't generate wrapper offsets.
///
/// When a split position aligns exactly with child token boundaries (perfect split),
/// no wrapper offset should be added. Wrapper offsets are only needed when the split
/// cuts through the middle of a child token (unperfect split with inner_offset).
///
/// This test creates a SplitVertexCache with:
/// - Position 3: perfect split (inner_offset = None) at child boundary
/// - Position 5: unperfect split (inner_offset = Some) inside a child
///
/// Then verifies that:
/// - `is_split_perfect_at_index` correctly identifies perfect vs unperfect splits
/// - Wrapper offsets are only generated for unperfect splits
#[test]
fn test_perfect_split_no_wrapper_offset() {
    use crate::{
        SplitVertexCache,
        split::{
            TokenTracePos,
            cache::position::SplitPositionCache,
        },
    };
    use context_trace::graph::vertex::pattern::id::PatternId;

    // Create a pattern ID for testing
    let pattern_id = PatternId::default();

    // Create a SplitVertexCache with a perfect split at position 3
    // Perfect split means inner_offset = None (split aligns with child boundary)
    let cache_with_perfect_split = SplitVertexCache {
        positions: BTreeMap::from_iter([(
            nz!(3),
            SplitPositionCache {
                top: Default::default(),
                pattern_splits: context_trace::HashMap::from_iter([(
                    pattern_id,
                    TokenTracePos {
                        inner_offset: None, // Perfect split - no inner offset
                        sub_index: 3,
                    },
                )]),
            },
        )]),
    };

    // Verify that the split at index 0 (position 3) is detected as perfect
    assert!(
        cache_with_perfect_split.is_split_perfect_at_index(0),
        "Split with inner_offset=None should be detected as perfect"
    );

    // Create a SplitVertexCache with an unperfect split at position 5
    // Unperfect split means inner_offset = Some (split inside a child token)
    let cache_with_unperfect_split = SplitVertexCache {
        positions: BTreeMap::from_iter([(
            nz!(5),
            SplitPositionCache {
                top: Default::default(),
                pattern_splits: context_trace::HashMap::from_iter([(
                    pattern_id,
                    TokenTracePos {
                        inner_offset: Some(nz!(2)), // Unperfect - 2 chars into child
                        sub_index: 1,
                    },
                )]),
            },
        )]),
    };

    // Verify that the split at index 0 (position 5) is detected as unperfect
    assert!(
        !cache_with_unperfect_split.is_split_perfect_at_index(0),
        "Split with inner_offset=Some should be detected as unperfect"
    );

    // Test with mixed splits: one perfect, one unperfect
    let cache_with_mixed_splits = SplitVertexCache {
        positions: BTreeMap::from_iter([
            (
                nz!(3),
                SplitPositionCache {
                    top: Default::default(),
                    pattern_splits: context_trace::HashMap::from_iter([(
                        pattern_id,
                        TokenTracePos {
                            inner_offset: None, // Perfect
                            sub_index: 3,
                        },
                    )]),
                },
            ),
            (
                nz!(5),
                SplitPositionCache {
                    top: Default::default(),
                    pattern_splits: context_trace::HashMap::from_iter([(
                        pattern_id,
                        TokenTracePos {
                            inner_offset: Some(nz!(2)), // Unperfect
                            sub_index: 1,
                        },
                    )]),
                },
            ),
        ]),
    };

    // Index 0 corresponds to position 3 (perfect)
    assert!(
        cache_with_mixed_splits.is_split_perfect_at_index(0),
        "First split (position 3) should be perfect"
    );
    // Index 1 corresponds to position 5 (unperfect)
    assert!(
        !cache_with_mixed_splits.is_split_perfect_at_index(1),
        "Second split (position 5) should be unperfect"
    );
}

/// Test that RequiredPartitions only includes wrapper when there's an unperfect split.
///
/// For prefix mode:
/// - Target range is always required
/// - Wrapper range is only required if the boundary split is unperfect
#[test]
fn test_required_partitions_perfect_vs_unperfect() {
    use crate::{
        SplitVertexCache,
        split::{
            TokenTracePos,
            cache::position::SplitPositionCache,
        },
    };
    use context_trace::graph::vertex::pattern::id::PatternId;

    let pattern_id = PatternId::default();

    // Case 1: Perfect split - wrapper should NOT be required
    let cache_perfect = SplitVertexCache {
        positions: BTreeMap::from_iter([(
            nz!(3),
            SplitPositionCache {
                top: Default::default(),
                pattern_splits: context_trace::HashMap::from_iter([(
                    pattern_id,
                    TokenTracePos {
                        inner_offset: None, // Perfect split
                        sub_index: 3,
                    },
                )]),
            },
        )]),
    };

    let _target_range = PartitionRange::from(0..=0);
    let _wrapper_range = PartitionRange::from(0..=1); // Would include one more partition

    // With perfect split, only target should be required (no wrapper)
    // The boundary is at target_end (0), which corresponds to split index 0
    // Since that split is perfect, wrapper shouldn't be added
    assert!(
        cache_perfect.is_split_perfect_at_index(0),
        "Boundary split should be perfect"
    );

    // Case 2: Unperfect split - wrapper SHOULD be required
    let cache_unperfect = SplitVertexCache {
        positions: BTreeMap::from_iter([(
            nz!(3),
            SplitPositionCache {
                top: Default::default(),
                pattern_splits: context_trace::HashMap::from_iter([(
                    pattern_id,
                    TokenTracePos {
                        inner_offset: Some(nz!(1)), // Unperfect split
                        sub_index: 2,
                    },
                )]),
            },
        )]),
    };

    assert!(
        !cache_unperfect.is_split_perfect_at_index(0),
        "Boundary split should be unperfect"
    );
}
