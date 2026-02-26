//! Search path visualization types for incremental path construction.
//!
//! Models the traversal of an `IndexRangePath` during search operations.
//! Each step modifies `(start_path, root, end_path)` and is emitted as a
//! structured tracing event scoped by a unique `path_id`.
//!
//! Both Rust and TypeScript can reconstruct the full path graph from a
//! sequence of [`PathTransition`] steps read from a log file.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Edge reference — identifies an edge in the GraphSnapshot
// ---------------------------------------------------------------------------

/// Reference to a directed edge in the hypergraph snapshot.
///
/// Matches the fields of [`SnapshotEdge`](super::snapshot::SnapshotEdge):
/// `(from, to, pattern_idx, sub_index)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct EdgeRef {
    /// Source vertex index (traversal origin).
    /// For start_path edges: the child (pointing upward toward parent).
    /// For end_path edges: the parent (pointing downward toward child).
    pub from: usize,
    /// Target vertex index (traversal destination).
    /// For start_path edges: the parent (pointing upward).
    /// For end_path edges: the child (pointing downward).
    pub to: usize,
    /// Pattern index (0-based enumeration order in snapshot).
    pub pattern_idx: usize,
    /// Position of child within that pattern.
    pub sub_index: usize,
}

// ---------------------------------------------------------------------------
// Path segment — a node in the reconstructed path graph
// ---------------------------------------------------------------------------

/// A node in the search path, referencing a vertex in the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct PathNode {
    /// Vertex index in the snapshot.
    pub index: usize,
    /// Token width (atom count).
    pub width: usize,
}

// ---------------------------------------------------------------------------
// Path transitions — the incremental operations on (start_path, root, end_path)
// ---------------------------------------------------------------------------

/// Transition describing how the search path changed at this step.
///
/// The search path models an `IndexRangePath = (start_path, root, end_path)`:
///
/// - **start_path** grows upward (parent exploration): each `PushParent`
///   appends a `ChildLocation` linking current root → parent.
/// - **root** is set when the first child match converts a parent candidate
///   into a `CompareState`.
/// - **end_path** grows downward (child comparison / prefix decomposition):
///   each `PushChild` appends a `ChildLocation` for child-ward descent.
///
/// Reconstruction: apply transitions in order to build the path graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub enum PathTransition {
    /// Search starts at a leaf token.
    /// Path state: `start_node = node` (no start_path, no root, no end_path).
    SetStartNode {
        node: PathNode,
    },

    /// Parent candidate accepted — extend start_path upward.
    /// Edge goes from child → parent (bottom-up).
    PushParent {
        /// The parent vertex we moved up to.
        parent: PathNode,
        /// Edge from previous top-of-start-path to this parent.
        edge: EdgeRef,
    },

    /// Root established — first child match found.
    /// Converts parent candidate into a `CompareState` with a root.
    SetRoot {
        /// The root vertex.
        root: PathNode,
        /// Edge from the last start_path node into the root
        /// (the root is a parent of the start-path top, containing
        /// the first matched child pattern).
        edge: EdgeRef,
    },

    /// Child candidate — extend end_path downward (prefix decomposition).
    /// Edge goes from parent → child (top-down).
    PushChild {
        /// The child vertex we descended into.
        child: PathNode,
        /// Edge from current end_path bottom to this child.
        edge: EdgeRef,
    },

    /// Backtrack end_path — pop last child (mismatch at this level).
    PopChild,

    /// Replace the last element of end_path
    /// (different child of the same parent tried).
    ReplaceChild {
        /// The new child vertex.
        child: PathNode,
        /// Edge from parent to new child.
        edge: EdgeRef,
    },

    /// Child comparison succeeded at this cursor position.
    ChildMatch {
        /// Atom position in the query after match.
        cursor_pos: usize,
    },

    /// Child comparison failed.
    ChildMismatch {
        /// Atom position where mismatch was detected.
        cursor_pos: usize,
    },

    /// This search path is complete.
    Done {
        /// Whether the search found a match.
        success: bool,
    },
}

