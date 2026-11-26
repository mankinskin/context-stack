//! Serializable payload types of commands and results.
//!
//! Graph internals (`Token`, `VertexIndex`, compositions) never cross the
//! API: results carry plain indices, widths and (truncated) labels. All
//! offsets and widths are in atoms (chars).

pub mod logs;

use std::collections::BTreeMap;

use serde::{
    Deserialize,
    Serialize,
};

use crate::ts_export;

/// Integer-keyed maps: JSON object keys are strings, and inside the
/// flattened [`crate::Response`] serde cannot parse them as integers, so
/// the keys are read as strings and parsed here.
mod histogram {
    use std::collections::BTreeMap;

    use serde::{
        Deserialize,
        Deserializer,
        de::Error,
    };

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D
    ) -> Result<BTreeMap<usize, usize>, D::Error> {
        BTreeMap::<String, usize>::deserialize(deserializer)?
            .into_iter()
            .map(|(key, count)| {
                key.parse().map(|key| (key, count)).map_err(|_| {
                    D::Error::custom(format!("invalid histogram key '{key}'"))
                })
            })
            .collect()
    }
}

/// A graph: a name in the store (`<store>/<name>.ctx`) or a path ending in
/// `.ctx`.
pub type GraphRef = String;

ts_export! {
    /// A vertex by index or by its text.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum VertexRef {
        Index(usize),
        Text(String),
    }

    /// A vertex as seen by API clients.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct TokenInfo {
        pub index: usize,
        /// Width in atoms.
        pub width: usize,
        /// The vertex's text, truncated to the label width of the request.
        pub label: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pub label_truncated: bool,
    }

    /// Half-open atom range `[start, end)`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct Span {
        pub start: usize,
        pub end: usize,
    }

    // ---------------------------------------------------------------
    // Read inputs
    // ---------------------------------------------------------------

    /// A read input. Stdin is not a wire input: clients read it and send
    /// `Text` with label `"<stdin>"`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum Input {
        Text {
            text: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            label: Option<String>,
        },
        /// A file read by the executing process; refused unless the session
        /// allows file access below a directory.
        File { path: String },
    }

    /// How every input source is split into reader inputs.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Split {
        /// The whole source is one input.
        #[default]
        None,
        /// One input per non-empty line.
        Lines,
        /// One input per blank-line separated paragraph.
        Paragraphs,
    }

    /// When a mutating command saves the graph.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum SavePolicy {
        /// The session's policy (one-shot CLI and default server: save now).
        #[default]
        Auto,
        Now,
        /// Keep the change in memory until `save`, `close` or shutdown.
        Defer,
    }

    /// One entry of a graph's input log (application metadata, not a graph
    /// mark).
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct InputRecord {
        pub id: u32,
        /// Text label, file path or `"<stdin>"`.
        pub label: String,
        /// Vertex index of the whole input.
        pub token: usize,
        /// Length in chars.
        pub width: usize,
        /// RFC 3339.
        pub read_at: String,
    }

    // ---------------------------------------------------------------
    // Search
    // ---------------------------------------------------------------

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum SearchView {
        #[default]
        Summary,
        /// The search response: coverage, span and start/end paths.
        Path,
        /// Distinct ancestors containing the match, narrowest first.
        Contexts,
        /// Positions of the query in the read inputs.
        Occurrences,
        /// Direct parents of the matched vertex.
        Parents,
    }

    /// Result size and work limits; `None` takes the session default.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(default)]
    pub struct Limits {
        /// Maximum number of items (contexts, occurrences, parents, inputs).
        pub limit: Option<usize>,
        /// Chars of context on each side of a snippet.
        pub context_width: Option<usize>,
        /// Maximum label length in chars.
        pub label_width: Option<usize>,
        /// Maximum number of parent steps from the match.
        pub max_depth: Option<usize>,
        /// Work budget: visited `(ancestor, offset)` pairs.
        pub max_visited: Option<usize>,
    }

    /// Outcome of a search, included in every view.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum SearchOutcome {
        /// Some query chars were never read (or the query is empty).
        NotFound { unknown_chars: Vec<char> },
        /// The query is a vertex.
        Vertex { token: TokenInfo },
        /// The query occurs inside `host` (the narrowest vertex containing
        /// it) at `span`.
        InHost { host: TokenInfo, span: Span },
        /// Only the query prefix of `matched_chars` chars was found (inside
        /// `host` at `span`); the full query is not in the graph.
        Partial {
            host: TokenInfo,
            span: Span,
            matched_chars: usize,
            query_chars: usize,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SearchResult {
        pub graph: String,
        pub query: String,
        pub outcome: SearchOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub path: Option<PathInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub contexts: Option<ContextList>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub occurrences: Option<OccurrenceList>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub parents: Option<Vec<ParentInfo>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Coverage {
        EntireRoot,
        Prefix,
        Postfix,
        Range,
    }

    /// One step of a search path: `child` occurs in `parent` at `offset`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PathStep {
        pub parent: TokenInfo,
        pub offset: usize,
        pub child: TokenInfo,
    }

    /// The search response (`Response` of context-search) as data.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct PathInfo {
        pub root: TokenInfo,
        pub coverage: Coverage,
        /// Matched atoms in offsets of the root.
        pub span: Span,
        /// Query position of the match start.
        pub query_start: usize,
        /// Query position of the match end.
        pub checkpoint_position: usize,
        pub query_exhausted: bool,
        /// Top-down, root entry first.
        pub start_path: Vec<PathStep>,
        pub end_path: Vec<PathStep>,
        #[cfg_attr(feature = "ts-gen", ts(type = "number"))]
        pub generation: u64,
    }

    /// Text around a match.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct Snippet {
        pub left: String,
        pub matched: String,
        pub right: String,
        /// More text precedes `left`.
        pub left_truncated: bool,
        /// More text follows `right`.
        pub right_truncated: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ContextItem {
        pub ancestor: TokenInfo,
        /// Offsets of the match inside the ancestor, ascending.
        pub offsets: Vec<usize>,
        /// Snippet at the first offset.
        pub snippet: Snippet,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ContextList {
        /// Number of distinct ancestors; `None` if the work budget was hit.
        pub total: Option<usize>,
        pub truncated: bool,
        pub items: Vec<ContextItem>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct OccurrenceItem {
        pub input_id: u32,
        pub label: String,
        /// Char offset in the input.
        pub offset: usize,
        /// 1-based line and column (chars) of the offset.
        pub line: usize,
        pub column: usize,
        pub snippet: Snippet,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct OccurrenceList {
        /// Number of occurrences in all inputs; `None` if the work budget
        /// was hit.
        pub total: Option<usize>,
        pub truncated: bool,
        pub inputs_with_matches: usize,
        /// Ordered by input id, then offset.
        pub items: Vec<OccurrenceItem>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ParentInfo {
        pub parent: TokenInfo,
        pub offsets: Vec<usize>,
    }

    // ---------------------------------------------------------------
    // Graph information
    // ---------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GraphSummary {
        pub name: String,
        pub path: String,
        pub vertex_count: usize,
        pub atom_count: usize,
        pub composite_count: usize,
        pub input_count: usize,
        pub total_input_chars: usize,
        #[cfg_attr(feature = "ts-gen", ts(type = "number"))]
        pub generation: u64,
        pub created_at: String,
        pub modified_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts-gen", ts(type = "number | null"))]
        pub file_size: Option<u64>,
        /// Open in this session.
        pub open: bool,
        /// Unsaved changes in this session.
        pub dirty: bool,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GraphStats {
        pub summary: GraphSummary,
        pub max_width: usize,
        /// Width → number of vertices.
        #[serde(deserialize_with = "histogram::deserialize")]
        pub width_histogram: BTreeMap<usize, usize>,
        /// Parent occurrence count → number of vertices.
        #[serde(deserialize_with = "histogram::deserialize")]
        pub parent_count_histogram: BTreeMap<usize, usize>,
        /// Average number of occurrences per composition.
        pub avg_composition_size: f64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct OccurrenceInfo {
        pub offset: usize,
        pub child: TokenInfo,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct VertexInfo {
        pub token: TokenInfo,
        pub is_atom: bool,
        /// Maximal child occurrences by offset (empty for atoms).
        pub composition: Vec<OccurrenceInfo>,
        pub parents: Vec<ParentInfo>,
        /// Ids of the inputs this vertex is the whole of.
        pub inputs: Vec<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct InputList {
        pub total: usize,
        pub items: Vec<InputRecord>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ValidationReport {
        pub valid: bool,
        /// Structural errors (`PositionedGraph::validate`).
        pub errors: Vec<String>,
        /// Composites that are not inputs and have fewer than two parent
        /// occurrences (retention invariant G6).
        pub retention_violations: Vec<TokenInfo>,
    }

    // ---------------------------------------------------------------
    // Read
    // ---------------------------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub enum StepKind {
        Unknown,
        Forward { width: usize },
        Expansion { postfix: usize, expansion: usize },
    }

    /// One committed reader step (`context_read::Step`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct StepInfo {
        pub kind: StepKind,
        /// Root width after the step.
        pub root_width: usize,
        pub dissolved: usize,
        pub discarded: usize,
        pub stabilized: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ReadInputReport {
        pub id: u32,
        pub label: String,
        pub token: TokenInfo,
        pub chars: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub steps: Option<Vec<StepInfo>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SaveReport {
        pub path: String,
        #[cfg_attr(feature = "ts-gen", ts(type = "number"))]
        pub bytes: u64,
        #[cfg_attr(feature = "ts-gen", ts(type = "number"))]
        pub generation: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ReadReport {
        pub graph: String,
        pub inputs: Vec<ReadInputReport>,
        /// Labels of empty inputs that were skipped.
        pub skipped: Vec<String>,
        pub vertex_count_before: usize,
        pub vertex_count_after: usize,
        /// `None` if the save was deferred or this was a dry run.
        pub saved: Option<SaveReport>,
        pub dry_run: bool,
    }

    // ---------------------------------------------------------------
    // Render, export, import
    // ---------------------------------------------------------------

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum RenderFormat {
        #[default]
        Ascii,
        AsciiDag,
        Dot,
        Mermaid,
        /// Vertices and edges as data, for viewers.
        Json,
    }

    /// Size gate of render and content-keyed export; `None` takes the
    /// default (64 vertices, 4 096 label chars). `force` lifts the defaults
    /// but not the session's hard cap.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(default)]
    pub struct Gate {
        pub max_vertices: Option<usize>,
        pub max_label_chars: Option<usize>,
        pub force: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RenderEdge {
        pub parent: usize,
        pub child: usize,
        pub offset: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RenderGraph {
        pub vertices: Vec<TokenInfo>,
        pub edges: Vec<RenderEdge>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RenderOutput {
        pub format: RenderFormat,
        /// Text output (all formats except `json`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        /// Data output (`json`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub graph: Option<RenderGraph>,
        pub vertices: usize,
        pub edges: usize,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExportFormat {
        /// Content-keyed v3 interchange document (oracle format); gated.
        #[default]
        InterchangeV3,
        /// The `.ctx` file bytes, base64 encoded.
        Native,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ExportOutput {
        pub format: ExportFormat,
        /// JSON text (`interchange_v3`) or base64 (`native`).
        pub data: String,
        #[cfg_attr(feature = "ts-gen", ts(type = "number"))]
        pub bytes: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "format", rename_all = "snake_case")]
    pub enum ImportData {
        /// A v3 interchange JSON document.
        InterchangeV3 { document: String },
        /// `.ctx` file bytes, base64 encoded.
        Native { base64: String },
    }
}
