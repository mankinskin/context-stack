use crate::graph::{
    Hypergraph,
    getters::{
        ErrorReason,
        vertex::VertexSet,
    },
    kind::GraphKind,
    vertex::{
        has_vertex_data::HasVertexData,
        has_vertex_index::HasVertexIndex,
        location::pattern::IntoPatternLocation,
        parent::PatternIndex,
        pattern::pattern_range::PatternRangeIndex,
        token::Token,
        wide::Wide,
    },
};
use itertools::Itertools;
use std::ops::Range;

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn get_common_pattern_in_parent(
        &self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
        parent: impl HasVertexIndex,
    ) -> Result<PatternIndex, ErrorReason> {
        let mut parents = self
            .get_pattern_parents(pattern, parent)?
            .into_iter()
            .enumerate();
        parents
            .next()
            .and_then(|(_, first)| {
                first
                    .pattern_indices
                    .iter()
                    .find(|pix| {
                        parents.all(|(i, post)| {
                            post.exists_at_pos_in_pattern(
                                pix.pattern_id,
                                pix.sub_index + i,
                            )
                        })
                    })
                    .cloned()
            })
            .ok_or(ErrorReason::NoTokenPatterns)
    }
    #[track_caller]
    pub(crate) fn expect_common_pattern_in_parent(
        &self,
        pattern: impl IntoIterator<Item = impl HasVertexIndex>,
        parent: impl HasVertexIndex,
    ) -> PatternIndex {
        self.get_common_pattern_in_parent(pattern, parent)
            .expect("No common pattern in parent for tokens.")
    }
    pub(crate) fn get_pattern_range<R: PatternRangeIndex>(
        &self,
        id: impl IntoPatternLocation,
        range: R,
    ) -> Result<&[Token], ErrorReason> {
        let loc = id.into_pattern_location();
        self.get_vertex(loc.parent)?
            .get_child_pattern_range(&loc.pattern_id, range)
    }
    #[track_caller]
    pub fn expect_pattern_range<R: PatternRangeIndex>(
        &self,
        id: impl IntoPatternLocation,
        range: R,
    ) -> &[Token] {
        let loc = id.into_pattern_location();
        self.expect_vertex(loc.parent)
            .expect_child_pattern_range(&loc.pattern_id, range)
    }
    /// get sub-vertex at range relative to index
    /// FIXME: can crash if range does not have an exact match in the root vertex
    pub fn get_vertex_subrange(
        &self,
        vertex: impl HasVertexData,
        range: Range<usize>,
    ) -> Token {
        let mut data = vertex.vertex(&self);
        let mut wrap = 0..data.width().0;
        assert!(wrap.start <= range.start && wrap.end >= range.end);

        while range != wrap {
            let next = data
                .top_down_containment_nodes()
                .into_iter()
                .map(|(pos, c)| (wrap.start + pos, c))
                .map(|(pos, c)| (c.vertex_index(), pos..pos + c.width().0))
                .find_or_first(|(_, w)| {
                    w.start == range.start || w.end == range.end
                })
                .unwrap();

            data = self.expect_vertex(next.0);
            wrap = next.1;
        }

        data.to_child()
    }
}
