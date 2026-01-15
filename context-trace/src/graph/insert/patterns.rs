//! Multiple pattern insertion and management operations

use itertools::Itertools;

use crate::{
    Hypergraph,
    graph::{
        getters::vertex::VertexSet,
        kind::GraphKind,
        vertex::{
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
    /// Add pattern to existing node
    pub fn add_pattern_with_update(
        &self,
        index: impl HasVertexIndex,
        pattern: Pattern,
    ) -> PatternId {
        let indices = pattern.into_pattern();
        let (width, indices, tokens) = self.to_width_indices_children(indices);
        let pattern_id = PatternId::default();
        self.with_vertex_mut(index.vertex_index(), |data| {
            data.add_pattern_no_update(pattern_id, Pattern::from(tokens));
        }).expect("Vertex should exist");
        self.add_parents_to_pattern_nodes(
            indices,
            Token::new(index, width),
            pattern_id,
        );
        pattern_id
    }

    /// Add patterns to existing node
    pub fn add_patterns_with_update(
        &self,
        index: impl HasVertexIndex,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> Vec<PatternId> {
        let index = index.vertex_index();
        patterns
            .into_iter()
            .map(|p| self.add_pattern_with_update(index, p))
            .collect()
    }

    pub fn insert_patterns_with_ids(
        &self,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> (Token, Vec<PatternId>) {
        let patterns = patterns.into_iter().collect_vec();
        let mut ids = Vec::with_capacity(patterns.len());
        let mut patterns = patterns.into_iter();
        let first = patterns.next().expect("Tried to insert no patterns");
        let (node, first_id) = self.insert_pattern_with_id(first);
        ids.push(first_id.unwrap());
        for pat in patterns {
            ids.push(self.add_pattern_with_update(node, pat));
        }
        (node, ids)
    }

    /// Create new node from multiple patterns
    pub fn insert_patterns(
        &self,
        patterns: impl IntoIterator<Item = impl IntoPattern>,
    ) -> Token {
        let patterns = patterns
            .into_iter()
            .map(IntoPattern::into_pattern)
            .collect_vec();
        patterns
            .iter()
            .find(|p| p.len() == 1)
            .map(|p| *p.first().unwrap())
            .unwrap_or_else(|| {
                let mut patterns = patterns.into_iter();
                let first =
                    patterns.next().expect("Tried to insert no patterns");
                let node = self.insert_pattern(first);
                for pat in patterns {
                    self.add_pattern_with_update(node, pat);
                }
                node
            })
    }
}

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    #[track_caller]
    pub(crate) fn try_insert_patterns(
        &self,
        patterns: impl IntoIterator<Item = Pattern>,
    ) -> Option<Token> {
        let patterns = patterns
            .into_iter()
            .map(IntoPattern::into_pattern)
            .collect_vec();
        if patterns.is_empty() {
            None
        } else {
            Some(self.insert_patterns(patterns))
        }
    }
}
