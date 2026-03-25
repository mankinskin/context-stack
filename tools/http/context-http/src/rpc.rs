//! RPC endpoint — `POST /api/execute`.
//!
//! This is the primary command dispatch endpoint. It accepts a `Command` JSON
//! body, dispatches it through the `WorkspaceManager`, and returns a
//! `CommandResult` JSON response. The contract mirrors the MCP adapter's
//! semantics exactly.
//!
//! # JSON Format
//!
//! The `Command` enum uses `#[serde(tag = "command", rename_all = "snake_case")]`,
//! so a command is represented as a flat JSON object with a `"command"` discriminant:
//!
//! ```json
//! { "command": "create_workspace", "name": "my-graph" }
//! ```
//!
//! Clients may optionally include a `"trace": true` field in the same object
//! to enable per-command tracing capture. The `trace` field is silently ignored
//! by serde when deserializing `Command` (unknown fields are allowed), so we
//! parse it separately.

use context_api::{
    commands::{
        execute,
        execute_traced,
        Command,
        CommandResult,
    },
    types::TraceSummary,
};
use serde::Serialize;
use tracing::{
    info,
    warn,
};
use viewer_api::axum::{
    extract::State,
    Json,
};

use crate::{
    error::HttpError,
    state::AppState,
};

/// Raw JSON input that may contain both a `Command` and an optional `trace`
/// flag.
///
/// Because `Command` uses `#[serde(tag = "command")]` (internally tagged),
/// the discriminant field is `"command"` — the same level as the payload
/// fields. We cannot use an `#[serde(untagged)]` envelope wrapper without
/// ambiguity, so instead we:
///
/// 1. Deserialize the body as a raw `serde_json::Value`.
/// 2. Extract and remove the `"trace"` field (if present).
/// 3. Deserialize the remaining value as a `Command`.
///
/// This keeps the wire format simple — clients just add `"trace": true`
/// alongside the command fields:
///
/// ```json
/// { "command": "insert_sequence", "workspace": "ws", "text": "hello", "trace": true }
/// ```

/// Successful response from the execute endpoint.
#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    /// The command result.
    pub result: CommandResult,
    /// Optional trace summary (present when `trace: true` was requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceSummary>,
}

