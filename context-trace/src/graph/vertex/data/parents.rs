//! Parent relationship operations for VertexData.
//!
//! Methods for managing and querying parent vertices and their pattern indices.

use super::core::VertexData;
use crate::{
    TokenWidth,
    graph::{
        getters::ErrorReason,
        vertex::{
            VertexIndex,
            has_vertex_index::HasVertexIndex,
            location::child::ChildLocation,
            parent::{
                Parent,
                PatternIndex,
            },
            pattern::id::PatternId,
        },
    },
};
use either::Either;

impl VertexData {
    /// Get parent relationship by vertex index
    pub(crate) fn get_parent(
        &self,
        index: impl HasVertexIndex,
    ) -> Result<&Parent, ErrorReason> {
        let index = index.vertex_index();
        self.parents
            .get(&index)
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }

    /// Get mutable parent relationship by vertex index
    pub(crate) fn get_parent_mut(
        &mut self,
        index: impl HasVertexIndex,
    ) -> Result<&mut Parent, ErrorReason> {
        let index = index.vertex_index();
        self.parents
            .get_mut(&index)
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }

    /// Get parent relationship, panicking if not found
    #[track_caller]
    pub(crate) fn expect_parent(
        &self,
        index: impl HasVertexIndex,
    ) -> &Parent {
        self.get_parent(index).unwrap()
    }

    /// Get mutable parent relationship, panicking if not found
    #[track_caller]
    pub(crate) fn expect_parent_mut(
        &mut self,
        index: impl HasVertexIndex,
    ) -> &mut Parent {
        self.get_parent_mut(index).unwrap()
    }

    /// Add a parent relationship at the given location
    pub(crate) fn add_parent(
        &mut self,
        loc: ChildLocation,
    ) {
        if let Some(parent) = self.parents.get_mut(&loc.parent.vertex_index()) {
            parent.add_pattern_index(loc.pattern_id, loc.sub_index);
        } else {
            let mut parent_rel = Parent::new(loc.parent.width);
            parent_rel.add_pattern_index(loc.pattern_id, loc.sub_index);
            self.parents.insert(loc.parent.vertex_index(), parent_rel);
        }
        // not while indexing
        //self.validate_links();
    }

    /// Remove all relationships to a parent vertex
    pub(crate) fn remove_parent(
        &mut self,
        vertex: impl HasVertexIndex,
    ) {
        self.parents.remove(&vertex.vertex_index());
        // not while indexing
        //self.validate_links();
    }

    /// Remove a specific parent pattern index
    pub(crate) fn remove_parent_index(
        &mut self,
        vertex: impl HasVertexIndex,
        pattern: PatternId,
        index: usize,
    ) {
        if let Some(parent) = self.parents.get_mut(&vertex.vertex_index()) {
            if parent.pattern_indices.len() > 1 {
                parent.remove_pattern_index(pattern, index);
            } else {
                self.parents.remove(&vertex.vertex_index());
            }
        }
        // not while indexing
        //self.validate_links();
    }

    /// Get iterator of parents below specified width threshold
    pub(crate) fn get_parents_below_width(
        &self,
        width_ceiling: Option<TokenWidth>,
    ) -> impl Iterator<Item = (&VertexIndex, &Parent)> + Clone {
        let parents = self.parents();
        // optionally filter parents by width
        if let Some(ceil) = width_ceiling {
            Either::Left(
                parents
                    .iter()
                    .filter(move |(_, parent)| parent.get_width() < ceil),
            )
        } else {
            Either::Right(parents.iter())
        }
    }

    /// Get parent pattern index starting at specified offset
    pub(crate) fn get_parent_to_starting_at(
        &self,
        parent_index: impl HasVertexIndex,
        index_offset: usize,
    ) -> Result<PatternIndex, ErrorReason> {
        let index = parent_index.vertex_index();
        self.get_parent(index)
            .ok()
            .and_then(|parent| parent.get_index_at_pos(index_offset))
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }

    /// Get parent pattern index at prefix (offset 0)
    pub(crate) fn get_parent_at_prefix_of(
        &self,
        index: impl HasVertexIndex,
    ) -> Result<PatternIndex, ErrorReason> {
        self.get_parent_to_starting_at(index, 0)
    }

    /// Get parent pattern index at postfix (end of pattern)
    pub(crate) fn get_parent_at_postfix_of(
        &self,
        vertex: &VertexData,
    ) -> Result<PatternIndex, ErrorReason> {
        self.get_parent(vertex.vertex_index())
            .ok()
            .and_then(|parent| parent.get_index_at_postfix_of(vertex))
            .ok_or(ErrorReason::ErrorReasoningParent(vertex.vertex_index()))
    }

    // Commented out methods - preserved for future use
    /*
    pub(crate) fn get_parents_at_prefix(&self) -> HashMap<VertexIndex, PatternId> {
        self.get_parents_with_index_at(0)
    }

    pub(crate) fn get_parents_at_postfix(
        &self,
        graph: &crate::graph::Hypergraph,
    ) -> HashMap<VertexIndex, PatternId> {
        self.parents
            .iter()
            .filter_map(|(id, parent)| {
                parent
                    .get_index_at_postfix_of(graph.expect_vertex(id))
                    .map(|pat| (*id, pat.pattern_id))
            })
            .collect()
    }

    pub(crate) fn get_parents_with_index_at(
        &self,
        offset: usize,
    ) -> HashMap<VertexIndex, PatternId> {
        self.parents
            .iter()
            .filter_map(|(id, parent)| {
                parent
                    .get_index_at_pos(offset)
                    .map(|pat| (*id, pat.pattern_id))
            })
            .collect()
    }

    pub(crate) fn filter_parent_to(
        &self,
        parent: impl HasVertexIndex,
        cond: impl Fn(&&Parent) -> bool,
    ) -> Result<&'_ Parent, ErrorReason> {
        let index = parent.vertex_index();
        self.get_parent(index)
            .ok()
            .filter(cond)
            .ok_or(ErrorReason::ErrorReasoningParent(index))
    }

    pub(crate) fn get_parent_to_ending_at(
        &self,
        parent_key: impl HasVertexKey,
        offset: usize,
    ) -> Result<&'_ Parent, ErrorReason> {
        self.filter_parent_to(parent_key, |parent| {
            offset
                .checked_sub(self.width)
                .map(|p| parent.exists_at_pos(p))
                .unwrap_or(false)
        })
    }
    */
}
