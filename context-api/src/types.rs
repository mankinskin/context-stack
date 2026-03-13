//! Public API types for the context-api crate.
//!
//! These types form the stable external interface. Internal graph types
//! (`Token`, `VertexData`, `VertexIndex`, etc.) are never exposed directly;
//! instead, conversion functions build these API types from graph internals.
//!
//! ## Phase 2 Types
//!
//! - [`SearchResult`] — result of a search operation (complete/partial/not-found)
//! - [`PartialMatchInfo`] / [`PartialMatchKind`] — details about partial matches
//! - [`InsertResult`] — result of an insert operation (new or existing)
//! - [`PatternReadResult`] / [`ReadNode`] — recursive decomposition tree
//! - [`ValidationReport`] — graph integrity check results

use serde::{
    Deserialize,
    Serialize,
};

use context_trace::{
    VertexSet,
    graph::{
        Hypergraph,
        kind::BaseGraphKind,
        vertex::{
            VertexIndex,
            token::Token,
        },
    },
};

// Re-export snapshot types from context-trace so consumers don't need to
// depend on context-trace directly.
pub use context_trace::graph::snapshot::{
    GraphSnapshot as Snapshot,
    SnapshotEdge,
    SnapshotNode,
};

// ---------------------------------------------------------------------------
// TokenRef — how callers identify tokens across the API boundary
// ---------------------------------------------------------------------------

/// Reference to a token in the graph by numeric index or string label.
///
/// Adapters (CLI, MCP, HTTP) serialize this as part of command payloads.
/// Resolution into an actual vertex happens inside the API layer.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(untagged)]
pub enum TokenRef {
    /// Direct vertex index (numeric).
    Index(usize),
    /// Label string — single char resolves as atom, multi-char as sequence.
    Label(String),
}

impl std::fmt::Display for TokenRef {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            TokenRef::Index(i) => write!(f, "index({i})"),
            TokenRef::Label(s) => write!(f, "label(\"{s}\")"),
        }
    }
}

// ---------------------------------------------------------------------------
// AtomInfo
// ---------------------------------------------------------------------------

/// Information about a single atom (character) vertex.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct AtomInfo {
    /// Vertex index in the hypergraph.
    pub index: usize,
    /// The character value of this atom.
    pub ch: char,
}

impl AtomInfo {
    /// Build an `AtomInfo` from a graph token and its character value.
    pub fn new(
        token: Token,
        ch: char,
    ) -> Self {
        Self {
            index: token.index.0,
            ch,
        }
    }
}

// ---------------------------------------------------------------------------
// TokenInfo
// ---------------------------------------------------------------------------

/// Lightweight information about any vertex (atom or pattern).
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct TokenInfo {
    /// Vertex index in the hypergraph.
    pub index: usize,
    /// Human-readable label (e.g. `"a"` for an atom, `"abc"` for a merged token).
    pub label: String,
    /// Token width (1 for atoms, >1 for patterns).
    pub width: usize,
}

impl TokenInfo {
    /// Build a `TokenInfo` by reading vertex data from the graph.
    ///
    /// Returns `None` if the vertex index does not exist.
    pub fn from_graph(
        graph: &Hypergraph<BaseGraphKind>,
        token: Token,
    ) -> Option<Self> {
        let data = graph.get_vertex_data(token.index).ok()?;
        let label = graph.vertex_data_string(data);
        Some(Self {
            index: token.index.0,
            label,
            width: token.width.0,
        })
    }

    /// Build a `TokenInfo` from a vertex index, looking up width from graph.
    pub fn from_index(
        graph: &Hypergraph<BaseGraphKind>,
        index: VertexIndex,
    ) -> Option<Self> {
        let data = graph.get_vertex_data(index).ok()?;
        let token = data.to_token();
        let label = graph.vertex_data_string(data);
        Some(Self {
            index: index.0,
            label,
            width: token.width.0,
        })
    }
}

// ---------------------------------------------------------------------------
// PatternInfo
// ---------------------------------------------------------------------------

/// Information about a newly created pattern vertex.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct PatternInfo {
    /// Vertex index of the new pattern.
    pub index: usize,
    /// Human-readable label (concatenation of child atom chars).
    pub label: String,
    /// Token width (number of atoms spanned).
    pub width: usize,
    /// The direct children of this pattern.
    pub children: Vec<TokenInfo>,
}

// ---------------------------------------------------------------------------
// VertexInfo
// ---------------------------------------------------------------------------