/// `POST /api/execute`
///
/// Accepts a `Command` JSON body and dispatches it through the
/// `WorkspaceManager`. An optional `"trace": true` field in the request
/// body enables per-command tracing capture.
///
/// # Request Body
///
/// ```json
/// { "command": "create_workspace", "name": "my-graph" }
/// ```
///
/// With tracing:
/// ```json
/// { "command": "insert_sequence", "workspace": "ws", "text": "hello", "trace": true }
/// ```
///
/// # Responses
///
/// - **200 OK** — `ExecuteResponse` JSON on success.
/// - **4xx / 5xx** — `HttpErrorBody` JSON on failure (see `error` module).
pub async fn execute_command(
    State(state): State<AppState>,
    Json(mut body): Json<serde_json::Value>,
) -> Result<Json<ExecuteResponse>, HttpError> {
    // 1. Extract the optional trace flag from the raw JSON.
    let trace_enabled = body
        .as_object()
        .and_then(|obj| obj.get("trace"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 2. Remove the `trace` field so it doesn't interfere with Command
    //    deserialization (Command uses `deny_unknown_fields` or may simply
    //    not expect it — either way, stripping it is safer).
    if let Some(obj) = body.as_object_mut() {
        obj.remove("trace");
    }

    // 3. Deserialize the remaining JSON as a Command.
    let command: Command = serde_json::from_value(body).map_err(|e| {
        HttpError::bad_request(format!("Invalid command JSON: {e}"))
    })?;

    info!(
        ?command,
        trace = trace_enabled,
        "Executing command via HTTP RPC"
    );

    // Derive a capture config *before* we move the command into the blocking
    // closure.  `capture_config_for` briefly locks the manager to resolve the
    // workspace log directory, then drops the lock immediately.
    let capture_cfg = if trace_enabled {
        state.capture_config_for(&command)
    } else {
        None
    };

    // `WorkspaceManager` methods are synchronous (in-memory graph with
    // file-locking), so we run them on the blocking thread pool to avoid
    // stalling the async runtime.
    let manager = state.manager.clone();

    let (result, trace_summary) = if trace_enabled {
        tokio::task::spawn_blocking(move || {
            let mut mgr = manager.lock().map_err(|e| {
                HttpError::internal(format!("Mutex poisoned: {e}"))
            })?;
            execute_traced(&mut mgr, command, capture_cfg.as_ref())
                .map_err(HttpError::from)
        })
        .await
        .map_err(|e| {
            warn!("spawn_blocking join error: {e}");
            HttpError::internal(format!("Task join error: {e}"))
        })??
    } else {
        let cmd_result = tokio::task::spawn_blocking(move || {
            let mut mgr = manager.lock().map_err(|e| {
                HttpError::internal(format!("Mutex poisoned: {e}"))
            })?;
            execute(&mut mgr, command).map_err(HttpError::from)
        })
        .await
        .map_err(|e| {
            warn!("spawn_blocking join error: {e}");
            HttpError::internal(format!("Task join error: {e}"))
        })??;

        (cmd_result, None)
    };

    Ok(Json(ExecuteResponse {
        result,
        trace: trace_summary,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse a JSON string through the same pipeline the handler uses.
    fn parse_input(json: &str) -> (Command, bool) {
        let mut body: serde_json::Value = serde_json::from_str(json).unwrap();

        let trace_enabled = body
            .as_object()
            .and_then(|obj| obj.get("trace"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(obj) = body.as_object_mut() {
            obj.remove("trace");
        }

        let command: Command = serde_json::from_value(body).unwrap();
        (command, trace_enabled)
    }

    #[test]
    fn deserialize_list_workspaces() {
        let (cmd, trace) = parse_input(r#"{"command":"list_workspaces"}"#);
        assert!(!trace);
        assert!(matches!(cmd, Command::ListWorkspaces));
    }

    #[test]
    fn deserialize_create_workspace() {
        let (cmd, trace) =
            parse_input(r#"{"command":"create_workspace","name":"my-ws"}"#);
        assert!(!trace);
        match cmd {
            Command::CreateWorkspace { name } => assert_eq!(name, "my-ws"),
            other => panic!("expected CreateWorkspace, got: {other:?}"),
        }
    }

    #[test]
    fn deserialize_with_trace_true() {
        let (cmd, trace) =
            parse_input(r#"{"command":"list_workspaces","trace":true}"#);
        assert!(trace);
        assert!(matches!(cmd, Command::ListWorkspaces));
    }

    #[test]
    fn deserialize_with_trace_false() {
        let (cmd, trace) = parse_input(
            r#"{"command":"create_workspace","name":"ws","trace":false}"#,
        );
        assert!(!trace);
        assert!(matches!(cmd, Command::CreateWorkspace { .. }));
    }

    #[test]
    fn deserialize_insert_sequence_with_trace() {
        let (cmd, trace) = parse_input(
            r#"{"command":"insert_sequence","workspace":"ws","text":"hello","trace":true}"#,
        );
        assert!(trace);
        match cmd {
            Command::InsertSequence { workspace, text } => {
                assert_eq!(workspace, "ws");
                assert_eq!(text, "hello");
            },
            other => panic!("expected InsertSequence, got: {other:?}"),
        }
    }

    #[test]
    fn deserialize_missing_trace_defaults_to_false() {
        let (_, trace) = parse_input(r#"{"command":"list_workspaces"}"#);
        assert!(!trace);
    }

    #[test]
    fn invalid_command_json_is_err() {
        let body: serde_json::Value =
            serde_json::from_str(r#"{"not_a_command": true}"#).unwrap();
        let result: Result<Command, _> = serde_json::from_value(body);
        assert!(result.is_err());
    }
}
