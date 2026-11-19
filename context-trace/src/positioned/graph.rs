//! Positioned graph storage, insertion and validation.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
        HashMap,
    },
    fmt,
    ops::Range,
    sync::{
        Arc,
        RwLock,
        RwLockReadGuard,
        RwLockWriteGuard,
    },
};

use super::{
    composition::{
        Composition,
        CompositionError,
        Occurrence,
    },
    location::{
        IntervalView,
        OccurrencePath,
    },
};
use crate::{
    AtomOf,
    AtomPosition,
    BaseGraphKind,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::VertexIndex,
};

/// What a vertex consists of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VertexNode<A> {
    Atom(A),
    Composite(Composition),
}

/// A retained vertex with its composition and reciprocal parent links.
#[derive(Clone, Debug)]
pub struct PositionedVertex<G: GraphKind = BaseGraphKind> {
    pub(crate) index: VertexIndex,
    pub(crate) key: VertexKey,
    pub(crate) width: TokenWidth,
    pub(crate) node: VertexNode<AtomOf<G>>,
    /// Parent vertex → offsets at which this vertex occurs in it.
    pub(crate) parents: BTreeMap<VertexIndex, BTreeSet<AtomPosition>>,
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
}

/// Insertion or validation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PositionedError {
    Composition {
        owner: Option<VertexIndex>,
        error: CompositionError,
    },
    UnknownVertex(VertexIndex),
    /// Stored or referenced width disagrees with the vertex.
    Width {
        vertex: VertexIndex,
        expected: TokenWidth,
        found: TokenWidth,
    },
    /// Overlapping children disagree on the atom at `position`.
    ContentMismatch {
        owner: Option<VertexIndex>,
        position: AtomPosition,
    },
    /// Content already represented by `existing`.
    DuplicateContent {
        existing: VertexIndex,
        duplicate: Option<VertexIndex>,
    },
    /// Occurrence `child@offset` in `parent` lacks its reciprocal link (or
    /// the link has no occurrence).
    ParentLink {
        parent: VertexIndex,
        child: VertexIndex,
        offset: AtomPosition,
    },
    /// Vertex stored at the wrong slot.
    Index {
        slot: usize,
        vertex: VertexIndex,
    },
}

impl fmt::Display for PositionedError {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let owner = |owner: &Option<VertexIndex>| {
            owner.map_or("new vertex".to_string(), |o| format!("vertex {}", o.0))
        };
        match self {
            Self::Composition { owner: o, error } => {
                write!(f, "{}: {error}", owner(o))
            },
            Self::UnknownVertex(v) => write!(f, "unknown vertex {}", v.0),
            Self::Width {
                vertex,
                expected,
                found,
            } => write!(
                f,
                "vertex {}: width {} expected, found {}",
                vertex.0, expected.0, found.0
            ),
            Self::ContentMismatch { owner: o, position } => {
                write!(f, "{}: children disagree at position {position}", owner(o))
            },
            Self::DuplicateContent {
                existing,
                duplicate,
            } => write!(
                f,
                "{} duplicates the content of vertex {}",
                owner(duplicate),
                existing.0
            ),
            Self::ParentLink {
                parent,
                child,
                offset,
            } => write!(
                f,
                "occurrence {}@{offset} in vertex {} has no reciprocal link",
                child.0, parent.0
            ),
            Self::Index { slot, vertex } => {
                write!(f, "vertex {} stored at slot {slot}", vertex.0)
            },
        }
    }
}

impl std::error::Error for PositionedError {}

/// Positioned composition graph with its own storage.
///
/// Vertices are dense (`VertexIndex(i)` is slot `i`) and never removed. The
/// only way to add a composite is [`Self::insert`], which accepts complete
/// compositions only, so gap, wrapper or slice vertices cannot be built.
/// Not internally synchronized; share it through [`PositionedGraphRef`].
/// Compare graphs with [`super::diff`], not by identity.
#[derive(Clone, Debug)]
pub struct PositionedGraph<G: GraphKind = BaseGraphKind> {
    pub(crate) vertices: Vec<PositionedVertex<G>>,
    pub(crate) by_content: HashMap<Vec<AtomOf<G>>, VertexIndex>,
}

impl<G: GraphKind> Default for PositionedGraph<G> {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            by_content: HashMap::new(),
        }
    }
}

