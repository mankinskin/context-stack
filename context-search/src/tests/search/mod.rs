pub mod ancestor;
pub mod consecutive;
pub mod insert_scenarios;
pub mod parent;
pub mod trace_cache;

#[cfg(test)]
use {
    crate::search::Find,
    crate::{
        cursor::{
            checkpointed::Checkpointed,
            PatternCursor,
        },
        state::end::{
            range::RangeEnd,
            PathCoverage,
        },
        state::matched::MatchResult,
    },
    context_trace::tests::env::Env1,

    context_trace::*,
    pretty_assertions::assert_eq,

    std::iter::FromIterator,
};

#[cfg(test)]
fn assert_cache_entry(
    actual_cache: &TraceCache,
    expected_cache: &TraceCache,
    key: Token,
    label: &str,
) {
    if !actual_cache.entries.contains_key(&key.index) {
        let actual_keys: Vec<_> = actual_cache.entries.keys().collect();
        panic!(
            "Cache entry missing for {} (token {})\n\
            Expected cache to contain key: {:?}\n\
            Actual cache contains {} entries with keys: {:?}\n\
            Full actual cache:\n{:#?}",
            label,
            key,
            key.index,
            actual_cache.entries.len(),
            actual_keys,
            actual_cache
        );
    }

    assert_eq!(
        actual_cache.entries[&key.index], expected_cache.entries[&key.index],
        "Cache entry mismatch for {} (token {})",
        label, key
    );
}
#[macro_export]
macro_rules! assert_indices {
    ($graph:ident, $($name:ident),*) => {
        $(
        let $name = $graph
            .find_ancestor(stringify!($name).chars())
            .unwrap()
            .expect_complete(stringify!($name))
            .root_parent();
        )*
    };
}
#[test]
fn find_sequence() {
    let Env1 {
        graph,
        abc,
        ababababcdefghi,
        a,
        ..
    } = &*Env1::get_expected();
    assert_eq!(
        graph.find_ancestor("a".chars()),
        Err(ErrorReason::SingleIndex(Box::new(IndexWithPath {
            index: *a,
            path: vec![*a].into(),
        }))),
    );
    let query = graph.graph().expect_atom_children("abc".chars());
    let abc_found = graph.find_ancestor(&query).unwrap();
    assert!(abc_found.query_exhausted(), "Query should be exhausted");
    match &abc_found.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(path.root_parent(), *abc, "Should match abc root");
        },
        _ => panic!("Expected EntireRoot path"),
    }
    let query = graph
        .graph()
        .expect_atom_children("ababababcdefghi".chars());
    let ababababcdefghi_found = graph.find_ancestor(&query).unwrap();
    assert!(
        ababababcdefghi_found.query_exhausted(),
        "Query should be exhausted"
    );
    match &ababababcdefghi_found.end.path {
        PathCoverage::EntireRoot(ref path) => {
            assert_eq!(
                path.root_parent(),
                *ababababcdefghi,
                "Should match ababababcdefghi root"
            );
        },
        _ => panic!("Expected EntireRoot path"),
    }
}
#[test]
fn find_pattern1() {
    let mut base_graph =
        context_trace::graph::Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(base_graph, {a, b, x, y, z});
    // index 6
    insert_patterns!(base_graph,
        (yz, y_z_id) => [y, z],
        (xab, x_a_b_id) => [x, a, b],
    );
    insert_patterns!(base_graph,
        _xyz => [x, yz],
        _xabz => [xab, z],
    );
    insert_patterns!(base_graph,
        (xabyz, xab_yz_id) => [xab, yz]
    );

    let _tracing = context_trace::init_test_tracing!(&base_graph);
    let graph_ref = HypergraphRef::from(base_graph);

    let query = vec![a, b, y, x];
    let aby_found = graph_ref
        .find_ancestor(query.clone())
        .expect("Search failed");

    let expected_cache = build_trace_cache!(
        xab => (
            BU { 1 => a -> (x_a_b_id, 1) },
            TD {},
        ),
        xabyz => (
            BU { 2 => xab -> (xab_yz_id, 0) },
            TD { 2 => yz -> (xab_yz_id, 1) },
        ),
        yz => (
            BU {},
            TD { 2 => y -> (y_z_id, 0) },
        ),
    );

    // Assert cache entries with better error messages
    assert_cache_entry(&aby_found.cache, &expected_cache, xab, "xab");
    assert_cache_entry(&aby_found.cache, &expected_cache, xabyz, "xabyz");
    assert_cache_entry(&aby_found.cache, &expected_cache, yz, "yz");

    assert_eq!(aby_found.cache.entries.len(), 5);
    assert_eq!(
        aby_found.end,
        MatchResult {
            path: PathCoverage::Range(RangeEnd {
                entry_pos: 2.into(),
                exit_pos: 2.into(),
                target: DownKey::new(y, 2.into()),
                end_pos: 3.into(),
                path: RootedRangePath::new(
                    PatternLocation::new(xabyz, xab_yz_id),
                    RolePath::new(
                        0,
                        vec![ChildLocation::new(xab, x_a_b_id, 1)],
                    ),
                    RolePath::new(1, vec![ChildLocation::new(yz, y_z_id, 0)],),
                ),
            }),
            cursor: crate::state::matched::CheckpointedCursor::AtCheckpoint(
                Checkpointed::<PatternCursor>::new(PatternCursor {
                    path: RootedRangePath::new(
                        query.clone(),
                        RolePath::new(0, vec![]),
                        RolePath::new_empty(2),
                    ),
                    atom_position: 3.into(),
                    _state: std::marker::PhantomData,
                }),
            ),
        }
    );
}
