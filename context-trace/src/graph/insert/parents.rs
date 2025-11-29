//! Parent management operations

use crate::{
    HashSet,
    Hypergraph,
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
                let w = self.expect_vertex(index.vertex_index()).width;
                width += w.0;
                (index, Token::new(index, w))
            })
            .unzip();
        (width, a, b)
    }

    /// Adds a parent to all nodes in a pattern
    #[track_caller]
    pub(super) fn add_parents_to_pattern_nodes<
        I: HasVertexIndex,
        P: ToToken,
    >(
        &mut self,
        pattern: Vec<I>,
        parent: P,
        pattern_id: PatternId,
    ) {
        for (i, token) in pattern.into_iter().enumerate() {
            let node = self.expect_vertex_mut(token.vertex_index());
            node.add_parent(ChildLocation::new(
                parent.to_child(),
                pattern_id,
                i,
            ));
        }
    }

    pub(crate) fn add_pattern_parent(
        &mut self,
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
                let c = self.expect_vertex_mut(c.to_child());
                c.add_parent(ChildLocation::new(
                    parent.to_child(),
                    pattern_id,
                    pos,
                ));
            });
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn append_to_pattern(
        &mut self,
        parent: impl ToToken,
        pattern_id: PatternId,
        new: impl IntoIterator<Item = impl ToToken>,
    ) -> Token {
        let new: Vec<_> = new.into_iter().map(|c| c.to_child()).collect();
        if new.is_empty() {
            return parent.to_child();
        }
        let width = pattern_width(&new);
        let (offset, width) = {
            let vertex = self.expect_vertex(parent.vertex_index());
            let pattern = vertex.expect_child_pattern(&pattern_id).clone();
            for c in pattern.into_iter().collect::<HashSet<_>>() {
                let c = self.expect_vertex_mut(c.to_child());
                c.get_parent_mut(parent.vertex_index()).unwrap().width += width;
            }
            let vertex = self.expect_vertex_mut(parent.vertex_index());
            let pattern = vertex.expect_child_pattern_mut(&pattern_id);
            let offset = pattern.len();
            pattern.extend(new.iter());
            vertex.width += width;
            (offset, vertex.width)
        };
        let parent = Token::new(parent.vertex_index(), width);
        self.add_pattern_parent(parent, new, pattern_id, offset);
        parent
    }
}
