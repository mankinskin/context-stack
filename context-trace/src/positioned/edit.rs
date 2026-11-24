//! `GraphEdit`: the only mutation layer of the positioned graph (insert
//! plan rev 3, §8, D8).
//!
//! Every operation changes one vertex plus the parent links of the children
//! it adds or removes, and checks strict preconditions first: a rejected
//! operation leaves the graph unchanged. There is no rollback of earlier
//! operations of the same edit (D10). Outside this crate the layer requires
//! the hidden feature `edit`, enabled only by `context-insert` and
//! `context-read`.
#![cfg_attr(not(feature = "edit"), allow(dead_code))]

use std::collections::{
    BTreeMap,
    BTreeSet,
};

#[cfg(debug_assertions)]
use super::check::composed_label;
#[cfg(any(debug_assertions, feature = "test-api"))]
use super::{
    access::NodeRef,
    check::{
        check_overlap_closure,
        validate_vertex,
    },
    navigate::Navigate,
};
use super::{
    access::{
        CompositionRead,
        GraphRead,
        GraphWrite,
        VertexRead,
        raw::{
            SEAL,
            VertexWrite,
            Write as _,
        },
    },
    check::{
        check_child,
        expect_vertex,
    },
    composition::{
        Composition,
        Occurrence,
        maximal,
    },
    graph::{
        PositionedError,
        PositionedGraph,
        VertexNode,
    },
};
#[cfg(any(debug_assertions, feature = "test-api"))]
use crate::graph::vertex::has_vertex_index::HasVertexIndex;
use crate::{
    AtomOf,
    GraphKind,
    Token,
    TokenWidth,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        has_vertex_index::ToToken,
        wide::Wide,
    },
};

/// Record of one edit: created tokens and changed existing vertices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication<I: VertexId = CommittedIndex> {
    /// Created tokens in creation order.
    pub created: Vec<Token<I>>,
    /// One entry per changed vertex that existed before the edit.
    pub edits: Vec<VertexEdit<I>>,
}

impl<I: VertexId> Default for Publication<I> {
    fn default() -> Self {
        Self {
            created: Vec::new(),
            edits: Vec::new(),
        }
    }
}

