//! Range insertion operations

use crate::{
    graph::{
        getters::{
            ErrorReason,
            vertex::VertexSet,
        },
        kind::GraphKind,
        vertex::{
            location::pattern::IntoPatternLocation,
            pattern::{
                IntoPattern,
                Pattern,
                pattern_range::{
                    PatternRangeIndex,
                    get_child_pattern_range,
                },
            },
            token::Token,
        },
    },
};

impl<G> crate::graph::Hypergraph<G>
where
    G: GraphKind,
{
    #[track_caller]
    pub(crate) fn try_insert_range_in(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Result<Token, Token>, ErrorReason> {
        let location = location.into_pattern_location();
        let vertex = self.expect_vertex(location.parent);
        vertex
            .get_child_pattern(&location.pattern_id)
            .map(|pattern| pattern.to_vec())
            .and_then(|pattern| {
                let pattern = Pattern::from(pattern);
                get_child_pattern_range(
                    &location.pattern_id,
                    &pattern,
                    range.clone(),
                )
                .and_then(|inner| {
                    if inner.is_empty() {
                        Err(ErrorReason::EmptyRange)
                    } else if inner.len() == 1 {
                        Ok(Ok(*inner.first().unwrap()))
                    } else if pattern.len() > inner.len() {
                        let c = self.insert_pattern(inner.into_pattern());
                        self.replace_in_pattern(location, range, c);
                        Ok(Ok(c))
                    } else {
                        Ok(Err(location.parent))
                    }
                })
            })
    }

    #[track_caller]
    pub(crate) fn insert_range_in(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Token, ErrorReason> {
        self.try_insert_range_in(location, range)
            .and_then(|c| c.or(Err(ErrorReason::Unnecessary)))
    }

    #[track_caller]
    pub(crate) fn insert_range_in_or_default(
        &mut self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
    ) -> Result<Token, ErrorReason> {
        self.try_insert_range_in(location, range).map(|c| match c {
            Ok(c) => c,
            Err(c) => c,
        })
    }
}
