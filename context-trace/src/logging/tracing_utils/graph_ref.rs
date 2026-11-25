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

#[cfg(all(feature = "legacy", any(test, feature = "test-api")))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for &crate::legacy::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::legacy::graph::test_graph::register_test_graph(self);
    }
}

#[cfg(all(feature = "legacy", any(test, feature = "test-api")))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for &crate::legacy::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::legacy::graph::test_graph::register_test_graph_ref(self);
    }
}

#[cfg(all(feature = "legacy", any(test, feature = "test-api")))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G> for crate::legacy::Hypergraph<G> {
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        crate::legacy::graph::test_graph::register_test_graph(&self);
    }
}

#[cfg(all(feature = "legacy", any(test, feature = "test-api")))]
impl<G: crate::graph::kind::GraphKind> AsGraphRef<G>
    for crate::legacy::HypergraphRef<G>
{
    fn register_test_graph(self)
    where
        G: Send + Sync + 'static,
        G::Atom: std::fmt::Display,
    {
        // Use the new register_test_graph_ref to avoid cloning
        crate::legacy::graph::test_graph::register_test_graph_ref(&self);
    }
}
