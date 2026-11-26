pub use context_trace::graph::vertex::token::*;
use crate::graph::vertex::{
    data::VertexData,
    has_vertex_index::HasVertexIndex,
    location::{SubLocation, child::ChildLocation, pattern::PatternLocation},
    pattern::id::PatternId,
    VertexIndex,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubToken {
    pub(crate) token: Token,
    pub(crate) location: SubLocation,
}
pub trait HasToken {
    fn token(&self) -> Token;
}
impl HasToken for SubToken {
    fn token(&self) -> Token {
        self.token
    }
}
pub trait HasSubLocation {
    fn sub_location(&self) -> &SubLocation;
}
impl HasSubLocation for SubToken {
    fn sub_location(&self) -> &SubLocation {
        &self.location
    }
}
/// Row-model locations of a token (formerly inherent `Token` methods).
pub trait TokenLocation {
    fn to_pattern_location(self, pattern_id: PatternId) -> PatternLocation;
    fn to_child_location(self, sub: SubLocation) -> ChildLocation;
}
impl TokenLocation for Token {
    fn to_pattern_location(self, pattern_id: PatternId) -> PatternLocation {
        PatternLocation::new(self, pattern_id)
    }
    fn to_child_location(self, sub: SubLocation) -> ChildLocation {
        ChildLocation::new(self, sub.pattern_id, sub.sub_index)
    }
}
impl HasVertexIndex for VertexData {
    type Index = VertexIndex;
    fn vertex_index(&self) -> VertexIndex {
        self.token.vertex_index()
    }
}
