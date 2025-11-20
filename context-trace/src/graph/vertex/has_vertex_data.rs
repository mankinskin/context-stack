use std::ops::{
    Deref,
    DerefMut,
};

use crate::graph::{
    Hypergraph,
    getters::vertex::VertexSet,
    kind::GraphKind,
    vertex::{
        data::VertexData,
        has_vertex_index::HasVertexIndex,
        key::VertexKey,
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
        let vertex_data = graph.get_vertex(index).ok()?;
        Some(graph.vertex_data_string(vertex_data))
    }
}

impl<T: HasVertexIndex> HasVertexStringRepr for T {}

pub(crate) trait HasVertexDataMut: HasVertexData {
    fn vertex_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        self,
        graph: &'a mut R,
    ) -> &'a mut VertexData
    where
        Self: 'a;

    fn vertex_ref_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        &'a mut self,
        graph: &'a mut R,
    ) -> &'a mut VertexData;
}

impl HasVertexDataMut for Token {
    fn vertex_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        self,
        graph: &'a mut R,
    ) -> &'a mut VertexData
    where
        Self: 'a,
    {
        graph.expect_vertex_mut(self.vertex_index())
    }
    fn vertex_ref_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        &'a mut self,
        graph: &'a mut R,
    ) -> &'a mut VertexData {
        graph.expect_vertex_mut(self.vertex_index())
    }
}

impl<V: HasVertexDataMut> HasVertexDataMut for &'_ mut V {
    fn vertex_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        self,
        graph: &'a mut R,
    ) -> &'a mut VertexData
    where
        Self: 'a,
    {
        V::vertex_ref_mut(self, graph)
    }
    fn vertex_ref_mut<
        'a,
        G: GraphKind + 'a,
        R: Deref<Target = Hypergraph<G>> + DerefMut,
    >(
        &'a mut self,
        graph: &'a mut R,
    ) -> &'a mut VertexData {
        V::vertex_ref_mut(*self, graph)
    }
}
//impl<G: GraphKind> VertexedMut<G> for &mut VertexData {
//    fn vertex_mut<'a, R: Deref<Target=Hypergraph<G>> + DerefMut>(
//        self,
//        _graph: &'a mut R,
//    ) -> &'a mut VertexData
//        where Self: 'a
//    {
//        self
//    }
//    fn vertex_ref_mut<'a, R: Deref<Target=Hypergraph<G>> + DerefMut>(
//        &'a mut self,
//        _graph: &'a mut R,
//    ) -> &'a mut VertexData {
//        *self
//    }
//}

pub trait HasVertexData: Sized {
    fn vertex<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &'a R,
    ) -> &'a VertexData
    where
        Self: 'a;
    fn vertex_ref<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        &'a self,
        graph: &'a R,
    ) -> &'a VertexData;
}

impl HasVertexData for Token {
    fn vertex<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &'a R,
    ) -> &'a VertexData
    where
        Self: 'a,
    {
        graph.expect_vertex(self.vertex_index())
    }
    fn vertex_ref<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        &'a self,
        graph: &'a R,
    ) -> &'a VertexData {
        graph.expect_vertex(self.vertex_index())
    }
}
impl HasVertexData for VertexKey {
    fn vertex<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &'a R,
    ) -> &'a VertexData
    where
        Self: 'a,
    {
        graph.expect_vertex(self)
    }
    fn vertex_ref<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        &'a self,
        graph: &'a R,
    ) -> &'a VertexData {
        graph.expect_vertex(self)
    }
}

impl<V: HasVertexData> HasVertexData for &'_ V {
    fn vertex<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &'a R,
    ) -> &'a VertexData
    where
        Self: 'a,
    {
        V::vertex_ref(self, graph)
    }
    fn vertex_ref<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        &'a self,
        graph: &'a R,
    ) -> &'a VertexData {
        V::vertex_ref(*self, graph)
    }
}

impl<V: HasVertexData> HasVertexData for &'_ mut V {
    fn vertex<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        self,
        graph: &'a R,
    ) -> &'a VertexData
    where
        Self: 'a,
    {
        V::vertex_ref(self, graph)
    }
    fn vertex_ref<'a, G: GraphKind + 'a, R: Deref<Target = Hypergraph<G>>>(
        &'a self,
        graph: &'a R,
    ) -> &'a VertexData {
        V::vertex_ref(*self, graph)
    }
}
