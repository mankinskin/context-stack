//! Commands, the request/response envelope and command results.
//!
//! Every transport (CLI, HTTP, WebSocket, MCP) sends a [`Request`] to
//! [`crate::Session::execute`] and returns its [`Response`] unchanged.
//!
//! JSON shapes:
//! - request: `{"id": 1, "command": "search", "graph": "g", "query": "ab"}`
//! - success: `{"id": 1, "ok": {"search": {...}}}`
//! - failure: `{"id": 1, "error": {"kind": "not_found", "message": "..."}}`

use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    error::{
        ApiError,
        ErrorResponse,
    },
    ts_export,
    types::*,
};

ts_export! {
    /// A command of the context API. `read` (and `create`, `import`) are the
    /// only commands that add vertices; there is no insert command.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(tag = "command", rename_all = "snake_case")]
    pub enum Command {
        // ── lifecycle ────────────────────────────────────────────
        /// Create an empty graph.
        New {
            graph: GraphRef,
            #[serde(default)]
            overwrite: bool,
        },
        /// Graphs in the store.
        List,
        Info { graph: GraphRef },
        Delete { graph: GraphRef },
        /// Save an open graph (sessions with deferred saving).
        Save { graph: GraphRef },
        /// Close an open graph, saving it first if `save`.
        Close {
            graph: GraphRef,
            #[serde(default)]
            save: bool,
        },

        // ── read (the only mutation) ─────────────────────────────
        Read {
            graph: GraphRef,
            inputs: Vec<Input>,
            #[serde(default)]
            split: Split,
            /// Create the graph if it does not exist.
            #[serde(default)]
            create: bool,
            #[serde(default)]
            save: SavePolicy,
            /// Include reader steps in the report.
            #[serde(default)]
            steps: bool,
            /// Read into a copy; report without changing the graph.
            #[serde(default)]
            dry_run: bool,
        },
        /// `new` + `read` + save.
        Create {
            graph: GraphRef,
            inputs: Vec<Input>,
            #[serde(default)]
            split: Split,
            #[serde(default)]
            overwrite: bool,
        },

        // ── queries ──────────────────────────────────────────────
        Search {
            graph: GraphRef,
            query: String,
            #[serde(default)]
            view: SearchView,
            #[serde(default)]
            limits: Limits,
        },
        Vertex {
            graph: GraphRef,
            vertex: VertexRef,
            #[serde(default)]
            limits: Limits,
        },
        Inputs {
            graph: GraphRef,
            #[serde(default)]
            limits: Limits,
        },
        Stats { graph: GraphRef },
        Validate { graph: GraphRef },
        Render {
            graph: GraphRef,
            #[serde(default)]
            format: RenderFormat,
            /// Render only the descendants of this vertex.
            #[serde(default)]
            root: Option<VertexRef>,
            /// Maximum child steps below `root`.
            #[serde(default)]
            depth: Option<usize>,
            #[serde(default)]
            gate: Gate,
            #[serde(default)]
            limits: Limits,
        },
        Export {
            graph: GraphRef,
            #[serde(default)]
            format: ExportFormat,
            #[serde(default)]
            gate: Gate,
        },
        Import {
            graph: GraphRef,
            data: ImportData,
            #[serde(default)]
            overwrite: bool,
        },

        // ── trace logs (`<graph>.ctx.logs/`) ─────────────────────
        #[cfg(feature = "logs")]
        ListLogs {
            graph: GraphRef,
            #[serde(default)]
            pattern: Option<String>,
            #[serde(default = "default_log_limit")]
            limit: usize,
        },
        #[cfg(feature = "logs")]
        GetLog {
            graph: GraphRef,
            filename: String,
            #[serde(default)]
            filter: Option<String>,
            #[serde(default = "default_log_limit")]
            limit: usize,
            #[serde(default)]
            offset: usize,
        },
        #[cfg(feature = "logs")]
        QueryLog {
            graph: GraphRef,
            filename: String,
            query: String,
            #[serde(default = "default_log_limit")]
            limit: usize,
        },
        #[cfg(feature = "logs")]
        AnalyzeLog { graph: GraphRef, filename: String },
        #[cfg(feature = "logs")]
        SearchLogs {
            graph: GraphRef,
            query: String,
            #[serde(default = "default_search_limit_per_file")]
            limit_per_file: usize,
        },
        #[cfg(feature = "logs")]
        DeleteLog { graph: GraphRef, filename: String },
        #[cfg(feature = "logs")]
        DeleteLogs {
            graph: GraphRef,
            #[serde(default)]
            older_than_days: Option<u32>,
        },
    }
}

#[cfg(feature = "logs")]
fn default_log_limit() -> usize {
    100
}

#[cfg(feature = "logs")]
fn default_search_limit_per_file() -> usize {
    10
}

