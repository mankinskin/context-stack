//! Vertex objects of an overlay: [`OverlayVertex`] (read) and
//! [`OverlayVertexMut`] (write).

use std::{
    collections::BTreeSet,
    fmt,
};

use either::Either;

use super::{
    Lifted,
    OverlayView,
    StagedVertex,
    staging::{
        BaseDelta,
        StageMap,
        has_link,
        insert_link,
        remove_link,
    },
};
use crate::{
    AtomOf,
    AtomPosition,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
        StageIndex,
        VertexId,
        has_vertex_index::HasVertexIndex,
        wide::Wide,
    },
    positioned::{
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

/// The stored vertex behind an overlay index.
enum Entry<'a, G: GraphKind> {
    Base(&'a PositionedVertex<G>),
    Staged(&'a StagedVertex<G>),
}

/// Vertex object of an [`OverlayView`]: the view and a known index (two
/// words, as the committed vertex object; the stored vertex is looked up
/// per access, which is cheaper than copying it around).
pub struct OverlayVertex<'a, G: GraphKind> {
    pub(super) view: &'a OverlayView<'a, G>,
    pub(super) index: StageIndex,
}

impl<G: GraphKind> Clone for OverlayVertex<'_, G> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<G: GraphKind> Copy for OverlayVertex<'_, G> {}

impl<'a, G: GraphKind> OverlayVertex<'a, G> {
    #[inline(always)]
    pub(super) fn get(
        view: &'a OverlayView<'a, G>,
        index: StageIndex,
    ) -> Option<Self> {
        let known = match index.split() {
            Either::Left(committed) => committed.0 < view.base.vertices.len(),
            Either::Right(staged) => view
                .stage
                .vertices
                .get(staged.slot())
                .is_some_and(Option::is_some),
        };
        known.then_some(Self { view, index })
    }
    #[inline(always)]
    fn entry(self) -> Entry<'a, G> {
        match self.index.split() {
            Either::Left(committed) =>
                Entry::Base(&self.view.base.vertices[committed.0]),
            Either::Right(staged) =>
                Entry::Staged(self.view.stage.staged(staged.slot())),
        }
    }
    /// The changes of this committed vertex, if any.
    #[inline(always)]
    fn delta(
        self,
        vertex: &PositionedVertex<G>,
    ) -> Option<&'a BaseDelta> {
        let deltas = &self.view.stage.deltas;
        if deltas.is_empty() {
            None
        } else {
            deltas.get(&vertex.index)
        }
    }
    fn width_of(
        self,
        index: StageIndex,
    ) -> TokenWidth {
        match index.split() {
            Either::Left(committed) =>
                self.view.base.vertices[committed.0].width,
            Either::Right(staged) =>
                self.view.stage.staged(staged.slot()).width,
        }
    }
}

impl<G: GraphKind> fmt::Debug for OverlayVertex<'_, G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "#{}(w{})", self.index, self.width().0)?;
        #[cfg(debug_assertions)]
        write!(f, " {:?}", self.label())?;
        Ok(())
    }
}

impl<G: GraphKind> HasVertexIndex for OverlayVertex<'_, G> {
    type Index = StageIndex;
    fn vertex_index(&self) -> StageIndex {
        self.index
    }
}

impl<G: GraphKind> Wide for OverlayVertex<'_, G> {
    fn width(&self) -> TokenWidth {
        match self.entry() {
            Entry::Base(vertex) => vertex.width,
            Entry::Staged(vertex) => vertex.width,
        }
    }
}

