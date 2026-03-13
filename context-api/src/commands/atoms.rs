//! Atom commands — add, get, and list atoms in an open workspace.
//!
//! Atoms are the leaf vertices of the hypergraph, each representing a single
//! character. They are deduplicated by character value: inserting the same
//! character twice returns the same vertex index.

use std::collections::HashSet;

use context_trace::graph::vertex::atom::Atom;

use crate::{
    error::{
        ApiError,
        AtomError,
    },
    types::AtomInfo,
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Add a single atom (character) to the graph.
    ///
    /// Atoms are **deduplicated**: if an atom with the same character value
    /// already exists, its existing index is returned without creating a new
    /// vertex.
    ///
    /// # Errors
    ///
    /// - `AtomError::WorkspaceNotOpen` if the workspace is not currently open.
    pub fn add_atom(
        &mut self,
        ws_name: &str,
        ch: char,
    ) -> Result<AtomInfo, AtomError> {
        let ws = self.get_workspace_mut(ws_name).map_err(|_| {
            AtomError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph = ws.graph_mut();

        // Check if the atom already exists (deduplication).
        if let Ok(index) = graph.get_atom_index(Atom::Element(ch)) {
            return Ok(AtomInfo { index: index.0, ch });
        }

        // Insert new atom.
        let token = graph.insert_atom(Atom::Element(ch));
        Ok(AtomInfo {
            index: token.index.0,
            ch,
        })
    }

    /// Add multiple atoms (characters) to the graph in bulk.
    ///
    /// Each character is deduplicated individually — existing atoms are
    /// returned without creating duplicates. The result preserves the
    /// insertion order of the input `Vec`, skipping duplicate characters.
    ///
    /// # Errors
    ///
    /// - `AtomError::WorkspaceNotOpen` if the workspace is not currently open.
    pub fn add_atoms(
        &mut self,
        ws_name: &str,
        chars: Vec<char>,
    ) -> Result<Vec<AtomInfo>, AtomError> {
        let ws = self.get_workspace_mut(ws_name).map_err(|_| {
            AtomError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph = ws.graph_mut();

        let mut seen = HashSet::with_capacity(chars.len());
        let mut results = Vec::with_capacity(chars.len());
        for ch in chars {
            // Skip duplicate characters in the input, preserving first occurrence order.
            if !seen.insert(ch) {
                continue;
            }
            // Deduplicate: check for existing atom first.
            let index = if let Ok(idx) = graph.get_atom_index(Atom::Element(ch))
            {
                idx.0
            } else {
                graph.insert_atom(Atom::Element(ch)).index.0
            };
            results.push(AtomInfo { index, ch });
        }

        Ok(results)
    }

    /// Look up a single atom by its character value.
    ///
    /// Returns `Ok(Some(AtomInfo))` if found, `Ok(None)` if the character has
    /// not been added as an atom.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn get_atom(
        &self,
        ws_name: &str,
        ch: char,
    ) -> Result<Option<AtomInfo>, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        let graph = ws.graph();

        match graph.get_atom_index(Atom::Element(ch)) {
            Ok(index) => Ok(Some(AtomInfo { index: index.0, ch })),
            Err(_) => Ok(None),
        }
    }

    /// List all atoms in the workspace, sorted by vertex index.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn list_atoms(
        &self,
        ws_name: &str,
    ) -> Result<Vec<AtomInfo>, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        let graph = ws.graph();

        let mut atoms: Vec<AtomInfo> = graph
            .atom_iter()
            .filter_map(|(atom, token)| {
                // Only include `Element` atoms (skip Start/End sentinels).
                match atom {
                    Atom::Element(ch) => Some(AtomInfo {
                        index: token.index.0,
                        ch,
                    }),
                    _ => None,
                }
            })
            .collect();

        atoms.sort_by_key(|a| a.index);
        Ok(atoms)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::manager::WorkspaceManager;

    /// Helper: create a `WorkspaceManager` backed by a temporary directory
    /// with a workspace already created and open.
    fn setup(ws_name: &str) -> (tempfile::TempDir, WorkspaceManager) {
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let mut mgr = WorkspaceManager::new(tmp.path().to_path_buf());
        mgr.create_workspace(ws_name).unwrap();
        (tmp, mgr)
    }

    #[test]
    fn add_atom_returns_atom_info() {
        let (_tmp, mut mgr) = setup("ws");
        let info = mgr.add_atom("ws", 'a').unwrap();
        assert_eq!(info.ch, 'a');
    }

    #[test]
    fn add_atom_is_idempotent() {
        let (_tmp, mut mgr) = setup("ws");
        let first = mgr.add_atom("ws", 'x').unwrap();
        let second = mgr.add_atom("ws", 'x').unwrap();
        assert_eq!(
            first.index, second.index,
            "same char should yield same index"
        );
    }

    #[test]
    fn add_atom_workspace_not_open() {
        let (_tmp, mut mgr) = setup("ws");
        let err = mgr.add_atom("nope", 'a').unwrap_err();
        match err {
            AtomError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "nope");
            },
        }
    }

    #[test]
    fn add_atoms_bulk() {
        let (_tmp, mut mgr) = setup("ws");
        let chars: Vec<char> = vec!['a', 'b', 'c'];
        let infos = mgr.add_atoms("ws", chars).unwrap();
        assert_eq!(infos.len(), 3);
        // Should preserve input order.
        assert_eq!(infos[0].ch, 'a');
        assert_eq!(infos[1].ch, 'b');
        assert_eq!(infos[2].ch, 'c');
    }

    #[test]
    fn add_atoms_deduplicates() {
        let (_tmp, mut mgr) = setup("ws");
        let first = mgr.add_atom("ws", 'a').unwrap();

        let chars: Vec<char> = vec!['a', 'b'];
        let infos = mgr.add_atoms("ws", chars).unwrap();

        // 'a' should keep its original index.
        let a_info = infos.iter().find(|i| i.ch == 'a').unwrap();
        assert_eq!(a_info.index, first.index);

        // Total unique atoms should be 2.
        assert_eq!(infos.len(), 2);
    }

    #[test]
    fn get_atom_existing() {
        let (_tmp, mut mgr) = setup("ws");
        mgr.add_atom("ws", 'q').unwrap();

        let result = mgr.get_atom("ws", 'q').unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().ch, 'q');
    }

    #[test]
    fn get_atom_missing() {
        let (_tmp, mgr) = setup("ws");
        let result = mgr.get_atom("ws", 'z').unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn list_atoms_empty() {
        let (_tmp, mgr) = setup("ws");
        let atoms = mgr.list_atoms("ws").unwrap();
        assert!(atoms.is_empty());
    }

    #[test]
    fn list_atoms_returns_all_sorted() {
        let (_tmp, mut mgr) = setup("ws");
        mgr.add_atom("ws", 'c').unwrap();
        mgr.add_atom("ws", 'a').unwrap();
        mgr.add_atom("ws", 'b').unwrap();

        let atoms = mgr.list_atoms("ws").unwrap();
        assert_eq!(atoms.len(), 3);

        // Should be sorted by index (insertion order here).
        let chars: Vec<char> = atoms.iter().map(|a| a.ch).collect();
        assert_eq!(chars, vec!['c', 'a', 'b']); // sorted by index = insertion order
    }

    #[test]
    fn list_atoms_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.list_atoms("nope").unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }
}