/// Occurrences added to and removed from the composition of `vertex`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexEdit<I: VertexId = CommittedIndex> {
    pub vertex: I,
    pub added: Vec<Occurrence<I>>,
    pub removed: Vec<Occurrence<I>>,
}

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

    /// The existing atom vertex for `value`, or a new one (a mutation).
    pub fn atom(
        &mut self,
        value: AtomOf<S::Kind>,
    ) -> Token<S::Index> {
        if let Some(atom) = self.graph.atom(&value) {
            return atom.to_token();
        }
        self.mutate();
        let index =
            self.graph
                .push(SEAL, VertexNode::Atom(value), TokenWidth(1));
        #[cfg(debug_assertions)]
        self.vertex_mut(index).set_label(value.to_string());
        self.graph.register_atom(SEAL, value, index);
        let token = Token::new(index, 1);
        self.created.push(token);
        self.touched.insert(index);
        token
    }

    /// A new composite vertex with the complete composition `occurrences`;
    /// its children gain parent links.
    ///
    /// Rejects malformed compositions ([`Composition::new`]) and unknown
    /// children or width mismatches. Debug and `test-api` builds also reject
    /// existing content (G1), overlap disagreement and overlaps that are not
    /// vertices (G4).
    pub fn create(
        &mut self,
        occurrences: Vec<Occurrence<S::Index>>,
    ) -> Result<Token<S::Index>, Error<S>> {
        let composition = Composition::new(occurrences).map_err(|error| {
            PositionedError::Composition { owner: None, error }
        })?;
        for occurrence in composition.occurrences() {
            check_child(&*self.graph, occurrence)?;
        }
        let index = self.graph.next_index(SEAL);
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.check_new_content(index, &composition)?;

        self.mutate();
        let width = composition.width();
        #[cfg(debug_assertions)]
        let label = composed_label(
            &*self.graph,
            composition.occurrences().iter().copied(),
        );
        for occurrence in composition.occurrences() {
            self.link(index, occurrence);
        }
        let pushed =
            self.graph
                .push(SEAL, VertexNode::Composite(composition), width);
        debug_assert_eq!(pushed, index);
        #[cfg(debug_assertions)]
        self.vertex_mut(index).set_label(label);
        let token = Token::new(index, width);
        self.created.push(token);
        self.touched.insert(index);
        Ok(token)
    }

    /// `comp(vertex) := maximal(comp(vertex) ∪ added)`. Occurrences contained
    /// in added ones lose their link to `vertex`.
    ///
    /// Each added occurrence must start inside `vertex` (`OutsideVertex`)
    /// and end within it (`WidthChange`). Debug and `test-api` builds also
    /// check that each added token equals `vertex` at its offset
    /// (`OverlapMismatch` against the whole vertex) and G4 for the result.
    /// `Atom` if `vertex` is an atom.
    pub fn add_occurrences(
        &mut self,
        vertex: S::Index,
        added: Vec<Occurrence<S::Index>>,
    ) -> Result<(), Error<S>> {
        let (width, old) = self.composite(vertex)?;
        for occurrence in &added {
            if occurrence.offset.0 >= width.0 {
                return Err(PositionedError::OutsideVertex {
                    vertex,
                    occurrence: *occurrence,
                });
            }
            if occurrence.end().0 > width.0 {
                return Err(PositionedError::WidthChange { vertex });
            }
            check_child(&*self.graph, occurrence)?;
        }
        let mut union = old.clone();
        union.extend(added.iter().copied());
        let composition =
            Composition::new(maximal(union)).map_err(|error| {
                PositionedError::Composition {
                    owner: Some(vertex),
                    error,
                }
            })?;
        #[cfg(any(debug_assertions, feature = "test-api"))]
        {
            let whole = Occurrence::new(0, Token::new(vertex, width));
            for occurrence in &added {
                self.check_agrees(vertex, whole, *occurrence)?;
            }
            check_overlap_closure(
                &*self.graph,
                vertex,
                composition.occurrences().iter().copied(),
            )?;
        }
        self.recompose(vertex, &old, composition);
        Ok(())
    }

    /// `comp(vertex) := maximal(comp(vertex) ∪ {occurrence})` with the width
    /// growing to `occurrence.end()`; returns the new token of `vertex`.
    ///
    /// Only for vertices without parents (`HasParents`), and `occurrence`
    /// must start at or before the end of `vertex` and reach beyond it
    /// (`OutsideVertex`). Debug and `test-api` builds also check that the
    /// occurrence agrees with `vertex` on their overlap (`OverlapMismatch`)
    /// and G4 for the result. `Atom` if `vertex` is an atom.
    pub fn extend(
        &mut self,
        vertex: S::Index,
        occurrence: Occurrence<S::Index>,
    ) -> Result<Token<S::Index>, Error<S>> {
        let (width, old) = self.composite(vertex)?;
        if expect_vertex(&*self.graph, vertex)?.parents().next().is_some() {
            return Err(PositionedError::HasParents(vertex));
        }
        if occurrence.offset.0 > width.0 || occurrence.end().0 <= width.0 {
            return Err(PositionedError::OutsideVertex { vertex, occurrence });
        }
        check_child(&*self.graph, &occurrence)?;
        let mut union = old.clone();
        union.push(occurrence);
        let composition =
            Composition::new(maximal(union)).map_err(|error| {
                PositionedError::Composition {
                    owner: Some(vertex),
                    error,
                }
            })?;
        #[cfg(any(debug_assertions, feature = "test-api"))]
        {
            let whole = Occurrence::new(0, Token::new(vertex, width));
            self.check_agrees(vertex, whole, occurrence)?;
            check_overlap_closure(
                &*self.graph,
                vertex,
                composition.occurrences().iter().copied(),
            )?;
        }
        let new_width = composition.width();
        #[cfg(debug_assertions)]
        let label = composed_label(
            &*self.graph,
            composition.occurrences().iter().copied(),
        );
        self.recompose(vertex, &old, composition);
        let mut handle = self.vertex_mut(vertex);
        handle.set_width(new_width);
        #[cfg(debug_assertions)]
        handle.set_label(label);
        Ok(Token::new(vertex, new_width))
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
