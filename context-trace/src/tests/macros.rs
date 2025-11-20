#[cfg(test)]
use crate::*;

#[macro_export]
macro_rules! insert_patterns {
    ($graph:ident,
        $(
            $name:ident => [
                $([$($pat:expr),*]),*$(,)?
            ]
        ),*$(,)?
    ) => {

        $(
            let $name = $crate::HasGraphMut::graph_mut(&mut $graph).insert_patterns([$($ crate::Pattern::from(vec![$($pat),*])),*]);
        )*
    };
    ($graph:ident,
        $(
            $name:ident =>
                [$($pat:expr),*]
        ),*$(,)?
    ) => {

        $(
            let $name = $crate::HasGraphMut::graph_mut(&mut $graph).insert_pattern([$($pat),*]);
        )*
    };
    ($graph:ident,
        $(
            ($name:ident, $idname:ident) => [
                $([$($pat:expr),*]),*$(,)?
            ]
        ),*$(,)?
    ) => {

        $(
            let ($name, $idname) = $crate::HasGraphMut::graph_mut(&mut $graph).insert_patterns_with_ids([$($crate::Pattern::from(vec![$($pat),*])),*]);
        )*
    };
    ($graph:ident,
        $(
            ($name:ident, $idname:ident) =>
                [$($pat:expr),*]
        ),*$(,)?
    ) => {

        $(
            let ($name, $idname) = $crate::HasGraphMut::graph_mut(&mut $graph).insert_pattern_with_id([$($pat),*]);
            let $idname = $idname.unwrap();
        )*
    };
}

#[macro_export]
macro_rules! assert_patterns {
    ($graph:ident,
        $(
            $name:ident => [
                $([$($pat:expr),*]),*$(,)?
            ]
        ),*$(,)?
    ) => {

        let g = $graph.graph();
        $(
            let pats: HashSet<_> =
                $crate::HasVertexData::vertex(&$name, &g).child_pattern_set().into_iter().collect();
            assert_eq!(pats, hashset![$($crate::Pattern::from(vec![$($pat),*])),*]);
        )*
        #[allow(dropping_references)]
        drop(g);
    };
}

#[macro_export]
macro_rules! expect_atoms {
    ($graph:ident, {$($name:ident),*}) => {

        let g = $graph.graph();
        $(let $name = g.expect_atom_child($crate::charify::charify!($name));)*
        #[allow(dropping_references)]
        drop(g);
    };
}
#[macro_export]
macro_rules! insert_atoms {
    ($graph:ident, {$($name:ident),*}) => {
        use itertools::Itertools;
        let ($($name),*) = $crate::trace::has_graph::HasGraphMut::graph_mut(&mut $graph)
            .insert_atoms([
                $(
                    $crate::graph::vertex::atom::Atom::Element($crate::charify::charify!($name))
                ),*
            ])
            .into_iter()
            .next_tuple()
            .unwrap();
    };
}
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
    let mut graph = HypergraphRef::default();
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
    let mut graph = HypergraphRef::default();
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

