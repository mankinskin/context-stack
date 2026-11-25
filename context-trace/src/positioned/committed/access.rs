//! The committed store: [`PositionedGraph`], its lock guards and
//! [`PositionedGraphRef`] behind the store traits (staging plan §3.8).

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    fmt,
    sync::{
        RwLockReadGuard,
        RwLockWriteGuard,
    },
};

use super::{
    PositionedGraph,
    PositionedGraphRef,
};
use crate::{
    AtomOf,
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::{
        CommittedIndex,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
    positioned::{
        model::{
            Composition,
            PositionedVertex,
            VertexNode,
        },
        store::{
            Generation,
            GraphRead,
            HasGraphMut,
            NodeRef,
            VertexRead,
            VertexState,
            impl_has_graph,
            impl_has_graph_mut,
            raw::{
                self,
                Seal,
            },
        },
    },
};

/// Vertex object of the committed store: the stored vertex and the graph
/// it belongs to.
pub struct PositionedVertexRef<'a, G: GraphKind> {
    graph: &'a PositionedGraph<G>,
    vertex: &'a PositionedVertex<G>,
}

impl<G: GraphKind> Clone for PositionedVertexRef<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for PositionedVertexRef<'_, G> {}

impl<'a, G: GraphKind> PositionedVertexRef<'a, G> {
    /// The stored vertex data (storage access).
    pub fn data(self) -> &'a PositionedVertex<G> {
        self.vertex
    }
    /// The composition; `None` for atoms.
    pub fn composition(self) -> Option<&'a Composition> {
        self.vertex.composition()
    }
    pub fn key(self) -> VertexKey {
        self.vertex.key
    }
}

impl<G: GraphKind> fmt::Debug for PositionedVertexRef<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self.vertex, f)
    }
}

impl<G: GraphKind> fmt::Display for PositionedVertexRef<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        fmt::Display::fmt(self.vertex, f)
    }
}

impl<G: GraphKind> HasVertexIndex for PositionedVertexRef<'_, G> {
    type Index = CommittedIndex;
    fn vertex_index(&self) -> CommittedIndex {
        self.vertex.index
    }
}

impl<G: GraphKind> Wide for PositionedVertexRef<'_, G> {
    fn width(&self) -> TokenWidth {
        self.vertex.width
    }
}

impl<'a, G: GraphKind> VertexRead<'a> for PositionedVertexRef<'a, G> {
    type Kind = G;
    type Composition = &'a Composition;

    fn node(self) -> NodeRef<'a, AtomOf<G>, &'a Composition> {
        match &self.vertex.node {
            VertexNode::Atom(atom) => NodeRef::Atom(atom),
            VertexNode::Composite(composition) =>
                NodeRef::Composite(composition),
        }
    }
    fn parents(self) -> impl Iterator<Item = (Token, AtomPosition)> {
        let vertices = &self.graph.vertices;
        self.vertex
            .parents
            .iter()
            .flat_map(move |(&parent, offsets)| {
                let parent = vertices[parent.0].token();
                offsets.iter().map(move |&offset| (parent, offset))
            })
    }
    fn parent_count(self) -> usize {
        self.vertex.parents.values().map(BTreeSet::len).sum()
    }
    fn is_parent_at(
        self,
        parent: CommittedIndex,
        offset: AtomPosition,
    ) -> bool {
        self.vertex
            .parents
            .get(&parent)
            .is_some_and(|offsets| offsets.contains(&offset))
    }
    fn state(self) -> VertexState {
        VertexState::Committed
    }
    fn resolve(
        self,
        index: CommittedIndex,
    ) -> Option<Self> {
        self.graph.vertex(index)
    }
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str {
        &self.vertex.label
    }
}

impl<G: GraphKind> GraphRead for PositionedGraph<G> {
    type Kind = G;
    type Index = CommittedIndex;
    type Vertex<'a>
        = PositionedVertexRef<'a, G>
    where
        Self: 'a;

    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = CommittedIndex>,
    ) -> Option<PositionedVertexRef<'_, G>> {
        self.vertices.get(index.vertex_index().0).map(|vertex| {
            PositionedVertexRef {
                graph: self,
                vertex,
            }
        })
    }
    fn atom(
        &self,
        value: &AtomOf<G>,
    ) -> Option<PositionedVertexRef<'_, G>> {
        self.atoms.get(value).and_then(|&index| self.vertex(index))
    }
    fn vertices(
        &self
    ) -> impl Iterator<Item = PositionedVertexRef<'_, G>> + '_ {
        self.vertices.iter().map(move |vertex| PositionedVertexRef {
            graph: self,
            vertex,
        })
    }
    fn generation(&self) -> Generation {
        Generation::committed(self.generation)
    }
}

impl<G: GraphKind> raw::Write for PositionedGraph<G> {
    type VertexMut<'a>
        = &'a mut PositionedVertex<G>
    where
        Self: 'a;

