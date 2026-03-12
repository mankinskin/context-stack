//! Debug / introspection commands — snapshot, statistics, and validation.
//!
//! These commands provide read-only insight into the current state of an open
//! workspace's hypergraph. They are useful for CLI output, testing, and
//! debugging.

use context_trace::{
    VertexSet,
    graph::snapshot::GraphSnapshot,
};

use crate::{
    error::ApiError,
    types::{
        GraphStatistics,
        ValidationReport,
    },
    workspace::manager::WorkspaceManager,
};

impl WorkspaceManager {
    /// Produce a compact snapshot of the workspace's hypergraph.
    ///
    /// The snapshot includes all vertices (as `SnapshotNode`) and all
    /// parent→child edges (as `SnapshotEdge`), suitable for serialization
    /// to JSON and consumption by frontends or diagnostics tools.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn get_snapshot(
        &self,
        ws_name: &str,
    ) -> Result<GraphSnapshot, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        Ok(ws.graph().to_graph_snapshot())
    }

    /// Compute aggregate statistics for the workspace's hypergraph.
    ///
    /// Returns counts (vertices, atoms, patterns, edges) and the maximum
    /// token width across all vertices.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn get_statistics(
        &self,
        ws_name: &str,
    ) -> Result<GraphStatistics, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        Ok(GraphStatistics::from_graph(ws.graph()))
    }

    /// Validate the integrity of the workspace's hypergraph.
    ///
    /// Performs basic structural checks:
    /// - All child pattern tokens reference existing vertices.
    /// - Token widths match the sum of their children's widths (for non-atoms).
    ///
    /// Returns a [`ValidationReport`] with any issues found.
    ///
    /// # Errors
    ///
    /// - `ApiError::Workspace(WorkspaceError::NotOpen)` if the workspace is not
    ///   currently open.
    pub fn validate_graph(
        &self,
        ws_name: &str,
    ) -> Result<ValidationReport, ApiError> {
        let ws = self.get_workspace(ws_name)?;
        let graph = ws.graph();

        let mut issues = Vec::new();
        let mut vertex_count: usize = 0;

        for (_key, data) in graph.vertex_iter() {
            vertex_count += 1;

            // Check: all child pattern tokens reference existing vertices
            for (_pid, pattern) in data.child_patterns().iter() {
                for token in pattern.iter() {
                    if graph.get_vertex_data(token.index).is_err() {
                        issues.push(format!(
                            "Vertex {} has child token {} which does not exist",
                            data.to_token().index.0,
                            token.index.0
                        ));
                    }
                }
            }

            // Check: token width matches sum of children widths (for non-atoms)
            if !data.child_patterns().is_empty() {
                for (_pid, pattern) in data.child_patterns().iter() {
                    let child_width_sum: usize =
                        pattern.iter().map(|t| t.width.0).sum();
                    let vertex_width = data.to_token().width.0;
                    if child_width_sum != vertex_width {
                        issues.push(format!(
                            "Vertex {} has width {} but children sum to {}",
                            data.to_token().index.0,
                            vertex_width,
                            child_width_sum
                        ));
                    }
                }
            }
        }

        Ok(ValidationReport {
            valid: issues.is_empty(),
            vertex_count,
            issues,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::workspace::manager::WorkspaceManager;
    use std::collections::HashSet;

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
        let char_set: HashSet<char> = chars.chars().collect();
        mgr.add_atoms(ws, char_set).unwrap();
    }

    // -- validate_graph ------------------------------------------------------

    #[test]
    fn validate_empty_graph() {
        let (_tmp, mgr) = setup("ws");
        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid);
        assert_eq!(report.vertex_count, 0);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn validate_graph_with_atoms() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid);
        assert_eq!(report.vertex_count, 3);
    }

    #[test]
    fn validate_graph_with_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid);
        assert_eq!(report.vertex_count, 3);
    }

    #[test]
    fn validate_graph_after_insert() {
        let (_tmp, mut mgr) = setup("ws");
        mgr.insert_sequence("ws", "hello world").unwrap();

        let report = mgr.validate_graph("ws").unwrap();
        assert!(report.valid, "issues: {:?}", report.issues);
    }

    #[test]
    fn validate_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.validate_graph("nope").unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }

    #[test]
    fn validate_report_serializable() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        let report = mgr.validate_graph("ws").unwrap();
        let json = serde_json::to_string(&report);
        assert!(json.is_ok(), "validation report should be serializable");
    }

    // -- get_snapshot --------------------------------------------------------

    #[test]
    fn snapshot_empty_graph() {
        let (_tmp, mgr) = setup("ws");
        let snap = mgr.get_snapshot("ws").unwrap();
        assert!(snap.nodes.is_empty());
        assert!(snap.edges.is_empty());
    }

    #[test]
    fn snapshot_with_atoms() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        let snap = mgr.get_snapshot("ws").unwrap();
        assert_eq!(snap.nodes.len(), 2);
        assert!(snap.edges.is_empty(), "atoms have no child patterns");
    }

    #[test]
    fn snapshot_with_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let snap = mgr.get_snapshot("ws").unwrap();
        // 2 atoms + 1 pattern = 3 nodes
        assert_eq!(snap.nodes.len(), 3);
        // The pattern has 1 child pattern with 2 children → 2 edges
        assert_eq!(snap.edges.len(), 2);
    }

    #[test]
    fn snapshot_nodes_sorted_by_index() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "cba");

        let snap = mgr.get_snapshot("ws").unwrap();
        for i in 0..snap.nodes.len().saturating_sub(1) {
            assert!(
                snap.nodes[i].index < snap.nodes[i + 1].index,
                "snapshot nodes should be sorted by index"
            );
        }
    }

    #[test]
    fn snapshot_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.get_snapshot("nope").unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }

    #[test]
    fn snapshot_serializable_to_json() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let snap = mgr.get_snapshot("ws").unwrap();
        let json = serde_json::to_string(&snap);
        assert!(json.is_ok(), "snapshot should be serializable to JSON");
    }

    // -- get_statistics ------------------------------------------------------

    #[test]
    fn statistics_empty_graph() {
        let (_tmp, mgr) = setup("ws");
        let stats = mgr.get_statistics("ws").unwrap();
        assert_eq!(stats.vertex_count, 0);
        assert_eq!(stats.atom_count, 0);
        assert_eq!(stats.pattern_count, 0);
        assert_eq!(stats.max_width, 0);
        assert_eq!(stats.edge_count, 0);
    }

    #[test]
    fn statistics_with_atoms() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "abc");

        let stats = mgr.get_statistics("ws").unwrap();
        assert_eq!(stats.vertex_count, 3);
        assert_eq!(stats.atom_count, 3);
        assert_eq!(stats.pattern_count, 0);
        assert_eq!(stats.max_width, 1);
        assert_eq!(stats.edge_count, 0);
    }

    #[test]
    fn statistics_with_pattern() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");
        mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

        let stats = mgr.get_statistics("ws").unwrap();
        assert_eq!(stats.vertex_count, 3);
        assert_eq!(stats.atom_count, 2);
        assert_eq!(stats.pattern_count, 1);
        assert_eq!(stats.max_width, 2);
        // The pattern "ab" has one child pattern [a, b] → 2 edges
        assert_eq!(stats.edge_count, 2);
    }

    #[test]
    fn statistics_workspace_not_open() {
        let (_tmp, mgr) = setup("ws");
        let err = mgr.get_statistics("nope").unwrap_err();
        assert_eq!(err.kind(), "workspace");
    }

    #[test]
    fn statistics_serializable_to_json() {
        let (_tmp, mut mgr) = setup("ws");
        add_atoms(&mut mgr, "ws", "ab");

        let stats = mgr.get_statistics("ws").unwrap();
        let json = serde_json::to_string(&stats);
        assert!(json.is_ok(), "statistics should be serializable to JSON");
    }
}
