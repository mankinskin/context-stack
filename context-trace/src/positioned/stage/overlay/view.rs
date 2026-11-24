//! [`OverlayView`]: the store of a read with an overlay stage.

use std::{
    cell::RefMut,
    sync::RwLockReadGuard,
};

use either::Either;

use super::{
    OverlayVertex,
    OverlayVertexMut,
    StagedVertex,
    StagingGraph,
    staging::Links,
    vertex::VertexMut,
};
use crate::{
    AtomOf,
    BaseGraphKind,
    GraphKind,
    TokenWidth,
    graph::vertex::{
        StageIndex,
        StagedIndex,
        VertexId,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
    },
    positioned::{
        committed::PositionedGraph,
        model::VertexNode,
        stage::Stage,
        store::{
            Generation,
            GraphRead,
            HasGraph,
            HasGraphMut,
            raw::{
                self,
                Seal,
            },
        },
    },
};

/// The store of a read with an overlay stage: the base read-locked (also
/// for writes, which go into the stage) and the stage.
pub struct OverlayView<'a, G: GraphKind = BaseGraphKind> {
    pub(super) base: RwLockReadGuard<'a, PositionedGraph<G>>,
    pub(super) stage: RefMut<'a, StagingGraph<G>>,
}

impl<G: GraphKind> OverlayView<'_, G> {
    /// Snapshot of the labels of base and stage (staged indices resolve
    /// through [`TokenNameResolver::staged_token_name`]).
    ///
    /// [`TokenNameResolver::staged_token_name`]: crate::graph::vertex::name_resolver::TokenNameResolver::staged_token_name
    #[cfg(debug_assertions)]
    pub fn name_resolver(&self) -> OverlayLabels {
        OverlayLabels {
            base: self.base.vertices.iter().map(|v| v.label.clone()).collect(),
            staged: self
                .stage
                .vertices
                .iter()
                .map(|v| v.label.clone())
                .collect(),
        }
    }
    /// Install [`Self::name_resolver`] for the current thread. Staged
    /// labels change at commit: reinstall the base's resolver afterwards.
    #[cfg(all(debug_assertions, any(test, feature = "test-api")))]
    pub fn register_name_resolver(&self) {
        crate::graph::vertex::name_resolver::set_token_name_resolver(Box::new(
            self.name_resolver(),
        ));
    }
}

/// Labels of an overlay, resolving committed and staged indices.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct OverlayLabels {
    base: Vec<String>,
    staged: Vec<String>,
}

#[cfg(debug_assertions)]
impl crate::graph::vertex::name_resolver::TokenNameResolver for OverlayLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.base.get(index).cloned()
    }
    fn staged_token_name(
        &self,
        slot: usize,
    ) -> Option<String> {
        self.staged.get(slot).cloned()
    }
}

impl<G: GraphKind> GraphRead for OverlayView<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    type Vertex<'a>
        = OverlayVertex<'a, G>
    where
        Self: 'a;

    fn vertex(
        &self,
        index: impl HasVertexIndex<Index = StageIndex>,
    ) -> Option<OverlayVertex<'_, G>> {
        OverlayVertex::get(self, index.vertex_index())
    }
    fn atom(
        &self,
        value: &AtomOf<G>,
    ) -> Option<OverlayVertex<'_, G>> {
        let index = match self.stage.atoms.get(value) {
            Some(&staged) => staged.into(),
            None => StageIndex::from(*self.base.atoms.get(value)?),
        };
        self.vertex(index)
    }
    fn vertices(&self) -> impl Iterator<Item = OverlayVertex<'_, G>> + '_ {
        (0..self.base.vertices.len())
            .map(|slot| StageIndex::lift(VertexIndex(slot)))
            .chain(
                (0..self.stage.vertices.len())
                    .map(|slot| StagedIndex::new(slot).into()),
            )
            .map(move |index| OverlayVertex { view: self, index })
    }
    fn generation(&self) -> Generation {
        Generation {
            base: self.stage.base_generation,
            stage: self.stage.generation,
        }
    }
}

impl<G: GraphKind> raw::Write for OverlayView<'_, G> {
    type VertexMut<'a>
        = OverlayVertexMut<'a, G>
    where
        Self: 'a;

    fn vertex_mut(
        &mut self,
        _: Seal,
        index: StageIndex,
    ) -> Option<OverlayVertexMut<'_, G>> {
        let stage = &mut *self.stage;
        match index.split() {
            Either::Left(committed) =>
                Some(OverlayVertexMut(VertexMut::Base {
                    vertex: self.base.vertices.get(committed.0)?,
                    deltas: &mut stage.deltas,
                })),
            Either::Right(staged) => stage
                .vertices
                .get_mut(staged.slot())
                .map(|vertex| OverlayVertexMut(VertexMut::Staged(vertex))),
        }
    }
    fn next_index(
        &self,
        _: Seal,
    ) -> StageIndex {
        StagedIndex::new(self.stage.vertices.len()).into()
    }
    fn push(
        &mut self,
        _: Seal,
        node: VertexNode<AtomOf<G>, StageIndex>,
        width: TokenWidth,
    ) -> StageIndex {
        let index = StagedIndex::new(self.stage.vertices.len());
        self.stage.vertices.push(StagedVertex {
            width,
            node,
            parents: Links::new(),
            #[cfg(debug_assertions)]
            label: String::new(),
        });
        index.into()
    }
    fn register_atom(
        &mut self,
        _: Seal,
        value: AtomOf<G>,
        index: StageIndex,
    ) {
        let staged = index
            .staged()
            .unwrap_or_else(|| panic!("atom {index} is committed already"));
        self.stage.atoms.insert(value, staged);
    }
    fn bump_generation(
        &mut self,
        _: Seal,
    ) {
        self.stage.generation += 1;
    }
}

impl<G: GraphKind> Stage for OverlayView<'_, G> {
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_ {
        (0..self.stage.vertices.len()).map(StagedIndex::new)
    }
}

impl<G: GraphKind> HasGraph for OverlayView<'_, G> {
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

impl<G: GraphKind> HasGraphMut for OverlayView<'_, G> {
    type GuardMut<'a>
        = &'a mut Self
    where
        Self: 'a;
    fn graph_mut(&mut self) -> &mut Self {
        self
    }
}
