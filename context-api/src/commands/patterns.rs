//! Pattern commands — create simple patterns, get vertex info, list vertices.
//!
//! A "simple pattern" is a pattern composed entirely of existing atom vertices
//! that do not yet belong to any other pattern. This is the most constrained
//! (and safest) way to build up the graph incrementally.
//!
//! The `get_vertex` and `list_vertices` commands provide read access to the
//! full vertex data (atoms and patterns alike).

use context_trace::graph::vertex::{
    VertexIndex,
    atom::Atom,
};

use crate::{
    error::{
        ApiError,
        PatternError,
    },
    types::{
        PatternInfo,
        TokenInfo,
        VertexInfo,
    },
    validation,
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Create a simple pattern from a sequence of atom characters.
    ///
    /// The atoms must already exist in the graph, must not belong to any
    /// existing pattern, and the sequence must contain at least 2 characters.
    /// See [`validation::validate_simple_pattern`] for the full set of rules.
    ///
    /// # Arguments
    ///
    /// * `ws_name` — name of the open workspace.
    /// * `atoms` — ordered sequence of character values for the pattern's
    ///   children. Each character must correspond to an existing atom.
    ///
    /// # Errors
    ///
    /// - `PatternError::WorkspaceNotOpen` if the workspace is not currently open.
    /// - `PatternError::TooShort` if fewer than 2 atoms are given.
    /// - `PatternError::DuplicateAtomInInput` if a character appears more than once.
    /// - `PatternError::AtomNotFound` if a character has not been added as an atom.
    /// - `PatternError::AtomAlreadyInPattern` if an atom already has a parent pattern.
    pub fn add_simple_pattern(
        &mut self,
        ws_name: &str,
        atoms: Vec<char>,
    ) -> Result<PatternInfo, PatternError> {
        // 1. Validate — this borrows the workspace immutably.
        {
            let ws = self.get_workspace(ws_name).map_err(|_| {
                PatternError::WorkspaceNotOpen {
                    workspace: ws_name.to_string(),
                }
            })?;
            validation::validate_simple_pattern(ws.graph(), &atoms)?;
        }

        // 2. Resolve atom chars to tokens and insert the pattern.
        let ws = self.get_workspace_mut(ws_name).map_err(|_| {
            PatternError::WorkspaceNotOpen {
                workspace: ws_name.to_string(),
            }
        })?;
        let graph = ws.graph_mut();

        // Look up tokens for each atom character.
        // Safe to unwrap: we just validated that all atoms exist.
        let tokens: Vec<_> = atoms
            .iter()
            .map(|&ch| graph.expect_atom_child(Atom::Element(ch)))
            .collect();

        // Insert the pattern into the graph.
        let pattern_token = graph.insert_pattern(tokens);

        // 3. Build the result.
        let children: Vec<TokenInfo> = atoms
            .iter()
            .filter_map(|&ch| {
                let idx = graph.get_atom_index(Atom::Element(ch)).ok()?;
                TokenInfo::from_index(graph, idx)
            })
            .collect();

        let label: String = atoms.iter().collect();
        let width = atoms.len();

        Ok(PatternInfo {
            index: pattern_token.index.0,
            label,
            width,
            children,
        })
    }

    /// Get detailed information about a single vertex by its index.
    ///
    /// Returns `Ok(Some(VertexInfo))` if the vertex exists, `Ok(None)` if
    /// no vertex with the given index is found.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn get_vertex(
        &self,
        ws_name: &str,
        index: usize,
    ) -> Result<Option<VertexInfo>, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        Ok(VertexInfo::from_graph(ws.graph(), VertexIndex(index)))
    }

    /// List all vertices in the workspace graph, sorted by index.
    ///
    /// Returns lightweight `TokenInfo` entries (index, label, width) for
    /// every vertex — both atoms and patterns.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn list_vertices(
        &self,
        ws_name: &str,
    ) -> Result<Vec<TokenInfo>, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        let graph = ws.graph();

        let mut vertices: Vec<TokenInfo> = graph
            .vertex_iter()
            .filter_map(|(_key, data)| {
                let token = data.to_token();
                TokenInfo::from_graph(graph, token)
            })
            .collect();

        vertices.sort_by_key(|v| v.index);
        Ok(vertices)
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

    /// Helper: add atoms for all characters in the string.
    fn add_atoms(
        mgr: &mut WorkspaceManager,
        ws: &str,
        chars: &str,
    ) {
        let char_vec: Vec<char> = chars.chars().collect();
        mgr.add_atoms(ws, char_vec).unwrap();
    }

    // -- add_simple_pattern --------------------------------------------------

    #[test]
    fn add_simple_pattern_succeeds() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        let info = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();
        assert_eq!(info.label, "ab");
        assert_eq!(info.width, 2);
        assert_eq!(info.children.len(), 2);
        assert_eq!(info.children[0].label, "a");
        assert_eq!(info.children[1].label, "b");
    }

    #[test]
    fn add_simple_pattern_three_atoms() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "xyz");

        let info = mgr.add_simple_pattern("ws", vec!['x', 'y', 'z']).unwrap();
        assert_eq!(info.label, "xyz");
        assert_eq!(info.width, 3);
        assert_eq!(info.children.len(), 3);
    }

    #[test]
    fn add_simple_pattern_too_short() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "a");

        match mgr.add_simple_pattern("ws", vec!['a']) {
            Err(PatternError::TooShort { len: 1 }) => {},
            other => panic!("expected TooShort(1), got: {other:?}"),
        }
    }

    #[test]
    fn add_simple_pattern_empty() {
        let (_tmp, mut mgr) = setup("ws");

        match mgr.add_simple_pattern("ws", vec![]) {
            Err(PatternError::TooShort { len: 0 }) => {},
            other => panic!("expected TooShort(0), got: {other:?}"),
        }
    }

    #[test]
    fn add_simple_pattern_duplicate_atom_in_input() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        match mgr.add_simple_pattern("ws", vec!['a', 'a']) {
            Err(PatternError::DuplicateAtomInInput { ch: 'a' }) => {},
            other =>
                panic!("expected DuplicateAtomInInput('a'), got: {other:?}"),
        }
    }

    #[test]
    fn add_simple_pattern_atom_not_found() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "a");

        match mgr.add_simple_pattern("ws", vec!['a', 'z']) {
            Err(PatternError::AtomNotFound { ch: 'z' }) => {},
            other => panic!("expected AtomNotFound('z'), got: {other:?}"),
        }
    }

    #[test]
    fn add_simple_pattern_atom_already_in_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");

        // Create pattern "ab" — now 'a' and 'b' have parents.
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        // Trying to use 'a' in another pattern should fail.
        match mgr.add_simple_pattern("ws", vec!['a', 'c']) {
            Err(PatternError::AtomAlreadyInPattern { ch: 'a', .. }) => {},
            other =>
                panic!("expected AtomAlreadyInPattern('a'), got: {other:?}"),
        }
    }

    #[test]
    fn add_simple_pattern_workspace_not_open() {
        let (_tmp, mut mgr) = setup("ws");

        match mgr.add_simple_pattern("nope", vec!['a', 'b']) {
            Err(PatternError::WorkspaceNotOpen { workspace }) => {
                assert_eq!(workspace, "nope");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other:?}"),
        }
    }

    // -- get_vertex ----------------------------------------------------------

    #[test]
    fn get_vertex_atom() {
        let (_tmp, mut mgr) = setup("ws");
        let atom_info = mgr.add_atom("ws", 'a').unwrap();

        let vertex = mgr.get_vertex("ws", atom_info.index).unwrap();
        assert!(vertex.is_some());

        let v = vertex.unwrap();
        assert_eq!(v.index, atom_info.index);
        assert_eq!(v.label, "a");
        assert_eq!(v.width, 1);
        assert!(v.is_atom);
        assert!(v.children.is_empty());
        assert_eq!(v.parent_count, 0);
    }

    #[test]
    fn get_vertex_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        let pat = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let vertex = mgr.get_vertex("ws", pat.index).unwrap();
        assert!(vertex.is_some());

        let v = vertex.unwrap();
        assert_eq!(v.index, pat.index);
        assert_eq!(v.label, "ab");
        assert_eq!(v.width, 2);
        assert!(!v.is_atom);
        assert_eq!(v.children.len(), 1); // one child pattern
        assert_eq!(v.children[0].len(), 2); // two children in pattern
        assert_eq!(v.parent_count, 0);
    }

    #[test]
    fn get_vertex_atom_with_parent() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        let _pat = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        // Atom 'a' should now have 1 parent.
        let a_info = mgr.get_atom("ws", 'a').unwrap().unwrap();
        let vertex = mgr.get_vertex("ws", a_info.index).unwrap().unwrap();
        assert!(vertex.is_atom);
        assert_eq!(vertex.parent_count, 1);
    }

    #[test]
    fn get_vertex_nonexistent() {
        let (_tmp, mgr) = setup("ws");
        let result = mgr.get_vertex("ws", 9999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_vertex_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.get_vertex("nope", 0).unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }

    // -- list_vertices -------------------------------------------------------

    #[test]
    fn list_vertices_empty() {
        let (_tmp, mgr) = setup("ws");
        let vertices = mgr.list_vertices("ws").unwrap();
        assert!(vertices.is_empty());
    }

    #[test]
    fn list_vertices_atoms_only() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");

        let vertices = mgr.list_vertices("ws").unwrap();
        assert_eq!(vertices.len(), 3);

        // Should be sorted by index.
        for i in 0..vertices.len() - 1 {
            assert!(
                vertices[i].index < vertices[i + 1].index,
                "vertices should be sorted by index"
            );
        }
    }

    #[test]
    fn list_vertices_includes_patterns() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let vertices = mgr.list_vertices("ws").unwrap();
        // 2 atoms + 1 pattern = 3
        assert_eq!(vertices.len(), 3);

        // The last vertex should be the pattern with width 2.
        let pattern_vertex = vertices.iter().find(|v| v.width == 2);
        assert!(pattern_vertex.is_some());
        assert_eq!(pattern_vertex.unwrap().label, "ab");
    }

    #[test]
    fn list_vertices_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.list_vertices("nope").unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }
}
