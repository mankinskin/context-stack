use crate::{
    HasPatternId,
    graph::{
        Hypergraph,
        getters::{
            ErrorReason,
            vertex::{
                GetVertexIndex,
                VertexSet,
            },
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

#[allow(dead_code)]
impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn get_pattern_at(
        &self,
        location: impl IntoPatternLocation,
    ) -> Result<&Pattern, ErrorReason> {
        let location = location.into_pattern_location();
        let vertex = self.get_vertex(location.parent)?;
        vertex
            .child_patterns()
            .get(&location.pattern_id())
            .ok_or(ErrorReason::NoTokenPatterns) // todo: better error
    }
    #[track_caller]
    pub fn expect_pattern_at(
        &self,
        location: impl IntoPatternLocation,
    ) -> &Pattern {
        let location = location.into_pattern_location();
        self.get_pattern_at(location).unwrap_or_else(|_| {
            panic!("Pattern not found at location {:#?}", location)
        })
    }
    pub(crate) fn get_pattern_mut_at(
        &mut self,
        location: impl IntoPatternLocation,
    ) -> Result<&mut Pattern, ErrorReason> {
        let location = location.into_pattern_location();
        let vertex = self.get_vertex_mut(location.parent)?;
        let tokens = vertex.child_patterns_mut();
        tokens
            .get_mut(&location.pattern_id())
            .ok_or(ErrorReason::NoTokenPatterns) // todo: better error
    }
    #[track_caller]
    pub(crate) fn expect_pattern_mut_at(
        &mut self,
        location: impl IntoPatternLocation,
    ) -> &mut Pattern {
        let location = location.into_pattern_location();
        self.get_pattern_mut_at(location).unwrap_or_else(|_| {
            panic!("Pattern not found at location {:#?}", location)
        })
    }
    pub(crate) fn child_patterns_of(
        &self,
        index: impl GetVertexIndex,
    ) -> Result<&ChildPatterns, ErrorReason> {
        self.get_vertex(index.get_vertex_index(self))
            .map(|vertex| vertex.child_patterns())
    }
    pub(crate) fn get_pattern_of(
        &self,
        index: impl HasVertexIndex,
        pid: PatternId,
    ) -> Result<&Pattern, ErrorReason> {
        self.get_vertex(index.vertex_index())
            .and_then(|vertex| vertex.get_child_pattern(&pid))
    }
    #[track_caller]
    pub(crate) fn expect_child_pattern(
        &self,
        index: impl GetVertexIndex,
        pid: PatternId,
    ) -> &Pattern {
        self.expect_vertex(index.get_vertex_index(self))
            .expect_child_pattern(&pid)
    }
    #[track_caller]
    pub fn expect_child_patterns(
        &self,
        index: impl GetVertexIndex,
    ) -> &ChildPatterns {
        self.expect_vertex(index.get_vertex_index(self))
            .child_patterns()
    }

    #[track_caller]
    pub(crate) fn expect_any_child_pattern(
        &self,
        index: impl GetVertexIndex,
    ) -> (&PatternId, &Pattern) {
        self.expect_vertex(index.get_vertex_index(self))
            .expect_any_child_pattern()
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
