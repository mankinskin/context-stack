//! Error types for the context-api crate.
//!
//! Defines per-domain error enums composed into a top-level `ApiError`.
//! Each domain (workspace, atom, pattern, search, insert, read) has its own
//! error type with specific variants. Phase 2 error types (Search, Insert, Read)
//! are defined as placeholders.

use std::fmt;

/// Top-level error type for any API command.
///
/// Wraps domain-specific errors so callers can match on the broad category
/// or drill into the specific variant.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Workspace lifecycle errors (create, open, close, save, delete, lock).
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    /// Atom operation errors.
    #[error(transparent)]
    Atom(#[from] AtomError),

    /// Pattern operation errors.
    #[error(transparent)]
    Pattern(#[from] PatternError),

    /// Search operation errors (Phase 2).
    #[error(transparent)]
    Search(#[from] SearchError),

    /// Insert operation errors (Phase 2).
    #[error(transparent)]
    Insert(#[from] InsertError),

    /// Read operation errors (Phase 2).
    #[error(transparent)]
    Read(#[from] ReadError),
}

// ---------------------------------------------------------------------------
// Workspace errors
// ---------------------------------------------------------------------------

/// Errors related to workspace lifecycle management.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// The requested workspace does not exist on disk.
    #[error("workspace not found: '{name}'")]
    NotFound { name: String },

    /// A workspace with this name already exists.
    #[error("workspace already exists: '{name}'")]
    AlreadyExists { name: String },

    /// The workspace is not currently open in the manager.
    #[error("workspace not open: '{name}'")]
    NotOpen { name: String },

    /// The workspace is already open in the manager.
    #[error("workspace already open: '{name}'")]
    AlreadyOpen { name: String },

    /// An I/O error occurred during a filesystem operation.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Could not acquire the file lock (another process holds it).
    #[error(
        "lock conflict on workspace '{name}' — another process may have it open"
    )]
    LockConflict { name: String },

    /// Serialization or deserialization failed (bincode or JSON).
    #[error("serialization error: {0}")]
    SerializationError(String),
}

// ---------------------------------------------------------------------------
// Atom errors
// ---------------------------------------------------------------------------

/// Errors related to atom operations.
#[derive(Debug, thiserror::Error)]
pub enum AtomError {
    /// The target workspace is not open.
    #[error("workspace not open: '{workspace}'")]
    WorkspaceNotOpen { workspace: String },
}

// ---------------------------------------------------------------------------
// Pattern errors
// ---------------------------------------------------------------------------

/// Errors related to pattern operations.
#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    /// The target workspace is not open.
    #[error("workspace not open: '{workspace}'")]
    WorkspaceNotOpen { workspace: String },

    /// One of the specified atoms does not exist in the graph.
    #[error("atom not found: '{ch}'")]
    AtomNotFound { ch: char },

    /// The input pattern is too short (need at least 2 atoms).
    #[error("pattern too short: got {len} atoms, need at least 2")]
    TooShort { len: usize },

    /// An atom in the input already belongs to an existing pattern.
    #[error(
        "atom '{ch}' already belongs to pattern at vertex {existing_parent}"
    )]
    AtomAlreadyInPattern { ch: char, existing_parent: usize },

    /// The input contains duplicate atom characters.
    #[error("duplicate atom in input: '{ch}'")]
    DuplicateAtomInInput { ch: char },

    /// A vertex was not found by index.
    #[error("vertex not found at index {index}")]
    VertexNotFound { index: usize },

    /// An internal error from the underlying graph.
    #[error("internal graph error: {0}")]
    InternalError(String),
}

// ---------------------------------------------------------------------------
// Phase 2 placeholder errors
// ---------------------------------------------------------------------------

/// Errors related to search operations (Phase 2).
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// The target workspace is not open.
    #[error("workspace not open: '{workspace}'")]
    WorkspaceNotOpen { workspace: String },

    /// A referenced token could not be found.
    #[error("token not found: {description}")]
    TokenNotFound { description: String },

    /// The search query is too short.
    #[error("search query too short")]
    QueryTooShort,

    /// An internal error from the underlying search algorithm.
    #[error("internal search error: {0}")]
    InternalError(String),
}

/// Errors related to insert operations (Phase 2).
#[derive(Debug, thiserror::Error)]
pub enum InsertError {
    /// The target workspace is not open.
    #[error("workspace not open: '{workspace}'")]
    WorkspaceNotOpen { workspace: String },

    /// A referenced token could not be found.
    #[error("token not found: {description}")]
    TokenNotFound { description: String },

    /// The insert query is too short (need at least 2 tokens).
    #[error("insert query too short")]
    QueryTooShort,

    /// An internal error from the underlying insert algorithm.
    #[error("internal insert error: {0}")]
    InternalError(String),
}

/// Errors related to read operations (Phase 2).
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// The target workspace is not open.
    #[error("workspace not open: '{workspace}'")]
    WorkspaceNotOpen { workspace: String },

    /// The vertex at the given index was not found.
    #[error("vertex not found at index {index}")]
    VertexNotFound { index: usize },

    /// An internal error from the underlying read algorithm.
    #[error("internal read error: {0}")]
    InternalError(String),
}

// ---------------------------------------------------------------------------
// Display for ApiError variants in JSON-friendly contexts
// ---------------------------------------------------------------------------

