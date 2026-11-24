//! Stored vertices: [`VertexNode`], [`PositionedVertex`] and the debug
//! label snapshot [`PositionedLabels`].

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    fmt,
};

use super::Composition;
#[cfg(debug_assertions)]
use crate::graph::vertex::name_resolver::TokenNameResolver;
use crate::{
    AtomOf,
    AtomPosition,
    BaseGraphKind,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::{
        CommittedIndex,
        VertexId,
        VertexIndex,
    },
    positioned::store::VertexState,
};

/// What a vertex consists of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VertexNode<A, I: VertexId = CommittedIndex> {
    Atom(A),
    Composite(Composition<I>),
}

/// A retained vertex with its composition and reciprocal parent links.
#[derive(Clone, Debug)]
pub struct PositionedVertex<G: GraphKind = BaseGraphKind> {
    pub(crate) index: VertexIndex,
    pub(crate) key: VertexKey,
    pub(crate) width: TokenWidth,
    /// `Staged` only while a tagged stage is open (staging plan §5).
    pub(crate) state: VertexState,
    pub(crate) node: VertexNode<AtomOf<G>>,
    /// Parent vertex → offsets at which this vertex occurs in it.
    pub(crate) parents: BTreeMap<VertexIndex, BTreeSet<AtomPosition>>,
    /// Expanded content, for diagnostics only; never read by algorithms.
    #[cfg(debug_assertions)]
    pub(crate) label: String,
}

impl<G: GraphKind> PositionedVertex<G> {
    pub fn index(&self) -> VertexIndex {
        self.index
    }
    pub fn key(&self) -> VertexKey {
        self.key
    }
    pub fn width(&self) -> TokenWidth {
        self.width
    }
    pub fn token(&self) -> Token {
        Token::new(self.index, self.width)
    }
    pub fn state(&self) -> VertexState {
        self.state
    }
    pub fn node(&self) -> &VertexNode<AtomOf<G>> {
        &self.node
    }
    /// `None` for atoms.
    pub fn composition(&self) -> Option<&Composition> {
        match &self.node {
            VertexNode::Composite(composition) => Some(composition),
            VertexNode::Atom(_) => None,
        }
    }
    pub fn parents(&self) -> &BTreeMap<VertexIndex, BTreeSet<AtomPosition>> {
        &self.parents
    }
    /// Debug-only expanded content, for diagnostics.
    #[cfg(debug_assertions)]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Index and width; debug builds append the label.
impl<G: GraphKind> fmt::Display for PositionedVertex<G> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "#{}(w{})", self.index.0, self.width.0)?;
        #[cfg(debug_assertions)]
        write!(f, " {:?}", self.label)?;
        Ok(())
    }
}

/// Snapshot of the debug labels, resolving vertex index → label.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct PositionedLabels(pub(crate) Vec<String>);

#[cfg(debug_assertions)]
impl TokenNameResolver for PositionedLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.0.get(index).cloned()
    }
}
