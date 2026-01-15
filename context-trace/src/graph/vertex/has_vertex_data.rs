use std::ops::Deref;

use crate::graph::{
    Hypergraph,
    getters::vertex::VertexSet,
    kind::GraphKind,
    vertex::{
        data::VertexData,
        has_vertex_index::HasVertexIndex,
        token::Token,
    },
};

/// Trait for formatting vertex-related types with string representations from a graph
///
/// This trait provides a method to get the string representation of a vertex
/// from a hypergraph. It works similarly to `HasVertexData` by taking a graph reference.
///
/// # Example
///
/// ```rust
/// use context_trace::graph::vertex::has_vertex_data::HasVertexStringRepr;
/// # use context_trace::{Hypergraph, BaseGraphKind};
/// # use context_trace::graph::vertex::atom::Atom;
///
/// # let mut graph: Hypergraph<BaseGraphKind> = Hypergraph::default();
/// # let a = graph.insert_atom(Atom::Element('a'));
/// // Get string representation from a graph
/// if let Some(s) = a.vertex_string_repr(&&graph) {
///     println!("Token string: {}", s);
/// }
/// ```
pub trait HasVertexStringRepr: HasVertexIndex {
    /// Get the string representation of this vertex from the given graph
    fn vertex_string_repr<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        &self,
        graph: &R,
    ) -> Option<String> {
        let index = self.vertex_index();
        let vertex_data = graph.get_vertex_data(index).ok()?;
        Some(graph.vertex_data_string(vertex_data))
    }
}

impl<T: HasVertexIndex> HasVertexStringRepr for T {}

/// Trait for types that can get vertex data from a graph.
/// 
/// With the new DashMap-based interior mutability, we return owned `VertexData`
/// instead of references.
pub trait HasVertexData: Sized {
    /// Get vertex data from a graph, returning an owned copy.
    fn vertex<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &R,
    ) -> VertexData;
    
    /// Get vertex data from a graph reference, returning an owned copy.
    fn vertex_ref<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        &self,
        graph: &R,
    ) -> VertexData;
}

impl HasVertexData for Token {
    fn vertex<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &R,
    ) -> VertexData {
        graph.expect_vertex_data(self.vertex_index())
    }
    
    fn vertex_ref<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        &self,
        graph: &R,
    ) -> VertexData {
        graph.expect_vertex_data(self.vertex_index())
    }
}

impl<V: HasVertexData + Clone> HasVertexData for &'_ V {
    fn vertex<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &R,
    ) -> VertexData {
        V::vertex_ref(self, graph)
    }
    
    fn vertex_ref<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        &self,
        graph: &R,
    ) -> VertexData {
        V::vertex_ref(*self, graph)
    }
}

impl<V: HasVertexData + Clone> HasVertexData for &'_ mut V {
    fn vertex<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &R,
    ) -> VertexData {
        V::vertex_ref(self, graph)
    }
    
    fn vertex_ref<G: GraphKind, R: Deref<Target = Hypergraph<G>>>(
        &self,
        graph: &R,
    ) -> VertexData {
        V::vertex_ref(*self, graph)
    }
}