/// Create RootedRolePath variants with convenient syntax
///
/// This macro simplifies creation of IndexRangePath, PatternRangePath,
/// IndexStartPath, IndexEndPath, PatternStartPath, PatternEndPath, etc.
///
/// # Syntax
///
/// Range paths (with start and end):
/// - `rooted_path!(Range: root, start: entry, end: exit)`
/// - `rooted_path!(Range: root, start: (entry, [child...]), end: (exit, [child...]))`
///
/// Single-role paths (Start or End only):
/// - `rooted_path!(Start: root, entry)`
/// - `rooted_path!(End: root, entry)`
/// - `rooted_path!(Start: root, (entry, [child...]))`
/// - `rooted_path!(End: root, (entry, [child...]))`
///
/// # Examples
///
/// ```ignore
/// // IndexRangePath with simple entry/exit
/// let path = rooted_path!(Range: root, start: 0, end: 2);
///
/// // PatternRangePath with pattern root
/// let pattern = Pattern::from(vec![a, b, c]);
/// let path = rooted_path!(Range: pattern, start: 0, end: 2);
///
/// // With nested child locations
/// let path = rooted_path!(Range: root,
///     start: (0, [child_loc]),
///     end: (2, [child_loc1, child_loc2])
/// );
///
/// // IndexStartPath
/// let path = rooted_path!(Start: root, 0);
///
/// // PatternEndPath
/// let path = rooted_path!(End: pattern, 2);
///
/// // With children
/// let path = rooted_path!(End: root, (1, [child_loc]));
/// ```
#[macro_export]
macro_rules! rooted_path {
    // Range paths with child paths: rooted_path!(Range: root, start: (entry, [children]), end: (exit, [children]))
    (Range: $root:expr, start: ($start_entry:expr, [$($start_child:expr),* $(,)?]), end: ($end_entry:expr, [$($end_child:expr),* $(,)?])) => {
        $crate::RootedRangePath::new(
            $root,
            $crate::RolePath::new($start_entry, vec![$($start_child),*]),
            $crate::RolePath::new($end_entry, vec![$($end_child),*]),
        )
    };

    // Range with start children, end empty
    (Range: $root:expr, start: ($start_entry:expr, [$($start_child:expr),* $(,)?]), end: $end_entry:expr) => {
        $crate::RootedRangePath::new(
            $root,
            $crate::RolePath::new($start_entry, vec![$($start_child),*]),
            $crate::RolePath::new_empty($end_entry),
        )
    };

    // Range with end children, start empty
    (Range: $root:expr, start: $start_entry:expr, end: ($end_entry:expr, [$($end_child:expr),* $(,)?])) => {
        $crate::RootedRangePath::new(
            $root,
            $crate::RolePath::new_empty($start_entry),
            $crate::RolePath::new($end_entry, vec![$($end_child),*]),
        )
    };

    // Range paths: rooted_path!(Range: root, start: entry, end: exit)
    (Range: $root:expr, start: $start_entry:expr, end: $end_entry:expr) => {
        $crate::RootedRangePath::new(
            $root,
            $crate::RolePath::new_empty($start_entry),
            $crate::RolePath::new_empty($end_entry),
        )
    };

    // Single-role paths with children: rooted_path!(Start: root, (entry, [children]))
    (Start: $root:expr, ($entry:expr, [$($child:expr),* $(,)?])) => {
        $crate::RootedRolePath::<$crate::Start, _>::new(
            $root,
            $crate::RolePath::new($entry, vec![$($child),*]),
        )
    };

    (End: $root:expr, ($entry:expr, [$($child:expr),* $(,)?])) => {
        $crate::RootedRolePath::<$crate::End, _>::new(
            $root,
            $crate::RolePath::new($entry, vec![$($child),*]),
        )
    };

    // Single-role paths: rooted_path!(Start: root, entry)
    (Start: $root:expr, $entry:expr) => {
        $crate::RootedRolePath::<$crate::Start, _>::new(
            $root,
            $crate::RolePath::new_empty($entry),
        )
    };

    (End: $root:expr, $entry:expr) => {
        $crate::RootedRolePath::<$crate::End, _>::new(
            $root,
            $crate::RolePath::new_empty($entry),
        )
    };
}

/// Register a graph for token string representations in tests
///
/// This macro provides a convenient way to enable string representations
/// for tokens in test output. After calling this, tokens will display
/// their string representation (e.g., "abc") in addition to their index
/// and width when formatted.
///
/// # Example
/// ```ignore
/// let mut graph = HypergraphRef::default();
/// insert_atoms!(graph, {a, b, c});
///
/// // Enable string representations
/// register_test_graph!(graph);
///
/// // Now tokens show their content: T0w1("a")
/// println!("{}", a);
/// ```
#[macro_export]
macro_rules! register_test_graph {
    ($graph:ident) => {
        #[cfg(test)]
        $crate::graph::test_graph::register_test_graph($graph.graph());
    };
    ($graph:expr) => {
        #[cfg(test)]
        $crate::graph::test_graph::register_test_graph(&$graph);
    };
}

