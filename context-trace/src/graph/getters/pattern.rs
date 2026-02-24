use crate::{
    HasPatternId,
    graph::{
        Hypergraph,
        getters::{
            ErrorReason,
            vertex::GetVertexIndex,
        },
        kind::GraphKind,
        vertex::{
            ChildPatterns,
            has_vertex_index::HasVertexIndex,
            location::pattern::IntoPatternLocation,
            pattern::{
                Pattern,
                id::PatternId,
                pattern_range::PatternRangeIndex,
                pattern_width,
            },
        },
    },
};

use super::vertex::VertexSet;

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn get_pattern_at(
        &self,
        location: impl IntoPatternLocation,
    ) -> Result<Pattern, ErrorReason> {
        let location = location.into_pattern_location();
        self.with_vertex(location.parent, |vertex| {
            vertex
                .child_patterns()
                .get(&location.pattern_id())
                .cloned()
                .ok_or(ErrorReason::NoTokenPatterns)
        })?
    }
    #[track_caller]
    pub fn expect_pattern_at(
        &self,
        location: impl IntoPatternLocation,
    ) -> Pattern {
        let location = location.into_pattern_location();
        self.get_pattern_at(location).unwrap_or_else(|_| {
            panic!("Pattern not found at location {:#?}", location)
        })
    }
    pub(crate) fn child_patterns_of(
        &self,
        index: impl GetVertexIndex,
    ) -> Result<ChildPatterns, ErrorReason> {
        self.with_vertex(index.get_vertex_index(self), |vertex| {
            vertex.child_patterns().clone()
        })
    }
    pub(crate) fn get_pattern_of(
        &self,
        index: impl HasVertexIndex,
        pid: PatternId,
    ) -> Result<Pattern, ErrorReason> {
        self.with_vertex(index.vertex_index(), |vertex| {
            vertex.get_child_pattern(&pid).cloned()
        })?
    }
    #[track_caller]
    pub(crate) fn expect_child_pattern(
        &self,
        index: impl GetVertexIndex,
        pid: PatternId,
    ) -> Pattern {
        self.expect_vertex_data(index.get_vertex_index(self))
            .expect_child_pattern(&pid)
            .clone()
    }
    #[track_caller]
    pub fn expect_child_patterns(
        &self,
        index: impl GetVertexIndex,
    ) -> ChildPatterns {
        self.expect_vertex_data(index.get_vertex_index(self))
            .child_patterns()
            .clone()
    }

    #[track_caller]
    pub(crate) fn expect_any_child_pattern(
        &self,
        index: impl GetVertexIndex,
    ) -> (PatternId, Pattern) {
        let data = self.expect_vertex_data(index.get_vertex_index(self));
        let (pid, pat) = data.expect_any_child_pattern();
        (*pid, pat.clone())
    }
    #[track_caller]
    pub(crate) fn expect_pattern_range_width(
        &self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> usize {
        pattern_width(self.expect_pattern_range(location, range)).0
    }
}
