//! Public tests for TraceCache behavior
//!
//! Exercises high-level TraceCache operations available to callers of
//! the `context-trace` crate: creation, force_mut (create-on-access),
//! existence checks, add_state with EditKind, and Extend merging behavior.

#[cfg(test)]
use crate::{
    trace::{
        BottomUp,
        cache::new::NewTraceEdge,
    },
    *,
};

#[test]
fn trace_cache_new_contains_start_vertex_but_no_positions() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        ab => [a, b]
    );

    // create cache starting at `ab`
    let cache = TraceCache::new(ab);

    // the vertex should exist, but there are no position entries yet
    assert!(cache.exists_vertex(&ab));
    let key = DirectedKey::from(ab);
    assert!(!cache.exists(&key));
}

#[test]
fn trace_cache_force_mut_creates_position_and_is_gettable() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        ab => [a, b]
    );

    let mut cache = TraceCache::new(ab);
    let key = DirectedKey::from(ab);

    // force_mut should create a PositionCache at the directed key
    let _ = cache.force_mut(&key);

    // now exists should be true and get should return the created cache
    assert!(cache.exists(&key));
    let got = cache.get(&key).expect("position cache present");
    // default PositionCache is empty
    assert_eq!(got.num_parents(), 0);
    assert_eq!(got.num_bu_edges(), 0);
}

#[test]
fn trace_cache_add_state_creates_new_entries() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    let mut cache = TraceCache::new(a);

    // Create an edit representing a bottom-up trace from 'a' to 'ab'
    let edit = NewTraceEdge::<BottomUp> {
        prev: UpKey {
            index: a,
            pos: 1.into(),
        },
        target: UpKey {
            index: ab,
            pos: 1.into(),
        },
        location: ChildLocation::new(ab, ab_id, 0),
    };

    // Add the state with edges
    let (key, was_new) = cache.add_state(edit, true);

    // Should be a new entry
    assert!(was_new);
    assert_eq!(key.index, ab);

    // The cache should now contain the new position
    assert!(cache.exists(&key));
}

#[test]
fn trace_cache_add_state_idempotent_for_existing_entries() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    let mut cache = TraceCache::new(a);

    let edit = NewTraceEdge::<BottomUp> {
        prev: UpKey {
            index: a,
            pos: 1.into(),
        },
        target: UpKey {
            index: ab,
            pos: 1.into(),
        },
        location: ChildLocation::new(ab, ab_id, 0),
    };

    // Add the state first time
    let (_key1, was_new1) = cache.add_state(edit.clone(), true);
    assert!(was_new1);

    // Add the same state again
    let (_key2, was_new2) = cache.add_state(edit, true);

    // Should not be new the second time
    assert!(!was_new2);
}

#[test]
fn trace_cache_add_state_with_edges_creates_bottom_edges() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        ab => [a, b],
    );
    insert_patterns!(graph,
        (abc, abc_id) => [ab, c]
    );

    let mut cache = TraceCache::new(b);

    // First add 'ab' to cache
    let key_ab = DirectedKey::up(ab, 2);
    cache.force_mut(&key_ab);

    // Create a parent edit (bottom-up from 'ab' to 'abc')
    let edit = NewTraceEdge::<BottomUp> {
        prev: UpKey {
            index: ab,
            pos: 2.into(),
        },
        target: UpKey {
            index: abc,
            pos: 2.into(),
        },
        location: ChildLocation::new(abc, abc_id, 0),
    };

    // Add with edges
    let (key, was_new) = cache.add_state(edit, true);
    assert!(was_new);

    // Check that bottom edge was created
    let pos_cache = cache.get(&key).expect("position cache exists");
    assert!(pos_cache.num_bu_edges() > 0);
}

#[test]
fn trace_cache_add_state_without_edges_creates_no_bottom_edges() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    let mut cache = TraceCache::new(a);

    let edit = NewTraceEdge::<BottomUp> {
        prev: UpKey {
            index: a,
            pos: 1.into(),
        },
        target: UpKey {
            index: ab,
            pos: 1.into(),
        },
        location: ChildLocation::new(ab, ab_id, 0),
    };

    // Add without edges (add_edges = false)
    let (key, _was_new) = cache.add_state(edit, false);

    // Check that no bottom edges were created
    let pos_cache = cache.get(&key).expect("position cache exists");
    assert_eq!(pos_cache.num_bu_edges(), 0);
}

#[test]
fn trace_cache_extend_merges_entries() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_id) => [l, d],
        (heldld, heldld_id) => [h, e, ld, ld]
    );

    let a = build_trace_cache!(
        heldld => (
            BU {},
            TD {2 => ld -> (heldld_id, 2) },
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
    );

    let b = build_trace_cache!(
        l => (
            BU {},
            TD {},
        ),
    );

    // merge b into a using Extend
    let mut a_clone = a.clone();
    a_clone.extend(b.entries);

    // entries from `a` should still be present in the merged cache
    for (k, _v) in a.entries.iter() {
        assert!(a_clone.entries.contains_key(k));
    }
}

#[test]
fn trace_cache_extend_merges_positions_for_same_vertex() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b]
    );

    // Create two caches with different positions for the same vertex
    let cache_a = build_trace_cache!(
        ab => (
            BU { 1 => a -> (ab_id, 0) },
            TD {},
        ),
    );

    let cache_b = build_trace_cache!(
        ab => (
            BU { 2 => b -> (ab_id, 1) },
            TD {},
        ),
    );

    // Merge them
    let mut merged = cache_a.clone();
    merged.extend(cache_b.entries);

    // The merged cache should have both bottom-up positions
    let vertex_cache = merged.get_vertex(&ab).expect("vertex exists");
    assert!(vertex_cache.bottom_up.get(&1.into()).is_some());
    assert!(vertex_cache.bottom_up.get(&2.into()).is_some());
}

#[test]
fn trace_cache_multiple_directed_positions() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, _abc_id) => [a, b, c]
    );

    let mut cache = TraceCache::new(abc);

    // Add multiple bottom-up positions
    let key_bu_1 = DirectedKey::up(abc, 1);
    cache.force_mut(&key_bu_1);

    let key_bu_2 = DirectedKey::up(abc, 2);
    cache.force_mut(&key_bu_2);

    // Add a top-down position
    let key_td_1 = DirectedKey::down(abc, 1);
    cache.force_mut(&key_td_1);

    // Verify all positions exist
    assert!(cache.exists(&key_bu_1));
    assert!(cache.exists(&key_bu_2));
    assert!(cache.exists(&key_td_1));

    // Verify they're in the correct direction caches
    let vertex_cache = cache.get_vertex(&abc).expect("vertex exists");
    assert!(vertex_cache.bottom_up.get(&1.into()).is_some());
    assert!(vertex_cache.bottom_up.get(&2.into()).is_some());
    assert!(vertex_cache.top_down.get(&1.into()).is_some());
}
