pub(crate) mod atom;
pub(crate) mod child;
pub(crate) mod parent;
pub(crate) mod pattern;
pub(crate) mod utils;
pub(crate) mod vertex;

use std::borrow::Borrow;

use crate::{
    graph::{
        Hypergraph,
        kind::GraphKind,
        vertex::{
            VertexIndex,
            has_vertex_index::HasVertexIndex,
            pattern::{
                Pattern,
                id::PatternId,
            },
            token::Token,
        },
    },
    path::structs::rooted::pattern_range::PatternRangePath,
};
use derive_new::new;
pub(crate) use vertex::VertexSet;

#[derive(Debug, Clone, Eq, PartialEq, new)]
pub struct IndexWithPath {
    pub index: Token,
    pub path: PatternRangePath,
}
impl From<IndexWithPath> for Token {
    fn from(val: IndexWithPath) -> Self {
        val.index
    }
}
impl Borrow<Token> for IndexWithPath {
    fn borrow(&self) -> &Token {
        &self.index
    }
}
impl From<PatternRangePath> for IndexWithPath {
    fn from(path: PatternRangePath) -> Self {
        let index = *path.root.first().unwrap();
        IndexWithPath { index, path }
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ErrorReason {
    EmptyPatterns,
    NoParents,
    NoTokenPatterns,
    NotFound,
    ErrorReasoningParent(VertexIndex),
    InvalidPattern(PatternId),
    InvalidChild(usize),
    InvalidPatternRange(PatternId, Pattern, String),
    SingleIndex(Box<IndexWithPath>),
    ParentMatchingPartially,
    UnknownKey,
    UnknownIndex,
    UnknownAtom,
    Unnecessary,
    EmptyRange,
}

impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn expect_index_width(
        &self,
        index: &impl HasVertexIndex,
    ) -> usize {
        self.expect_vertex(index.vertex_index()).width.0
    }
}