/// Detailed information about a single vertex (atom or pattern).
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct VertexInfo {
    /// Vertex index in the hypergraph.
    pub index: usize,
    /// Human-readable label.
    pub label: String,
    /// Token width.
    pub width: usize,
    /// Whether this vertex is an atom (width == 1 and has no children).
    pub is_atom: bool,
    /// Child patterns — one `Vec<TokenInfo>` per child pattern of this vertex.
    pub children: Vec<Vec<TokenInfo>>,
    /// Number of parent vertices.
    pub parent_count: usize,
}

impl VertexInfo {
    /// Build a `VertexInfo` from a vertex index, reading all data from the graph.
    ///
    /// Returns `None` if the vertex does not exist.
    pub fn from_graph(
        graph: &Hypergraph<BaseGraphKind>,
        index: VertexIndex,
    ) -> Option<Self> {
        let data = graph.get_vertex_data(index).ok()?;
        let token = data.to_token();
        let label = graph.vertex_data_string(data.clone());
        let is_atom = graph.get_atom_by_key(&data.key()).is_some();

        // Build child pattern lists.
        // Sort by PatternId for deterministic ordering.
        let mut sorted_patterns: Vec<_> =
            data.child_patterns().iter().collect();
        sorted_patterns.sort_by_key(|(pid, _)| *pid);

        let children: Vec<Vec<TokenInfo>> = sorted_patterns
            .into_iter()
            .map(|(_pid, pattern)| {
                pattern
                    .iter()
                    .filter_map(|child_token| {
                        TokenInfo::from_graph(graph, *child_token)
                    })
                    .collect()
            })
            .collect();

        let parent_count = data.parents().len();

        Some(Self {
            index: index.0,
            label,
            width: token.width.0,
            is_atom,
            children,
            parent_count,
        })
    }
}

// ---------------------------------------------------------------------------
// WorkspaceInfo
// ---------------------------------------------------------------------------

/// Summary information about a workspace.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct WorkspaceInfo {
    /// Workspace name (also the directory name under `.context-engine/`).
    pub name: String,
    /// Number of vertices in the graph (0 if workspace is not open).
    pub vertex_count: usize,
    /// Number of atom vertices (0 if not open).
    pub atom_count: usize,
    /// Number of non-atom pattern vertices (0 if not open).
    pub pattern_count: usize,
    /// ISO-8601 timestamp when the workspace was created.
    pub created_at: String,
    /// ISO-8601 timestamp when the workspace was last modified.
    pub modified_at: String,
}

// ---------------------------------------------------------------------------
// GraphStatistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about the graph inside a workspace.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct GraphStatistics {
    /// Total number of vertices.
    pub vertex_count: usize,
    /// Number of atom (leaf) vertices.
    pub atom_count: usize,
    /// Number of non-atom (pattern) vertices.
    pub pattern_count: usize,
    /// Maximum token width among all vertices.
    pub max_width: usize,
    /// Total number of parent→child edges across all patterns.
    pub edge_count: usize,
}

impl GraphStatistics {
    /// Compute statistics from a live hypergraph.
    pub fn from_graph(graph: &Hypergraph<BaseGraphKind>) -> Self {
        let mut vertex_count: usize = 0;
        let mut atom_count: usize = 0;
        let mut max_width: usize = 0;
        let mut edge_count: usize = 0;

        for (_key, data) in graph.vertex_iter() {
            vertex_count += 1;
            let token = data.to_token();
            let w = token.width.0;
            if w > max_width {
                max_width = w;
            }

            // An atom has width 1 and is tracked in the atoms map.
            let is_atom = graph.get_atom_by_key(&data.key()).is_some();
            if is_atom {
                atom_count += 1;
            }

            // Count edges: sum of child-token counts across all patterns.
            for (_pid, pattern) in data.child_patterns().iter() {
                edge_count += pattern.len();
            }
        }

        let pattern_count = vertex_count.saturating_sub(atom_count);

        Self {
            vertex_count,
            atom_count,
            pattern_count,
            max_width,
            edge_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2 — Algorithm result types
// ---------------------------------------------------------------------------

/// Result of a search operation.
///
/// Indicates whether the query was found as a complete vertex, partially
/// matched, or not found at all.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct SearchResult {
    /// Whether the full query was found as a single existing vertex.
    pub complete: bool,
    /// The matched token (if complete).
    pub token: Option<TokenInfo>,
    /// Whether the entire query was consumed during search.
    pub query_exhausted: bool,
    /// Partial match information (if incomplete).
    pub partial: Option<PartialMatchInfo>,
}

/// Details about a partial match from a search operation.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct PartialMatchInfo {
    /// How the query was partially matched.
    pub kind: PartialMatchKind,
    /// The root token of the partial match path (if available).
    pub root_token: Option<TokenInfo>,
}

/// The kind of partial match found during a search.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum PartialMatchKind {
    /// Matched from the start of the query (postfix remaining).
    Postfix,
    /// Matched from the end (prefix remaining).
    Prefix,
    /// Matched a range in the middle.
    Range,
    /// No match at all / unknown partial state.
    None,
}

