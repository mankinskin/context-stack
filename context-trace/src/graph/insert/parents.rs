//! Parent management operations

use crate::{
    HashSet,
    Hypergraph,
    Wide,
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            location::child::ChildLocation,
            pattern::{
                IntoPattern,
                id::PatternId,
                pattern_width,
            },
            token::Token,
            wide::WideMut,
        },
    },
};

impl<G: GraphKind> Hypergraph<G> {
    /// Utility: builds total width, indices and tokens for pattern
    pub(super) fn to_width_indices_children(
        &self,
        indices: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> (usize, Vec<crate::graph::vertex::VertexIndex>, Vec<Token>) {
        let mut width = 0;
        let (a, b) = indices
            .into_iter()
            .map(|index| {
                let index = index.vertex_index();
                let w = self.expect_vertex_data(index.vertex_index()).width();
                width += w.0;
                (index, Token::new(index, w))
            })
            .unzip();
        (width, a, b)
    }

    /// Adds a parent to all nodes in a pattern
    #[track_caller]
    pub fn add_parents_to_pattern_nodes<
        I: HasVertexIndex,
        P: ToToken,
    >(
        &self,
        pattern: Vec<I>,
        parent: P,
        pattern_id: PatternId,
    ) {
        for (i, token) in pattern.into_iter().enumerate() {
            self.with_vertex_mut(token.vertex_index(), |node| {
                node.add_parent(ChildLocation::new(
                    parent.to_token(),
                    pattern_id,
                    i,
                ));
            }).expect("Vertex should exist");
        }
    }

    pub(crate) fn add_pattern_parent(
        &self,
        parent: impl ToToken,
        pattern: impl IntoPattern,
        pattern_id: PatternId,
        start: usize,
    ) {
        pattern
            .into_pattern()
            .into_iter()
            .enumerate()
            .for_each(|(pos, c)| {
                let pos = start + pos;
                self.with_vertex_mut(c.vertex_index(), |node| {
                    node.add_parent(ChildLocation::new(
                        parent.to_token(),
                        pattern_id,
                        pos,
                    ));
                }).expect("Vertex should exist");
            });
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub fn append_to_pattern(
        &self,
        parent: impl ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<Item = impl ToToken>,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_token()).collect();
        if new.is_empty() {
            return parent.to_token();
        }
        let width = pattern_width(&new);
        
        // Get pattern and update child parents
        let pattern = self.with_vertex(parent.vertex_index(), |vertex| {
            vertex.expect_child_pattern(&pattern_id).clone()
        }).expect("Parent vertex should exist");
        
        for c in pattern.into_iter().collect::<HashSet<_>>() {
            self.with_vertex_mut(c.vertex_index(), |node| {
                node.get_parent_mut(parent.vertex_index()).unwrap().width += width;
            }).expect("Child vertex should exist");
        }
        
        // Update parent vertex
        let (offset, final_width) = self.with_vertex_mut(parent.vertex_index(), |vertex| {
            let pattern = vertex.expect_child_pattern_mut(&pattern_id);
            let offset = pattern.len();
            pattern.extend(new.iter());
            *vertex.width_mut() += width.0;
            (offset, vertex.width())
        }).expect("Parent vertex should exist");
        
        let parent = Token::new(parent.vertex_index(), final_width);
        self.add_pattern_parent(parent, new, pattern_id, offset);
        parent
    }
}
