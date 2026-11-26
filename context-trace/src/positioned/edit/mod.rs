//! `GraphEdit`: the only mutation layer of the positioned graph (insert
//! plan rev 3, §8, D8).
//!
//! Every operation changes one vertex plus the parent links of the children
//! it adds or removes, and checks strict preconditions first: a rejected
//! operation leaves the graph unchanged. There is no rollback of earlier
//! operations of the same edit (D10). Outside this crate the layer requires
//! the hidden feature `edit`, enabled only by `context-insert` and
//! `context-read`.
//!
//! The edit itself and its bookkeeping live here, the public operations in
//! `operations.rs`, dissolution of staged vertices in `dissolve.rs`, early
//! stabilization by fixed parent occurrences in `stable.rs`, the
//! [`Publication`] record in `publication.rs` and the store-generic checks
//! shared with `PositionedGraph::validate` in `check.rs`.
#![cfg_attr(not(feature = "edit"), allow(dead_code))]

pub(crate) mod check;
mod dissolve;
mod operations;
mod publication;
mod stable;

pub use self::{
    dissolve::{
        Candidates,
        Removal,
        Settled,
    },
    publication::{
        Publication,
        VertexEdit,
    },
};

use std::collections::{
    BTreeMap,
    BTreeSet,
};

use self::check::expect_vertex;
#[cfg(any(debug_assertions, feature = "test-api"))]
use self::check::{
    check_overlap_closure,
    validate_vertex,
};
#[cfg(any(debug_assertions, feature = "test-api"))]
use crate::{
    AtomOf,
    graph::vertex::has_vertex_index::HasVertexIndex,
    positioned::store::{
        Navigate,
        NodeRef,
    },
};
use crate::{
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::wide::Wide,
    positioned::{
        committed::PositionedGraph,
        model::{
            Composition,
            Occurrence,
            PositionedError,
            VertexNode,
        },
        store::{
            CompositionRead,
            GraphRead,
            GraphWrite,
            VertexRead,
            raw::{
                SEAL,
                VertexWrite,
            },
        },
    },
};

impl<G: GraphKind> PositionedGraph<G> {
    /// Open an edit (see [`GraphEdit`]).
    #[cfg(feature = "edit")]
    #[doc(hidden)]
    pub fn edit(&mut self) -> GraphEdit<'_, Self> {
        GraphEdit::new(self)
    }
    /// Open an edit (see [`GraphEdit`]).
    #[cfg(not(feature = "edit"))]
    pub(crate) fn edit(&mut self) -> GraphEdit<'_, Self> {
        GraphEdit::new(self)
    }
}

/// Vertex-local, checked mutations of a store, written once for every
/// [`GraphWrite`] store.
pub struct GraphEdit<'g, S: GraphWrite + ?Sized> {
    graph: &'g mut S,
    created: Vec<Token<S::Index>>,
    edits: BTreeMap<S::Index, VertexEdit<S::Index>>,
    touched: BTreeSet<S::Index>,
    bumped: bool,
}

type Error<S> = PositionedError<<S as GraphRead>::Index>;

impl<'g, S: GraphWrite + ?Sized> GraphEdit<'g, S> {
    /// Open an edit on `graph`.
    #[cfg(feature = "edit")]
    #[doc(hidden)]
    pub fn new(graph: &'g mut S) -> Self {
        Self::open(graph)
    }
    /// Open an edit on `graph`.
    #[cfg(not(feature = "edit"))]
    pub(crate) fn new(graph: &'g mut S) -> Self {
        Self::open(graph)
    }
    fn open(graph: &'g mut S) -> Self {
        Self {
            graph,
            created: Vec::new(),
            edits: BTreeMap::new(),
            touched: BTreeSet::new(),
            bumped: false,
        }
    }

    /// Read access while edits are applied.
    pub fn graph(&self) -> &S {
        self.graph
    }

    fn mutate(&mut self) {
        if !self.bumped {
            self.graph.bump_generation(SEAL);
            self.bumped = true;
        }
    }