impl<G: GraphKind> PositionedGraph<G> {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
    pub fn vertices(&self) -> impl Iterator<Item = &PositionedVertex<G>> {
        self.vertices.iter()
    }
    pub fn vertex(
        &self,
        index: VertexIndex,
    ) -> Option<&PositionedVertex<G>> {
        self.vertices.get(index.0)
    }
    fn expect(
        &self,
        index: VertexIndex,
    ) -> Result<&PositionedVertex<G>, PositionedError> {
        self.vertex(index).ok_or(PositionedError::UnknownVertex(index))
    }
    /// The vertex whose content is `content`, if any.
    pub fn find(
        &self,
        content: &[AtomOf<G>],
    ) -> Option<Token> {
        self.by_content
            .get(content)
            .map(|&index| self.vertices[index.0].token())
    }

    /// Content of a vertex, reconstructed from its composition.
    pub fn content(
        &self,
        index: VertexIndex,
    ) -> Option<Vec<AtomOf<G>>> {
        match &self.vertex(index)?.node {
            VertexNode::Atom(atom) => Some(vec![*atom]),
            VertexNode::Composite(composition) =>
                self.compose_content(composition, None).ok(),
        }
    }

    /// Overlay the children's contents at their offsets; overlapping children
    /// must agree. Coverage is guaranteed by [`Composition`].
    fn compose_content(
        &self,
        composition: &Composition,
        owner: Option<VertexIndex>,
    ) -> Result<Vec<AtomOf<G>>, PositionedError> {
        let mut content: Vec<Option<AtomOf<G>>> =
            vec![None; composition.width().0];
        for occurrence in composition.occurrences() {
            let child = self.expect(occurrence.child.index)?;
            if child.width != occurrence.child.width {
                return Err(PositionedError::Width {
                    vertex: child.index,
                    expected: child.width,
                    found: occurrence.child.width,
                });
            }
            let atoms = self
                .content(child.index)
                .ok_or(PositionedError::UnknownVertex(child.index))?;
            for (i, atom) in atoms.into_iter().enumerate() {
                let slot = &mut content[occurrence.offset.0 + i];
                match slot {
                    Some(existing) if *existing != atom =>
                        return Err(PositionedError::ContentMismatch {
                            owner,
                            position: occurrence.offset + i,
                        }),
                    _ => *slot = Some(atom),
                }
            }
        }
        Ok(content.into_iter().map(Option::unwrap).collect())
    }

    /// Insert an atom, or return the existing vertex for it.
    pub fn insert_atom(
        &mut self,
        atom: AtomOf<G>,
    ) -> Token {
        if let Some(token) = self.find(&[atom]) {
            return token;
        }
        let index = VertexIndex(self.vertices.len());
        self.vertices.push(PositionedVertex {
            index,
            key: VertexKey::default(),
            width: TokenWidth(1),
            node: VertexNode::Atom(atom),
            parents: BTreeMap::new(),
        });
        self.by_content.insert(vec![atom], index);
        Token::new(index, 1)
    }

    /// Insert a composite from its complete composition and link its children.
    ///
    /// Rejects incomplete or malformed compositions, unknown children,
    /// disagreeing overlaps and content that already has a vertex.
    pub fn insert(
        &mut self,
        occurrences: Vec<Occurrence>,
    ) -> Result<Token, PositionedError> {
        let composition = Composition::new(occurrences)
            .map_err(|error| PositionedError::Composition { owner: None, error })?;
        let content = self.compose_content(&composition, None)?;
        if let Some(&existing) = self.by_content.get(&content) {
            return Err(PositionedError::DuplicateContent {
                existing,
                duplicate: None,
            });
        }
        let index = VertexIndex(self.vertices.len());
        let width = composition.width();
        for occurrence in composition.occurrences() {
            self.vertices[occurrence.child.index.0]
                .parents
                .entry(index)
                .or_default()
                .insert(occurrence.offset);
        }
        self.vertices.push(PositionedVertex {
            index,
            key: VertexKey::default(),
            width,
            node: VertexNode::Composite(composition),
            parents: BTreeMap::new(),
        });
        self.by_content.insert(content, index);
        Ok(Token::new(index, width))
    }

    /// `range` of `owner` as a view, if it lies within the owner.
    pub fn interval(
        &self,
        owner: VertexIndex,
        range: Range<AtomPosition>,
    ) -> Option<IntervalView> {
        let width = self.vertex(owner)?.width.0;
        (range.start <= range.end && range.end.0 <= width)
            .then_some(IntervalView { owner, range })
    }

