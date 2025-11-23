//! Basic vertex insertion operations

use crate::{
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
            data::{
                VertexData,
                VertexDataBuilder,
            },
            key::VertexKey,
            token::Token,
        },
    },
};

impl<G> crate::graph::Hypergraph<G>
where
    G: GraphKind,
{
    #[allow(dead_code)]
    pub(crate) fn insert_vertex_builder(
        &mut self,
        builder: VertexDataBuilder,
    ) -> Token {
        let data = self.finish_vertex_builder(builder);
        self.insert_vertex_data(data)
    }

    #[allow(dead_code)]
    pub(crate) fn finish_vertex_builder(
        &mut self,
        mut builder: VertexDataBuilder,
    ) -> VertexData {
        builder.index(self.next_vertex_index()).build().unwrap()
    }

    /// Insert raw vertex data
    pub(crate) fn insert_vertex_data(
        &mut self,
        data: VertexData,
    ) -> Token {
        let c = Token::new(data.vertex_index(), data.width);
        self.graph.insert(data.key, data);
        c
    }

    pub(crate) fn validate_vertex(
        &self,
        index: impl crate::graph::vertex::has_vertex_index::HasVertexIndex,
    ) {
        self.expect_vertex(index.vertex_index()).validate()
    }
}
