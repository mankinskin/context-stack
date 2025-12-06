//! Single pattern insertion operations

use crate::{
    Hypergraph,
    graph::{
        kind::GraphKind,
        vertex::{
            data::VertexData,
            has_vertex_index::HasVertexIndex,
            pattern::{
                IntoPattern,
                Pattern,
                id::PatternId,
            },
            token::Token,
        },
    },
};

impl<G: GraphKind> Hypergraph<G> {
    /// Create new node from a pattern
    #[track_caller]
    pub fn insert_pattern_with_id(
        &mut self,
        pattern: impl IntoPattern,
    ) -> (Token, Option<PatternId>) {
        let indices = pattern.into_pattern();
        let (c, id) = match indices.len() {
            0 => (None, None),
            1 => (
                Some(self.to_child(indices.first().unwrap().vertex_index())),
                None,
            ),
            _ => {
                let (c, id) = self.force_insert_pattern_with_id(indices);
                (Some(c), Some(id))
            },
        };
        (c.expect("Tried to index empty pattern!"), id)
    }

    /// Create new node from a pattern (even if single index)
    pub(crate) fn force_insert_pattern_with_id(
        &mut self,
        pattern: impl IntoPattern,
    ) -> (Token, PatternId) {
        let indices = pattern.into_pattern();
        let (width, indices, tokens) = self.to_width_indices_children(indices);
        let index = self.next_vertex_index();
        let mut new_data = VertexData::new(Token::new(index, width));
        let pattern_id = PatternId::default();
        new_data.add_pattern_no_update(pattern_id, Pattern::from(tokens));
        let index = self.insert_vertex_data(new_data);
        self.add_parents_to_pattern_nodes(indices, index, pattern_id);
        (index, pattern_id)
    }

    /// Create new node from a pattern
    pub fn insert_pattern(
        &mut self,
        pattern: impl IntoPattern,
    ) -> Token {
        let indices = pattern.into_pattern();
        self.insert_pattern_with_id(indices).0
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    /// Create new node from a pattern
    pub(crate) fn force_insert_pattern(
        &mut self,
        indices: impl IntoPattern,
    ) -> Token {
        self.force_insert_pattern_with_id(indices).0
    }
}
