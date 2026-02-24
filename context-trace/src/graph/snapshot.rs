//! Graph snapshot serialization for log viewer integration.
//!
//! Emits a compact JSON representation of the hypergraph as a tracing event
//! so the log-viewer frontend can reconstruct and render the 3D graph.

use serde::Serialize;

use crate::graph::{Hypergraph, kind::GraphKind};

/// A compact, serializable snapshot of the hypergraph topology.
#[derive(Debug, Clone, Serialize)]
pub struct GraphSnapshot {
    /// All vertices in the graph.
    pub nodes: Vec<SnapshotNode>,
    /// All parent→child edges derived from child patterns.
    pub edges: Vec<SnapshotEdge>,
}

/// A single vertex in the snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotNode {
    /// Vertex index (numeric id).
    pub index: usize,
    /// Human-readable label (e.g. "abc" for a merged token).
    pub label: String,
    /// Token width (1 for atoms, >1 for merged).
    pub width: usize,
    /// Whether this is a leaf atom vertex.
    pub is_atom: bool,
}

/// A directed edge from parent vertex to child vertex.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotEdge {
    /// Parent vertex index.
    pub from: usize,
    /// Child vertex index.
    pub to: usize,
    /// Which pattern of the parent this edge belongs to (0-based).
    pub pattern_idx: usize,
    /// Position of the child within that pattern.
    pub sub_index: usize,
}

impl<G: GraphKind> Hypergraph<G>
where
    G::Atom: std::fmt::Display,
{
    /// Produce a compact [`GraphSnapshot`] of the current graph state.
    pub fn to_graph_snapshot(&self) -> GraphSnapshot {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for (_key, data) in self.vertex_iter() {
            let vi = data.to_token().index.0;
            let width = data.to_token().width.0;
            let label = self.vertex_data_string(data.clone());

            let key = data.to_token().index;
            let vertex_key = self.expect_key_for_index(key);
            let is_atom = self.get_atom_by_key(&vertex_key).is_some();

            nodes.push(SnapshotNode {
                index: vi,
                label,
                width,
                is_atom,
            });

            // Extract edges from child patterns
            for (pat_idx, (_pid, pattern)) in
                data.child_patterns().iter().enumerate()
            {
                for (sub_idx, token) in pattern.iter().enumerate() {
                    edges.push(SnapshotEdge {
                        from: vi,
                        to: token.index.0,
                        pattern_idx: pat_idx,
                        sub_index: sub_idx,
                    });
                }
            }
        }

        // Sort nodes by index for deterministic output
        nodes.sort_by_key(|n| n.index);
        edges.sort_by_key(|e| (e.from, e.pattern_idx, e.sub_index));

        GraphSnapshot { nodes, edges }
    }

    /// Emit the graph snapshot as a structured tracing event.
    ///
    /// The log-viewer frontend looks for entries with
    /// `message == "graph_snapshot"` and parses the `graph_data` field.
    pub fn emit_graph_snapshot(&self) {
        let snapshot = self.to_graph_snapshot();
        let json =
            serde_json::to_string(&snapshot).unwrap_or_default();
        tracing::info!(
            graph_data = %json,
            node_count = snapshot.nodes.len(),
            edge_count = snapshot.edges.len(),
            "graph_snapshot"
        );
    }
}

// ---------------------------------------------------------------------------
// Search state snapshots — DEPRECATED, use graph::visualization types instead
// ---------------------------------------------------------------------------

// Re-export the new visualization types for backwards compatibility during migration
pub use super::visualization::{GraphOpEvent, LocationInfo, OperationType, QueryInfo, Transition};
