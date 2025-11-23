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
            let pats: HashSet<_> =
                $crate::HasVertexData::vertex(&$name, &g).child_pattern_set().into_iter().collect();
            assert_eq!(pats, hashset![$($crate::Pattern::from(vec![$($pat),*])),*]);
        )*
        #[allow(dropping_references)]
        drop(g);
    };
}
