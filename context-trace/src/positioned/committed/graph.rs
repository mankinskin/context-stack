//! Positioned graph storage and validation; mutation lives in
//! `crate::positioned::edit`.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        RwLock,
        RwLockReadGuard,
        RwLockWriteGuard,
    },
};

#[cfg(any(debug_assertions, feature = "test-api"))]
use std::ops::Range;

#[cfg(any(debug_assertions, feature = "test-api"))]
use crate::{
    TokenWidth,
    positioned::{
        edit::check::check_overlap_closure,
        store::Navigate,
    },
};
#[cfg(debug_assertions)]
use crate::positioned::model::PositionedLabels;
use crate::{
    AtomOf,
    BaseGraphKind,
    GraphKind,
    graph::vertex::{
        CommittedIndex,
        VertexIndex,
        has_vertex_index::HasVertexIndex,
    },
    positioned::{
        edit::check::validate_vertex,
        model::{
            PositionedError,
            PositionedVertex,
        },
        store::GraphRead,
    },
};
#[cfg(all(debug_assertions, any(test, feature = "test-api")))]
use crate::graph::vertex::name_resolver::set_token_name_resolver;

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
    /// The stored vertex data at `index` (storage access; algorithms use
    /// the vertex objects of [`GraphRead::vertex`]).
    pub fn vertex_data(
        &self,
        index: impl HasVertexIndex<Index = CommittedIndex>,
    ) -> Option<&PositionedVertex<G>> {
        self.vertices.get(index.vertex_index().0)
    }

    /// Snapshot of all debug labels as a
    /// [`TokenNameResolver`](crate::graph::vertex::name_resolver::TokenNameResolver).
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

    /// Check the structural composition rules, recorded widths and reciprocal
    /// parent links.
    ///
    /// Debug and `test-api` builds additionally check content by traversal
    /// and overlap closure (G4) (see [`Self::validate_content`]); release
    /// builds never expand content.
    pub fn validate(&self) -> Result<(), PositionedError> {
        for (slot, vertex) in self.vertices().enumerate() {
            let index = vertex.vertex_index();
            if index.0 != slot {
                return Err(PositionedError::Index {
                    slot,
                    vertex: index,
                });
            }
            validate_vertex(vertex)?;
        }
        #[cfg(any(debug_assertions, feature = "test-api"))]
        self.validate_content()?;
        Ok(())
    }

    /// Retention invariant G6 (dissolution plan §3.1): every composite not
    /// in `inputs` has at least two parent occurrences, else `ParentCount`.
    /// Separate from [`Self::validate`]: insertion outside a read does not
    /// keep G6.
    #[cfg(any(test, feature = "test-api"))]
    pub fn validate_retention(
        &self,
        inputs: impl IntoIterator<Item = CommittedIndex>,
    ) -> Result<(), PositionedError> {
        use crate::positioned::store::VertexRead;
        let inputs: std::collections::HashSet<CommittedIndex> =
            inputs.into_iter().collect();
        for vertex in self.vertices() {
            let index = vertex.vertex_index();
            if vertex.node().composite().is_none() || inputs.contains(&index) {
                continue;
            }
            let count = vertex.parent_count();
            if count < 2 {
                return Err(PositionedError::ParentCount {
                    vertex: index,
                    count,
                });
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
            self.vertex(owner)
                .and_then(|v| v.read_interval(range.start.into()..range.end.into()))
                .expect("structurally valid interval")
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
                check_overlap_closure(
                    self,
                    vertex.index,
                    composition.occurrences().iter().copied(),
                )?;
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
