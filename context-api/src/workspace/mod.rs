//! Workspace module — owns the in-memory hypergraph, metadata, and file lock.
//!
//! A `Workspace` is the central unit of work in the context-api. It holds:
//! - The `HypergraphRef<BaseGraphKind>` (shared reference with interior mutability)
//! - `WorkspaceMetadata` (name, timestamps, description)
//! - An optional `WorkspaceLock` (exclusive write lock on disk)
//! - A dirty flag tracking unsaved modifications

pub mod manager;
pub mod metadata;
pub mod persistence;

use std::path::PathBuf;

use context_trace::graph::{
    Hypergraph,
    HypergraphRef,
    kind::BaseGraphKind,
};

use crate::types::WorkspaceInfo;

use self::{
    metadata::WorkspaceMetadata,
    persistence::WorkspaceLock,
};

/// An open workspace with its hypergraph loaded in memory.
///
/// The workspace stores a `HypergraphRef` (which is `Arc<Hypergraph>` with
/// interior mutability via `DashMap`). This allows the search, insert, and
/// read algorithm crates to operate directly on a shared reference without
/// requiring exclusive ownership. Mutations go through `&self` methods on
/// `Hypergraph` using per-vertex locks.
#[derive(Debug)]
pub struct Workspace {
    /// Workspace name (matches the directory name under `.context-engine/`).
    pub(crate) name: String,

    /// Absolute path to the workspace directory on disk.
    pub(crate) dir: PathBuf,

    /// The in-memory hypergraph (shared, interior-mutable).
    pub(crate) graph: HypergraphRef<BaseGraphKind>,

    /// Human-readable metadata (timestamps, description).
    pub(crate) metadata: WorkspaceMetadata,

    /// Exclusive file lock held while the workspace is open.
    /// `Some` = write lock acquired, `None` = opened without locking (e.g. tests).
    /// The lock is held for its RAII side-effect (released on drop).
    #[allow(dead_code)]
    pub(crate) lock: Option<WorkspaceLock>,

    /// Whether the graph has been mutated since the last save.
    pub(crate) dirty: bool,
}

impl Workspace {
    /// Get an immutable reference to the underlying hypergraph.
    ///
    /// Since `HypergraphRef` derefs to `&Hypergraph`, this provides full
    /// read access to the graph. Due to interior mutability, even `&Hypergraph`
    /// supports mutation operations (insert_atom, insert_pattern, etc.)
    /// through DashMap's per-vertex locks.
    pub fn graph(&self) -> &Hypergraph<BaseGraphKind> {
        &self.graph
    }

    /// Get a clone of the `HypergraphRef` for use with algorithm crates.
    ///
    /// The search, insert, and read crates expect `HypergraphRef` (or types
    /// that deref to `&Hypergraph`). Cloning is cheap (Arc clone).
    pub fn graph_ref(&self) -> HypergraphRef<BaseGraphKind> {
        self.graph.clone()
    }

    /// Get a reference to the `HypergraphRef` without cloning.
    pub fn graph_ref_borrow(&self) -> &HypergraphRef<BaseGraphKind> {
        &self.graph
    }

    /// Mark the workspace as dirty and return a reference to the graph.
    ///
    /// Since `Hypergraph` uses interior mutability (DashMap), mutations
    /// happen through `&self`. This method exists to set the dirty flag
    /// signaling that unsaved changes exist.
    pub(crate) fn graph_mut(&mut self) -> &Hypergraph<BaseGraphKind> {
        self.dirty = true;
        &self.graph
    }

    /// The workspace name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether there are unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the workspace as having unsaved changes.
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Build a `WorkspaceInfo` summary from the current live state.
    pub fn to_info(&self) -> WorkspaceInfo {
        let graph: &Hypergraph<BaseGraphKind> = &self.graph;

        // Count atoms by iterating over vertex data and checking the atoms map.
        let vertex_count = graph.vertex_count();
        let mut atom_count: usize = 0;

        for (_key, data) in graph.vertex_iter() {
            if graph.get_atom_by_key(&data.key()).is_some() {
                atom_count += 1;
            }
        }

        let pattern_count = vertex_count.saturating_sub(atom_count);

        WorkspaceInfo {
            name: self.name.clone(),
            vertex_count,
            atom_count,
            pattern_count,
            created_at: self.metadata.created_at.to_rfc3339(),
            modified_at: self.metadata.modified_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::vertex::atom::Atom;

    /// Helper: build a `Workspace` in memory (no disk, no lock).
    fn test_workspace(name: &str) -> Workspace {
        Workspace {
            name: name.to_string(),
            dir: PathBuf::from("/tmp/fake"),
            graph: HypergraphRef::from(Hypergraph::default()),
            metadata: WorkspaceMetadata::new(name),
            lock: None,
            dirty: false,
        }
    }

    #[test]
    fn new_workspace_is_not_dirty() {
        let ws = test_workspace("clean");
        assert!(!ws.is_dirty());
    }

    #[test]
    fn graph_mut_marks_dirty() {
        let mut ws = test_workspace("mutable");
        assert!(!ws.is_dirty());
        let _ = ws.graph_mut();
        assert!(ws.is_dirty());
    }

    #[test]
    fn to_info_empty_graph() {
        let ws = test_workspace("empty");
        let info = ws.to_info();
        assert_eq!(info.name, "empty");
        assert_eq!(info.vertex_count, 0);
        assert_eq!(info.atom_count, 0);
        assert_eq!(info.pattern_count, 0);
    }

    #[test]
    fn to_info_with_atoms_and_pattern() {
        let mut ws = test_workspace("demo");
        let graph = ws.graph_mut();
        let ta = graph.insert_atom(Atom::Element('a'));
        let tb = graph.insert_atom(Atom::Element('b'));
        let _p = graph.insert_pattern(vec![ta, tb]);

        let info = ws.to_info();
        assert_eq!(info.vertex_count, 3);
        assert_eq!(info.atom_count, 2);
        assert_eq!(info.pattern_count, 1);
    }

    #[test]
    fn name_accessor() {
        let ws = test_workspace("my-ws");
        assert_eq!(ws.name(), "my-ws");
    }

    #[test]
    fn graph_accessor_returns_same_graph() {
        let mut ws = test_workspace("g");
        ws.graph_mut().insert_atom(Atom::Element('z'));
        assert_eq!(ws.graph().vertex_count(), 1);
    }

    #[test]
    fn graph_ref_clone_is_cheap() {
        let ws = test_workspace("ref");
        let ref1 = ws.graph_ref();
        let ref2 = ws.graph_ref();
        // Both point to the same underlying graph
        assert_eq!(ref1.vertex_count(), ref2.vertex_count());
    }

    #[test]
    fn graph_ref_shares_mutations() {
        let ws = test_workspace("shared");
        let ref1 = ws.graph_ref();
        // Insert via the graph reference
        ref1.insert_atom(Atom::Element('x'));
        // Should be visible through the workspace's graph
        assert_eq!(ws.graph().vertex_count(), 1);
    }

    #[test]
    fn mark_dirty_sets_flag() {
        let mut ws = test_workspace("dirty");
        assert!(!ws.is_dirty());
        ws.mark_dirty();
        assert!(ws.is_dirty());
    }
}