/// Result of an insert operation.
///
/// Contains the token representing the inserted (or already-existing) vertex,
/// plus a flag indicating whether the vertex was newly created.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct InsertResult {
    /// The token representing the inserted or existing pattern.
    pub token: TokenInfo,
    /// `true` if this vertex already existed in the graph; `false` if newly
    /// created by this insert.
    pub already_existed: bool,
}

/// Result of a read (decomposition) operation on a vertex.
///
/// Contains the root vertex info, its full leaf text, and a recursive
/// decomposition tree.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct PatternReadResult {
    /// The root vertex being read.
    pub root: TokenInfo,
    /// The full text (concatenated leaf atoms in order).
    pub text: String,
    /// Recursive decomposition tree.
    pub tree: ReadNode,
}

/// A node in the recursive decomposition tree.
///
/// Atoms are leaf nodes (no children). Pattern vertices have children
/// representing one decomposition of the pattern.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ReadNode {
    /// The token at this node.
    pub token: TokenInfo,
    /// Children of this node (empty for atoms / leaf nodes).
    pub children: Vec<ReadNode>,
}

/// Report from a graph validation check.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct ValidationReport {
    /// Whether the graph passed all checks.
    pub valid: bool,
    /// Total number of vertices checked.
    pub vertex_count: usize,
    /// List of issues found (empty if `valid` is `true`).
    pub issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::vertex::atom::Atom;

    #[test]
    fn token_ref_display() {
        assert_eq!(TokenRef::Index(42).to_string(), "index(42)");
        assert_eq!(TokenRef::Label("abc".into()).to_string(), "label(\"abc\")");
    }

    #[test]
    fn token_ref_serde_round_trip_index() {
        let tr = TokenRef::Index(7);
        let json = serde_json::to_string(&tr).unwrap();
        let deser: TokenRef = serde_json::from_str(&json).unwrap();
        assert_eq!(tr, deser);
    }

    #[test]
    fn token_ref_serde_round_trip_label() {
        let tr = TokenRef::Label("hello".into());
        let json = serde_json::to_string(&tr).unwrap();
        let deser: TokenRef = serde_json::from_str(&json).unwrap();
        assert_eq!(tr, deser);
    }

    #[test]
    fn atom_info_serde() {
        let info = AtomInfo { index: 0, ch: 'a' };
        let json = serde_json::to_string(&info).unwrap();
        let deser: AtomInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deser);
    }

    #[test]
    fn workspace_info_serde() {
        let info = WorkspaceInfo {
            name: "demo".into(),
            vertex_count: 10,
            atom_count: 5,
            pattern_count: 5,
            created_at: "2025-01-01T00:00:00Z".into(),
            modified_at: "2025-01-02T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: WorkspaceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deser);
    }

    #[test]
    fn graph_statistics_from_empty_graph() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let stats = GraphStatistics::from_graph(&graph);
        assert_eq!(stats.vertex_count, 0);
        assert_eq!(stats.atom_count, 0);
        assert_eq!(stats.pattern_count, 0);
        assert_eq!(stats.max_width, 0);
        assert_eq!(stats.edge_count, 0);
    }

    #[test]
    fn graph_statistics_with_atoms() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        graph.insert_atom(Atom::Element('a'));
        graph.insert_atom(Atom::Element('b'));

        let stats = GraphStatistics::from_graph(&graph);
        assert_eq!(stats.vertex_count, 2);
        assert_eq!(stats.atom_count, 2);
        assert_eq!(stats.pattern_count, 0);
        assert_eq!(stats.max_width, 1);
        assert_eq!(stats.edge_count, 0);
    }

    #[test]
    fn graph_statistics_with_pattern() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let ta = graph.insert_atom(Atom::Element('a'));
        let tb = graph.insert_atom(Atom::Element('b'));
        let _pattern = graph.insert_pattern(vec![ta, tb]);

        let stats = GraphStatistics::from_graph(&graph);
        assert_eq!(stats.vertex_count, 3);
        assert_eq!(stats.atom_count, 2);
        assert_eq!(stats.pattern_count, 1);
        assert_eq!(stats.max_width, 2);
        // The pattern vertex "ab" has one child pattern with 2 children → 2 edges
        assert_eq!(stats.edge_count, 2);
    }

    #[test]
    fn vertex_info_from_graph_atom() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let ta = graph.insert_atom(Atom::Element('x'));

        let info = VertexInfo::from_graph(&graph, ta.index)
            .expect("vertex should exist");
        assert_eq!(info.index, ta.index.0);
        assert_eq!(info.label, "x");
        assert_eq!(info.width, 1);
        assert!(info.is_atom);
        assert!(info.children.is_empty());
        assert_eq!(info.parent_count, 0);
    }

    #[test]
    fn vertex_info_from_graph_pattern() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let ta = graph.insert_atom(Atom::Element('a'));
        let tb = graph.insert_atom(Atom::Element('b'));
        let pattern = graph.insert_pattern(vec![ta, tb]);

        let info = VertexInfo::from_graph(&graph, pattern.index)
            .expect("vertex should exist");
        assert_eq!(info.index, pattern.index.0);
        assert_eq!(info.label, "ab");
        assert_eq!(info.width, 2);
        assert!(!info.is_atom);
        assert_eq!(info.children.len(), 1); // one child pattern
        assert_eq!(info.children[0].len(), 2); // two children in it
        assert_eq!(info.parent_count, 0);
    }

    #[test]
    fn vertex_info_nonexistent() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        assert!(VertexInfo::from_graph(&graph, VertexIndex(999)).is_none());
    }

    #[test]
    fn token_info_from_graph() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let ta = graph.insert_atom(Atom::Element('z'));

        let info =
            TokenInfo::from_graph(&graph, ta).expect("token should exist");
        assert_eq!(info.index, ta.index.0);
        assert_eq!(info.label, "z");
        assert_eq!(info.width, 1);
    }

    #[test]
    fn token_info_from_index() {
        let graph = Hypergraph::<BaseGraphKind>::default();
        let ta = graph.insert_atom(Atom::Element('q'));

        let info = TokenInfo::from_index(&graph, ta.index)
            .expect("vertex should exist");
        assert_eq!(info.index, ta.index.0);
        assert_eq!(info.label, "q");
        assert_eq!(info.width, 1);
    }

    #[test]
    fn pattern_info_serde() {
        let info = PatternInfo {
            index: 5,
            label: "ab".into(),
            width: 2,
            children: vec![
                TokenInfo {
                    index: 0,
                    label: "a".into(),
                    width: 1,
                },
                TokenInfo {
                    index: 1,
                    label: "b".into(),
                    width: 1,
                },
            ],
        };
        let json = serde_json::to_string(&info).unwrap();
        let deser: PatternInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deser);
    }

    #[test]
    fn graph_statistics_serde() {
        let stats = GraphStatistics {
            vertex_count: 10,
            atom_count: 5,
            pattern_count: 5,
            max_width: 4,
            edge_count: 12,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deser: GraphStatistics = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deser);
    }

    // -- Phase 2 type tests -------------------------------------------------

    #[test]
    fn search_result_complete_serde() {
        let result = SearchResult {
            complete: true,
            token: Some(TokenInfo {
                index: 5,
                label: "ab".into(),
                width: 2,
            }),
            query_exhausted: true,
            partial: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deser);
    }

    #[test]
    fn search_result_partial_serde() {
        let result = SearchResult {
            complete: false,
            token: None,
            query_exhausted: false,
            partial: Some(PartialMatchInfo {
                kind: PartialMatchKind::Postfix,
                root_token: Some(TokenInfo {
                    index: 3,
                    label: "abc".into(),
                    width: 3,
                }),
            }),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deser);
    }

    #[test]
    fn insert_result_serde() {
        let result = InsertResult {
            token: TokenInfo {
                index: 7,
                label: "hello".into(),
                width: 5,
            },
            already_existed: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: InsertResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deser);
    }

    #[test]
    fn pattern_read_result_serde() {
        let result = PatternReadResult {
            root: TokenInfo {
                index: 5,
                label: "ab".into(),
                width: 2,
            },
            text: "ab".into(),
            tree: ReadNode {
                token: TokenInfo {
                    index: 5,
                    label: "ab".into(),
                    width: 2,
                },
                children: vec![
                    ReadNode {
                        token: TokenInfo {
                            index: 0,
                            label: "a".into(),
                            width: 1,
                        },
                        children: vec![],
                    },
                    ReadNode {
                        token: TokenInfo {
                            index: 1,
                            label: "b".into(),
                            width: 1,
                        },
                        children: vec![],
                    },
                ],
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        let deser: PatternReadResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deser);
    }

    #[test]
    fn validation_report_serde() {
        let report = ValidationReport {
            valid: false,
            vertex_count: 10,
            issues: vec!["vertex 3 has dangling child reference".into()],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deser: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, deser);
    }

    #[test]
    fn partial_match_kind_variants() {
        for kind in [
            PartialMatchKind::Postfix,
            PartialMatchKind::Prefix,
            PartialMatchKind::Range,
            PartialMatchKind::None,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let deser: PartialMatchKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, deser);
        }
    }
}
