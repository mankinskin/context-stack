//! Unified visualization types for graph operations (search, insert, read).
//!
//! These types are emitted as tracing events and consumed by the log-viewer
//! frontend to render step-by-step animations of algorithm execution.

use serde::Serialize;
use ts_rs::TS;

use crate::logging::tracing_utils::current_correlation_fields;

use super::search_path::{
    EdgeRef,
    VizPathGraph,
};

// Re-export PathNode for downstream crates
pub use super::search_path::PathNode;

// ---------------------------------------------------------------------------
// Operation Types
// ---------------------------------------------------------------------------

// ts-rs export_to path: see context_api::TS_EXPORT_DIR for the convention.
/// Operation type for categorizing events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
pub enum OperationType {
    Search,
    Insert,
    Read,
    Query,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
pub enum Transition {
    // ══════════════════════════════════════════════════════════════════════
    // Search-specific transitions (only emitted by context-search)
    // ══════════════════════════════════════════════════════════════════════
    /// Initial entry point — the first event for a search operation.
    ///
    /// **Emitted by:** `context-search` (search start).
    /// **Frontend:** Node gets `viz-start` (bright cyan, pulsing glow).
    StartNode {
        /// Token the operation starts at (index + width).
        node: PathNode,
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
        /// Node we are ascending from (index + width).
        from: PathNode,
        /// Parent node being explored (index + width).
        to: PathNode,
        /// Position within parent where `from` appears.
        entry_pos: usize,
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
        /// Parent node we are descending from (index + width).
        from: PathNode,
        /// Child node being explored (index + width).
        to: PathNode,
        /// Index within parent's pattern.
        child_index: usize,
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
        /// The child node that matched (index + width).
        node: PathNode,
        /// Atom position in the query where match occurred.
        cursor_pos: usize,
    },

    /// Child comparison failed — the child token does not match the query.
    ///
    /// **Emitted by:** `context-search` (during `process_child_comparison`).
    /// **Frontend:** Node gets `viz-mismatched` (red). `QueryInfo.active_token`
    ///   set to this node.
    ChildMismatch {
        /// The child node that mismatched (index + width).
        node: PathNode,
        /// Atom position where mismatch was detected.
        cursor_pos: usize,
        /// Token that was expected (index + width).
        expected: PathNode,
        /// Token that was found in the graph (index + width).
        actual: PathNode,
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
        /// Node that was rejected (index + width).
        node: PathNode,
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
        /// Root node being explored (index + width).
        root: PathNode,
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
        /// Token being split (index + width).
        node: PathNode,
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
        /// Newly created root token (index + width).
        node: PathNode,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "op", rename_all = "snake_case")]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
pub struct GraphMutation {
    /// Mutation operations applied at this step, in order.
    pub ops: Vec<DeltaOp>,
}

// ---------------------------------------------------------------------------
// Graph Replay Envelope (v1)
// ---------------------------------------------------------------------------

/// Current replay envelope schema version.
pub const GRAPH_REPLAY_SCHEMA_V1: &str = "graph-replay/v1";

/// Deterministic replay step payload for one graph operation frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
pub struct GraphReplayStep {
    /// Monotonically increasing step index within the operation stream.
    pub step: usize,
    /// Transition to apply for this frame.
    pub transition: Transition,
    /// Location hints for node/edge highlighting.
    pub location: LocationInfo,
    /// Query cursor context for search/read operations.
    pub query: QueryInfo,
    /// Human-readable description for this step.
    pub description: String,
    /// Snapshot-after transition for deterministic rendering.
    pub path_graph: VizPathGraph,
    /// Optional graph mutation payload (insert/update operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_mutation: Option<GraphMutation>,
}

/// Replay envelope used by log-viewer to reconstruct operation playback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
pub struct GraphReplayEnvelope {
    /// Replay schema version.
    pub schema_version: String,
    /// Canonical operation identifier (currently `path_id`).
    pub operation_id: String,
    /// Path-scoped stream identifier used by existing graph-op consumers.
    pub path_id: String,
    /// Operation category.
    pub op_type: OperationType,
    /// Optional cross-store correlation identifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_id: Option<String>,
    /// Deterministic replay frame.
    pub step: GraphReplayStep,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ReplayCorrelationIds {
    operation_id: Option<String>,
    run_id: Option<String>,
    session_id: Option<String>,
    journal_id: Option<String>,
}

fn parse_query_pairs(input: &str) -> impl Iterator<Item = (&str, &str)> {
    input.split('&').filter_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        Some((k.trim(), v.trim()))
    })
}

