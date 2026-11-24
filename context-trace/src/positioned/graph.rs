//! Positioned graph storage and validation; mutation lives in `super::edit`.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
        HashMap,
    },
    fmt,
    sync::{
        Arc,
        RwLock,
        RwLockReadGuard,
        RwLockWriteGuard,
    },
};

#[cfg(any(debug_assertions, test, feature = "test-api"))]
use std::ops::Range;

#[cfg(any(debug_assertions, test, feature = "test-api"))]
use super::location::IntervalView;
#[cfg(any(test, feature = "test-api"))]
use super::location::OccurrenceLocation;
use super::composition::{
    Composition,
    CompositionError,
    Occurrence,
};
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
};
#[cfg(debug_assertions)]
use crate::graph::vertex::name_resolver::TokenNameResolver;
#[cfg(all(debug_assertions, any(test, feature = "test-api")))]
use crate::graph::vertex::name_resolver::set_token_name_resolver;

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
pub struct PositionedLabels(Vec<String>);

#[cfg(debug_assertions)]
impl TokenNameResolver for PositionedLabels {
    fn token_name(
        &self,
        index: usize,
    ) -> Option<String> {
        self.0.get(index).cloned()
    }
}

/// Edit or validation failure.
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
    /// `first` and `second` denote the same content.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    DuplicateContent {
        first: VertexIndex,
        second: VertexIndex,
    },
    /// Adjacent occurrences in `owner` denote different atoms on their overlap.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    OverlapMismatch {
        owner: VertexIndex,
        left: Occurrence,
        right: Occurrence,
    },
    /// Adjacent occurrences in `owner` overlap, but their overlap is not a
    /// vertex (G4).
    #[cfg(any(debug_assertions, feature = "test-api"))]
    OverlapNotVertex {
        owner: VertexIndex,
        left: Occurrence,
        right: Occurrence,
    },
    /// The vertex has parents, so its width must not change.
    HasParents(VertexIndex),
    /// `occurrence` does not lie where the edit of `vertex` requires it.
    OutsideVertex {
        vertex: VertexIndex,
        occurrence: Occurrence,
    },
    /// The edit would change the width of `vertex`.
    WidthChange { vertex: VertexIndex },
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
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::DuplicateContent { first, second } => write!(
                f,
                "vertices {} and {} have the same content",
                first.0, second.0
            ),
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::OverlapMismatch { owner, left, right } => write!(
                f,
                "vertex {}: occurrences {}@{} and {}@{} disagree on their overlap",
                owner.0, left.child.index.0, left.offset, right.child.index.0, right.offset
            ),
            #[cfg(any(debug_assertions, feature = "test-api"))]
            Self::OverlapNotVertex { owner, left, right } => write!(
                f,
                "vertex {}: the overlap of {}@{} and {}@{} is not a vertex",
                owner.0, left.child.index.0, left.offset, right.child.index.0, right.offset
            ),
            Self::HasParents(vertex) => write!(f, "vertex {} has parents", vertex.0),
            Self::OutsideVertex { vertex, occurrence } => write!(
                f,
                "occurrence {}@{} lies outside the editable range of vertex {}",
                occurrence.child.index.0, occurrence.offset, vertex.0
            ),
            Self::WidthChange { vertex } => {
                write!(f, "edit would change the width of vertex {}", vertex.0)
            },
        }
    }
}

impl std::error::Error for PositionedError {}

/// Positioned composition graph with its own storage.
///
/// Vertices are dense (`VertexIndex(i)` is slot `i`) and never removed.
/// Vertices and compositions change only through `GraphEdit`
/// ([`Self::edit`]), whose operations accept complete compositions only, so
/// gap, wrapper or slice vertices cannot be built.
///
/// No index stores expanded content: the only value-keyed index maps atom
/// values to atom vertices. Composite vertices are reached structurally
/// (reader plan rev 4, R1/R2).
/// Not internally synchronized; share it through [`PositionedGraphRef`].
/// Compare graphs with `diff` (`test-api`), not by identity.
#[derive(Clone, Debug)]
pub struct PositionedGraph<G: GraphKind = BaseGraphKind> {
    pub(crate) vertices: Vec<PositionedVertex<G>>,
    /// Atom value → atom vertex.
    pub(crate) atoms: HashMap<AtomOf<G>, VertexIndex>,
    /// Bumped by the first mutation of every edit.
    pub(crate) generation: u64,
}

impl<G: GraphKind> Default for PositionedGraph<G> {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            atoms: HashMap::new(),
            generation: 0,
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
    pub(crate) fn expect(
        &self,
        index: VertexIndex,
    ) -> Result<&PositionedVertex<G>, PositionedError> {
        self.vertex(index).ok_or(PositionedError::UnknownVertex(index))
    }
    /// The child of `occurrence` exists and has the recorded width.
    pub(crate) fn check_child(
        &self,
        occurrence: &Occurrence,
    ) -> Result<(), PositionedError> {
        let child = self.expect(occurrence.child.index)?;
        if child.width != occurrence.child.width {
            return Err(PositionedError::Width {
                vertex: child.index,
                expected: child.width,
                found: occurrence.child.width,
            });
        }
        Ok(())
    }
    /// Children's labels joined in order, each from the first position not
    /// covered by the previous occurrences. Assumes one `char` per atom
    /// label (true for `char` atoms).
    #[cfg(debug_assertions)]
    pub(crate) fn composed_label(
        &self,
        composition: &Composition,
    ) -> String {
        let mut label = String::new();
        let mut end: usize = 0;
        for occurrence in composition.occurrences() {
            let child = &self.vertices[occurrence.child.index.0].label;
            let skip = end.saturating_sub(occurrence.offset.0);
            label.extend(child.chars().skip(skip));
            end = end.max(occurrence.end().0);
        }
        label
    }

