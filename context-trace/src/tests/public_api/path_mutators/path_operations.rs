//! Tests for combined path operations and TraceCtx usage
//!
//! These tests demonstrate real-world usage patterns of path mutators
//! in conjunction with TraceCtx, similar to how context-search and
//! context-insert would use them.

use crate::{
    path::mutators::move_path::{
        leaf::MoveLeaf,
        root::MoveRootIndex,
    },
    *,
};
use std::ops::ControlFlow;

#[test]
fn trace_ctx_postfix_traces_path_upward() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
    );
    insert_patterns!(graph,
        _abc => [ab, c]
    );

    // Create a postfix path starting from 'b' within 'ab'
    let loc = ChildLocation::new(ab, ab_id, 1);
    let start_path = IndexStartPath::new_location(loc);

    // Create TraceCtx and trace the postfix command
    let mut ctx = TraceCtx {
        trav: graph.graph(),
        cache: TraceCache::new(b),
    };

    let command = PostfixCommand {
        path: start_path,
        add_edges: true,
        root_up_key: UpKey {
            index: ab,
            pos: 1.into(),
        },
    };

    command.trace(&mut ctx);

    // Verify the cache contains expected entries
    assert!(ctx.cache.exists_vertex(&b));
    assert!(ctx.cache.exists_vertex(&ab));
}

#[test]
fn trace_ctx_prefix_traces_path_downward() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
    );
    insert_patterns!(graph,
        _abc => [ab, c]
    );

    // Create a prefix path ending at 'a' within 'ab'
    let loc = ChildLocation::new(ab, ab_id, 0);
    let end_path = IndexEndPath::new_location(loc);

    // Create TraceCtx starting from 'ab'
    let mut ctx = TraceCtx {
        trav: graph.graph(),
        cache: TraceCache::new(ab),
    };

    let command = PrefixCommand {
        path: end_path,
        add_edges: true,
        end_pos: AtomPosition::from(0), // Test case - position at start
    };

    command.trace(&mut ctx);

    // Verify the cache contains the traced path
    assert!(ctx.cache.exists_vertex(&ab));
    assert!(ctx.cache.exists_vertex(&a));
}

#[test]
fn trace_ctx_range_demonstrates_basic_usage() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph,
        (abc, abc_id) => [a, b, c]
    );

    // Demonstrate basic IndexRangePath construction
    // In real usage, RangeCommand would be used for bidirectional tracing
    let start_loc = ChildLocation::new(abc, abc_id, 0);
    let start = IndexStartPath::new_location(start_loc);
    let end_loc = ChildLocation::new(abc, abc_id, 2);
    let end = IndexEndPath::new_location(end_loc);

    // IndexRangePath needs a root and role paths
    let root = start.root.clone();
    let range_path = IndexRangePath {
        root,
        start: start.role_path,
        end: end.role_path,
    };

    // Verify the range path was constructed correctly
    // (Actual tracing with RangeCommand requires more complex setup)
    assert_eq!(range_path.start.root_entry, 0);
    assert_eq!(range_path.end.root_entry, 2);
}

#[test]
fn path_append_and_trace_creates_nested_path() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (cd, _cd_id) => [c, d],
        (abcd, abcd_id) => [ab, cd]
    );

    // Start with a path and append to it
    let loc = ChildLocation::new(abcd, abcd_id, 0);
    let mut path = IndexEndPath::new_location(loc);

    // Append the first child location of 'ab' (pointing to 'a')
    let child_loc = ChildLocation::new(ab, ab_id, 0);
    path.path_append(child_loc);

    // Verify the sub_path now has one entry
    assert_eq!(path.sub_path.path.len(), 1);
    assert_eq!(path.sub_path.path[0].parent, ab);
    assert_eq!(path.sub_path.path[0].pattern_id, ab_id);
    assert_eq!(path.sub_path.path[0].sub_index, 0);
}

#[test]
fn move_root_and_leaf_combined() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (cd, _cd_id) => [c, d],
        (abcd, abcd_id) => [ab, cd]
    );

    // Create a path with both root and leaf
    let loc = ChildLocation::new(abcd, abcd_id, 0);
    let mut path = IndexEndPath::new_location(loc);
    let child_loc = ChildLocation::new(ab, ab_id, 0);
    path.path_append(child_loc);

    // Move the root index
    let root_result =
        MoveRootIndex::<Right>::move_root_index(&mut path, &graph);
    assert_eq!(root_result, ControlFlow::Continue(()));
    assert_eq!(path.root_entry, 1);

    // Move the leaf within the pattern
    let leaf_result = MoveLeaf::<Right>::move_leaf(
        path.sub_path.path.last_mut().unwrap(),
        &graph,
    );
    assert_eq!(leaf_result, ControlFlow::Continue(()));
    assert_eq!(path.sub_path.path[0].sub_index, 1);
}

#[test]
fn trace_cache_accumulates_across_multiple_commands() {
    let _tracing = init_test_tracing!();

    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d});
    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (cd, cd_id) => [c, d]
    );

    // Create TraceCtx
    let mut ctx = TraceCtx {
        trav: graph.graph(),
        cache: TraceCache::new(a),
    };

    // Trace first path (postfix from 'a' to 'ab')
    let loc1 = ChildLocation::new(ab, ab_id, 0);
    let path1 = IndexStartPath::new_location(loc1);
    let cmd1 = PostfixCommand {
        path: path1,
        add_edges: true,
        root_up_key: UpKey {
            index: ab,
            pos: 0.into(),
        },
    };
    cmd1.trace(&mut ctx);

    // Trace second path (postfix from 'c' to 'cd')
    // First add 'c' to cache
    ctx.cache.force_mut(&DirectedKey::from(c));

    let loc2 = ChildLocation::new(cd, cd_id, 0);
    let path2 = IndexStartPath::new_location(loc2);
    let cmd2 = PostfixCommand {
        path: path2,
        add_edges: true,
        root_up_key: UpKey {
            index: cd,
            pos: 0.into(),
        },
    };
    cmd2.trace(&mut ctx);

    // Verify cache accumulated both traces
    assert!(ctx.cache.exists_vertex(&a));
    assert!(ctx.cache.exists_vertex(&ab));
    assert!(ctx.cache.exists_vertex(&c));
    assert!(ctx.cache.exists_vertex(&cd));
}
