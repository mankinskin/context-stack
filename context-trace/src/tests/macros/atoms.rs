//! Atom insertion and querying macros for tests.
//!
//! Provides convenient macros for working with atom vertices in test graphs.

/// Get existing atom vertices by identifier name
///
/// Looks up atom vertices by their character representation and binds them to variables.
/// Panics if atoms don't exist.
///
/// # Example
/// ```ignore
/// expect_atoms!(graph, {h, e, l, l, o});
/// // Now you have variables: h, e, l, o (note: duplicate 'l' uses same variable)
/// ```
#[macro_export]
macro_rules! expect_atoms {
    ($graph:ident, {$($name:ident),*}) => {

        let g = $graph.graph();
        $(let $name = g.expect_atom_child($crate::charify::charify!($name));)*
        #[allow(dropping_references)]
        drop(g);
    };
}

/// Insert atom vertices into a graph
///
/// Creates new atom vertices with character representations and binds them to variables.
/// The macro uses the variable name as the character value.
///
/// # Example
/// ```ignore
/// let mut graph = HypergraphRef::default();
/// insert_atoms!(graph, {a, b, c, d});
/// // Now you have atom vertices: a, b, c, d
/// ```
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
