//! Log-specific REST convenience endpoints.
//!
//! These provide ergonomic HTTP access to log operations without
//! requiring callers to construct a full `Command` JSON body.

use context_api::{
    commands::{
        Command,
        CommandResult,
    },
    types::{
        LogAnalysis,
        LogDeleteResult,
        LogEntryInfo,
        LogFileInfo,
        LogFileSearchResult,
    },
};
use serde::{
    Deserialize,
    Serialize,
};
use tracing::{
    info,
    instrument,
};
use viewer_api::axum::{
    extract::{
        Path,
        Query,
        State,
    },
    Json,
};

use crate::{
    error::HttpError,
    state::AppState,
};

// ---------------------------------------------------------------------------
// Query parameter structs
// ---------------------------------------------------------------------------

fn default_limit_100() -> usize {
    100
}
fn default_limit_10() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct ListLogsQuery {
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct GetLogQuery {
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Deserialize)]
pub struct QueryLogQuery {
    pub jq: String,
    #[serde(default = "default_limit_100")]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchLogsQuery {
    pub jq: String,
    #[serde(default = "default_limit_10")]
    pub limit_per_file: usize,
}

#[derive(Debug, Deserialize)]
pub struct DeleteLogsQuery {
    #[serde(default)]
    pub older_than_days: Option<u32>,
}

// ---------------------------------------------------------------------------
// Response wrappers (for structured REST responses)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct LogEntriesResponse {
    pub filename: String,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub returned: usize,
    pub entries: Vec<LogEntryInfo>,
}

#[derive(Debug, Serialize)]
pub struct LogQueryResponse {
    pub query: String,
    pub matches: usize,
    pub entries: Vec<LogEntryInfo>,
}