    /// Whether every step of `path` is a stored occurrence.
    pub fn contains_path(
        &self,
        path: &OccurrencePath,
    ) -> bool {
        path.steps().iter().all(|step| {
            self.vertex(step.parent)
                .and_then(PositionedVertex::composition)
                .is_some_and(|composition| {
                    composition.occurrences().iter().any(|o| {
                        o.offset == step.offset && o.child.index == step.child
                    })
                })
        })
    }

    /// Check the oracle's structural rules 1-7 (rule 8, corpus roots, belongs
    /// to the interchange snapshot), reciprocal parent links and one vertex
    /// per content.
    pub fn validate(&self) -> Result<(), PositionedError> {
        let mut seen = HashMap::new();
        for (slot, vertex) in self.vertices.iter().enumerate() {
            let index = vertex.index;
            if index.0 != slot {
                return Err(PositionedError::Index {
                    slot,
                    vertex: index,
                });
            }
            let content = match &vertex.node {
                VertexNode::Atom(atom) => {
                    if vertex.width.0 != 1 {
                        return Err(PositionedError::Width {
                            vertex: index,
                            expected: TokenWidth(1),
                            found: vertex.width,
                        });
                    }
                    vec![*atom]
                },
                VertexNode::Composite(composition) => {
                    let error = |error| PositionedError::Composition {
                        owner: Some(index),
                        error,
                    };
                    // Order is guaranteed by construction; re-check the rest.
                    Composition::new(composition.occurrences().to_vec())
                        .map_err(error)?;
                    if composition.width() != vertex.width {
                        return Err(PositionedError::Width {
                            vertex: index,
                            expected: composition.width(),
                            found: vertex.width,
                        });
                    }
                    for occurrence in composition.occurrences() {
                        let linked = self
                            .expect(occurrence.child.index)?
                            .parents
                            .get(&index)
                            .is_some_and(|offsets| offsets.contains(&occurrence.offset));
                        if !linked {
                            return Err(PositionedError::ParentLink {
                                parent: index,
                                child: occurrence.child.index,
                                offset: occurrence.offset,
                            });
                        }
                    }
                    self.compose_content(composition, Some(index))?
                },
            };
            for (&parent, offsets) in &vertex.parents {
                let composition = self.expect(parent)?.composition();
                for &offset in offsets {
                    let present = composition.is_some_and(|c| {
                        c.occurrences()
                            .iter()
                            .any(|o| o.offset == offset && o.child.index == index)
                    });
                    if !present {
                        return Err(PositionedError::ParentLink {
                            parent,
                            child: index,
                            offset,
                        });
                    }
                }
            }
            if let Some(existing) = seen.insert(content.clone(), index) {
                return Err(PositionedError::DuplicateContent {
                    existing,
                    duplicate: Some(index),
                });
            }
            if self.by_content.get(&content) != Some(&index) {
                return Err(PositionedError::DuplicateContent {
                    existing: self.by_content.get(&content).copied().unwrap_or(index),
                    duplicate: Some(index),
                });
            }
        }
        if self.by_content.len() != self.vertices.len() {
            return Err(PositionedError::Index {
                slot: self.vertices.len(),
                vertex: VertexIndex(self.by_content.len()),
            });
        }
        Ok(())
    }
}

/// Shared handle to a [`PositionedGraph`].
///
/// Thread-safety decision (Step 2): plain maps behind one `Arc<RwLock<_>>`.
/// Readers share the lock, insertion takes it exclusively. Finer-grained
/// concurrency (e.g. `DashMap` like the row model) waits until the reader
/// needs it.
#[derive(Clone, Debug, Default)]
pub struct PositionedGraphRef<G: GraphKind = BaseGraphKind>(
    Arc<RwLock<PositionedGraph<G>>>,
);

impl<G: GraphKind> PositionedGraphRef<G> {
    pub fn new(graph: PositionedGraph<G>) -> Self {
        Self(Arc::new(RwLock::new(graph)))
    }
    pub fn read(&self) -> RwLockReadGuard<'_, PositionedGraph<G>> {
        self.0.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn write(&self) -> RwLockWriteGuard<'_, PositionedGraph<G>> {
        self.0.write().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<G: GraphKind> From<PositionedGraph<G>> for PositionedGraphRef<G> {
    fn from(graph: PositionedGraph<G>) -> Self {
        Self::new(graph)
    }
}
