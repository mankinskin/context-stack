//! Basic vertex insertion operations

use crate::{
    HasVertexIndex,
    Hypergraph,
    Wide,
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
            data::{
                VertexData,
                VertexDataBuilder,
            },
            token::Token,
        },
    },
};

impl<G: GraphKind> Hypergraph<G> {
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
        builder: VertexDataBuilder,
    ) -> VertexData {
        // Extract width from builder (defaults to TokenWidth(1) if not set)
        let width = builder.width.unwrap_or(crate::TokenWidth(1));
        let index = self.next_vertex_index();
        let token = Token::new(index, width);

        builder.build(token)
    }

    /// Insert raw vertex data
    pub fn insert_vertex_data(
        &mut self,
        data: VertexData,
    ) -> Token {
        let c = Token::new(data.vertex_index(), data.width());
        self.graph.insert(data.key, data);
        c
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn validate_vertex(
        &self,
        index: impl crate::graph::vertex::has_vertex_index::HasVertexIndex,
    ) {
        self.expect_vertex(index.vertex_index()).validate()
    }
}