    fn vertex_mut(
        &mut self,
        _: Seal,
        index: CommittedIndex,
    ) -> Option<&mut PositionedVertex<G>> {
        self.vertices.get_mut(index.0)
    }
    fn next_index(
        &self,
        _: Seal,
    ) -> CommittedIndex {
        VertexIndex(self.vertices.len())
    }
    fn push(
        &mut self,
        _: Seal,
        node: VertexNode<AtomOf<G>>,
        width: TokenWidth,
    ) -> CommittedIndex {
        let index = VertexIndex(self.vertices.len());
        self.vertices.push(PositionedVertex {
            index,
            key: VertexKey::default(),
            width,
            #[cfg(debug_assertions)]
            label: String::new(),
            node,
            parents: BTreeMap::new(),
        });
        index
    }
    fn register_atom(
        &mut self,
        _: Seal,
        value: AtomOf<G>,
        index: CommittedIndex,
    ) {
        self.atoms.insert(value, index);
    }
    fn bump_generation(
        &mut self,
        _: Seal,
    ) {
        self.generation += 1;
    }
}

impl<G: GraphKind> raw::VertexWrite for &mut PositionedVertex<G> {
    type Kind = G;
    type Index = CommittedIndex;
    fn set_node(
        &mut self,
        node: VertexNode<AtomOf<G>>,
    ) {
        self.node = node;
    }
    fn set_width(
        &mut self,
        width: TokenWidth,
    ) {
        self.width = width;
    }
    fn link_parent(
        &mut self,
        parent: CommittedIndex,
        offset: AtomPosition,
    ) {
        self.parents.entry(parent).or_default().insert(offset);
    }
    fn unlink_parent(
        &mut self,
        parent: CommittedIndex,
        offset: AtomPosition,
    ) {
        if let Some(offsets) = self.parents.get_mut(&parent) {
            offsets.remove(&offset);
            if offsets.is_empty() {
                self.parents.remove(&parent);
            }
        }
    }
    #[cfg(debug_assertions)]
    fn set_label(
        &mut self,
        label: String,
    ) {
        self.label = label;
    }
}

/// The lock guards of [`PositionedGraphRef`] are views of the graph they
/// lock.
macro_rules! forward_graph_read {
    ($guard:ident) => {
        impl<G: GraphKind> GraphRead for $guard<'_, PositionedGraph<G>> {
            type Kind = G;
            type Index = CommittedIndex;
            type Vertex<'a>
                = PositionedVertexRef<'a, G>
            where
                Self: 'a;
            fn vertex(
                &self,
                index: impl HasVertexIndex<Index = CommittedIndex>,
            ) -> Option<PositionedVertexRef<'_, G>> {
                (**self).vertex(index)
            }
            fn atom(
                &self,
                value: &AtomOf<G>,
            ) -> Option<PositionedVertexRef<'_, G>> {
                (**self).atom(value)
            }
            fn vertices(
                &self
            ) -> impl Iterator<Item = PositionedVertexRef<'_, G>> + '_ {
                (**self).vertices()
            }
            fn generation(&self) -> Generation {
                (**self).generation()
            }
        }
    };
}
forward_graph_read!(RwLockReadGuard);
forward_graph_read!(RwLockWriteGuard);

impl<G: GraphKind> raw::Write for RwLockWriteGuard<'_, PositionedGraph<G>> {
    type VertexMut<'a>
        = &'a mut PositionedVertex<G>
    where
        Self: 'a;
    fn vertex_mut(
        &mut self,
        seal: Seal,
        index: CommittedIndex,
    ) -> Option<&mut PositionedVertex<G>> {
        (**self).vertex_mut(seal, index)
    }
    fn next_index(
        &self,
        seal: Seal,
    ) -> CommittedIndex {
        (**self).next_index(seal)
    }
    fn push(
        &mut self,
        seal: Seal,
        node: VertexNode<AtomOf<G>>,
        width: TokenWidth,
    ) -> CommittedIndex {
        (**self).push(seal, node, width)
    }
    fn register_atom(
        &mut self,
        seal: Seal,
        value: AtomOf<G>,
        index: CommittedIndex,
    ) {
        (**self).register_atom(seal, value, index)
    }
    fn bump_generation(
        &mut self,
        seal: Seal,
    ) {
        (**self).bump_generation(seal)
    }
}

impl_has_graph! {
    impl<G: GraphKind> for PositionedGraph<G>, Kind = G, Index = CommittedIndex,
    self => self; <'a> &'a Self
}
impl_has_graph_mut! {
    impl<G: GraphKind> for PositionedGraph<G>, self => self; <'a> &'a mut Self
}

// The handle locks per call, as it locks per step today.
impl_has_graph! {
    impl<G: GraphKind> for PositionedGraphRef<G>, Kind = G, Index = CommittedIndex,
    self => self.read(); <'a> RwLockReadGuard<'a, PositionedGraph<G>>
}
impl_has_graph_mut! {
    impl<G: GraphKind> for PositionedGraphRef<G>, self => self.write();
    <'a> RwLockWriteGuard<'a, PositionedGraph<G>>
}
// A shared handle still locks for writing (interior mutability, as legacy
// `HypergraphRef`), so `&PositionedGraphRef` is a write handle too.
impl<G: GraphKind> HasGraphMut for &PositionedGraphRef<G> {
    type GuardMut<'a>
        = RwLockWriteGuard<'a, PositionedGraph<G>>
    where
        Self: 'a;
    fn graph_mut(&mut self) -> Self::GuardMut<'_> {
        self.write()
    }
}
