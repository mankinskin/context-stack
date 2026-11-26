//! Error model of the context API.
//!
//! [`ApiError`] is the single error type of [`crate::Session::execute`].
//! Transports never inspect the variants: they serialize an
//! [`ErrorResponse`] whose [`ErrorKind`] maps to exit codes (CLI) and status
//! codes (HTTP).

use std::fmt;

use serde::{
    Deserialize,
    Serialize,
};

use crate::ts_export;

/// Errors of every command.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("graph '{graph}' not found")]
    GraphNotFound { graph: String },

    #[error("graph '{graph}' already exists")]
    GraphExists { graph: String },

    /// Another process holds the graph's lock; `holder` is the server URL
    /// from the graph's discovery file, if one is running.
    #[error("graph '{graph}' is locked by another process{}", holder.as_ref().map(|h| format!(" (server {h})")).unwrap_or_default())]
    Locked { graph: String, holder: Option<String> },

    /// Another command of this session uses the graph.
    #[error("graph '{graph}' is in use by another command; retry when it finishes")]
    Busy { graph: String },

    /// A row-model workspace directory from before the positioned stack.
    #[error("'{graph}' is a legacy workspace; re-create the graph from its source text with `context-cli create`")]
    LegacyWorkspace { graph: String },

    #[error("vertex {vertex} not found")]
    VertexNotFound { vertex: String },

    /// A text vertex reference that is not a vertex; `host` is the narrowest
    /// vertex containing it, if any.
    #[error("'{text}' is not a vertex{}", host.as_ref().map(|h| format!("; it occurs inside {h}")).unwrap_or_default())]
    NotAVertex { text: String, host: Option<String> },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A file input was refused (no file access configured, or outside the
    /// allowed directory).
    #[error("file access refused: {0}")]
    FileAccess(String),

    /// The render or export gate refused the request.
    #[error("graph too large to render: {vertices} vertices, {label_chars} label chars (limit {max_vertices} vertices / {max_label_chars} chars)")]
    TooLarge {
        vertices: usize,
        label_chars: usize,
        max_vertices: usize,
        max_label_chars: usize,
    },

    /// A graph file is unreadable (bad magic, version, checksum or content).
    #[error("corrupt graph file '{path}': {reason}")]
    Corrupt { path: String, reason: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("log error: {0}")]
    Log(#[from] LogError),

    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A bug: an invariant failed or an algorithm panicked.
    #[error("internal error: {0}")]
    Internal(String),
}

ts_export! {
    /// Error category, stable across transports.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ErrorKind {
        NotFound,
        AlreadyExists,
        Locked,
        InvalidInput,
        Forbidden,
        TooLarge,
        Corrupt,
        Io,
        Unsupported,
        Internal,
    }
}

impl ApiError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            ApiError::GraphNotFound { .. }
            | ApiError::VertexNotFound { .. }
            | ApiError::NotAVertex { .. } => ErrorKind::NotFound,
            ApiError::GraphExists { .. } => ErrorKind::AlreadyExists,
            ApiError::Locked { .. } | ApiError::Busy { .. } => ErrorKind::Locked,
            ApiError::LegacyWorkspace { .. } | ApiError::InvalidInput(_) =>
                ErrorKind::InvalidInput,
            ApiError::FileAccess(_) => ErrorKind::Forbidden,
            ApiError::TooLarge { .. } => ErrorKind::TooLarge,
            ApiError::Corrupt { .. } => ErrorKind::Corrupt,
            ApiError::Io(_) => ErrorKind::Io,
            ApiError::Log(LogError::FileNotFound { .. }) => ErrorKind::NotFound,
            ApiError::Log(LogError::QueryError(_)) => ErrorKind::InvalidInput,
            ApiError::Log(_) => ErrorKind::Io,
            ApiError::Unsupported(_) => ErrorKind::Unsupported,
            ApiError::Internal(_) => ErrorKind::Internal,
        }
    }

    /// A suggestion for the user, if there is a standard one.
    pub fn hint(&self) -> Option<String> {
        match self {
            ApiError::GraphNotFound { graph } => Some(format!(
                "create it with `context-cli new {graph}` or pass --create to read"
            )),
            ApiError::Locked { holder: Some(url), .. } =>
                Some(format!("run the command against the server: --server {url}")),
            ApiError::TooLarge { .. } => Some(
                "render a subgraph with --root \"<text>\" [--depth N], or pass --force"
                    .into(),
            ),
            _ => None,
        }
    }
}

ts_export! {
    /// Serializable error of a [`crate::Response`].
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ErrorResponse {
        pub kind: ErrorKind,
        pub message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hint: Option<String>,
    }
}

impl From<&ApiError> for ErrorResponse {
    fn from(err: &ApiError) -> Self {
        Self {
            kind: err.kind(),
            message: err.to_string(),
            hint: err.hint(),
        }
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\nhint: {hint}")?;
        }
        Ok(())
    }
}

/// Errors of log operations.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("log file not found: '{filename}'")]
    FileNotFound { filename: String },

    #[error("log I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JQ query error: {0}")]
    QueryError(String),

    #[error("log directory error: {0}")]
    DirectoryError(String),
}
