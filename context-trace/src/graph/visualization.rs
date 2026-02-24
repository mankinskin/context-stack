//! Unified visualization types for graph operations (search, insert, read).
//!
//! These types are emitted as tracing events and consumed by the log-viewer
//! frontend to render step-by-step animations of algorithm execution.

use serde::Serialize;
use ts_rs::TS;

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
    StartNode { node: usize },

    /// Exploring a parent node (bottom-up traversal)
    VisitParent {
        from: usize,
        to: usize,
        /// Position in parent where we entered
        entry_pos: usize,
    },

    /// Exploring a child node (top-down traversal)
    VisitChild {
        from: usize,
        to: usize,
        /// Child index within parent's pattern
        child_index: usize,
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
    RootExplore { root: usize },

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
/// let event = GraphOpEvent {
///     step: 0,
///     op_type: OperationType::Search,
///     transition: Transition::StartNode { node: 42 },
///     location: LocationInfo { selected_node: Some(42), ..Default::default() },
///     query: QueryInfo { query_tokens: vec![1, 2, 3], cursor_position: 0, query_width: 3 },
///     description: "Search started at node 42".into(),
/// };
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
        transition: Transition,
        description: impl Into<String>,
    ) -> Self {
        Self {
            step,
            op_type: OperationType::Search,
            transition,
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: description.into(),
        }
    }

    /// Create a new insert event.
    pub fn insert(
        step: usize,
        transition: Transition,
        description: impl Into<String>,
    ) -> Self {
        Self {
            step,
            op_type: OperationType::Insert,
            transition,
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: description.into(),
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
        }
    }
}