    fn vertex_mut(
        &mut self,
        index: S::Index,
    ) -> S::VertexMut<'_> {
        self.graph.vertex_mut(SEAL, index).expect("checked vertex")
    }

    /// Validate the touched vertices (debug and `test-api` builds; panics on
    /// a violation, which is a bug) and return the record.
    pub fn finish(self) -> Publication<S::Index> {
        #[cfg(any(debug_assertions, feature = "test-api"))]
        if let Err(error) = self.validate_touched() {
            panic!("GraphEdit::finish: {error}");
        }
        Publication {
            created: self.created,
            edits: self
                .edits
                .into_values()
                .filter(|edit| {
                    !edit.added.is_empty() || !edit.removed.is_empty()
                })
                .collect(),
        }
    }

    /// Width and occurrences of a composite `vertex`; `Atom` for an atom.
    fn composite(
        &self,
        vertex: S::Index,
    ) -> Result<(TokenWidth, Vec<Occurrence<S::Index>>), Error<S>> {
        let node = expect_vertex(&*self.graph, vertex)?;
        let composition = node
            .node()
            .composite()
            .ok_or(PositionedError::Atom(vertex))?;
        Ok((node.width(), composition.occurrences().collect()))
    }

    fn link(
        &mut self,
        parent: S::Index,
        occurrence: &Occurrence<S::Index>,
    ) {
        self.vertex_mut(occurrence.child.index)
            .link_parent(parent, occurrence.offset);
        self.touched.insert(occurrence.child.index);
    }

    fn unlink(
        &mut self,
        parent: S::Index,
        occurrence: &Occurrence<S::Index>,
    ) {
        self.vertex_mut(occurrence.child.index)
            .unlink_parent(parent, occurrence.offset);
        self.touched.insert(occurrence.child.index);
    }

    /// Replace the composition of `vertex` by `composition`, update links of
    /// added and removed children and record the change.
    fn recompose(
        &mut self,
        vertex: S::Index,
        old: &[Occurrence<S::Index>],
        composition: Composition<S::Index>,
    ) {
        let new = composition.occurrences();
        let removed: Vec<Occurrence<S::Index>> =
            old.iter().filter(|o| !new.contains(o)).copied().collect();
        let added: Vec<Occurrence<S::Index>> =
            new.iter().filter(|o| !old.contains(o)).copied().collect();
        if added.is_empty() && removed.is_empty() {
            return;
        }
        self.mutate();
        for occurrence in &removed {
            self.unlink(vertex, occurrence);
        }
        for occurrence in &added {
            self.link(vertex, occurrence);
        }
        self.vertex_mut(vertex)
            .set_node(VertexNode::Composite(composition));
        self.touched.insert(vertex);
        if self.created.iter().any(|token| token.index == vertex) {
            return;
        }
        let edit = self.edits.entry(vertex).or_insert_with(|| VertexEdit {
            vertex,
            added: Vec::new(),
            removed: Vec::new(),
        });
        for occurrence in removed {
            match edit.added.iter().position(|o| *o == occurrence) {
                Some(i) => {
                    edit.added.remove(i);
                },
                None => edit.removed.push(occurrence),
            }
        }
        for occurrence in added {
            match edit.removed.iter().position(|o| *o == occurrence) {
                Some(i) => {
                    edit.removed.remove(i);
                },
                None => edit.added.push(occurrence),
            }
        }
    }

    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn content(
        &self,
        vertex: S::Index,
        range: std::ops::Range<usize>,
    ) -> Vec<AtomOf<S::Kind>> {
        self.graph
            .vertex(vertex)
            .and_then(|v| v.read_interval(range.start.into()..range.end.into()))
            .expect("structurally valid interval")
    }

    /// `left` and `right` (occurrences in `owner`) agree on their overlap.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn check_agrees(
        &self,
        owner: S::Index,
        left: Occurrence<S::Index>,
        right: Occurrence<S::Index>,
    ) -> Result<(), Error<S>> {
        let start = left.offset.0.max(right.offset.0);
        let end = left.end().0.min(right.end().0);
        if start < end
            && self.content(
                left.child.index,
                start - left.offset.0..end - left.offset.0,
            ) != self.content(
                right.child.index,
                start - right.offset.0..end - right.offset.0,
            )
        {
            return Err(PositionedError::OverlapMismatch {
                owner,
                left,
                right,
            });
        }
        Ok(())
    }

    /// Overlap agreement, overlap closure (G4) and new content (G1) of a
    /// composition about to be created as `index`.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn check_new_content(
        &self,
        index: S::Index,
        composition: &Composition<S::Index>,
    ) -> Result<(), Error<S>> {
        let occurrences = composition.occurrences();
        for pair in occurrences.windows(2) {
            self.check_agrees(index, pair[0], pair[1])?;
        }
        check_overlap_closure(
            &*self.graph,
            index,
            occurrences.iter().copied(),
        )?;
        let mut content = Vec::with_capacity(composition.width().0);
        for occurrence in occurrences {
            let from = content.len() - occurrence.offset.0;
            content.extend(self.content(
                occurrence.child.index,
                from.min(occurrence.child.width.0)..occurrence.child.width.0,
            ));
        }
        let width = composition.width();
        for vertex in self.graph.vertices().filter(|v| v.width() == width) {
            if self.content(vertex.vertex_index(), 0..width.0) == content {
                return Err(PositionedError::DuplicateContent {
                    first: vertex.vertex_index(),
                    second: index,
                });
            }
        }
        Ok(())
    }

    /// Structure and links of every touched vertex and its children, and G4
    /// for touched compositions.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn validate_touched(&self) -> Result<(), Error<S>> {
        let mut vertices = self.touched.clone();
        for &vertex in &self.touched {
            if let Some(composition) =
                self.graph.vertex(vertex).and_then(|v| v.node().composite())
            {
                vertices
                    .extend(composition.occurrences().map(|o| o.child.index));
            }
        }
        for vertex in vertices {
            let node = expect_vertex(&*self.graph, vertex)?;
            validate_vertex(node)?;
            if let (true, NodeRef::Composite(composition)) =
                (self.touched.contains(&vertex), node.node())
            {
                check_overlap_closure(
                    &*self.graph,
                    vertex,
                    composition.occurrences(),
                )?;
            }
        }
        Ok(())
    }
}
