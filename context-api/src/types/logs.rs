//! Log file types (trace capture and log queries).

use std::collections::HashMap;

use serde::{
    Deserialize,
    Serialize,
};

use crate::ts_export;


ts_export! {
    /// Information about a log file in a workspace.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LogFileInfo {
        /// The log filename (not the full path).
        pub filename: String,
        /// File size in bytes.
        pub size: u64,
        /// Last modified time as RFC 3339 string.
        pub modified: String,
        /// The command name that produced this log.
        pub command: String,
        /// Whether the log contains graph snapshot data.
        #[serde(default)]
        pub has_graph_snapshot: bool,
        /// Whether the log contains search operations.
        #[serde(default)]
        pub has_search_ops: bool,
        /// Whether the log contains insert operations.
        #[serde(default)]
        pub has_insert_ops: bool,
        /// Whether the log contains search path transitions.
        #[serde(default)]
        pub has_search_paths: bool,
    }
}

ts_export! {
    /// A parsed log entry — the simplified API view of a full `LogEntry`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LogEntryInfo {
        /// Entry number (1-based).
        pub entry_number: usize,
        /// Log level (TRACE, DEBUG, INFO, WARN, ERROR).
        pub level: String,
        /// Timestamp string (if present).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<String>,
        /// The log message.
        pub message: String,
        /// Event type (event, span_enter, span_exit, span_new, span_close).
        pub event_type: String,
        /// Span name (if this is a span event).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub span_name: Option<String>,
        /// Indentation depth (number of parent spans).
        pub depth: usize,
        /// Additional fields as a JSON value.
        #[schemars(with = "serde_json::Value")]
        pub fields: serde_json::Value,
        /// Source file location.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source_file: Option<String>,
        /// Source line number.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source_line: Option<u32>,
    }
}

ts_export! {
    /// Analysis summary of a log file.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LogAnalysis {
        /// Total number of log entries.
        pub total_entries: usize,
        /// Entry count by log level.
        pub by_level: HashMap<String, usize>,
        /// Entry count by event type.
        pub by_event_type: HashMap<String, usize>,
        /// Span summaries.
        pub spans: Vec<SpanSummary>,
        /// Error entries (level == ERROR).
        pub errors: Vec<LogEntryInfo>,
    }
}

ts_export! {
    /// Summary of a tracing span.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct SpanSummary {
        /// Span name.
        pub name: String,
        /// Number of times this span was entered.
        pub count: usize,
        /// Whether any error events occurred inside this span.
        pub has_errors: bool,
    }
}

ts_export! {
    /// Brief trace summary for inclusion in execute responses.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct TraceSummary {
        /// The log filename.
        pub log_file: String,
        /// Total number of entries captured.
        pub entry_count: usize,
        /// Event counts by category (e.g., "search": 12, "insert": 35).
        pub event_summary: HashMap<String, usize>,
        /// Duration of the captured execution in milliseconds.
        pub duration_ms: u64,
    }
}

ts_export! {
    /// Result of deleting log files.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LogDeleteResult {
        /// Number of log files deleted.
        pub deleted_count: usize,
        /// Total bytes freed.
        pub freed_bytes: u64,
    }
}

ts_export! {
    /// Per-file results from a cross-file log search.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct LogFileSearchResult {
        /// The log filename.
        pub filename: String,
        /// Number of matching entries in this file.
        pub matches: usize,
        /// The matching entries.
        pub entries: Vec<LogEntryInfo>,
    }
}

