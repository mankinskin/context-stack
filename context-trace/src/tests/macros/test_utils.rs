//! Test utility macros.
//!
//! Provides macros for test graph registration and debugging.

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
