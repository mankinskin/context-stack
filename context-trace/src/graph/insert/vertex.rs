//! Basic vertex insertion operations

use std::sync::atomic::Ordering;

use crate::{
    HasVertexIndex,
    Hypergraph,
    Wide,
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
            VertexEntry,
            VertexIndex,
            data::{
                VertexData,
                VertexDataBuilder,
            },
            token::Token,
        },
    },
};

impl<G: GraphKind> Hypergraph<G> {
    /// Allocate a new vertex index atomically.
    pub(crate) fn alloc_vertex_index(&self) -> VertexIndex {
        VertexIndex::from(self.next_id.fetch_add(1, Ordering::SeqCst))
    }
    
    #[allow(dead_code)]
    pub fn insert_vertex_builder(
        &self,
        builder: VertexDataBuilder,
    ) -> Token {
        let data = self.finish_vertex_builder(builder);
        self.insert_vertex_data(data)
    }

    #[allow(dead_code)]
    pub fn finish_vertex_builder(
        &self,
        builder: VertexDataBuilder,
    ) -> VertexData {
        // Extract width from builder (defaults to TokenWidth(1) if not set)
        let width = builder.width.unwrap_or(crate::TokenWidth(1));
        let index = self.alloc_vertex_index();
        let token = Token::new(index, width);

        builder.build(token)
    }

    /// Insert raw vertex data
    pub fn insert_vertex_data(
        &self,
        data: VertexData,
    ) -> Token {
        let token = Token::new(data.vertex_index(), data.width());
        let key = data.key;
        let index = data.vertex_index();
        
        // Insert into all maps
        self.graph.insert(key, VertexEntry::new(data));
        self.key_to_index.insert(key, index);
        self.index_to_key.insert(index, key);
        
        token
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn validate_vertex(
        &self,
        index: impl crate::graph::vertex::has_vertex_index::HasVertexIndex,
    ) {
        self.expect_vertex_data(index.vertex_index()).validate()
    }
}