impl ApiError {
    /// Return a string tag identifying the error category.
    ///
    /// Useful for serializing error responses in adapters (CLI, MCP, HTTP).
    pub fn kind(&self) -> &'static str {
        match self {
            ApiError::Workspace(_) => "workspace",
            ApiError::Atom(_) => "atom",
            ApiError::Pattern(_) => "pattern",
            ApiError::Search(_) => "search",
            ApiError::Insert(_) => "insert",
            ApiError::Read(_) => "read",
        }
    }
}

/// Serializable error response for adapter layers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    /// Error category tag (e.g. "workspace", "atom", "pattern").
    pub kind: String,
    /// Human-readable error message.
    pub message: String,
}

impl From<&ApiError> for ErrorResponse {
    fn from(err: &ApiError) -> Self {
        Self {
            kind: err.kind().to_string(),
            message: err.to_string(),
        }
    }
}

impl fmt::Display for ErrorResponse {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

// ---------------------------------------------------------------------------
// Convenience conversions
// ---------------------------------------------------------------------------

/// Allow converting a `WorkspaceError` into a `PatternError::WorkspaceNotOpen`
/// when a workspace lookup fails inside a pattern command.
impl From<WorkspaceError> for PatternError {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::NotOpen { name } =>
                PatternError::WorkspaceNotOpen { workspace: name },
            other => PatternError::InternalError(other.to_string()),
        }
    }
}

/// Allow converting a `WorkspaceError` into an `AtomError::WorkspaceNotOpen`.
impl From<WorkspaceError> for AtomError {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::NotOpen { name } =>
                AtomError::WorkspaceNotOpen { workspace: name },
            // AtomError only has WorkspaceNotOpen, so anything else is a bug;
            // we panic here because it shouldn't happen in normal usage.
            other =>
                panic!("unexpected workspace error in atom context: {other}"),
        }
    }
}

/// Allow converting a `WorkspaceError` into a `SearchError::WorkspaceNotOpen`.
impl From<WorkspaceError> for SearchError {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::NotOpen { name } =>
                SearchError::WorkspaceNotOpen { workspace: name },
            other => SearchError::InternalError(other.to_string()),
        }
    }
}

/// Allow converting a `WorkspaceError` into an `InsertError::WorkspaceNotOpen`.
impl From<WorkspaceError> for InsertError {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::NotOpen { name } =>
                InsertError::WorkspaceNotOpen { workspace: name },
            other => InsertError::InternalError(other.to_string()),
        }
    }
}

/// Allow converting a `WorkspaceError` into a `ReadError::WorkspaceNotOpen`.
impl From<WorkspaceError> for ReadError {
    fn from(err: WorkspaceError) -> Self {
        match err {
            WorkspaceError::NotOpen { name } =>
                ReadError::WorkspaceNotOpen { workspace: name },
            other => ReadError::InternalError(other.to_string()),
        }
    }
}

/// Allow converting a `SearchError` into an `InsertError`.
///
/// Insert operations use search internally (token resolution, existence
/// checks), so search errors can propagate up through insert commands.
impl From<SearchError> for InsertError {
    fn from(err: SearchError) -> Self {
        match err {
            SearchError::WorkspaceNotOpen { workspace } =>
                InsertError::WorkspaceNotOpen { workspace },
            SearchError::TokenNotFound { description } =>
                InsertError::TokenNotFound { description },
            SearchError::QueryTooShort => InsertError::QueryTooShort,
            SearchError::InternalError(msg) => InsertError::InternalError(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_display_is_readable() {
        let err = ApiError::Workspace(WorkspaceError::NotFound {
            name: "demo".into(),
        });
        let msg = err.to_string();
        assert!(
            msg.contains("demo"),
            "message should contain workspace name"
        );
        assert!(
            msg.contains("not found"),
            "message should describe the error"
        );
    }

    #[test]
    fn error_response_round_trip() {
        let api_err = ApiError::Pattern(PatternError::TooShort { len: 1 });
        let resp = ErrorResponse::from(&api_err);
        assert_eq!(resp.kind, "pattern");
        assert!(resp.message.contains("too short"));

        let json = serde_json::to_string(&resp).unwrap();
        let deser: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.kind, resp.kind);
        assert_eq!(deser.message, resp.message);
    }

    #[test]
    fn workspace_error_into_pattern_error() {
        let ws_err = WorkspaceError::NotOpen {
            name: "test".into(),
        };
        let pat_err: PatternError = ws_err.into();
        match pat_err {
            PatternError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "test");
            },
            other => panic!("expected WorkspaceNotOpen, got: {other}"),
        }
    }

    #[test]
    fn workspace_error_into_atom_error() {
        let ws_err = WorkspaceError::NotOpen {
            name: "test".into(),
        };
        let atom_err: AtomError = ws_err.into();
        match atom_err {
            AtomError::WorkspaceNotOpen { workspace } => {
                assert_eq!(workspace, "test");
            },
        }
    }

    #[test]
    fn api_error_kind_tags() {
        assert_eq!(
            ApiError::Workspace(WorkspaceError::NotFound { name: "x".into() })
                .kind(),
            "workspace"
        );
        assert_eq!(
            ApiError::Atom(AtomError::WorkspaceNotOpen {
                workspace: "x".into()
            })
            .kind(),
            "atom"
        );
        assert_eq!(
            ApiError::Pattern(PatternError::TooShort { len: 0 }).kind(),
            "pattern"
        );
        assert_eq!(
            ApiError::Search(SearchError::QueryTooShort).kind(),
            "search"
        );
        assert_eq!(
            ApiError::Insert(InsertError::InternalError("x".into())).kind(),
            "insert"
        );
        assert_eq!(
            ApiError::Read(ReadError::VertexNotFound { index: 0 }).kind(),
            "read"
        );
    }
}
