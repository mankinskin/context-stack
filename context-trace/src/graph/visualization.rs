//! Unified visualization types for graph operations (search, insert, read).
//!
//! These types are emitted as tracing events and consumed by the log-viewer
//! frontend to render step-by-step animations of algorithm execution.

use serde::Serialize;
use ts_rs::TS;

use super::search_path::{EdgeRef, VizPathGraph};

// ---------------------------------------------------------------------------
// Operation Types
// ---------------------------------------------------------------------------

/// Operation type for categorizing events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub enum OperationType {
    Search,
    Insert,
    Read,
}

/// Parsed components of a namespaced `path_id`.
///
/// New format: `<op_type>/<module>/<semantic_id>`
/// e.g. `search/context-search/token-42-1234567890`
///
/// Legacy format (no slashes): treated as `semantic_id` only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPathId<'a> {
    /// Operation type prefix (e.g. "search", "insert", "read").
    /// `None` for legacy path_ids without slashes.
    pub op_type: Option<&'a str>,
    /// Module name (e.g. "context-search", "context-insert").
    /// `None` for legacy path_ids.
    pub module: Option<&'a str>,
    /// The semantic identifier portion.
    pub semantic_id: &'a str,
}

/// Parse a `path_id` string into its namespaced components.
///
/// Supports both new (`op/module/id`) and legacy (`search-42-...`) formats.
pub fn parse_path_id(path_id: &str) -> ParsedPathId<'_> {
    let mut parts = path_id.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(op), Some(module), Some(id)) => ParsedPathId {
            op_type: Some(op),
            module: Some(module),
            semantic_id: id,
        },
        _ => ParsedPathId {
            op_type: None,
            module: None,
            semantic_id: path_id,
        },
    }
}

