//! Test graph registration trait
//!
//! Provides the `AsGraphRef` trait for types that can register a Hypergraph
//! for human-readable token display in test output.

/// Trait for types that can provide access to a Hypergraph for test graph registration
#[cfg(any(test, feature = "test-api"))]
pub trait AsGraphRef<G: crate::graph::kind::GraphKind> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display;
}