impl<'a, G: GraphKind> VertexRead<'a> for OverlayVertex<'a, G> {
    type Kind = G;
    type Composition = Lifted<'a>;

    #[inline(always)]
    fn node(self) -> NodeRef<'a, AtomOf<G>, Lifted<'a>> {
        match self.entry() {
            Entry::Base(vertex) => match &vertex.node {
                VertexNode::Atom(atom) => NodeRef::Atom(atom),
                VertexNode::Composite(composition) => NodeRef::Composite(
                    match self
                        .delta(vertex)
                        .and_then(|delta| delta.composition.as_ref())
                    {
                        Some(changed) => Lifted::stage(changed),
                        None => Lifted::base(composition),
                    },
                ),
            },
            Entry::Staged(vertex) => match &vertex.node {
                VertexNode::Atom(atom) => NodeRef::Atom(atom),
                VertexNode::Composite(composition) =>
                    NodeRef::Composite(Lifted::stage(composition)),
            },
        }
    }
    fn parents(
        self
    ) -> impl Iterator<Item = (Token<StageIndex>, AtomPosition)> {
        let token =
            move |parent: StageIndex| Token::new(parent, self.width_of(parent));
        let flatten = move |(&parent, offsets): (
            &StageIndex,
            &'a BTreeSet<AtomPosition>,
        )| {
            offsets.iter().map(move |&offset| (token(parent), offset))
        };
        match self.entry() {
            Entry::Base(vertex) => {
                let delta = self.delta(vertex);
                let removed = delta.map(|delta| &delta.removed);
                let base = vertex.parents.iter().flat_map(
                    move |(&parent, offsets)| {
                        offsets
                            .iter()
                            .filter(move |&&offset| {
                                !removed.is_some_and(|removed| {
                                    has_link(removed, &parent, offset)
                                })
                            })
                            .map(move |&offset| {
                                (token(StageIndex::lift(parent)), offset)
                            })
                    },
                );
                let added = delta.into_iter().flat_map(move |delta| {
                    delta.added.iter().flat_map(flatten)
                });
                Either::Left(base.chain(added))
            },
            Entry::Staged(vertex) =>
                Either::Right(vertex.parents.iter().flat_map(flatten)),
        }
    }
    fn parent_count(self) -> usize {
        match self.entry() {
            Entry::Base(vertex) if self.delta(vertex).is_none() =>
                vertex.parents.values().map(BTreeSet::len).sum(),
            Entry::Staged(vertex) =>
                vertex.parents.values().map(BTreeSet::len).sum(),
            Entry::Base(_) => self.parents().count(),
        }
    }
    fn is_parent_at(
        self,
        parent: StageIndex,
        offset: AtomPosition,
    ) -> bool {
        match self.entry() {
            Entry::Base(vertex) => {
                let delta = self.delta(vertex);
                let in_base = parent.committed().is_some_and(|committed| {
                    has_link(&vertex.parents, &committed, offset)
                        && !delta.is_some_and(|delta| {
                            has_link(&delta.removed, &committed, offset)
                        })
                });
                in_base
                    || delta.is_some_and(|delta| {
                        has_link(&delta.added, &parent, offset)
                    })
            },
            Entry::Staged(vertex) => has_link(&vertex.parents, &parent, offset),
        }
    }
    fn state(self) -> VertexState {
        if self.index.is_staged() {
            VertexState::Staged
        } else {
            VertexState::Committed
        }
    }
    fn resolve(
        self,
        index: StageIndex,
    ) -> Option<Self> {
        Self::get(self.view, index)
    }
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str {
        match self.entry() {
            Entry::Base(vertex) => &vertex.label,
            Entry::Staged(vertex) => &vertex.label,
        }
    }
}

/// Write handle of one overlay vertex: staged vertices are written
/// directly, committed ones through their delta (the base is never
/// touched before the commit).
pub struct OverlayVertexMut<'a, G: GraphKind>(pub(super) VertexMut<'a, G>);

pub(super) enum VertexMut<'a, G: GraphKind> {
    Base {
        vertex: &'a PositionedVertex<G>,
        deltas: &'a mut StageMap<CommittedIndex, BaseDelta>,
    },
    Staged(&'a mut StagedVertex<G>),
}

impl<G: GraphKind> VertexMut<'_, G> {
    fn delta<'d>(
        deltas: &'d mut StageMap<CommittedIndex, BaseDelta>,
        vertex: &PositionedVertex<G>,
    ) -> &'d mut BaseDelta {
        deltas.entry(vertex.index).or_default()
    }
}

impl<G: GraphKind> raw::VertexWrite for OverlayVertexMut<'_, G> {
    type Kind = G;
    type Index = StageIndex;
    fn set_node(
        &mut self,
        node: VertexNode<AtomOf<G>, StageIndex>,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let VertexNode::Composite(composition) = node else {
                    panic!(
                        "committed vertex {} cannot become an atom",
                        vertex.index
                    );
                };
                VertexMut::delta(deltas, vertex).composition =
                    Some(composition);
            },
            VertexMut::Staged(vertex) => vertex.node = node,
        }
    }
    fn set_width(
        &mut self,
        width: TokenWidth,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, .. } => assert_eq!(
                vertex.width, width,
                "the width of committed vertex {} is immutable",
                vertex.index
            ),
            VertexMut::Staged(vertex) => vertex.width = width,
        }
    }
    fn link_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let delta = VertexMut::delta(deltas, vertex);
                let in_base = parent.committed().is_some_and(|committed| {
                    remove_link(&mut delta.removed, &committed, offset)
                        || has_link(&vertex.parents, &committed, offset)
                });
                if !in_base {
                    insert_link(&mut delta.added, parent, offset);
                }
            },
            VertexMut::Staged(vertex) =>
                insert_link(&mut vertex.parents, parent, offset),
        }
    }
    fn unlink_parent(
        &mut self,
        parent: StageIndex,
        offset: AtomPosition,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, deltas } => {
                let delta = VertexMut::delta(deltas, vertex);
                if remove_link(&mut delta.added, &parent, offset) {
                    return;
                }
                if let Some(committed) = parent.committed()
                    && has_link(&vertex.parents, &committed, offset)
                {
                    insert_link(&mut delta.removed, committed, offset);
                }
            },
            VertexMut::Staged(vertex) => {
                remove_link(&mut vertex.parents, &parent, offset);
            },
        }
    }
    /// Labels of committed vertices are fixed with their content.
    #[cfg(debug_assertions)]
    fn set_label(
        &mut self,
        label: String,
    ) {
        match &mut self.0 {
            VertexMut::Base { vertex, .. } =>
                debug_assert_eq!(vertex.label, label),
            VertexMut::Staged(vertex) => vertex.label = label,
        }
    }
}
