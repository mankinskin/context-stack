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
    ) -> &VertexParents {
        self.expect_vertex(index.vertex_index()).parents()
    }
    #[track_caller]
    pub(crate) fn expect_parent(
        &self,
        index: impl HasVertexIndex,
        parent: impl HasVertexIndex,
    ) -> &Parent {
        self.expect_vertex(index.vertex_index())
            .expect_parent(parent)
    }
    #[track_caller]
    pub(crate) fn expect_parent_mut(
        &mut self,
        index: impl HasVertexIndex,
        parent: impl HasVertexIndex,
    ) -> &mut Parent {
        self.expect_vertex_mut(index.vertex_index())
            .expect_parent_mut(parent)
    }
    #[track_caller]
    pub(crate) fn expect_parents_mut(
        &mut self,
        index: impl HasVertexIndex,
    ) -> &mut VertexParents {
        self.expect_vertex_mut(index.vertex_index()).parents_mut()
    }
    pub(crate) fn get_pattern_parents(
        &self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
        parent: impl HasVertexIndex,
    ) -> Result<Vec<Parent>, ErrorReason> {
        pattern
            .into_iter()
            .map(|index| {
                let vertex = self.expect_vertex(index.vertex_index());
                vertex.get_parent(parent.vertex_index()).cloned()
            })
            .collect()
    }
}
