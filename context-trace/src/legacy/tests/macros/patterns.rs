//! Pattern insertion and assertion macros for tests.
//!
//! Provides convenient macros for inserting patterns into graphs and asserting
//! their structure in tests.

/// Insert patterns into a graph with automatic variable binding
///
/// This macro provides several syntaxes for inserting patterns:
///
/// 1. Multiple patterns per vertex (returns vertex):
///    ```ignore
///    insert_patterns!(graph,
///        vertex_name => [
///            [atom1, atom2],
///            [atom3, atom4]
///        ]
///    );
///    ```
///
/// 2. Single pattern per vertex (returns vertex):
///    ```ignore
///    insert_patterns!(graph,
///        vertex_name => [atom1, atom2, atom3]
///    );
///    ```
///
/// 3. Multiple patterns with pattern IDs (returns tuple):
///    ```ignore
///    insert_patterns!(graph,
///        (vertex_name, id_name) => [
///            [atom1, atom2],
///            [atom3, atom4]
///        ]
///    );
///    ```
///
/// 4. Single pattern with pattern ID (returns tuple):
///    ```ignore
///    insert_patterns!(graph,
///        (vertex_name, id_name) => [atom1, atom2, atom3]
///    );
///    ```
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
            let $name = $graph.insert_patterns([$($ crate::legacy::Pattern::from(vec![$($pat),*])),*]);
        )*
    };
    ($graph:ident,
        $(
            $name:ident =>
                [$($pat:expr),*]
        ),*$(,)?
    ) => {

        $(
            let $name = $graph.insert_pattern([$($pat),*]);
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
            let ($name, $idname) = $graph.insert_patterns_with_ids([$($crate::legacy::Pattern::from(vec![$($pat),*])),*]);
        )*
    };
    ($graph:ident,
        $(
            ($name:ident, $idname:ident) =>
                [$($pat:expr),*]
        ),*$(,)?
    ) => {

        $(
            let ($name, $idname) = $graph.insert_pattern_with_id([$($pat),*]);
            let $idname = $idname.unwrap();
        )*
    };
}

/// Assert that vertices have expected patterns
///
/// Verifies that each vertex contains exactly the specified set of child patterns.
///
/// # Example
/// ```ignore
/// assert_patterns!(graph,
///     vertex1 => [
///         [a, b],
///         [c, d]
///     ],
///     vertex2 => [
///         [x, y, z]
///     ]
/// );
/// ```
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
            let found: $crate::legacy::HashSet<_> =
                $crate::legacy::HasVertexData::vertex(&$name, &g).child_pattern_set().into_iter().collect();
            let expected = ::std::iter::FromIterator::from_iter([
                $($crate::legacy::Pattern::from(vec![$($pat),*])),*
            ]);
            assert!(
                found == expected,
                "pattern mismatch for `{}`\nfound: {:#?}\nexpected: {:#?}",
                stringify!($name),
                found,
                expected
            );
        )*
        #[allow(dropping_references)]
        drop(g);
    };
}

#[cfg(test)]
mod tests {
    use crate::legacy::{HypergraphRef, insert_atoms, trace::has_graph::HasGraph};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    #[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]
    fn assertion_failure_labels_found_and_expected_patterns() {
        let graph = HypergraphRef::default();
        insert_atoms!(graph, {a, b});
        insert_patterns!(graph, pair => [a, b]);

        let panic = catch_unwind(AssertUnwindSafe(|| {
            assert_patterns!(graph, pair => [[a]]);
        }))
        .expect_err("assert_patterns! should fail for mismatched patterns");
        let message = panic
            .downcast_ref::<String>()
            .expect("assertion should include a formatted panic message");

        assert!(message.contains("found:"));
        assert!(message.contains("expected:"));
    }
}