#[test]
fn test_rooted_path_macro_range() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, (abc, abc_id) => [a, b, c]);

    // Test IndexRangePath
    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let path1: IndexRangePath = rooted_path!(Range: root, start: 0, end: 2);
    assert_eq!(path1.start.root_entry, 0);
    assert_eq!(path1.end.root_entry, 2);
    assert!(path1.start.path().is_empty());
    assert!(path1.end.path().is_empty());

    // Test PatternRangePath
    let pattern = Pattern::from(vec![a, b, c]);
    let path2: PatternRangePath =
        rooted_path!(Range: pattern, start: 0, end: 2);
    assert_eq!(path2.start.root_entry, 0);
    assert_eq!(path2.end.root_entry, 2);

    // Test with children - single child on each side
    let root2 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let child_loc = ChildLocation::new(abc, abc_id, 1);
    let path3: IndexRangePath = rooted_path!(Range: root2,
        start: (0, [child_loc]),
        end: (2, [child_loc])
    );
    assert_eq!(path3.start.path().len(), 1);
    assert_eq!(path3.end.path().len(), 1);

    // Test with multiple children on both sides
    let root3 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let child1 = ChildLocation::new(abc, abc_id, 0);
    let child2 = ChildLocation::new(abc, abc_id, 1);
    let child3 = ChildLocation::new(abc, abc_id, 2);
    let path4: IndexRangePath = rooted_path!(Range: root3,
        start: (0, [child1, child2]),
        end: (2, [child2, child3])
    );
    assert_eq!(path4.start.path().len(), 2);
    assert_eq!(path4.end.path().len(), 2);
    assert_eq!(path4.start.path()[0], child1);
    assert_eq!(path4.start.path()[1], child2);
    assert_eq!(path4.end.path()[0], child2);
    assert_eq!(path4.end.path()[1], child3);
}

#[test]
fn test_rooted_path_macro_single_role() {
    let mut graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, (abc, abc_id) => [a, b, c]);

    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );

    // Test IndexStartPath
    let start_path: IndexStartPath = rooted_path!(Start: root, 0);
    assert_eq!(start_path.root_entry, 0);
    assert!(start_path.path().is_empty());

    // Test IndexEndPath
    let root2 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let end_path: IndexEndPath = rooted_path!(End: root2, 2);
    assert_eq!(end_path.root_entry, 2);
    assert!(end_path.path().is_empty());

    // Test PatternEndPath
    let pattern = Pattern::from(vec![a, b, c]);
    let pattern_end: PatternEndPath = rooted_path!(End: pattern, 1);
    assert_eq!(pattern_end.root_entry, 1);

    // Test PatternStartPath (internal type)
    let pattern2 = Pattern::from(vec![a, b, c]);
    let pattern_start: PatternStartPath = rooted_path!(Start: pattern2, 0);
    assert_eq!(pattern_start.root_entry, 0);

    // Test with children - IndexStartPath
    let root3 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let child_loc = ChildLocation::new(abc, abc_id, 1);
    let start_with_child: IndexStartPath =
        rooted_path!(Start: root3, (0, [child_loc]));
    assert_eq!(start_with_child.path().len(), 1);

    // Test with children - IndexEndPath
    let root4 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let end_with_child: IndexEndPath =
        rooted_path!(End: root4, (2, [child_loc]));
    assert_eq!(end_with_child.path().len(), 1);

    // Test with children - PatternEndPath
    let pattern3 = Pattern::from(vec![a, b, c]);
    let pattern_end_with_child: PatternEndPath =
        rooted_path!(End: pattern3, (1, [child_loc]));
    assert_eq!(pattern_end_with_child.path().len(), 1);

    // Test with children - PatternStartPath
    let pattern4 = Pattern::from(vec![a, b, c]);
    let pattern_start_with_child: PatternStartPath =
        rooted_path!(Start: pattern4, (0, [child_loc]));
    assert_eq!(pattern_start_with_child.path().len(), 1);
}
