//! Test-only graph registry for token string representations
//!
//! This module provides a way to register a hypergraph for use in tests,
//! allowing tokens to dynamically compute their string representation from
//! the graph when formatting for logs.
//!
//! Uses thread-local storage to allow parallel test execution without
//! interference between tests.
//!
//! # Usage
//!
// Legacy doctest: runs only with `--features legacy-tests`.
#![cfg_attr(feature = "legacy-tests", doc = "```rust")]
#![cfg_attr(not(feature = "legacy-tests"), doc = "```rust,ignore")]
//! use context_trace_legacy::{Hypergraph, graph::test_graph};
//! # use context_trace_legacy::{BaseGraphKind, graph::vertex::atom::Atom};
//!
//! # fn main() {
//! let mut graph = Hypergraph::<BaseGraphKind>::default();
//! let a = graph.insert_atom(Atom::Element('a'));
//! let b = graph.insert_atom(Atom::Element('b'));
//! let c = graph.insert_atom(Atom::Element('c'));
//! let abc = graph.insert_pattern(vec![a, b, c]);
//!
//! // Register the graph for string representations
//! test_graph::register_test_graph(&graph);
//!
//! // Now tokens will show their string representation
//! println!("{}", a);    // Prints: "a"(0)
//! println!("{}", b);    // Prints: "b"(1)
//! println!("{}", abc);  // Prints: "abc"(3)
//!
//! // Clean up when done (optional, as thread-local storage is cleaned up automatically)
//! test_graph::clear_test_graph();
//! # }
//! ```

use crate::{
    Hypergraph,
    HypergraphRef,
    graph::{
        kind::GraphKind,
        vertex::{
            VertexIndex,
            name_resolver::{
                TokenNameResolver,
                clear_token_name_resolver,
                resolve_token_name,
                set_token_name_resolver,
            },
        },
    },
};
use std::sync::Arc;

// Implementation for Arc<Hypergraph<G>> - now with interior mutability,
// we can always read individual vertices without global locking
/// Local newtype: `Arc<Hypergraph>` is foreign to this crate (orphan rule).
pub(crate) struct GraphResolver<G: GraphKind>(pub(crate) Arc<Hypergraph<G>>);

impl<G: GraphKind> TokenNameResolver for GraphResolver<G>
where
    G::Atom: std::fmt::Display,
{
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        // Use try_get_vertex_data to avoid deadlocks when called from within
        // a write lock callback (e.g., during validation/formatting inside add_pattern).
        // If we can't acquire the read lock (because we're inside a write lock),
        // return None and let the caller fall back to a simpler format.
        self.0.try_get_vertex_data(VertexIndex::from(index))
            .map(|data| self.0.vertex_data_string(data))
    }
}

/// Register a HypergraphRef for use in tests
///
/// This allows tokens to look up their string representation when formatting.
/// The Arc<RwLock<Hypergraph>> is stored, so any new tokens added to the graph
/// after registration will automatically be visible in formatting.
/// Each test thread has its own graph registry, allowing parallel test execution.
///
/// # Example
/// ```ignore
/// let mut graph = HypergraphRef::default();
/// insert_atoms!(graph, {a, b, c});
///
/// register_test_graph_ref(&graph);
///
/// // Now tokens will show their string representation in logs
/// println!("{}", a); // "a"(0)
///
/// // Add more atoms - they'll automatically be visible too
/// insert_atoms!(graph, {d, e});
/// println!("{}", d); // "d"(3)
///
/// clear_test_graph();
/// ```
pub fn register_test_graph_ref<G: GraphKind + 'static>(graph: &HypergraphRef<G>)
where
    G::Atom: std::fmt::Display,
{
    // Clone the Arc (cheap - just increments ref count), not the graph
    let graph_arc = graph.0.clone();
    set_token_name_resolver(Box::new(GraphResolver(graph_arc)));
}

/// Register a Hypergraph for use in tests (legacy compatibility)
///
/// **Note:** This wraps the graph in an Arc to support the same
/// interface as HypergraphRef. However, since you only have a reference,
/// the graph is cloned. For dynamic updates, use `register_test_graph_ref` instead.
pub fn register_test_graph<G: GraphKind + 'static>(graph: &Hypergraph<G>)
where
    G::Atom: std::fmt::Display,
{
    // Clone the graph and wrap it - needed because we only have a reference
    let graph_clone = graph.clone();
    let graph_arc = Arc::new(graph_clone);
    set_token_name_resolver(Box::new(GraphResolver(graph_arc)));
}

/// Access the registered test graph to get string representation
pub fn get_token_string_from_test_graph(index: usize) -> Option<String> {
    resolve_token_name(index)
}

/// Clear the registered test graph
pub fn clear_test_graph() {
    clear_token_name_resolver();
}

/// `AsGraphRef` for the row model, so a `Hypergraph` can be passed to the
/// tracing helpers of `context_trace` (behind `test-api`).
///
/// With [`init_test_tracing!`](context_trace::init_test_tracing):
/// ```no_run
/// use context_trace::init_test_tracing;
/// use context_trace_legacy::Hypergraph;
///
/// #[test]
/// fn my_test() {
///     let graph = Hypergraph::default();
///     // ... build graph ...
///     let _tracing = init_test_tracing!(graph);
///     // Tokens will now show string representations
/// }
/// ```
///
/// With `TestTracing::init_with_graph`:
/// ```no_run
/// use context_trace::logging::tracing_utils::TestTracing;
/// use context_trace_legacy::Hypergraph;
///
/// #[test]
/// fn my_test() {
///     let graph = Hypergraph::default();
///     let _tracing = TestTracing::init_with_graph("my_test", &graph);
///     // Test code - tokens will show string representations
///     // Graph and log file will be cleaned up if test passes
/// }
/// ```
pub mod as_graph_ref {
    use context_trace::logging::tracing_utils::graph_ref::AsGraphRef;
    use crate::{GraphKind, Hypergraph, HypergraphRef};
    impl<G: GraphKind> AsGraphRef<G> for &Hypergraph<G> {
        fn register_test_graph(self) where G: Send + Sync + 'static, G::Atom: std::fmt::Display { super::register_test_graph(self); }
    }
    impl<G: GraphKind> AsGraphRef<G> for &HypergraphRef<G> {
        fn register_test_graph(self) where G: Send + Sync + 'static, G::Atom: std::fmt::Display { super::register_test_graph_ref(self); }
    }
    impl<G: GraphKind> AsGraphRef<G> for Hypergraph<G> {
        fn register_test_graph(self) where G: Send + Sync + 'static, G::Atom: std::fmt::Display { super::register_test_graph(&self); }
    }
    impl<G: GraphKind> AsGraphRef<G> for HypergraphRef<G> {
        fn register_test_graph(self) where G: Send + Sync + 'static, G::Atom: std::fmt::Display { super::register_test_graph_ref(&self); }
    }
}
