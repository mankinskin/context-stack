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

use super::{
    composition::{
        Composition,
        Occurrence,
        maximal,
    },
    graph::{
        PositionedError,
        PositionedGraph,
        PositionedVertex,
        VertexNode,
    },
};
use crate::{
    AtomOf,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::VertexIndex,
};

/// Record of one edit: created tokens and changed existing vertices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Publication {
    /// Created tokens in creation order.
    pub created: Vec<Token>,
    /// One entry per changed vertex that existed before the edit.
    pub edits: Vec<VertexEdit>,
}

/// Occurrences added to and removed from the composition of `vertex`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexEdit {
    pub vertex: VertexIndex,
    pub added: Vec<Occurrence>,
    pub removed: Vec<Occurrence>,
}

impl<G: GraphKind> PositionedGraph<G> {
    /// Open an edit (see [`GraphEdit`]).
    #[cfg(feature = "edit")]
    #[doc(hidden)]
    pub fn edit(&mut self) -> GraphEdit<'_, G> {
        GraphEdit::new(self)
    }
    /// Open an edit (see [`GraphEdit`]).
    #[cfg(not(feature = "edit"))]
    pub(crate) fn edit(&mut self) -> GraphEdit<'_, G> {
        GraphEdit::new(self)
    }
    /// Bumped once by the first mutation of every edit, so a stale search
    /// result can be detected.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Vertex-local, checked mutations of a [`PositionedGraph`].
pub struct GraphEdit<'g, G: GraphKind> {
    graph: &'g mut PositionedGraph<G>,
    created: Vec<Token>,
    edits: BTreeMap<VertexIndex, VertexEdit>,
    touched: BTreeSet<VertexIndex>,
    bumped: bool,
}

impl<'g, G: GraphKind> GraphEdit<'g, G> {
    fn new(graph: &'g mut PositionedGraph<G>) -> Self {
        Self {
            graph,
            created: Vec::new(),
            edits: BTreeMap::new(),
            touched: BTreeSet::new(),
            bumped: false,
        }
    }

    /// Read access while edits are applied.
    pub fn graph(&self) -> &PositionedGraph<G> {
        self.graph
    }

    fn mutate(&mut self) {
        if !self.bumped {
            self.graph.generation += 1;
            self.bumped = true;
        }
    }

