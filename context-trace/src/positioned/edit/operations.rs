//! The public operations of [`GraphEdit`]: atoms, new composites, added
//! occurrences and extension of staged vertices.

#[cfg(any(debug_assertions, feature = "test-api"))]
use super::check::check_overlap_closure;
#[cfg(debug_assertions)]
use super::check::composed_label;
use super::{
    Error,
    GraphEdit,
    check::{
        check_child,
        expect_vertex,
    },
};
use crate::{
    AtomOf,
    Token,
    TokenWidth,
    graph::vertex::has_vertex_index::ToToken,
    positioned::{
        model::{
            Composition,
            Occurrence,
            PositionedError,
            VertexNode,
            maximal,
        },
        store::{
            GraphRead,
            GraphWrite,
            VertexRead,
            VertexState,
            raw::{
                SEAL,
                VertexWrite,
                Write as _,
            },
        },
    },
};

impl<'g, S: GraphWrite + ?Sized> GraphEdit<'g, S> {
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
    /// Only for staged vertices (`NotStaged`; the content of committed
    /// vertices is immutable, staging plan §2.1) without parents
    /// (`HasParents`): on the committed store `extend` is always rejected.
    /// `occurrence`
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
        let node = expect_vertex(&*self.graph, vertex)?;
        if node.state() != VertexState::Staged {
            return Err(PositionedError::NotStaged(vertex));
        }
        if node.parents().next().is_some() {
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
}
