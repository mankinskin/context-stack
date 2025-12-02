use crate::graph::{
    Hypergraph,
    getters::{
        ErrorReason,
        vertex::VertexSet,
    },
    kind::GraphKind,
    vertex::{
        has_vertex_index::HasVertexIndex,
        location::child::{
            ChildLocation,
            IntoChildLocation,
        },
        pattern::Pattern,
        token::Token,
    },
};

impl<G: GraphKind> Hypergraph<G> {
    pub(crate) fn get_child_at(
        &self,
        location: impl IntoChildLocation,
    ) -> Result<&Token, ErrorReason> {
        let location = location.into_child_location();
        let pattern = self.get_pattern_at(location)?;
        pattern
            .get(location.sub_index)
            .ok_or(ErrorReason::NoTokenPatterns) // todo: better error
    }
    #[track_caller]
    pub fn expect_child_at(
        &self,
        location: impl IntoChildLocation,
    ) -> &Token {
        let location = location.into_child_location();
        self.get_child_at(location).unwrap_or_else(|_| {
            panic!("Token not found at location {:#?}", location)
        })
    }
    pub fn expect_child_offset(
        &self,
        loc: &ChildLocation,
    ) -> usize {
        self.expect_vertex(loc.vertex_index())
            .expect_child_offset(&loc.to_sub_location())
            .0
    }
    pub(crate) fn to_child(
        &self,
        index: impl HasVertexIndex,
    ) -> Token {
        Token::new(index.vertex_index(), self.expect_index_width(&index))
    }
    #[allow(dead_code)]
    pub(crate) fn get_child_mut_at(
        &mut self,
        location: impl IntoChildLocation,
    ) -> Result<&mut Token, ErrorReason> {
        let location = location.into_child_location();
        let pattern = self.get_pattern_mut_at(location)?;
        pattern
            .get_mut(location.sub_index)
            .ok_or(ErrorReason::NoTokenPatterns) // todo: better error
    }
    #[allow(dead_code)]
    pub(crate) fn expect_child_mut_at(
        &mut self,
        location: impl IntoChildLocation,
    ) -> &mut Token {
        let location = location.into_child_location();
        self.get_child_mut_at(location).unwrap_or_else(|_| {
            panic!("Token not found at location {:#?}", location)
        })
    }
    #[allow(dead_code)]
    pub(crate) fn expect_is_at_end(
        &self,
        location: &ChildLocation,
    ) -> bool {
        self.expect_vertex(location.vertex_index())
            .expect_pattern_len(&location.pattern_id)
            == location.sub_index + 1
    }
    #[allow(dead_code)]
    pub(crate) fn expect_child(
        &self,
        index: impl HasVertexIndex,
    ) -> Token {
        self.to_child(index)
    }
    #[allow(dead_code)]
    pub(crate) fn to_children(
        &self,
        indices: impl IntoIterator<Item = impl HasVertexIndex>,
    ) -> Pattern {
        indices.into_iter().map(|i| self.to_child(i)).collect()
    }
}
