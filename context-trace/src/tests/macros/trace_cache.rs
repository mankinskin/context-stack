//! Trace cache construction macro and tests.
//!
//! Provides macro for building TraceCache structures with a declarative syntax.

#[cfg(test)]
use crate::*;

/// Build a TraceCache with declarative syntax
///
/// This macro allows creating TraceCache structures with bottom-up and top-down
/// position caches in a readable format.
///
/// # Syntax
/// ```ignore
/// build_trace_cache!(
///     vertex_name => (
///         BU {
///             position => child -> (pattern_id, sub_index),
///             ...
///         },
///         TD {
///             position => child -> (pattern_id, sub_index),
///             ...
///         }
///     ),
///     ...
/// )
/// ```
///
/// # Example
/// ```ignore
/// let cache = build_trace_cache!(
///     root_vertex => (
///         BU { 1 => child1 -> (pattern_id, 0) },
///         TD { 2 => child2 -> (pattern_id, 1) },
///     )
/// );
/// ```
#[macro_export]
macro_rules! build_trace_cache {
    (
        $(
            $entry_root:ident => (BU {
                $(
                    $bu_pos:expr $(=> $($bu_child:ident -> ($bu_pid:expr, $bu_sub:expr)),*)?
                ),* $(,)?
            },
            TD {
                $(
                    $td_pos:expr $(=> $($td_child:ident -> ($td_pid:expr, $td_sub:expr)),*)?
                ),* $(,)?
            }
            $(,)?
        )
        ),*
            $(,)?
    ) => {
        $crate::TraceCache {
            entries: HashMap::from_iter([
                $(
                    ($entry_root.vertex_index(), VertexCache {
                        index: $entry_root,
                        bottom_up: $crate::DirectedPositions::from_iter([
                            $(
                                (
                                    $bu_pos.into(),
                                    PositionCache::new(
                                        Default::default(),
                                        HashMap::from_iter([
                                            $($(
                                                (
                                                    DirectedKey {
                                                        index: $bu_child,
                                                        pos: $crate::DirectedPosition::BottomUp($bu_pos.into()),
                                                    },
                                                    SubLocation::new($bu_pid, $bu_sub),
                                                )
                                            ),*)?
                                        ]),
                                    ),
                                ),
                            )*
                        ]),
                        top_down: $crate::DirectedPositions::from_iter([
                            $(
                                (
                                    $td_pos.into(),
                                    PositionCache::new(
                                        Default::default(),
                                        HashMap::from_iter([
                                            $($(
                                                (
                                                    DirectedKey {
                                                        index: $td_child,
                                                        pos: DirectedPosition::TopDown($td_pos.into()),
                                                    },
                                                    SubLocation::new($td_pid, $td_sub),
                                                ),
                                            ),*)?
                                        ]),
                                    ),
                                ),
                            )*
                        ]),
                    }),
                )*
            ]),
        }
    };
}

#[test]
fn test_build_trace_cache1() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    insert_atoms!(graph, {h, e, l, d});
    insert_patterns!(graph,
        (ld, ld_id) => [l, d],
        (heldld, heldld_id) => [h, e, ld, ld]
    );
    let cache = build_trace_cache!(
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
    assert_eq!(
        cache,
        TraceCache {
            entries: HashMap::from_iter([
                (
                    heldld.vertex_index(),
                    VertexCache {
                        index: heldld,
                        bottom_up: DirectedPositions::from_iter([]),
                        top_down: DirectedPositions::from_iter([(
                            2.into(),
                            PositionCache::new(
                                Default::default(),
                                HashMap::from_iter([(
                                    DirectedKey {
                                        index: ld,
                                        pos: DirectedPosition::TopDown(
                                            2.into(),
                                        ),
                                    },
                                    SubLocation::new(heldld_id, 2),
                                )]),
                            ),
                        )]),
                    }
                ),
                (
                    ld.vertex_index(),
                    VertexCache {
                        index: ld,
                        bottom_up: DirectedPositions::from_iter([]),
                        top_down: DirectedPositions::from_iter([(
                            2.into(),
                            PositionCache::new(
                                Default::default(),
                                HashMap::from_iter([(
                                    DirectedKey {
                                        index: l,
                                        pos: DirectedPosition::TopDown(
                                            2.into(),
                                        ),
                                    },
                                    SubLocation::new(ld_id, 0),
                                )]),
                            ),
                        )]),
                    }
                ),
                (
                    h.vertex_index(),
                    VertexCache {
                        index: h,
                        bottom_up: DirectedPositions::from_iter([]),
                        top_down: DirectedPositions::from_iter([]),
                    }
                ),
                (
                    l.vertex_index(),
                    VertexCache {
                        index: l,
                        bottom_up: DirectedPositions::from_iter([]),
                        top_down: DirectedPositions::from_iter([(
                            2.into(),
                            PositionCache::default()
                        )]),
                    }
                ),
            ]),
        }
    );
}

#[test]
fn test_build_trace_cache2() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c, d});

    insert_patterns!(graph,
        (ab, ab_id) => [a, b],
        (ababcd, ababcd_id) => [ab, ab, c, d]
    );
    let cache = build_trace_cache!(
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
    );
    assert_eq!(
        cache,
        TraceCache {
            entries: HashMap::from_iter([
                (
                    ababcd.vertex_index(),
                    VertexCache {
                        index: ababcd,
                        bottom_up: DirectedPositions::from_iter([(
                            1.into(),
                            PositionCache::new(
                                Default::default(),
                                HashMap::from_iter([(
                                    DirectedKey {
                                        index: ab,
                                        pos: DirectedPosition::BottomUp(
                                            1.into(),
                                        ),
                                    },
                                    SubLocation::new(ababcd_id, 1),
                                )]),
                            ),
                        )]),
                        top_down: DirectedPositions::from_iter([]),
                    }
                ),
                (
                    ab.vertex_index(),
                    VertexCache {
                        index: ab,
                        bottom_up: DirectedPositions::from_iter([(
                            1.into(),
                            PositionCache::new(
                                Default::default(),
                                HashMap::from_iter([(
                                    DirectedKey {
                                        index: b,
                                        pos: DirectedPosition::BottomUp(
                                            1.into(),
                                        ),
                                    },
                                    SubLocation::new(ab_id, 1),
                                )]),
                            ),
                        )]),
                        top_down: DirectedPositions::from_iter([]),
                    }
                ),
                (
                    b.vertex_index(),
                    VertexCache {
                        index: b,
                        bottom_up: DirectedPositions::from_iter([]),
                        top_down: DirectedPositions::from_iter([]),
                    }
                ),
            ]),
        }
    );
}
