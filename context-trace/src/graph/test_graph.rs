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
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::VertexIndex,
    },
};
use std::cell::RefCell;

/// Type-erased graph accessor for getting string representations
trait GraphStringGetter {
    fn get_token_string(
        &self,
        index: usize,
    ) -> Option<String>;
}

impl<G: GraphKind> GraphStringGetter for Hypergraph<G>
where
    G::Atom: std::fmt::Display,
{
    fn get_token_string(
        &self,
        index: usize,
    ) -> Option<String> {
        self.get_vertex(VertexIndex::from(index)).ok().map(|_| {
            <Hypergraph<G>>::index_string(self, VertexIndex::from(index))
        })
    }
}

thread_local! {
    /// Thread-local test graph registry
    /// Each test thread maintains its own graph reference, allowing parallel test execution
    static TEST_GRAPH: RefCell<Option<Box<dyn GraphStringGetter>>> = RefCell::new(None);
}

/// Register a graph for use in tests
///
/// This allows tokens to look up their string representation when formatting.
/// The graph is stored as a type-erased reference, so it works with any GraphKind.
/// Each test thread has its own graph registry, allowing parallel test execution.
///
/// # Example
/// ```ignore
/// let mut graph = Hypergraph::default();
/// insert_atoms!(graph, {a, b, c});
///
/// register_test_graph(&graph);
///
/// // Now tokens will show their string representation in logs
/// println!("{}", a); // T1w1("a")
///
/// clear_test_graph();
/// ```
pub fn register_test_graph<G: GraphKind + 'static>(graph: &Hypergraph<G>)
where
    G::Atom: std::fmt::Display,
{
    // We need to clone the graph to store it safely
    // In tests this is acceptable overhead
    let graph_clone = graph.clone();
    TEST_GRAPH.with(|tg| {
        *tg.borrow_mut() = Some(Box::new(graph_clone));
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