// ---------------------------------------------------------------------------
// VizPathGraph — the reconstructed path graph for highlighting
// ---------------------------------------------------------------------------

/// A reconstructed search path graph for visualization.
///
/// Contains the ordered list of nodes and edges that form the current
/// `IndexRangePath`. The frontend highlights these in the hypergraph view.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct VizPathGraph {
    /// The start node (leaf token where search began).
    pub start_node: Option<PathNode>,

    /// Nodes in the start_path (bottom-up, from start_node toward root).
    /// Does NOT include start_node itself or the root.
    pub start_path: Vec<PathNode>,

    /// Edges in the start_path (bottom-up: from=child, to=parent).
    pub start_edges: Vec<EdgeRef>,

    /// The root node (set when first child match occurs).
    pub root: Option<PathNode>,

    /// Edge connecting the top of start_path to the root (from=start_path_top, to=root).
    pub root_edge: Option<EdgeRef>,

    /// Nodes in the end_path (top-down, from root toward leaf).
    /// Does NOT include the root itself.
    pub end_path: Vec<PathNode>,

    /// Edges in the end_path (top-down: from=parent, to=child).
    pub end_edges: Vec<EdgeRef>,

    /// Current cursor position in the query.
    pub cursor_pos: usize,

    /// Whether this path is complete.
    pub done: bool,

    /// Whether this path resulted in a match (only meaningful when `done`).
    pub success: bool,
}

impl VizPathGraph {
    /// Create an empty path graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a single transition, mutating this path graph in place.
    ///
    /// Returns `Err` if the transition is invalid for the current state
    /// (e.g., `PushChild` before `SetRoot`).
    pub fn apply(&mut self, transition: &PathTransition) -> Result<(), String> {
        match transition {
            PathTransition::SetStartNode { node } => {
                if self.start_node.is_some() {
                    return Err("SetStartNode called twice".into());
                }
                self.start_node = Some(*node);
            },
            PathTransition::PushParent { parent, edge } => {
                if self.start_node.is_none() {
                    return Err("PushParent before SetStartNode".into());
                }
                if self.root.is_some() {
                    return Err("PushParent after SetRoot".into());
                }
                self.start_path.push(*parent);
                self.start_edges.push(*edge);
            },
            PathTransition::SetRoot { root, edge } => {
                if self.start_node.is_none() {
                    return Err("SetRoot before SetStartNode".into());
                }
                if self.root.is_some() {
                    return Err("SetRoot called twice".into());
                }
                self.root = Some(*root);
                self.root_edge = Some(*edge);
            },
            PathTransition::PushChild { child, edge } => {
                if self.root.is_none() {
                    return Err("PushChild before SetRoot".into());
                }
                self.end_path.push(*child);
                self.end_edges.push(*edge);
            },
            PathTransition::PopChild => {
                if self.end_path.is_empty() {
                    return Err("PopChild on empty end_path".into());
                }
                self.end_path.pop();
                self.end_edges.pop();
            },
            PathTransition::ReplaceChild { child, edge } => {
                if self.end_path.is_empty() {
                    return Err("ReplaceChild on empty end_path".into());
                }
                *self.end_path.last_mut().unwrap() = *child;
                *self.end_edges.last_mut().unwrap() = *edge;
            },
            PathTransition::ChildMatch { cursor_pos } => {
                self.cursor_pos = *cursor_pos;
            },
            PathTransition::ChildMismatch { cursor_pos } => {
                self.cursor_pos = *cursor_pos;
            },
            PathTransition::Done { success } => {
                self.done = true;
                self.success = *success;
            },
        }
        Ok(())
    }

    /// Reconstruct a path graph from a sequence of transitions.
    pub fn from_transitions(
        transitions: &[PathTransition],
    ) -> Result<Self, String> {
        let mut graph = Self::new();
        for (i, t) in transitions.iter().enumerate() {
            graph.apply(t).map_err(|e| {
                format!("step {i}: {e}")
            })?;
        }
        Ok(graph)
    }