impl OperationType {
    /// Infer the operation type from a `path_id` string.
    ///
    /// Returns `None` for unrecognised prefixes or legacy ids.
    pub fn from_path_id(path_id: &str) -> Option<Self> {
        let parsed = parse_path_id(path_id);
        match parsed.op_type {
            Some("search") => Some(Self::Search),
            Some("insert") => Some(Self::Insert),
            Some("read") => Some(Self::Read),
            _ => {
                // Legacy heuristic: "search-..." / "insert-..."
                if path_id.starts_with("search-") {
                    Some(Self::Search)
                } else if path_id.starts_with("insert-") {
                    Some(Self::Insert)
                } else {
                    None
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Transitions - describe what operation occurred (before → after)
// ---------------------------------------------------------------------------

/// Transition describes what operation occurred at this step.
///
/// Each variant represents a state change that can be visualized as an
/// animation frame. The frontend uses these to update node styling and
/// draw trace paths.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub enum Transition {
    // ══════════════════════════════════════════════════════════════════════
    // Common transitions (used by search, insert, read)
    // ══════════════════════════════════════════════════════════════════════
    /// Initial entry point - search/insert started at this node
    StartNode {
        node: usize,
        /// Token width (atom count) for path visualization
        width: usize,
    },

    /// Exploring a parent node (bottom-up traversal)
    VisitParent {
        from: usize,
        to: usize,
        /// Position in parent where we entered
        entry_pos: usize,
        /// Width of the parent node for path visualization
        width: usize,
        /// Edge connecting from → to in the snapshot
        edge: EdgeRef,
    },

    /// Exploring a child node (top-down traversal)
    VisitChild {
        from: usize,
        to: usize,
        /// Child index within parent's pattern
        child_index: usize,
        /// Width of the child node for path visualization
        width: usize,
        /// Edge connecting from → to in the snapshot
        edge: EdgeRef,
        /// Whether this replaces the current end_path tail (vs. push)
        replace: bool,
    },

    /// Child comparison succeeded - tokens match
    ChildMatch {
        node: usize,
        /// Atom position in the query where match occurred
        cursor_pos: usize,
    },

    /// Child comparison failed - tokens don't match
    ChildMismatch {
        node: usize,
        /// Atom position where mismatch was detected
        cursor_pos: usize,
        /// Expected token index
        expected: usize,
        /// Actual token index found
        actual: usize,
    },

    /// Operation complete
    Done {
        final_node: Option<usize>,
        success: bool,
    },

    // ══════════════════════════════════════════════════════════════════════
    // Search-specific transitions
    // ══════════════════════════════════════════════════════════════════════
    /// Popped a candidate from the BFS queue
    Dequeue {
        node: usize,
        /// Number of items remaining in queue
        queue_remaining: usize,
        /// Whether this is a parent or child candidate
        is_parent: bool,
    },

    /// Started exploring a root match via RootCursor
    RootExplore {
        root: usize,
        /// Width of the root node for path visualization
        width: usize,
        /// Edge connecting start_path top → root
        edge: EdgeRef,
    },

    /// Advanced match position within current root
    MatchAdvance {
        root: usize,
        /// Previous atom position
        prev_pos: usize,
        /// New atom position
        new_pos: usize,
    },

    /// Need to explore parents (root boundary reached)
    ParentExplore {
        current_root: usize,
        /// Parent candidates added to queue
        parent_candidates: Vec<usize>,
    },

    // ══════════════════════════════════════════════════════════════════════
    // Insert-specific transitions
    // ══════════════════════════════════════════════════════════════════════
    /// Starting a split operation on a node
    SplitStart {
        node: usize,
        /// Position where split occurs
        split_position: usize,
    },

    /// Split operation completed
    SplitComplete {
        original_node: usize,
        /// Left fragment (before split point)
        left_fragment: Option<usize>,
        /// Right fragment (after split point)
        right_fragment: Option<usize>,
    },

    /// Starting a join operation
    JoinStart {
        /// Nodes being joined
        nodes: Vec<usize>,
    },

    /// Join step - merging two fragments
    JoinStep {
        left: usize,
        right: usize,
        /// Result of joining (new or existing node)
        result: usize,
    },

    /// Join operation completed
    JoinComplete { result_node: usize },

    /// Creating a new pattern in the graph
    CreatePattern {
        /// Parent token that owns this pattern
        parent: usize,
        /// Pattern ID within the parent
        pattern_id: usize,
        /// Child token indices
        children: Vec<usize>,
    },

    /// Creating a new root node (top-level token)
    CreateRoot {
        node: usize,
        /// Width of the new token
        width: usize,
    },

    /// Updating an existing pattern
    UpdatePattern {
        parent: usize,
        pattern_id: usize,
        /// Old children
        old_children: Vec<usize>,
        /// New children
        new_children: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// Location Info - styling hints for the frontend
// ---------------------------------------------------------------------------

/// Location information for visualization styling.
///
/// The frontend maps these to CSS classes for node coloring:
/// - `selected_node` → bright cyan, pulsing
/// - `root_node` → gold ring
/// - `trace_path` → connected with trace line
/// - `completed_nodes` → green
/// - `pending_nodes` → orange (parents) / purple (children)
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct LocationInfo {
    /// Primary node being operated on (selected in UI)
    pub selected_node: Option<usize>,

    /// Root of current exploration (gold ring)
    pub root_node: Option<usize>,

    /// Path from root to current position (trace line)
    /// Ordered from root to leaf.
    pub trace_path: Vec<usize>,

    /// Nodes confirmed as complete/matched (green)
    pub completed_nodes: Vec<usize>,

    /// Nodes pending/queued - parents (orange)
    pub pending_parents: Vec<usize>,

    /// Nodes pending/queued - children (purple)
    pub pending_children: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Query Info - search pattern context
// ---------------------------------------------------------------------------

/// Information about the search/insert query.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct QueryInfo {
    /// Token indices in the query pattern
    pub query_tokens: Vec<usize>,

    /// Current cursor position (atom index) in the query
    pub cursor_position: usize,

    /// Total width of the query in atoms
    pub query_width: usize,

    /// Atom positions that have been confirmed as matched so far.
    /// The frontend uses this to highlight matched portions of the query.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_positions: Vec<usize>,

    /// Token index that was just compared (for match/mismatch highlighting).
    /// Points into the graph, not the query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_token: Option<usize>,
}

// ---------------------------------------------------------------------------
// Graph Delta - describes graph mutations for insert visualization
// ---------------------------------------------------------------------------

/// A single graph mutation operation.
///
/// Used to describe what changed in the graph during an insert step.
/// The frontend uses these to show before/after states and highlight
/// newly created or removed nodes/edges.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "op", rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub enum DeltaOp {
    /// A new node (token) was created in the graph
    AddNode {
        index: usize,
        /// Width (atom count) of the new token
        width: usize,
    },
    /// A node was removed from the graph
    RemoveNode { index: usize },
    /// A new edge was added (parent → child in a pattern)
    AddEdge {
        from: usize,
        to: usize,
        pattern_id: usize,
    },
    /// An edge was removed
    RemoveEdge {
        from: usize,
        to: usize,
        pattern_id: usize,
    },
    /// A node's data was updated (e.g. width changed after split)
    UpdateNode {
        index: usize,
        /// Human-readable description of what changed
        detail: String,
    },
}

/// Graph delta — describes mutations applied to the graph at a single step.
///
/// Carried as an optional field on `GraphOpEvent` so the frontend can
/// display before/after graph states during insert operations.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct GraphDelta {
    /// Mutation operations applied at this step, in order.
    pub ops: Vec<DeltaOp>,
}

impl GraphDelta {
    /// Create a delta with a single operation.
    pub fn single(op: DeltaOp) -> Self {
        Self { ops: vec![op] }
    }

    /// Create a delta from multiple operations.
    pub fn new(ops: Vec<DeltaOp>) -> Self {
        Self { ops }
    }

    /// Whether this delta is empty (no mutations).
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GraphOpEvent - the main event type
// ---------------------------------------------------------------------------

/// Unified event for graph operation visualization.
///
/// Emitted as a `tracing::info!` event with `message == "graph_op"`.
/// The log-viewer frontend parses the `graph_op` field as JSON.
///
/// # Example
///
/// ```ignore
/// let event = GraphOpEvent::search(0, "search-42-0", Transition::StartNode { node: 42 }, "Search started at node 42")
///     .with_location(LocationInfo::selected(42))
///     .with_query(QueryInfo::new(vec![1, 2, 3], 0, 3));
/// event.emit();
/// ```
#[derive(Debug, Clone, Serialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct GraphOpEvent {
    /// Monotonically increasing step counter (per operation)
    pub step: usize,

    /// Operation type (Search/Insert/Read)
    pub op_type: OperationType,

    /// The transition that occurred at this step
    pub transition: Transition,

    /// Location info for UI styling
    pub location: LocationInfo,

    /// Query/pattern information
    pub query: QueryInfo,

    /// Human-readable description of what happened
    pub description: String,

    /// Search path identifier (scopes to a particular operation).
    /// Every graph-op event belongs to exactly one path. Multiple
    /// concurrent operations in the same log are distinguished by this id.
    pub path_id: String,

    /// Full path graph snapshot AFTER applying the transition.
    /// Redundant (can reconstruct from transitions in order), but included
    /// for debugging and so the frontend can display the path without
    /// reconstructing from history.
    pub path_graph: VizPathGraph,

    /// Optional graph delta — describes mutations to the graph at this step.
    /// Populated for insert operations that modify the graph structure
    /// (split, join, create pattern, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_delta: Option<GraphDelta>,
}

impl GraphOpEvent {
    /// Emit this event as a structured tracing log entry.
    ///
    /// The log-viewer frontend looks for entries with
    /// `message == "graph_op"` and parses the `graph_op` field.
    pub fn emit(&self) {
        let json = serde_json::to_string(self).unwrap_or_default();
        tracing::info!(
            graph_op = %json,
            step = self.step,
            op_type = ?self.op_type,
            "graph_op"
        );
    }
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

impl GraphOpEvent {
    /// Create a new search event.
    pub fn search(
        step: usize,
        path_id: impl Into<String>,
        transition: Transition,
        path_graph: VizPathGraph,
        description: impl Into<String>,
    ) -> Self {
        Self {
            step,
            op_type: OperationType::Search,
            transition,
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: description.into(),
            path_id: path_id.into(),
            path_graph,
            graph_delta: None,
        }
    }

    /// Create a new insert event.
    pub fn insert(
        step: usize,
        path_id: impl Into<String>,
        transition: Transition,
        path_graph: VizPathGraph,
        description: impl Into<String>,
    ) -> Self {
        Self {
            step,
            op_type: OperationType::Insert,
            transition,
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: description.into(),
            path_id: path_id.into(),
            path_graph,
            graph_delta: None,
        }
    }

    /// Set location info.
    pub fn with_location(
        mut self,
        location: LocationInfo,
    ) -> Self {
        self.location = location;
        self
    }

    /// Set query info.
    pub fn with_query(
        mut self,
        query: QueryInfo,
    ) -> Self {
        self.query = query;
        self
    }

    /// Override the path graph snapshot.
    pub fn with_path_graph(
        mut self,
        graph: VizPathGraph,
    ) -> Self {
        self.path_graph = graph;
        self
    }

    /// Set graph delta information.
    pub fn with_graph_delta(
        mut self,
        delta: GraphDelta,
    ) -> Self {
        self.graph_delta = Some(delta);
        self
    }
}

impl LocationInfo {
    /// Create location info with just the selected node.
    pub fn selected(node: usize) -> Self {
        Self {
            selected_node: Some(node),
            ..Default::default()
        }
    }

    /// Set the root node.
    pub fn with_root(
        mut self,
        root: usize,
    ) -> Self {
        self.root_node = Some(root);
        self
    }

    /// Set the trace path.
    pub fn with_trace(
        mut self,
        path: Vec<usize>,
    ) -> Self {
        self.trace_path = path;
        self
    }

    /// Add completed nodes.
    pub fn with_completed(
        mut self,
        nodes: Vec<usize>,
    ) -> Self {
        self.completed_nodes = nodes;
        self
    }

    /// Set pending parent candidates.
    pub fn with_pending_parents(
        mut self,
        nodes: Vec<usize>,
    ) -> Self {
        self.pending_parents = nodes;
        self
    }

    /// Set pending child candidates.
    pub fn with_pending_children(
        mut self,
        nodes: Vec<usize>,
    ) -> Self {
        self.pending_children = nodes;
        self
    }
}

impl QueryInfo {
    /// Create query info from token indices.
    pub fn new(
        tokens: Vec<usize>,
        cursor: usize,
        width: usize,
    ) -> Self {
        Self {
            query_tokens: tokens,
            cursor_position: cursor,
            query_width: width,
            matched_positions: Vec::new(),
            active_token: None,
        }
    }
}