#[derive(Debug, Serialize)]
pub struct LogSearchResponse {
    pub query: String,
    pub files_with_matches: usize,
    pub results: Vec<LogFileSearchResult>,
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs
// ---------------------------------------------------------------------------

/// `GET /api/workspaces/:name/logs`
///
/// List trace log files for a workspace, with optional glob pattern and
/// limit.
#[instrument(name = "rest_list_logs", skip(state))]
pub async fn list_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ListLogsQuery>,
) -> Result<Json<Vec<LogFileInfo>>, HttpError> {
    info!(workspace = %name, pattern = ?query.pattern, limit = query.limit, "Listing logs via REST");

    let manager = state.manager.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::ListLogs {
                    workspace: name,
                    pattern: query.pattern,
                    limit: query.limit,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogList { logs } => Ok(Json(logs)),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename
// ---------------------------------------------------------------------------

/// `GET /api/workspaces/:name/logs/:filename`
///
/// Read a trace log file with optional level filter and pagination.
#[instrument(name = "rest_get_log", skip(state))]
pub async fn get_log(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
    Query(query): Query<GetLogQuery>,
) -> Result<Json<LogEntriesResponse>, HttpError> {
    info!(
        workspace = %name,
        filename = %filename,
        filter = ?query.filter,
        limit = query.limit,
        offset = query.offset,
        "Getting log via REST"
    );

    let manager = state.manager.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::GetLog {
                    workspace: name,
                    filename,
                    filter: query.filter,
                    limit: query.limit,
                    offset: query.offset,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogEntries {
            filename,
            total,
            offset,
            limit,
            returned,
            entries,
        } => Ok(Json(LogEntriesResponse {
            filename,
            total,
            offset,
            limit,
            returned,
            entries,
        })),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename/query
// ---------------------------------------------------------------------------

/// `GET /api/workspaces/:name/logs/:filename/query`
///
/// Run a JQ query against a trace log file.
#[instrument(name = "rest_query_log", skip(state))]
pub async fn query_log(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
    Query(query): Query<QueryLogQuery>,
) -> Result<Json<LogQueryResponse>, HttpError> {
    info!(
        workspace = %name,
        filename = %filename,
        jq = %query.jq,
        limit = query.limit,
        "Querying log via REST"
    );

    let manager = state.manager.clone();
    let jq = query.jq;
    let limit = query.limit;
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::QueryLog {
                    workspace: name,
                    filename,
                    query: jq,
                    limit,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogQueryResult {
            query,
            matches,
            entries,
        } => Ok(Json(LogQueryResponse {
            query,
            matches,
            entries,
        })),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename/analysis
// ---------------------------------------------------------------------------

/// `GET /api/workspaces/:name/logs/:filename/analysis`
///
/// Analyze a trace log file (statistics by level, event type, spans).
#[instrument(name = "rest_analyze_log", skip(state))]
pub async fn analyze_log(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
) -> Result<Json<LogAnalysis>, HttpError> {
    info!(
        workspace = %name,
        filename = %filename,
        "Analyzing log via REST"
    );

    let manager = state.manager.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::AnalyzeLog {
                    workspace: name,
                    filename,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogAnalysis(analysis) => Ok(Json(analysis)),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/search
// ---------------------------------------------------------------------------

/// `GET /api/workspaces/:name/logs/search`
///
/// Search across all trace logs in a workspace with a JQ query.
#[instrument(name = "rest_search_logs", skip(state))]
pub async fn search_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<SearchLogsQuery>,
) -> Result<Json<LogSearchResponse>, HttpError> {
    info!(
        workspace = %name,
        jq = %query.jq,
        limit_per_file = query.limit_per_file,
        "Searching logs via REST"
    );

    let manager = state.manager.clone();
    let jq = query.jq;
    let limit_per_file = query.limit_per_file;
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::SearchLogs {
                    workspace: name,
                    query: jq,
                    limit_per_file,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogSearchResult {
            query,
            files_with_matches,
            results,
        } => Ok(Json(LogSearchResponse {
            query,
            files_with_matches,
            results,
        })),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// DELETE /api/workspaces/:name/logs/:filename
// ---------------------------------------------------------------------------

/// `DELETE /api/workspaces/:name/logs/:filename`
///
/// Delete a specific trace log file.
#[instrument(name = "rest_delete_log", skip(state))]
pub async fn delete_log(
    State(state): State<AppState>,
    Path((name, filename)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, HttpError> {
    info!(
        workspace = %name,
        filename = %filename,
        "Deleting log via REST"
    );

    let manager = state.manager.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::DeleteLog {
                    workspace: name,
                    filename,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::Ok => Ok(Json(serde_json::json!({ "deleted": true }))),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// DELETE /api/workspaces/:name/logs
// ---------------------------------------------------------------------------

/// `DELETE /api/workspaces/:name/logs`
///
/// Delete trace log files, optionally only those older than N days.
#[instrument(name = "rest_delete_logs", skip(state))]
pub async fn delete_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<DeleteLogsQuery>,
) -> Result<Json<LogDeleteResult>, HttpError> {
    info!(
        workspace = %name,
        older_than_days = ?query.older_than_days,
        "Deleting logs via REST"
    );

    let manager = state.manager.clone();
    let result = tokio::task::spawn_blocking(
        move || -> Result<CommandResult, HttpError> {
            let mut mgr = manager
                .lock()
                .map_err(|e| HttpError::internal(e.to_string()))?;
            context_api::commands::execute(
                &mut mgr,
                Command::DeleteLogs {
                    workspace: name,
                    older_than_days: query.older_than_days,
                },
            )
            .map_err(HttpError::from)
        },
    )
    .await
    .map_err(|e| HttpError::internal(format!("Task join error: {e}")))??;

    match result {
        CommandResult::LogDeleteResult(delete_result) =>
            Ok(Json(delete_result)),
        other => Err(HttpError::internal(format!(
            "Unexpected result variant: {:?}",
            std::mem::discriminant(&other)
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_logs_query_defaults() {
        let q: ListLogsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.pattern, None);
        assert_eq!(q.limit, 100);
    }

    #[test]
    fn list_logs_query_with_values() {
        let q: ListLogsQuery =
            serde_json::from_str(r#"{"pattern":"insert","limit":50}"#).unwrap();
        assert_eq!(q.pattern, Some("insert".to_string()));
        assert_eq!(q.limit, 50);
    }

    #[test]
    fn get_log_query_defaults() {
        let q: GetLogQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.filter, None);
        assert_eq!(q.limit, 100);
        assert_eq!(q.offset, 0);
    }

    #[test]
    fn query_log_query_requires_jq() {
        let result: Result<QueryLogQuery, _> = serde_json::from_str("{}");
        assert!(result.is_err(), "jq field is required");
    }

    #[test]
    fn query_log_query_with_defaults() {
        let q: QueryLogQuery =
            serde_json::from_str(r#"{"jq":".level == \"ERROR\""}"#).unwrap();
        assert_eq!(q.limit, 100);
    }

    #[test]
    fn search_logs_query_requires_jq() {
        let result: Result<SearchLogsQuery, _> = serde_json::from_str("{}");
        assert!(result.is_err(), "jq field is required");
    }

    #[test]
    fn search_logs_query_defaults() {
        let q: SearchLogsQuery =
            serde_json::from_str(r#"{"jq":".level"}"#).unwrap();
        assert_eq!(q.limit_per_file, 10);
    }

    #[test]
    fn delete_logs_query_defaults() {
        let q: DeleteLogsQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(q.older_than_days, None);
    }

    #[test]
    fn delete_logs_query_with_days() {
        let q: DeleteLogsQuery =
            serde_json::from_str(r#"{"older_than_days":30}"#).unwrap();
        assert_eq!(q.older_than_days, Some(30));
    }
}
