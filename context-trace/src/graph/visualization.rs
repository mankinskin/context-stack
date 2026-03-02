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
/// Format: `<op_type>/<module>/<semantic_id>`
/// e.g. `search/context-search/token-42-1234567890`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPathId<'a> {
    /// Operation type prefix (e.g. "search", "insert", "read").
    pub op_type: Option<&'a str>,
    /// Module name (e.g. "context-search", "context-insert").
    pub module: Option<&'a str>,
    /// The semantic identifier portion.
    pub semantic_id: &'a str,
}

/// Parse a `path_id` string into its namespaced components.
///
/// Expects `<op_type>/<module>/<semantic_id>` format.
/// Returns `None` for op_type/module if the string has fewer than 3 `/`-separated parts.
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
    /// Returns `None` for unrecognised prefixes.
    pub fn from_path_id(path_id: &str) -> Option<Self> {
        let parsed = parse_path_id(path_id);
        match parsed.op_type {
            Some("search") => Some(Self::Search),
            Some("insert") => Some(Self::Insert),
            Some("read") => Some(Self::Read),
            _ => None,
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
    // Search-specific transitions (only emitted by context-search)
    // ══════════════════════════════════════════════════════════════════════

    /// Initial entry point — the first event for a search operation.
    ///
    /// **Emitted by:** `context-search` (search start).
    /// **Frontend:** Node gets `viz-start` (bright cyan, pulsing glow).
    StartNode {
        /// Token index the operation starts at.
        node: usize,
        /// Atom count of the start token (for path width calculation).
        width: usize,
    },

    /// Exploring a parent node during bottom-up traversal.
    ///
    /// Fires when ascending from a child to its parent to find the longest
    /// matching prefix. Typically follows a `CandidateMismatch` of a parent candidate.
    ///
    /// **Emitted by:** `context-search` (parent candidate exploration).
    /// **Frontend:** `to` gets `viz-candidate-parent` (orange, pulsing).
    ///   Edge colored as candidate (muted violet, 30% alpha).
    VisitParent {
        /// Node we are ascending from.
        from: usize,
        /// Parent node being explored.
        to: usize,
        /// Position within parent where `from` appears.
        entry_pos: usize,
        /// Width (atom count) of the parent node.
        width: usize,
        /// Edge connecting `from → to` in the snapshot.
        edge: EdgeRef,
    },

    /// Exploring a child node during top-down comparison.
    ///
    /// Fires when descending from a root/parent to verify children match
    /// the query pattern.
    ///
    /// **Emitted by:** `context-search` (child comparison walk).
    /// **Frontend:** `to` gets `viz-candidate-child` (purple, pulsing).
    ///   Edge colored as candidate (muted violet, 30% alpha).
    VisitChild {
        /// Parent node we are descending from.
        from: usize,
        /// Child node being explored.
        to: usize,
        /// Index within parent's pattern.
        child_index: usize,
        /// Width (atom count) of the child node.
        width: usize,
        /// Edge connecting `from → to` in the snapshot.
        edge: EdgeRef,
        /// `true` if this replaces the current `end_path` tail (vs. push).
        replace: bool,
    },

    /// Child comparison succeeded — the child token matches the query at `cursor_pos`.
    ///
    /// **Emitted by:** `context-search` (during `process_child_comparison`).
    /// **Frontend:** Node gets `viz-matched` (green). `QueryInfo.active_token`
    ///   set to this node; `matched_positions` updated.
    ChildMatch {
        /// The child node that matched.
        node: usize,
        /// Atom position in the query where match occurred.
        cursor_pos: usize,
    },

    /// Child comparison failed — the child token does not match the query.
    ///
    /// **Emitted by:** `context-search` (during `process_child_comparison`).
    /// **Frontend:** Node gets `viz-mismatched` (red). `QueryInfo.active_token`
    ///   set to this node.
    ChildMismatch {
        /// The child node that mismatched.
        node: usize,
        /// Atom position where mismatch was detected.
        cursor_pos: usize,
        /// Token index that was expected.
        expected: usize,
        /// Token index that was found in the graph.
        actual: usize,
    },

    /// Terminal event — the search operation completed.
    ///
    /// **Emitted by:** `context-search` (match found or queue exhausted).
    /// **Frontend:** If `success`, `final_node` gets `viz-completed`.
    Done {
        /// Result node if successful, `None` otherwise.
        final_node: Option<usize>,
        /// Whether the operation found a match.
        success: bool,
    },

    /// A candidate was rejected after processing (mismatch or skip).
    ///
    /// Fires when `ProcessResult::Skipped` is returned — the popped
    /// candidate did not produce a root match.
    ///
    /// **Frontend:** Node gets `viz-selected`. Remaining queue shown via
    ///   `LocationInfo.pending_parents` / `pending_children`.
    CandidateMismatch {
        /// Node that was rejected.
        node: usize,
        /// Items left in queue after this rejection.
        queue_remaining: usize,
        /// `true` if this was a parent candidate, `false` for child.
        is_parent: bool,
    },

    /// A parent candidate became the new root — confirmed match.
    ///
    /// Fires when a parent candidate becomes the new root — the highest
    /// point in the upward path from which we now explore children.
    ///
    /// **Frontend:** `root` gets `viz-root` (gold ring via `::before`).
    ///   Edge colored gold (`SP_ROOT_EDGE_COLOR`).
    CandidateMatch {
        /// Root node being explored.
        root: usize,
        /// Width of the root node.
        width: usize,
        /// Edge from start_path top → root.
        edge: EdgeRef,
    },

    /// Root boundary reached — need to explore further parents.
    ///
    /// Fires when the search has fully matched the current root but the
    /// query extends beyond it. Parent candidates are queued.
    ///
    /// **Frontend:** `parent_candidates` added to `pendingParents`. Overlay
    ///   renderer carries these forward across subsequent steps.
    ParentExplore {
        /// Current root whose boundary was reached.
        current_root: usize,
        /// Parent nodes added to the queue for further exploration.
        parent_candidates: Vec<usize>,
    },

    // ══════════════════════════════════════════════════════════════════════
    // Insert-specific transitions (only emitted by context-insert)
    // ══════════════════════════════════════════════════════════════════════

    /// Beginning a split operation — a token must be broken at `split_position`.
    ///
    /// **Emitted by:** `context-insert/src/insert/context.rs`.
    /// **Frontend:** Node gets `viz-split-source` (warm orange, pulsing).
    SplitStart {
        /// Token being split.
        node: usize,
        /// Atom position where the split occurs.
        split_position: usize,
    },

    /// Split completed — the original token is now two fragments.
    ///
    /// **Emitted by:** `context-insert/src/insert/context.rs`.
    /// **Frontend:** `original_node` → `viz-split-source`, `left_fragment` →
    ///   `viz-split-left`, `right_fragment` → `viz-split-right`. Insert edges
    ///   colored warm orange. `GraphMutation` typically carries `AddNode` for
    ///   fragments.
    SplitComplete {
        /// The original node that was split.
        original_node: usize,
        /// Left fragment (atoms before split point), if created.
        left_fragment: Option<usize>,
        /// Right fragment (atoms after split point), if created.
        right_fragment: Option<usize>,
    },

    /// Beginning a join operation — multiple fragments will merge.
    ///
    /// **Emitted by:** `context-insert/src/insert/context.rs`.
    /// **Frontend:** First node in `nodes` is the primary focus.
    JoinStart {
        /// Nodes being joined.
        nodes: Vec<usize>,
    },

    /// A pairwise merge within a join — two inputs produce one output.
    ///
    /// May fire multiple times per join. Emitted during frontier traversal.
    ///
    /// **Emitted by:** `context-insert/src/join/context/frontier.rs`.
    /// **Frontend:** `left` → `viz-join-left`, `right` → `viz-join-right`,
    ///   `result` → `viz-join-result` (green, pulsing). Edges to result
    ///   colored green (`INSERT_JOIN_EDGE_COLOR`).
    JoinStep {
        /// Left input node.
        left: usize,
        /// Right input node.
        right: usize,
        /// Result node (new or reused).
        result: usize,
    },

    /// Join completed — the final merged token is ready.
    ///
    /// **Emitted by:** `context-insert/src/insert/context.rs`.
    /// **Frontend:** `result_node` → `viz-join-result` (green glow).
    JoinComplete {
        /// Final result of the join.
        result_node: usize,
    },

    /// A new pattern (ordered sequence of children) was added to a parent.
    ///
    /// **Emitted by:** `context-insert/src/join/context/node/merge/iter.rs`.
    /// **Frontend:** `parent` → `viz-new-pattern` (yellow), each child →
    ///   `viz-new-pattern-child` (light yellow). Insert edges colored warm
    ///   orange.
    CreatePattern {
        /// Token that owns the new pattern.
        parent: usize,
        /// Pattern index within the parent.
        pattern_id: usize,
        /// Child token indices in the pattern.
        children: Vec<usize>,
    },

    /// A new top-level token (root) was created.
    ///
    /// **Emitted by:** `context-insert`.
    /// **Frontend:** Node gets `viz-new-root` (bright white-gold, pulsing).
    CreateRoot {
        /// Newly created root token.
        node: usize,
        /// Width (atom count) of the new root.
        width: usize,
    },

    /// An existing pattern's children were modified.
    ///
    /// Fires after split replaces a child with fragments, or when a join
    /// produces a new child sequence.
    ///
    /// **Emitted by:** `context-insert`.
    /// **Frontend:** `parent` → `viz-new-pattern` (yellow). Insert edges for
    ///   new children colored warm orange.
    UpdatePattern {
        /// Token whose pattern is being updated.
        parent: usize,
        /// Index of the updated pattern.
        pattern_id: usize,
        /// Previous child sequence.
        old_children: Vec<usize>,
        /// Updated child sequence.
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

/// Graph mutation — describes mutations applied to the graph at a single step.
///
/// Carried as an optional field on `GraphOpEvent` so the frontend can
/// display before/after graph states during insert operations.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(
    export,
    export_to = "../../../tools/log-viewer/frontend/src/types/generated/"
)]
pub struct GraphMutation {
    /// Mutation operations applied at this step, in order.
    pub ops: Vec<DeltaOp>,
}

impl GraphMutation {
    /// Create a mutation with a single operation.
    pub fn single(op: DeltaOp) -> Self {
        Self { ops: vec![op] }
    }

    /// Create a mutation from multiple operations.
    pub fn new(ops: Vec<DeltaOp>) -> Self {
        Self { ops }
    }

    /// Whether this mutation is empty (no operations).
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

// ---------------------------------------------------------------------------
// GraphOpEvent - the main event type
// ---------------------------------------------------------------------------

/// Unified event for graph operation visualization.
///
/// Emitted as a `tracing::info!` event with the description as the message.
/// The log-viewer frontend detects events by the presence of the `graph_op`
/// field and parses it as JSON.
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

    /// Optional graph mutation — describes mutations to the graph at this step.
    /// Populated for insert operations that modify the graph structure
    /// (split, join, create pattern, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_mutation: Option<GraphMutation>,
}

impl GraphOpEvent {
    /// Emit this event as a structured tracing log entry.
    ///
    /// The log-viewer frontend detects events by the presence of the
    /// `graph_op` field and parses it as JSON. The message is the
    /// human-readable description.
    pub fn emit(&self) {
        let json = serde_json::to_string(self).unwrap_or_default();
        tracing::info!(
            graph_op = %json,
            step = self.step,
            op_type = ?self.op_type,
            "{}",
            self.description,
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
            graph_mutation: None,
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
            graph_mutation: None,
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

    /// Set graph mutation information.
    pub fn with_graph_mutation(
        mut self,
        mutation: GraphMutation,
    ) -> Self {
        self.graph_mutation = Some(mutation);
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
