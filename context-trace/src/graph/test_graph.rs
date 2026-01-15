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
//! ```rust
//! use context_trace::{Hypergraph, graph::test_graph};
//! # use context_trace::graph::vertex::atom::Atom;
//!
//! # fn main() {
//! let mut graph = Hypergraph::default();
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
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::VertexIndex,
    },
};
use std::{
    cell::RefCell,
    sync::Arc,
};

/// Type-erased graph accessor for getting string representations
trait GraphStringGetter: Send + Sync {
    fn get_token_string(
        &self,
        index: usize,
    ) -> Option<String>;
}

// Implementation for Arc<Hypergraph<G>> - now with interior mutability,
// we can always read individual vertices without global locking
impl<G: GraphKind> GraphStringGetter for Arc<Hypergraph<G>>
where
    G::Atom: std::fmt::Display,
{
    fn get_token_string(
        &self,
        index: usize,
    ) -> Option<String> {
        // Use try_get_vertex_data to avoid deadlocks when called from within
        // a write lock callback (e.g., during validation/formatting inside add_pattern).
        // If we can't acquire the read lock (because we're inside a write lock),
        // return None and let the caller fall back to a simpler format.
        self.try_get_vertex_data(VertexIndex::from(index))
            .map(|data| self.vertex_data_string(data))
    }
}

thread_local! {
    /// Thread-local test graph registry
    /// Each test thread maintains its own graph reference, allowing parallel test execution
    /// Stores Arc<RwLock<Hypergraph>> so dynamic updates to the graph are visible
    static TEST_GRAPH: RefCell<Option<Box<dyn GraphStringGetter>>> = RefCell::new(None);
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
    TEST_GRAPH.with(|tg| {
        *tg.borrow_mut() = Some(Box::new(graph_arc));
    });
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
    TEST_GRAPH.with(|tg| {
        *tg.borrow_mut() = Some(Box::new(graph_arc));
    });
}

/// Access the registered test graph to get string representation
pub fn get_token_string_from_test_graph(index: usize) -> Option<String> {
    TEST_GRAPH.with(|tg| {
        tg.borrow()
            .as_ref()
            .and_then(|graph| graph.get_token_string(index))
    })
}

/// Clear the registered test graph
pub fn clear_test_graph() {
    TEST_GRAPH.with(|tg| {
        *tg.borrow_mut() = None;
    });
}
