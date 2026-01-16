//! Path construction macros for tests.
//!
//! Provides convenient macros for creating rooted role paths with various configurations.

#[cfg(test)]
use crate::{
    path::accessors::path_accessor::PathAccessor,
    *,
};

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

#[test]
fn test_rooted_path_macro_range() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
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
    let graph = HypergraphRef::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, c});
    insert_patterns!(graph, (abc, abc_id) => [a, b, c]);

    let root = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );

    // Test IndexStartPath
    let start_path: IndexStartPath = rooted_path!(Start: root, 0);
    assert_eq!(start_path.root_entry, 0);
    assert!(PathAccessor::path(&start_path).is_empty());

    // Test IndexEndPath
    let root2 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let end_path: IndexEndPath = rooted_path!(End: root2, 2);
    assert_eq!(end_path.root_entry, 2);
    assert!(PathAccessor::path(&end_path).is_empty());

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
    assert_eq!(PathAccessor::path(&start_with_child).len(), 1);

    // Test with children - IndexEndPath
    let root4 = IndexRoot::from(
        ChildLocation::new(abc, abc_id, 0).into_pattern_location(),
    );
    let child_loc2 = ChildLocation::new(abc, abc_id, 1);
    let end_with_child: IndexEndPath =
        rooted_path!(End: root4, (0, [child_loc2]));
    assert_eq!(PathAccessor::path(&end_with_child).len(), 1);

    // Test with children - PatternEndPath
    let pattern3 = Pattern::from(vec![a, b, c]);
    let child_loc3 = ChildLocation::new(abc, abc_id, 1);
    let pattern_end_with_child: PatternEndPath =
        rooted_path!(End: pattern3, (0, [child_loc3]));
    assert_eq!(PathAccessor::path(&pattern_end_with_child).len(), 1);
    // Test with children - PatternStartPath
    let pattern4 = Pattern::from(vec![a, b, c]);
    let child_loc4 = ChildLocation::new(abc, abc_id, 1);
    let pattern_start_with_child: PatternStartPath =
        rooted_path!(Start: pattern4, (0, [child_loc4]));
    assert_eq!(PathAccessor::path(&pattern_start_with_child).len(), 1);
    assert_eq!(PathAccessor::path(&pattern_start_with_child).len(), 1);
}