fn extract_correlation_ids(path_id: &str) -> ReplayCorrelationIds {
    let mut ids = ReplayCorrelationIds::default();

    // Support query-style metadata in path_id, e.g.
    // search/context-search/token-7-123?operation_id=op-1&run_id=r-1
    if let Some((_, query)) = path_id.split_once('?') {
        for (key, value) in parse_query_pairs(query) {
            if value.is_empty() {
                continue;
            }
            match key {
                "operation_id" => ids.operation_id = Some(value.to_string()),
                "run_id" => ids.run_id = Some(value.to_string()),
                "session_id" => ids.session_id = Some(value.to_string()),
                "journal_id" => ids.journal_id = Some(value.to_string()),
                _ => {},
            }
        }
    }

    // Support semicolon KV suffixes, e.g.
    // .../token-7-123;run_id=r-1;session_id=s-1
    if path_id.contains(';') {
        for segment in path_id.split(';').skip(1) {
            if let Some((key, value)) = segment.split_once('=') {
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match key.trim() {
                    "operation_id" if ids.operation_id.is_none() =>
                        ids.operation_id = Some(value.to_string()),
                    "run_id" if ids.run_id.is_none() =>
                        ids.run_id = Some(value.to_string()),
                    "session_id" if ids.session_id.is_none() =>
                        ids.session_id = Some(value.to_string()),
                    "journal_id" if ids.journal_id.is_none() =>
                        ids.journal_id = Some(value.to_string()),
                    _ => {},
                }
            }
        }
    }

    let span_ids = current_correlation_fields();
    if ids.operation_id.is_none() {
        ids.operation_id = span_ids.operation_id;
    }
    if ids.run_id.is_none() {
        ids.run_id = span_ids.run_id;
    }
    if ids.session_id.is_none() {
        ids.session_id = span_ids.session_id;
    }
    if ids.journal_id.is_none() {
        ids.journal_id = span_ids.journal_id;
    }

    // Environment fallback for callers that scope correlation at process level.
    if ids.run_id.is_none() {
        ids.run_id = std::env::var("CONTEXT_RUN_ID").ok();
    }
    if ids.session_id.is_none() {
        ids.session_id = std::env::var("CONTEXT_SESSION_ID").ok();
    }
    if ids.journal_id.is_none() {
        ids.journal_id = std::env::var("CONTEXT_JOURNAL_ID").ok();
    }

    ids
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "../../packages/context-types/src/generated/")]
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
    /// Convert this graph-op event into the `graph-replay/v1` envelope.
    pub fn to_replay_envelope(&self) -> GraphReplayEnvelope {
        let ids = extract_correlation_ids(&self.path_id);

        GraphReplayEnvelope {
            schema_version: GRAPH_REPLAY_SCHEMA_V1.to_string(),
            operation_id: ids.operation_id.unwrap_or_else(|| self.path_id.clone()),
            path_id: self.path_id.clone(),
            op_type: self.op_type,
            run_id: ids.run_id,
            session_id: ids.session_id,
            journal_id: ids.journal_id,
            step: GraphReplayStep {
                step: self.step,
                transition: self.transition.clone(),
                location: self.location.clone(),
                query: self.query.clone(),
                description: self.description.clone(),
                path_graph: self.path_graph.clone(),
                graph_mutation: self.graph_mutation.clone(),
            },
        }
    }

    /// Emit this event as a structured tracing log entry.
    ///
    /// The log-viewer frontend detects events by the presence of the
    /// `graph_op` field and parses it as JSON. The message is the
    /// human-readable description.
    pub fn emit(&self) {
        let json = serde_json::to_string(self).unwrap_or_default();
        let replay_json = serde_json::to_string(&self.to_replay_envelope())
            .unwrap_or_default();
        tracing::info!(
            graph_op = %json,
            graph_replay = %replay_json,
            step = self.step,
            operation_type = ?self.op_type,
            operation = "graph_op_event",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_envelope_uses_v1_schema_and_path_id_as_operation_id() {
        let event = GraphOpEvent {
            step: 7,
            op_type: OperationType::Search,
            transition: Transition::Done {
                final_node: Some(42),
                success: true,
            },
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: "done".to_string(),
            path_id: "search/context-search/token-42".to_string(),
            path_graph: VizPathGraph::default(),
            graph_mutation: None,
        };

        let envelope = event.to_replay_envelope();
        assert_eq!(envelope.schema_version, GRAPH_REPLAY_SCHEMA_V1);
        assert_eq!(envelope.operation_id, event.path_id);
        assert_eq!(envelope.path_id, "search/context-search/token-42");
        assert_eq!(envelope.op_type, OperationType::Search);
        assert_eq!(envelope.step.step, 7);
        assert_eq!(envelope.step.transition, event.transition);
    }

    #[test]
    fn replay_envelope_preserves_optional_mutation_payload() {
        let mutation = GraphMutation::single(DeltaOp::AddNode {
            index: 9,
            width: 3,
        });
        let event = GraphOpEvent {
            step: 1,
            op_type: OperationType::Insert,
            transition: Transition::CreateRoot {
                node: PathNode {
                    index: 9,
                    width: 3,
                },
            },
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: "create root".to_string(),
            path_id: "insert/context-insert/op-1".to_string(),
            path_graph: VizPathGraph::default(),
            graph_mutation: Some(mutation.clone()),
        };

        let envelope = event.to_replay_envelope();
        assert_eq!(envelope.step.graph_mutation, Some(mutation));
    }

    #[test]
    fn replay_envelope_extracts_ids_from_query_suffix() {
        let event = GraphOpEvent {
            step: 1,
            op_type: OperationType::Read,
            transition: Transition::Done {
                final_node: Some(9),
                success: true,
            },
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: "done".to_string(),
            path_id: "read/context-read/token-9-123?operation_id=op-9&run_id=run-1&session_id=s-1&journal_id=j-1".to_string(),
            path_graph: VizPathGraph::default(),
            graph_mutation: None,
        };

        let envelope = event.to_replay_envelope();
        assert_eq!(envelope.operation_id, "op-9");
        assert_eq!(envelope.run_id.as_deref(), Some("run-1"));
        assert_eq!(envelope.session_id.as_deref(), Some("s-1"));
        assert_eq!(envelope.journal_id.as_deref(), Some("j-1"));
    }

    #[test]
    fn replay_envelope_extracts_ids_from_semicolon_suffix() {
        let event = GraphOpEvent {
            step: 1,
            op_type: OperationType::Read,
            transition: Transition::Done {
                final_node: Some(9),
                success: true,
            },
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: "done".to_string(),
            path_id: "read/context-read/token-9-123;operation_id=op-9;run_id=run-1;session_id=s-1;journal_id=j-1".to_string(),
            path_graph: VizPathGraph::default(),
            graph_mutation: None,
        };

        let envelope = event.to_replay_envelope();
        assert_eq!(envelope.operation_id, "op-9");
        assert_eq!(envelope.run_id.as_deref(), Some("run-1"));
        assert_eq!(envelope.session_id.as_deref(), Some("s-1"));
        assert_eq!(envelope.journal_id.as_deref(), Some("j-1"));
    }

    #[test]
    fn replay_envelope_reads_correlation_from_active_span() {
        let _tracing = crate::init_test_tracing!();
        let span = tracing::info_span!(
            "replay_scope",
            operation_id = "op-span",
            run_id = "run-span",
            session_id = "session-span",
            journal_id = "journal-span"
        );
        let _entered = span.enter();

        let event = GraphOpEvent {
            step: 2,
            op_type: OperationType::Read,
            transition: Transition::Done {
                final_node: Some(3),
                success: true,
            },
            location: LocationInfo::default(),
            query: QueryInfo::default(),
            description: "done".to_string(),
            path_id: "read/context-read/token-3-123".to_string(),
            path_graph: VizPathGraph::default(),
            graph_mutation: None,
        };

        let envelope = event.to_replay_envelope();
        assert_eq!(envelope.operation_id, "op-span");
        assert_eq!(envelope.run_id.as_deref(), Some("run-span"));
        assert_eq!(envelope.session_id.as_deref(), Some("session-span"));
        assert_eq!(envelope.journal_id.as_deref(), Some("journal-span"));
    }
}