    /// Snapshot of all debug labels as a [`TokenNameResolver`].
    #[cfg(debug_assertions)]
    pub fn name_resolver(&self) -> PositionedLabels {
        PositionedLabels(self.vertices.iter().map(|v| v.label.clone()).collect())
    }

    /// Install [`Self::name_resolver`] for the current thread, so tokens and
    /// vertex indices display their labels (when `test-api` is enabled).
    #[cfg(all(debug_assertions, any(test, feature = "test-api")))]
    pub fn register_name_resolver(&self) {
        set_token_name_resolver(Box::new(self.name_resolver()));
    }

    /// `range` of `owner` as a view, if it lies within the owner.
    #[cfg(any(debug_assertions, test, feature = "test-api"))]
    pub fn interval(
        &self,
        owner: VertexIndex,
        range: Range<AtomPosition>,
    ) -> Option<IntervalView> {
        let width = self.vertex(owner)?.width.0;
        (range.start <= range.end && range.end.0 <= width)
            .then_some(IntervalView { owner, range })
    }

    /// Whether every step is a stored occurrence.
    #[cfg(any(test, feature = "test-api"))]
    pub fn contains_path<'a>(
        &self,
        steps: impl IntoIterator<Item = &'a OccurrenceLocation>,
    ) -> bool {
        steps.into_iter().all(|step| {
            self.vertex(step.parent)
                .and_then(PositionedVertex::composition)
                .is_some_and(|composition| {
                    composition.occurrences().iter().any(|o| {
                        o.offset == step.offset && o.child.index == step.child
                    })
                })
        })
    }

    /// Structure, width and reciprocal links of the vertex stored at `slot`.
    pub(crate) fn validate_vertex(
        &self,
        slot: usize,
        vertex: &PositionedVertex<G>,
    ) -> Result<(), PositionedError> {
        let index = vertex.index;
        if index.0 != slot {
            return Err(PositionedError::Index {
                slot,
                vertex: index,
            });
        }
        match &vertex.node {
            VertexNode::Atom(_) => {
                if vertex.width.0 != 1 {
                    return Err(PositionedError::Width {
                        vertex: index,
                        expected: TokenWidth(1),
                        found: vertex.width,
                    });
                }
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
                    self.check_child(occurrence)?;
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
            },
        }
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
        Ok(())
    }

    /// Check the structural composition rules, recorded widths and reciprocal
    /// parent links.
    ///
    /// Debug and `test-api` builds additionally check content by traversal
    /// and overlap closure (G4) (see [`Self::validate_content`]); release
    /// builds never expand content.
    pub fn validate(&self) -> Result<(), PositionedError> {
        for (slot, vertex) in self.vertices.iter().enumerate() {
            self.validate_vertex(slot, vertex)?;
        }
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.validate_content()?;
        Ok(())
    }

    /// G4, structurally: for adjacent overlapping `L@l`, `R@r`, the largest
    /// token starting at `r − l` in `L` ends at `|L|`, i.e. the overlap is a
    /// vertex. Non-adjacent pairs are not checked.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    pub(crate) fn check_overlap_closure(
        &self,
        owner: VertexIndex,
        occurrences: &[Occurrence],
    ) -> Result<(), PositionedError> {
        for pair in occurrences.windows(2) {
            let (left, right) = (pair[0], pair[1]);
            if right.offset >= left.end() {
                continue;
            }
            let start = right.offset.0 - left.offset.0;
            let closed = self
                .start_at(left.child.index, start)
                .is_some_and(|(token, _)| start + token.width.0 == left.child.width.0);
            if !closed {
                return Err(PositionedError::OverlapNotVertex { owner, left, right });
            }
        }
        Ok(())
    }

    /// One vertex per content, overlap agreement and overlap closure (G4),
    /// read by traversal into
    /// a temporary set dropped on return. Assumes the structural checks of
    /// [`Self::validate`] passed.
    #[cfg(any(debug_assertions, feature = "test-api"))]
    fn validate_content(&self) -> Result<(), PositionedError> {
        let read = |owner: VertexIndex, range: Range<usize>| {
            let view = IntervalView {
                owner,
                range: range.start.into()..range.end.into(),
            };
            self.read_interval(&view).expect("structurally valid interval")
        };
        let mut seen: HashMap<Vec<AtomOf<G>>, VertexIndex> = HashMap::new();
        for vertex in &self.vertices {
            if let Some(composition) = vertex.composition() {
                // Ends increase strictly, so an occurrence overlapping a
                // non-adjacent one also covers that overlap; checking adjacent
                // pairs implies agreement of all pairs.
                for pair in composition.occurrences().windows(2) {
                    let (left, right) = (pair[0], pair[1]);
                    let Some(overlap) = left.end().0.checked_sub(right.offset.0) else {
                        continue;
                    };
                    let width = left.child.width.0;
                    if read(left.child.index, width - overlap..width)
                        != read(right.child.index, 0..overlap)
                    {
                        return Err(PositionedError::OverlapMismatch {
                            owner: vertex.index,
                            left,
                            right,
                        });
                    }
                }
                self.check_overlap_closure(vertex.index, composition.occurrences())?;
            }
            let content = read(vertex.index, 0..vertex.width.0);
            if content.len() != vertex.width.0 {
                return Err(PositionedError::Width {
                    vertex: vertex.index,
                    expected: TokenWidth(content.len()),
                    found: vertex.width,
                });
            }
            if let Some(&first) = seen.get(&content) {
                return Err(PositionedError::DuplicateContent {
                    first,
                    second: vertex.index,
                });
            }
            seen.insert(content, vertex.index);
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