    /// All node indices referenced by this path (for highlighting).
    pub fn all_node_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        if let Some(start) = &self.start_node {
            indices.push(start.index);
        }
        for n in &self.start_path {
            indices.push(n.index);
        }
        if let Some(root) = &self.root {
            indices.push(root.index);
        }
        for n in &self.end_path {
            indices.push(n.index);
        }
        indices
    }

    /// All edge refs referenced by this path (for highlighting).
    pub fn all_edges(&self) -> Vec<EdgeRef> {
        let mut edges = Vec::new();
        edges.extend_from_slice(&self.start_edges);
        if let Some(re) = &self.root_edge {
            edges.push(*re);
        }
        edges.extend_from_slice(&self.end_edges);
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(index: usize, width: usize) -> PathNode {
        PathNode { index, width }
    }

    fn edge(from: usize, to: usize, pattern_idx: usize, sub_index: usize) -> EdgeRef {
        EdgeRef { from, to, pattern_idx, sub_index }
    }

    // ---------------------------------------------------------------
    // VizPathGraph::apply — valid transition sequences
    // ---------------------------------------------------------------

    #[test]
    fn test_set_start_node() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(5, 1) })
            .unwrap();
        assert_eq!(g.start_node, Some(node(5, 1)));
        assert!(g.start_path.is_empty());
        assert!(g.root.is_none());
    }

    #[test]
    fn test_push_parent_builds_start_path() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::PushParent {
            parent: node(10, 2),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        g.apply(&PathTransition::PushParent {
            parent: node(20, 4),
            edge: edge(10, 20, 0, 1),
        }).unwrap();

        assert_eq!(g.start_path.len(), 2);
        assert_eq!(g.start_path[0], node(10, 2));
        assert_eq!(g.start_path[1], node(20, 4));
        assert_eq!(g.start_edges.len(), 2);
        assert_eq!(g.start_edges[0], edge(1, 10, 0, 0));
        assert_eq!(g.start_edges[1], edge(10, 20, 0, 1));
    }

    #[test]
    fn test_set_root_after_parents() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::PushParent {
            parent: node(10, 2),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(20, 4),
            edge: edge(10, 20, 0, 0),
        }).unwrap();

        assert_eq!(g.root, Some(node(20, 4)));
        assert_eq!(g.root_edge, Some(edge(10, 20, 0, 0)));
    }

    #[test]
    fn test_push_child_builds_end_path() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        g.apply(&PathTransition::PushChild {
            child: node(3, 1),
            edge: edge(10, 3, 0, 1),
        }).unwrap();
        g.apply(&PathTransition::PushChild {
            child: node(2, 1),
            edge: edge(3, 2, 0, 0),
        }).unwrap();

        assert_eq!(g.end_path.len(), 2);
        assert_eq!(g.end_path[0], node(3, 1));
        assert_eq!(g.end_path[1], node(2, 1));
        assert_eq!(g.end_edges[0], edge(10, 3, 0, 1));
        assert_eq!(g.end_edges[1], edge(3, 2, 0, 0));
    }

    #[test]
    fn test_pop_child_backtracks() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        g.apply(&PathTransition::PushChild {
            child: node(3, 1),
            edge: edge(10, 3, 0, 1),
        }).unwrap();
        g.apply(&PathTransition::PopChild).unwrap();

        assert!(g.end_path.is_empty());
        assert!(g.end_edges.is_empty());
    }

    #[test]
    fn test_replace_child() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        g.apply(&PathTransition::PushChild {
            child: node(3, 1),
            edge: edge(10, 3, 0, 1),
        }).unwrap();
        g.apply(&PathTransition::ReplaceChild {
            child: node(4, 1),
            edge: edge(10, 4, 0, 2),
        }).unwrap();

        assert_eq!(g.end_path.len(), 1);
        assert_eq!(g.end_path[0], node(4, 1));
        assert_eq!(g.end_edges[0], edge(10, 4, 0, 2));
    }

    #[test]
    fn test_child_match_updates_cursor() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::ChildMatch { cursor_pos: 3 }).unwrap();
        assert_eq!(g.cursor_pos, 3);
    }

    #[test]
    fn test_done_sets_flags() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::Done { success: true }).unwrap();
        assert!(g.done);
        assert!(g.success);
    }

    // ---------------------------------------------------------------
    // VizPathGraph::apply — invalid transition sequences
    // ---------------------------------------------------------------

    #[test]
    fn test_double_start_node_fails() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        let err = g.apply(&PathTransition::SetStartNode { node: node(2, 1) });
        assert!(err.is_err());
    }

    #[test]
    fn test_push_parent_before_start_fails() {
        let mut g = VizPathGraph::new();
        let err = g.apply(&PathTransition::PushParent {
            parent: node(10, 2),
            edge: edge(10, 1, 0, 0),
        });
        assert!(err.is_err());
    }

    #[test]
    fn test_push_parent_after_root_fails() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        let err = g.apply(&PathTransition::PushParent {
            parent: node(20, 4),
            edge: edge(10, 20, 0, 0),
        });
        assert!(err.is_err());
    }

    #[test]
    fn test_push_child_before_root_fails() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        let err = g.apply(&PathTransition::PushChild {
            child: node(3, 1),
            edge: edge(10, 3, 0, 1),
        });
        assert!(err.is_err());
    }

    #[test]
    fn test_pop_child_empty_fails() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        let err = g.apply(&PathTransition::PopChild);
        assert!(err.is_err());
    }

    #[test]
    fn test_replace_child_empty_fails() {
        let mut g = VizPathGraph::new();
        g.apply(&PathTransition::SetStartNode { node: node(1, 1) }).unwrap();
        g.apply(&PathTransition::SetRoot {
            root: node(10, 3),
            edge: edge(1, 10, 0, 0),
        }).unwrap();
        let err = g.apply(&PathTransition::ReplaceChild {
            child: node(4, 1),
            edge: edge(10, 4, 0, 2),
        });
        assert!(err.is_err());
    }

    // ---------------------------------------------------------------
    // from_transitions — reconstruct from a list
    // ---------------------------------------------------------------

    #[test]
    fn test_full_search_path_reconstruction() {
        // Simulate: start(1) → parent(10) → root(20) → child(5) → match → done
        let transitions = vec![
            PathTransition::SetStartNode { node: node(1, 1) },
            PathTransition::PushParent {
                parent: node(10, 2),
                edge: edge(1, 10, 0, 0),
            },
            PathTransition::SetRoot {
                root: node(20, 4),
                edge: edge(10, 20, 0, 0),
            },
            PathTransition::PushChild {
                child: node(5, 1),
                edge: edge(20, 5, 0, 1),
            },
            PathTransition::ChildMatch { cursor_pos: 2 },
            PathTransition::Done { success: true },
        ];

        let g = VizPathGraph::from_transitions(&transitions).unwrap();

        assert_eq!(g.start_node, Some(node(1, 1)));
        assert_eq!(g.start_path, vec![node(10, 2)]);
        assert_eq!(g.root, Some(node(20, 4)));
        assert_eq!(g.end_path, vec![node(5, 1)]);
        assert_eq!(g.cursor_pos, 2);
        assert!(g.done);
        assert!(g.success);
        assert_eq!(g.all_node_indices(), vec![1, 10, 20, 5]);
        assert_eq!(g.all_edges().len(), 3); // start_edge + root_edge + end_edge
    }

    #[test]
    fn test_mismatch_path() {
        let transitions = vec![
            PathTransition::SetStartNode { node: node(1, 1) },
            PathTransition::SetRoot {
                root: node(10, 2),
                edge: edge(1, 10, 0, 0),
            },
            PathTransition::PushChild {
                child: node(3, 1),
                edge: edge(10, 3, 0, 1),
            },
            PathTransition::ChildMismatch { cursor_pos: 1 },
            PathTransition::Done { success: false },
        ];

        let g = VizPathGraph::from_transitions(&transitions).unwrap();

        assert_eq!(g.start_node, Some(node(1, 1)));
        assert!(g.start_path.is_empty());
        assert_eq!(g.root, Some(node(10, 2)));
        assert_eq!(g.end_path, vec![node(3, 1)]);
        assert_eq!(g.cursor_pos, 1);
        assert!(g.done);
        assert!(!g.success);
    }

    #[test]
    fn test_backtrack_and_retry_path() {
        let transitions = vec![
            PathTransition::SetStartNode { node: node(1, 1) },
            PathTransition::SetRoot {
                root: node(10, 3),
                edge: edge(1, 10, 0, 0),
            },
            PathTransition::PushChild {
                child: node(3, 1),
                edge: edge(10, 3, 0, 1),
            },
            PathTransition::ChildMismatch { cursor_pos: 1 },
            PathTransition::PopChild,
            PathTransition::PushChild {
                child: node(4, 1),
                edge: edge(10, 4, 0, 2),
            },
            PathTransition::ChildMatch { cursor_pos: 2 },
            PathTransition::Done { success: true },
        ];

        let g = VizPathGraph::from_transitions(&transitions).unwrap();

        assert_eq!(g.end_path, vec![node(4, 1)]);
        assert_eq!(g.cursor_pos, 2);
        assert!(g.success);
    }

    // ---------------------------------------------------------------
    // JSON serialization round-trip
    // ---------------------------------------------------------------

    #[test]
    fn test_path_transition_json_roundtrip() {
        let transitions = vec![
            PathTransition::SetStartNode { node: node(1, 1) },
            PathTransition::PushParent {
                parent: node(10, 2),
                edge: edge(1, 10, 0, 0),
            },
            PathTransition::SetRoot {
                root: node(20, 4),
                edge: edge(10, 20, 0, 0),
            },
            PathTransition::PushChild {
                child: node(5, 1),
                edge: edge(20, 5, 0, 1),
            },
            PathTransition::PopChild,
            PathTransition::ReplaceChild {
                child: node(6, 1),
                edge: edge(20, 6, 0, 2),
            },
            PathTransition::ChildMatch { cursor_pos: 2 },
            PathTransition::ChildMismatch { cursor_pos: 3 },
            PathTransition::Done { success: true },
        ];

        for t in &transitions {
            let json = serde_json::to_string(t).unwrap();
            let deserialized: PathTransition =
                serde_json::from_str(&json).unwrap();
            assert_eq!(*t, deserialized);
        }
    }

    #[test]
    fn test_viz_path_graph_json_roundtrip() {
        let transitions = vec![
            PathTransition::SetStartNode { node: node(1, 1) },
            PathTransition::PushParent {
                parent: node(10, 2),
                edge: edge(1, 10, 0, 0),
            },
            PathTransition::SetRoot {
                root: node(20, 4),
                edge: edge(10, 20, 0, 0),
            },
            PathTransition::PushChild {
                child: node(5, 1),
                edge: edge(20, 5, 0, 1),
            },
            PathTransition::Done { success: true },
        ];

        let g = VizPathGraph::from_transitions(&transitions).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let deserialized: VizPathGraph =
            serde_json::from_str(&json).unwrap();
        assert_eq!(g, deserialized);
    }

    // ---------------------------------------------------------------
    // Cross-language test fixture generation
    // ---------------------------------------------------------------

    /// A test fixture case: name, transitions, and expected output.
    #[derive(Serialize, Deserialize)]
    struct TestFixture {
        name: String,
        transitions: Vec<PathTransition>,
        expected: VizPathGraph,
    }

    /// Generate all cross-language test fixtures.
    fn build_fixtures() -> Vec<TestFixture> {
        vec![
            TestFixture {
                name: "simple_start_only".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                ]).unwrap(),
            },
            TestFixture {
                name: "start_with_parents".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::PushParent {
                        parent: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushParent {
                        parent: node(20, 4),
                        edge: edge(10, 20, 0, 1),
                    },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::PushParent {
                        parent: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushParent {
                        parent: node(20, 4),
                        edge: edge(10, 20, 0, 1),
                    },
                ]).unwrap(),
            },
            TestFixture {
                name: "full_search_path".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::PushParent {
                        parent: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::SetRoot {
                        root: node(20, 4),
                        edge: edge(10, 20, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(5, 1),
                        edge: edge(20, 5, 0, 1),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::PushParent {
                        parent: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::SetRoot {
                        root: node(20, 4),
                        edge: edge(10, 20, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(5, 1),
                        edge: edge(20, 5, 0, 1),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ]).unwrap(),
            },
            TestFixture {
                name: "mismatch_path".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ChildMismatch { cursor_pos: 1 },
                    PathTransition::Done { success: false },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 2),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ChildMismatch { cursor_pos: 1 },
                    PathTransition::Done { success: false },
                ]).unwrap(),
            },
            TestFixture {
                name: "backtrack_and_retry".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 3),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ChildMismatch { cursor_pos: 1 },
                    PathTransition::PopChild,
                    PathTransition::PushChild {
                        child: node(4, 1),
                        edge: edge(10, 4, 0, 2),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 3),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ChildMismatch { cursor_pos: 1 },
                    PathTransition::PopChild,
                    PathTransition::PushChild {
                        child: node(4, 1),
                        edge: edge(10, 4, 0, 2),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ]).unwrap(),
            },
            TestFixture {
                name: "replace_child".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 3),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ReplaceChild {
                        child: node(4, 1),
                        edge: edge(10, 4, 0, 2),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(1, 1) },
                    PathTransition::SetRoot {
                        root: node(10, 3),
                        edge: edge(1, 10, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(10, 3, 0, 1),
                    },
                    PathTransition::ReplaceChild {
                        child: node(4, 1),
                        edge: edge(10, 4, 0, 2),
                    },
                    PathTransition::ChildMatch { cursor_pos: 2 },
                    PathTransition::Done { success: true },
                ]).unwrap(),
            },
            TestFixture {
                name: "deep_nested_path".into(),
                transitions: vec![
                    PathTransition::SetStartNode { node: node(0, 1) },
                    PathTransition::PushParent {
                        parent: node(5, 2),
                        edge: edge(0, 5, 0, 0),
                    },
                    PathTransition::PushParent {
                        parent: node(10, 3),
                        edge: edge(5, 10, 0, 1),
                    },
                    PathTransition::SetRoot {
                        root: node(20, 6),
                        edge: edge(10, 20, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(15, 3),
                        edge: edge(20, 15, 0, 1),
                    },
                    PathTransition::PushChild {
                        child: node(8, 2),
                        edge: edge(15, 8, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(8, 3, 0, 1),
                    },
                    PathTransition::ChildMatch { cursor_pos: 5 },
                    PathTransition::Done { success: true },
                ],
                expected: VizPathGraph::from_transitions(&[
                    PathTransition::SetStartNode { node: node(0, 1) },
                    PathTransition::PushParent {
                        parent: node(5, 2),
                        edge: edge(0, 5, 0, 0),
                    },
                    PathTransition::PushParent {
                        parent: node(10, 3),
                        edge: edge(5, 10, 0, 1),
                    },
                    PathTransition::SetRoot {
                        root: node(20, 6),
                        edge: edge(10, 20, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(15, 3),
                        edge: edge(20, 15, 0, 1),
                    },
                    PathTransition::PushChild {
                        child: node(8, 2),
                        edge: edge(15, 8, 0, 0),
                    },
                    PathTransition::PushChild {
                        child: node(3, 1),
                        edge: edge(8, 3, 0, 1),
                    },
                    PathTransition::ChildMatch { cursor_pos: 5 },
                    PathTransition::Done { success: true },
                ]).unwrap(),
            },
        ]
    }

    #[test]
    fn generate_cross_language_fixtures() {
        let fixtures = build_fixtures();
        let json = serde_json::to_string_pretty(&fixtures).unwrap();
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..") // up to context-engine root
            .join("tools/log-viewer/frontend/src/search-path/test-fixtures.json");
        std::fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
        std::fs::write(&fixture_path, &json).unwrap();
        eprintln!("Wrote test fixtures to {}", fixture_path.display());
    }
}
