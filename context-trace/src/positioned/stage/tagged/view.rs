//! [`TaggedView`]: the store of a read with a tagged stage.

use std::sync::RwLockWriteGuard;

use super::{
    StageId,
    TaggedVertex,
    TaggedVertexMut,
    lookup,
};
use crate::{
    AtomOf,
    BaseGraphKind,
    GraphKind,
    TokenWidth,
    VertexKey,
    graph::vertex::{
        StageIndex,
        StagedIndex,
        VertexId,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
    },
    positioned::{
        committed::PositionedGraph,
        model::{
            PositionedVertex,
            VertexNode,
        },
        stage::Stage,
        store::{
            Generation,
            GraphRead,
            HasGraph,
            HasGraphMut,
            VertexState,
            raw::{
                self,
                Seal,
            },
        },
    },
};

/// The store of a read with a tagged stage: the committed graph
/// write-locked, with its stage open.
pub struct TaggedView<'a, G: GraphKind = BaseGraphKind> {
    pub(super) graph: RwLockWriteGuard<'a, PositionedGraph<G>>,
    pub(super) stage: StageId,
}

impl<G: GraphKind> TaggedView<'_, G> {
    /// Snapshot of the labels; staged indices resolve to the label of their
    /// slot.
    #[cfg(debug_assertions)]
    pub fn name_resolver(&self) -> TaggedLabels {
        TaggedLabels(
            self.graph.vertices.iter().map(|v| v.label.clone()).collect(),
        )
    }
    /// Install [`Self::name_resolver`] for the current thread.
    #[cfg(all(debug_assertions, any(test, feature = "test-api")))]
    pub fn register_name_resolver(&self) {
        crate::graph::vertex::name_resolver::set_token_name_resolver(Box::new(
            self.name_resolver(),
        ));
    }
}

/// Labels of a tagged graph by slot, for committed and staged indices.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct TaggedLabels(Vec<String>);

#[cfg(debug_assertions)]
impl crate::graph::vertex::name_resolver::TokenNameResolver for TaggedLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.0.get(index).cloned()
    }
    fn staged_token_name(
        &self,
        slot: usize,
    ) -> Option<String> {
        self.0.get(slot).cloned()
    }
}

impl<G: GraphKind> GraphRead for TaggedView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Vertex<'a>
        = TaggedVertex<'a, G>
    where
        Self: 'a;

    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = StageIndex>,
    ) -> Option<TaggedVertex<'_, G>> {
        TaggedVertex::get(&self.graph, index.vertex_index())
    }
    fn atom(
        &self,
        value: &AtomOf<G>,
    ) -> Option<TaggedVertex<'_, G>> {
        let &index = self.graph.atoms.get(value)?;
        self.vertex(StageIndex::lift(index))
    }
    fn vertices(&self) -> impl Iterator<Item = TaggedVertex<'_, G>> + '_ {
        let graph = &*self.graph;
        graph
            .vertices
            .iter()
            .map(move |vertex| TaggedVertex { graph, vertex })
    }
    fn generation(&self) -> Generation {
        Generation {
            base: self.stage.generation,
            stage: self.graph.generation - self.stage.generation,
        }
    }
}

impl<G: GraphKind> raw::Write for TaggedView<'_, G> {
    type VertexMut<'a>
        = TaggedVertexMut<'a, G>
    where
        Self: 'a;

    fn vertex_mut(
        &mut self,
        _: Seal,
        index: StageIndex,
    ) -> Option<TaggedVertexMut<'_, G>> {
        lookup(&self.graph, index)?;
        let fixed = index.slot() < self.stage.first;
        Some(TaggedVertexMut {
            vertex: &mut self.graph.vertices[index.slot()],
            fixed,
        })
    }
    /// The index of the next composite (atoms are committed at once).
    fn next_index(
        &self,
        _: Seal,
    ) -> StageIndex {
        StageIndex::tagged(self.graph.vertices.len(), true)
    }
    fn push(
        &mut self,
        _: Seal,
        node: VertexNode<AtomOf<G>, StageIndex>,
        width: TokenWidth,
    ) -> StageIndex {
        let slot = self.graph.vertices.len();
        let (node, state) = match node {
            VertexNode::Atom(atom) =>
                (VertexNode::Atom(atom), VertexState::Committed),
            VertexNode::Composite(composition) => (
                VertexNode::Composite(composition.into_slots()),
                VertexState::Staged,
            ),
        };
        self.graph.vertices.push(PositionedVertex {
            index: VertexIndex(slot),
            key: VertexKey::default(),
            width,
            state,
            #[cfg(debug_assertions)]
            label: String::new(),
            node,
            parents: Default::default(),
        });
        StageIndex::tagged(slot, state == VertexState::Staged)
    }
    fn register_atom(
        &mut self,
        _: Seal,
        value: AtomOf<G>,
        index: StageIndex,
    ) {
        let committed = index
            .committed()
            .unwrap_or_else(|| panic!("atom {index} is staged"));
        self.graph.atoms.insert(value, committed);
    }
    fn bump_generation(
        &mut self,
        _: Seal,
    ) {
        self.graph.generation += 1;
    }
}

impl<G: GraphKind> Stage for TaggedView<'_, G> {
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_ {
        self.graph.vertices[self.stage.first..]
            .iter()
            .filter(|vertex| vertex.state == VertexState::Staged)
            .map(|vertex| StagedIndex::new(vertex.index.0))
    }
}

impl<G: GraphKind> HasGraph for TaggedView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Guard<'a>
        = &'a Self
    where
        Self: 'a;
    fn graph(&self) -> &Self {
        self
    }
}

impl<G: GraphKind> HasGraphMut for TaggedView<'_, G> {
    type GuardMut<'a>
        = &'a mut Self
    where
        Self: 'a;
    fn graph_mut(&mut self) -> &mut Self {
        self
    }
}