impl Command {
    /// The graph the command targets (`None` for `list`).
    pub fn graph(&self) -> Option<&str> {
        match self {
            Command::List => None,
            Command::New { graph, .. }
            | Command::Info { graph }
            | Command::Delete { graph }
            | Command::Save { graph }
            | Command::Close { graph, .. }
            | Command::Read { graph, .. }
            | Command::Create { graph, .. }
            | Command::Search { graph, .. }
            | Command::Vertex { graph, .. }
            | Command::Inputs { graph, .. }
            | Command::Stats { graph }
            | Command::Validate { graph }
            | Command::Render { graph, .. }
            | Command::Export { graph, .. }
            | Command::Import { graph, .. } => Some(graph),
            #[cfg(feature = "logs")]
            Command::ListLogs { graph, .. }
            | Command::GetLog { graph, .. }
            | Command::QueryLog { graph, .. }
            | Command::AnalyzeLog { graph, .. }
            | Command::SearchLogs { graph, .. }
            | Command::DeleteLog { graph, .. }
            | Command::DeleteLogs { graph, .. } => Some(graph),
        }
    }

    /// Whether the command may change a graph or the store (needs the
    /// graph's writer slot and an exclusive lock).
    pub fn is_mutation(&self) -> bool {
        match self {
            Command::New { .. }
            | Command::Delete { .. }
            | Command::Save { .. }
            | Command::Close { .. }
            | Command::Create { .. }
            | Command::Import { .. } => true,
            Command::Read { dry_run, .. } => !dry_run,
            _ => false,
        }
    }

    /// The `snake_case` command name (the `"command"` tag).
    pub fn name(&self) -> &'static str {
        match self {
            Command::New { .. } => "new",
            Command::List => "list",
            Command::Info { .. } => "info",
            Command::Delete { .. } => "delete",
            Command::Save { .. } => "save",
            Command::Close { .. } => "close",
            Command::Read { .. } => "read",
            Command::Create { .. } => "create",
            Command::Search { .. } => "search",
            Command::Vertex { .. } => "vertex",
            Command::Inputs { .. } => "inputs",
            Command::Stats { .. } => "stats",
            Command::Validate { .. } => "validate",
            Command::Render { .. } => "render",
            Command::Export { .. } => "export",
            Command::Import { .. } => "import",
            #[cfg(feature = "logs")]
            Command::ListLogs { .. } => "list_logs",
            #[cfg(feature = "logs")]
            Command::GetLog { .. } => "get_log",
            #[cfg(feature = "logs")]
            Command::QueryLog { .. } => "query_log",
            #[cfg(feature = "logs")]
            Command::AnalyzeLog { .. } => "analyze_log",
            #[cfg(feature = "logs")]
            Command::SearchLogs { .. } => "search_logs",
            #[cfg(feature = "logs")]
            Command::DeleteLog { .. } => "delete_log",
            #[cfg(feature = "logs")]
            Command::DeleteLogs { .. } => "delete_logs",
        }
    }
}

ts_export! {
    /// A command with an optional client-chosen id, echoed in the response.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts-gen", ts(type = "number | null"))]
        pub id: Option<u64>,
        #[serde(flatten)]
        pub command: Command,
    }

    /// Result of a successful command.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum CommandResult {
        Graph(GraphSummary),
        Graphs { graphs: Vec<GraphSummary> },
        Deleted { graph: String },
        Saved(SaveReport),
        Closed {
            graph: String,
            saved: Option<SaveReport>,
        },
        Read(ReadReport),
        Search(SearchResult),
        Vertex(VertexInfo),
        Inputs(InputList),
        Stats(GraphStats),
        Validation(ValidationReport),
        Render(RenderOutput),
        Export(ExportOutput),
        #[cfg(feature = "logs")]
        LogList { logs: Vec<logs::LogFileInfo> },
        #[cfg(feature = "logs")]
        LogEntries {
            filename: String,
            total: usize,
            offset: usize,
            limit: usize,
            returned: usize,
            entries: Vec<logs::LogEntryInfo>,
        },
        #[cfg(feature = "logs")]
        LogQueryResult {
            query: String,
            matches: usize,
            entries: Vec<logs::LogEntryInfo>,
        },
        #[cfg(feature = "logs")]
        LogAnalysis(logs::LogAnalysis),
        #[cfg(feature = "logs")]
        LogSearchResult {
            query: String,
            files_with_matches: usize,
            results: Vec<logs::LogFileSearchResult>,
        },
        #[cfg(feature = "logs")]
        LogDeleteResult(logs::LogDeleteResult),
    }

    /// Success or failure of a command.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Outcome {
        Ok(CommandResult),
        Error(ErrorResponse),
    }

    /// Reply to a [`Request`].
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts-gen", ts(type = "number | null"))]
        pub id: Option<u64>,
        #[serde(flatten)]
        pub outcome: Outcome,
    }
}

impl Request {
    pub fn new(command: Command) -> Self {
        Self { id: None, command }
    }

    pub fn with_id(
        id: u64,
        command: Command,
    ) -> Self {
        Self {
            id: Some(id),
            command,
        }
    }
}

impl Response {
    pub fn from_result(
        id: Option<u64>,
        result: Result<CommandResult, ApiError>,
    ) -> Self {
        let outcome = match result {
            Ok(result) => Outcome::Ok(result),
            Err(err) => Outcome::Error(ErrorResponse::from(&err)),
        };
        Self { id, outcome }
    }

    pub fn into_result(self) -> Result<CommandResult, ErrorResponse> {
        match self.outcome {
            Outcome::Ok(result) => Ok(result),
            Outcome::Error(err) => Err(err),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self.outcome, Outcome::Ok(_))
    }
}
