use std::ops::ControlFlow;

use crate::{
    TokenWidth,
    direction::{
        Left,
        Right,
        pattern::PatternDirection,
    },
    graph::vertex::{
        ChildPatterns,
        has_vertex_index::HasVertexIndex,
        location::{
            PatternId,
            PatternLocation,
            SubLocation,
            pattern::IntoPatternLocation,
        },
        pattern::Pattern,
        token::Token,
        wide::Wide,
    },
    path::mutators::move_path::leaf::MoveLeaf,
    trace::has_graph::{
        HasGraph,
        TravDir,
    },
};
pub trait HasSubIndexMut: HasSubIndex {
    fn sub_index_mut(&mut self) -> &mut usize;
}
pub trait HasSubIndex {
    fn sub_index(&self) -> usize;
}
impl<T: HasSubIndex> HasSubIndex for &mut T {
    fn sub_index(&self) -> usize {
        (**self).sub_index()
    }
}
impl<T: HasSubIndex> HasSubIndex for &T {
    fn sub_index(&self) -> usize {
        (**self).sub_index()
    }
}
impl<T: HasSubIndexMut> HasSubIndexMut for &mut T {
    fn sub_index_mut(&mut self) -> &mut usize {
        (**self).sub_index_mut()
    }
}
impl HasSubIndex for ChildLocation {
    fn sub_index(&self) -> usize {
        self.sub_index
    }
}
impl HasSubIndexMut for ChildLocation {
    fn sub_index_mut(&mut self) -> &mut usize {
        &mut self.sub_index
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChildLocation {
    pub parent: Token,
    pub pattern_id: PatternId,
    pub sub_index: usize,
}

impl std::fmt::Display for ChildLocation {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // Token's Display already handles string representation if available
        write!(
            f,
            "ChildLocation({}, {}, {})",
            self.parent,
            &format!("{}", self.pattern_id)[..8],
            self.sub_index
        )
    }
}

impl crate::logging::compact_format::CompactFormat for ChildLocation {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        _indent: usize,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl MoveLeaf<Right> for ChildLocation {
    fn move_leaf<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = graph.expect_pattern_at(*self);
        if let Some(next) =
            TravDir::<G>::pattern_index_next(&pattern, self.sub_index)
        {
            self.sub_index = next;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}
impl MoveLeaf<Left> for ChildLocation {
    fn move_leaf<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let graph = trav.graph();
        let pattern = graph.expect_pattern_at(*self);
        if let Some(prev) =
            TravDir::<G>::pattern_index_prev(&pattern, self.sub_index)
        {
            self.sub_index = prev;
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}

impl ChildLocation {
    pub fn new(
        parent: Token,
        pattern_id: PatternId,
        sub_index: usize,
    ) -> Self {
        Self {
            parent,
            pattern_id,
            sub_index,
        }
    }
    pub(crate) fn to_sub_location(self) -> SubLocation {
        SubLocation {
            pattern_id: self.pattern_id,
            sub_index: self.sub_index,
        }
    }
}
#[allow(dead_code)]
impl ChildLocation {
    pub(crate) fn get_child_in<'a>(
        &self,
        patterns: &'a ChildPatterns,
    ) -> Option<&'a Token> {
        self.get_pattern_in(patterns)
            .and_then(|p| self.get_child_in_pattern(p))
    }
    pub(crate) fn expect_child_in<'a>(
        &self,
        patterns: &'a ChildPatterns,
    ) -> &'a Token {
        self.get_child_in(patterns)
            .expect("Expected Token not present in TokenPatterns!")
    }
    pub(crate) fn get_child_in_pattern<'a>(
        &self,
        pattern: &'a Pattern,
    ) -> Option<&'a Token> {
        pattern.get(self.sub_index)
    }
    pub(crate) fn expect_child_in_pattern<'a>(
        &self,
        pattern: &'a Pattern,
    ) -> &'a Token {
        self.get_child_in_pattern(pattern)
            .expect("Expected Token not present in TokenPatterns!")
    }
    pub(crate) fn get_pattern_in<'a>(
        &self,
        patterns: &'a ChildPatterns,
    ) -> Option<&'a Pattern> {
        patterns.get(&self.pattern_id)
    }
    pub(crate) fn expect_pattern_in<'a>(
        &self,
        patterns: &'a ChildPatterns,
    ) -> &'a Pattern {
        self.get_pattern_in(patterns)
            .expect("Expected Pattern not present in TokenPatterns!")
    }
    pub(crate) fn to_child_location(
        self,
        sub_index: usize,
    ) -> ChildLocation {
        ChildLocation { sub_index, ..self }
    }
    pub(crate) fn to_pattern_location(
        self,
        pattern_id: PatternId,
    ) -> PatternLocation {
        PatternLocation {
            parent: self.parent,
            pattern_id,
        }
    }
}

pub trait IntoChildLocation {
    fn into_child_location(self) -> ChildLocation;
}

impl IntoChildLocation for ChildLocation {
    fn into_child_location(self) -> ChildLocation {
        self
    }
}

impl IntoChildLocation for &ChildLocation {
    fn into_child_location(self) -> ChildLocation {
        *self
    }
}

impl IntoPatternLocation for ChildLocation {
    fn into_pattern_location(self) -> PatternLocation {
        PatternLocation {
            parent: self.parent,
            pattern_id: self.pattern_id,
        }
    }
}

impl HasVertexIndex for ChildLocation {
    fn vertex_index(&self) -> crate::graph::vertex::VertexIndex {
        self.parent.index
    }
}

impl Wide for ChildLocation {
    fn width(&self) -> TokenWidth {
        self.parent.width()
    }
}
