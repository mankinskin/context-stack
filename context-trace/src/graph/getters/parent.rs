use crate::graph::{
    Hypergraph,
    getters::{
        ErrorReason,
        vertex::VertexSet,
    },
    kind::GraphKind,
    vertex::{
        VertexParents,
        has_vertex_index::HasVertexIndex,
        parent::Parent,
    },
};

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    #[track_caller]
    pub(crate) fn expect_parents(
        &self,
        index: impl HasVertexIndex,
    ) -> VertexParents {
        self.expect_vertex_data(index.vertex_index())
            .parents()
            .clone()
    }
    #[track_caller]
    pub(crate) fn expect_parent(
        &self,
        index: impl HasVertexIndex,
        parent: impl HasVertexIndex,
    ) -> Parent {
        self.expect_vertex_data(index.vertex_index())
            .expect_parent(parent)
            .clone()
    }
    pub(crate) fn get_pattern_parents(
        &self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
        parent: impl HasVertexIndex,
    ) -> Result<Vec<Parent>, ErrorReason> {
        pattern
            .into_iter()
            .map(|index| {
                let vertex = self.expect_vertex_data(index.vertex_index());
                vertex.get_parent(parent.vertex_index()).cloned()
            })
            .collect()
    }
}
