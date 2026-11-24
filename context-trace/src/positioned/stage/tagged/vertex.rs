//! Vertex objects of a tagged stage: [`TaggedVertex`] (read) and
//! [`TaggedVertexMut`] (write).

use std::fmt;

use super::{
    TaggedComposition,
    first_slot,
    lookup,
};
use crate::{
    AtomOf,
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::{
        StageIndex,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
    positioned::{
        committed::PositionedGraph,
        model::{
            PositionedVertex,
            VertexNode,
        },
        store::{
            NodeRef,
            VertexRead,
            VertexState,
            raw,
        },
    },
};

/// Vertex object of a [`TaggedView`](super::TaggedView): the stored vertex and the graph (two
/// words, as the committed vertex object).
pub struct TaggedVertex<'a, G: GraphKind> {
    pub(super) graph: &'a PositionedGraph<G>,
    pub(super) vertex: &'a PositionedVertex<G>,
}

impl<G: GraphKind> Clone for TaggedVertex<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for TaggedVertex<'_, G> {}

impl<'a, G: GraphKind> TaggedVertex<'a, G> {
    #[inline(always)]
    pub(super) fn get(
        graph: &'a PositionedGraph<G>,
        index: StageIndex,
    ) -> Option<Self> {
        lookup(graph, index).map(|vertex| Self { graph, vertex })
    }
}

impl<G: GraphKind> fmt::Debug for TaggedVertex<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "#{}(w{})", self.vertex_index(), self.vertex.width.0)?;
        #[cfg(debug_assertions)]
        write!(f, " {:?}", self.vertex.label)?;
        Ok(())
    }
}

impl<G: GraphKind> HasVertexIndex for TaggedVertex<'_, G> {
    type Index = StageIndex;
    fn vertex_index(&self) -> StageIndex {
        StageIndex::tagged(
            self.vertex.index.0,
            self.vertex.state == VertexState::Staged,
        )
    }
}

impl<G: GraphKind> Wide for TaggedVertex<'_, G> {
    fn width(&self) -> TokenWidth {
        self.vertex.width
    }
}

impl<'a, G: GraphKind> VertexRead<'a> for TaggedVertex<'a, G> {
    type Kind = G;
    type Composition = TaggedComposition<'a>;

    #[inline(always)]
    fn node(self) -> NodeRef<'a, AtomOf<G>, TaggedComposition<'a>> {
        match &self.vertex.node {
            VertexNode::Atom(atom) => NodeRef::Atom(atom),
            VertexNode::Composite(composition) =>
                NodeRef::Composite(TaggedComposition {
                    composition,
                    first: first_slot(self.graph),
                }),
        }
    }
    fn parents(
        self
    ) -> impl Iterator<Item = (Token<StageIndex>, AtomPosition)> {
        let vertices = &self.graph.vertices;
        self.vertex
            .parents
            .iter()
            .flat_map(move |(&parent, offsets)| {
                let vertex = &vertices[parent.0];
                let parent = Token::new(
                    StageIndex::tagged(
                        parent.0,
                        vertex.state == VertexState::Staged,
                    ),
                    vertex.width,
                );
                offsets.iter().map(move |&offset| (parent, offset))
            })
    }
    fn parent_count(self) -> usize {
        self.vertex.parents.values().map(|offsets| offsets.len()).sum()
    }
    fn is_parent_at(
        self,
        parent: StageIndex,
        offset: AtomPosition,
    ) -> bool {
        self.vertex
            .parents
            .get(&VertexIndex(parent.slot()))
            .is_some_and(|offsets| offsets.contains(&offset))
    }
    fn state(self) -> VertexState {
        self.vertex.state
    }
    fn resolve(
        self,
        index: StageIndex,
    ) -> Option<Self> {
        Self::get(self.graph, index)
    }
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str {
        &self.vertex.label
    }
}

/// Write handle of one tagged vertex. Vertices committed before the stage
/// opened keep their content: width and label are checked, not written.
pub struct TaggedVertexMut<'a, G: GraphKind> {
    pub(super) vertex: &'a mut PositionedVertex<G>,
    pub(super) fixed: bool,
}

impl<G: GraphKind> raw::VertexWrite for TaggedVertexMut<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    fn set_node(
        &mut self,
        node: VertexNode<AtomOf<G>, StageIndex>,
    ) {
        self.vertex.node = match node {
            VertexNode::Composite(composition) =>
                VertexNode::Composite(composition.into_slots()),
            VertexNode::Atom(atom) => {
                assert!(
                    !self.fixed,
                    "committed vertex {} cannot become an atom",
                    self.vertex.index
                );
                VertexNode::Atom(atom)
            },
        };
    }
    fn set_width(
        &mut self,
        width: TokenWidth,
    ) {
        if self.fixed {
            assert_eq!(
                self.vertex.width, width,
                "the width of committed vertex {} is immutable",
                self.vertex.index
            );
        }
        self.vertex.width = width;
    }
    fn link_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        self.vertex
            .parents
            .entry(VertexIndex(parent.slot()))
            .or_default()
            .insert(offset);
    }
    fn unlink_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        let parent = VertexIndex(parent.slot());
        if let Some(offsets) = self.vertex.parents.get_mut(&parent) {
            offsets.remove(&offset);
            if offsets.is_empty() {
                self.vertex.parents.remove(&parent);
            }
        }
    }
    #[cfg(debug_assertions)]
    fn set_label(
        &mut self,
        label: String,
    ) {
        if self.fixed {
            debug_assert_eq!(self.vertex.label, label);
        } else {
            self.vertex.label = label;
        }
    }
}