    /// The existing atom vertex for `value`, or a new one (a mutation).
    pub fn atom(
        &mut self,
        value: AtomOf<G>,
    ) -> Token {
        if let Some(&index) = self.graph.atoms.get(&value) {
            return Token::new(index, 1);
        }
        self.mutate();
        let index = VertexIndex(self.graph.vertices.len());
        self.graph.vertices.push(PositionedVertex {
            index,
            key: VertexKey::default(),
            width: TokenWidth(1),
            #[cfg(debug_assertions)]
            label: value.to_string(),
            node: VertexNode::Atom(value),
            parents: BTreeMap::new(),
        });
        self.graph.atoms.insert(value, index);
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
        occurrences: Vec<Occurrence>,
    ) -> Result<Token, PositionedError> {
        let composition = Composition::new(occurrences).map_err(|error| {
            PositionedError::Composition { owner: None, error }
        })?;
        for occurrence in composition.occurrences() {
            self.graph.check_child(occurrence)?;
        }
        let index = VertexIndex(self.graph.vertices.len());
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.check_new_content(index, &composition)?;

        self.mutate();
        let width = composition.width();
        #[cfg(debug_assertions)]
        let label = self.graph.composed_label(&composition);
        for occurrence in composition.occurrences() {
            self.link(index, occurrence);
        }
        self.graph.vertices.push(PositionedVertex {
            index,
            key: VertexKey::default(),
            width,
            #[cfg(debug_assertions)]
            label,
            node: VertexNode::Composite(composition),
            parents: BTreeMap::new(),
        });
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
    pub fn add_occurrences(
        &mut self,
        vertex: VertexIndex,
        added: Vec<Occurrence>,
    ) -> Result<(), PositionedError> {
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
            self.graph.check_child(occurrence)?;
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
            self.graph
                .check_overlap_closure(vertex, composition.occurrences())?;
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
    /// and G4 for the result.
    pub fn extend(
        &mut self,
        vertex: VertexIndex,
        occurrence: Occurrence,
    ) -> Result<Token, PositionedError> {
        let (width, old) = self.composite(vertex)?;
        if !self.graph.vertices[vertex.0].parents.is_empty() {
            return Err(PositionedError::HasParents(vertex));
        }
        if occurrence.offset.0 > width.0 || occurrence.end().0 <= width.0 {
            return Err(PositionedError::OutsideVertex { vertex, occurrence });
        }
        self.graph.check_child(&occurrence)?;
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
            self.graph
                .check_overlap_closure(vertex, composition.occurrences())?;
        }
        let new_width = composition.width();
        self.recompose(vertex, &old, composition);
        self.graph.vertices[vertex.0].width = new_width;
        #[cfg(debug_assertions)]
        {
            let label = match &self.graph.vertices[vertex.0].node {
                VertexNode::Composite(composition) =>
                    self.graph.composed_label(composition),
                VertexNode::Atom(_) => unreachable!("checked composite"),
            };
            self.graph.vertices[vertex.0].label = label;
        }
        Ok(Token::new(vertex, new_width))
    }

    /// Validate the touched vertices (debug and `test-api` builds; panics on
    /// a violation, which is a bug) and return the record.
    pub fn finish(self) -> Publication {
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

    /// Width and occurrences of a composite `vertex`.
    fn composite(
        &self,
        vertex: VertexIndex,
    ) -> Result<(TokenWidth, Vec<Occurrence>), PositionedError> {
        let node = self.graph.expect(vertex)?;
        let composition =
            node.composition().ok_or(PositionedError::Composition {
                owner: Some(vertex),
                error: super::CompositionError::TooFew,
            })?;
        Ok((node.width, composition.occurrences().to_vec()))
    }

    fn link(
        &mut self,
        parent: VertexIndex,
        occurrence: &Occurrence,
    ) {
        self.graph.vertices[occurrence.child.index.0]
            .parents
            .entry(parent)
            .or_default()
            .insert(occurrence.offset);
        self.touched.insert(occurrence.child.index);
    }

    fn unlink(
        &mut self,
        parent: VertexIndex,
        occurrence: &Occurrence,
    ) {
        let parents =
            &mut self.graph.vertices[occurrence.child.index.0].parents;
        if let Some(offsets) = parents.get_mut(&parent) {
            offsets.remove(&occurrence.offset);
            if offsets.is_empty() {
                parents.remove(&parent);
            }
        }
        self.touched.insert(occurrence.child.index);
    }

    /// Replace the composition of `vertex` by `composition`, update links of
    /// added and removed children and record the change.
    fn recompose(
        &mut self,
        vertex: VertexIndex,
        old: &[Occurrence],
        composition: Composition,
    ) {
        let new = composition.occurrences();
        let removed: Vec<Occurrence> =
            old.iter().filter(|o| !new.contains(o)).copied().collect();
        let added: Vec<Occurrence> =
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
        self.graph.vertices[vertex.0].node = VertexNode::Composite(composition);
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
        vertex: VertexIndex,
        range: std::ops::Range<usize>,
    ) -> Vec<AtomOf<G>> {
        let view = super::IntervalView {
            owner: vertex,
            range: range.start.into()..range.end.into(),
        };
        self.graph
            .read_interval(&view)
            .expect("structurally valid interval")
    }

    /// `left` and `right` (occurrences in `owner`) agree on their overlap.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn check_agrees(
        &self,
        owner: VertexIndex,
        left: Occurrence,
        right: Occurrence,
    ) -> Result<(), PositionedError> {
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
        index: VertexIndex,
        composition: &Composition,
    ) -> Result<(), PositionedError> {
        let occurrences = composition.occurrences();
        for pair in occurrences.windows(2) {
            self.check_agrees(index, pair[0], pair[1])?;
        }
        self.graph.check_overlap_closure(index, occurrences)?;
        let mut content = Vec::with_capacity(composition.width().0);
        for occurrence in occurrences {
            let from = content.len() - occurrence.offset.0;
            content.extend(self.content(
                occurrence.child.index,
                from.min(occurrence.child.width.0)..occurrence.child.width.0,
            ));
        }
        let width = composition.width();
        for vertex in self.graph.vertices.iter().filter(|v| v.width == width) {
            if self.content(vertex.index, 0..width.0) == content {
                return Err(PositionedError::DuplicateContent {
                    first: vertex.index,
                    second: index,
                });
            }
        }
        Ok(())
    }

    /// Structure and links of every touched vertex and its children, and G4
    /// for touched compositions.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn validate_touched(&self) -> Result<(), PositionedError> {
        let mut vertices = self.touched.clone();
        for &vertex in &self.touched {
            if let Some(composition) =
                self.graph.vertex(vertex).and_then(|v| v.composition())
            {
                vertices.extend(composition.children());
            }
        }
        for vertex in vertices {
            let node = self.graph.expect(vertex)?;
            self.graph.validate_vertex(vertex.0, node)?;
            if let (true, Some(composition)) =
                (self.touched.contains(&vertex), node.composition())
            {
                self.graph
                    .check_overlap_closure(vertex, composition.occurrences())?;
            }
        }
        Ok(())
    }
}
