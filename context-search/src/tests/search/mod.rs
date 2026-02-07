pub(crate) mod ancestor;
pub(crate) mod consecutive;
pub(crate) mod insert_scenarios;
pub(crate) mod parent;
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
            .unwrap_or_else(|e| panic!("Failed to find index for {}: {:?}", stringify!($name), e))
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
    } = &*Env1::get();
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
    let base_graph = Hypergraph::<BaseGraphKind>::default();
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

/// Test that EntireRoot matches have cursor_position equal to root token width.
///
/// When a search finds an EntireRoot match (complete token), the cursor position
/// should be set to the width of the root token, not 0. This is important for
/// consecutive insertion operations that use the cursor position.
#[test]
fn test_entire_root_cursor_position_equals_token_width() {
    use crate::{
        search::context::AncestorSearchTraversal,
        Searchable,
    };

    let graph = HypergraphRef::default();
    // Create a pattern that will result in an EntireRoot match
    // ab exists as a complete token
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (abc, abc_id) => [ab, c]
    );

    // Search for "ab" - should find EntireRoot since "ab" is a complete token at the root
    let response = Searchable::<AncestorSearchTraversal<_>>::search(vec![a, b], graph.into());

    let response = response.expect("search should succeed");

    // Verify cursor_position equals root token width for EntireRoot matches
    if let PathCoverage::EntireRoot(_) = &response.end.path {
        let cursor_pos = response.cursor_position();
        let root_width = *response.end.root_parent().width();

        assert_eq!(
            *cursor_pos.as_ref(),
            usize::from(root_width),
            "EntireRoot cursor_position ({}) should equal root token width ({})",
            *cursor_pos.as_ref(),
            usize::from(root_width)
        );
    }
}

/// Test that searching for a non-existent pattern creates an EntireRoot with correct cursor position.
///
/// When no matches are found, the search creates an EntireRoot response for the query's first token.
/// The cursor position should be set to that token's width.
#[test]
fn test_no_match_entire_root_cursor_position() {
    use crate::{
        search::context::AncestorSearchTraversal,
        Searchable,
    };

    let graph = HypergraphRef::default();
    // Create graph with some patterns
    insert_atoms!(graph, {a, b, c, z});
    insert_patterns!(graph,
        (ab, _ab_id) => [a, b],
        (bc, _bc_id) => [b, c]
    );

    // Search for a pattern "zz" that doesn't exist in the graph
    let response = Searchable::<AncestorSearchTraversal<_>>::search(vec![z, z], graph.into());

    let response = response.expect("search should succeed even with no match");

    // This should be an EntireRoot (no match found)
    match &response.end.path {
        PathCoverage::EntireRoot(_) => {
            let cursor_pos = response.cursor_position();
            let root_width = *response.end.root_parent().width();

            assert_eq!(
                *cursor_pos.as_ref(),
                usize::from(root_width),
                "No-match EntireRoot cursor_position ({}) should equal root token width ({})",
                *cursor_pos.as_ref(),
                usize::from(root_width)
            );
        }
        other => {
            panic!(
                "Expected EntireRoot for non-existent pattern, got {:?}",
                other
            );
        }
    }
}

