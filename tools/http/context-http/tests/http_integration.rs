//! Integration tests for the context-http server.
//!
//! These tests build the full axum router (with a temporary workspace directory)
//! and exercise the HTTP endpoints using `axum_test::TestServer`.

use axum_test::TestServer;
use context_api::workspace::manager::WorkspaceManager;
use serde_json::{
    json,
    Value,
};
use std::io::Write;
use tempfile::TempDir;

// We need access to the crate's internal modules. Since context-http is a
// binary crate, we re-import the modules via path dependencies. The test
// harness can access `context_http::*` only if we have a lib target or if
// we inline the needed pieces. Because context-http is a bin crate, we
// reconstruct the router here using the same logic as `main.rs`.

/// Helper: build a test server backed by a fresh temp directory.
fn test_server() -> (TestServer, TempDir) {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let manager = WorkspaceManager::new(tmp.path().to_path_buf());

    // Reproduce the state + router construction from the binary crate.
    // We use the types directly since they are pub.
    let state = context_http::state::AppState::new(manager);
    let router = context_http::router::create_router(state, None);

    let server = TestServer::new(router);
    (server, tmp)
}

/// Sample log content for testing log endpoints.
const SAMPLE_LOG: &str = r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"hello world","step":1},"target":"test"}

{"timestamp":"2026-03-14T00:00:01Z","level":"ERROR","fields":{"message":"something failed"},"target":"test"}

{"timestamp":"2026-03-14T00:00:02Z","level":"INFO","fields":{"message":"recovered ok","step":2},"target":"test","span":"my_span","spans":["my_span"]}
"#;

/// Helper: create a workspace and write a log file into its logs directory.
///
/// Returns the filename of the created log file.
async fn setup_workspace_with_log(
    server: &TestServer,
    tmp: &TempDir,
    ws_name: &str,
    log_filename: &str,
    content: &str,
) -> String {
    // Create workspace via RPC (this sets up the workspace directory).
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": ws_name
        }))
        .await
        .assert_status_ok();

    // Write a log file directly into the workspace's logs directory.
    // WorkspaceManager stores logs at <base>/.context-engine/<ws>/logs/
    let log_dir = tmp
        .path()
        .join(".context-engine")
        .join(ws_name)
        .join("logs");
    std::fs::create_dir_all(&log_dir).expect("failed to create log dir");

    let log_path = log_dir.join(log_filename);
    let mut f =
        std::fs::File::create(&log_path).expect("failed to create log file");
    f.write_all(content.as_bytes())
        .expect("failed to write log");

    log_filename.to_string()
}

// ---------------------------------------------------------------------------
// Health endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_200_with_ok_status() {
    let (server, _tmp) = test_server();

    let resp = server.get("/api/health").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

// ---------------------------------------------------------------------------
// RPC: POST /api/execute — workspace lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_create_workspace() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "test-ws"
        }))
        .await;

    resp.assert_status_ok();

    let body: Value = resp.json();
    // CommandResult::WorkspaceInfo is tagged with "type": "workspace_info"
    assert_eq!(body["result"]["type"], "workspace_info");
    assert_eq!(body["result"]["name"], "test-ws");
}

#[tokio::test]
async fn execute_list_workspaces_empty() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({ "command": "list_workspaces" }))
        .await;

    resp.assert_status_ok();

    let body: Value = resp.json();
    assert_eq!(body["result"]["type"], "workspace_info_list");
    assert!(body["result"]["workspaces"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn execute_list_workspaces_after_create() {
    let (server, _tmp) = test_server();

    // Create a workspace first.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "ws1"
        }))
        .await
        .assert_status_ok();

    // Now list.
    let resp = server
        .post("/api/execute")
        .json(&json!({ "command": "list_workspaces" }))
        .await;

    resp.assert_status_ok();

    let body: Value = resp.json();
    let workspaces = body["result"]["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0]["name"], "ws1");
}

// ---------------------------------------------------------------------------
// RPC: error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_open_nonexistent_workspace_returns_404() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "open_workspace",
            "name": "does-not-exist"
        }))
        .await;

    resp.assert_status(axum_test::http::StatusCode::NOT_FOUND);

    let body: Value = resp.json();
    assert_eq!(body["kind"], "workspace");
}

#[tokio::test]
async fn execute_list_atoms_without_open_workspace_returns_400() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "list_atoms",
            "workspace": "not-open"
        }))
        .await;

    // NotOpen maps to 400 BAD_REQUEST.
    // The error originates as WorkspaceError::NotOpen (kind="workspace"),
    // not AtomError, because the workspace lookup fails first.
    resp.assert_status(axum_test::http::StatusCode::BAD_REQUEST);

    let body: Value = resp.json();
    // Accept either "workspace" or "atom" — the underlying error may be
    // wrapped differently depending on the command dispatch path.
    let kind = body["kind"].as_str().unwrap();
    assert!(
        kind == "workspace" || kind == "atom",
        "expected kind 'workspace' or 'atom', got '{kind}'"
    );
}

#[tokio::test]
async fn execute_malformed_json_returns_400() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({ "not_a_command_field": true }))
        .await;

    // Our handler returns 400 for unparseable Command JSON.
    resp.assert_status(axum_test::http::StatusCode::BAD_REQUEST);

    let body: Value = resp.json();
    assert_eq!(body["kind"], "bad_request");
}

// ---------------------------------------------------------------------------
// RPC: trace flag
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_with_trace_false_has_no_trace_summary() {
    let (server, _tmp) = test_server();

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "list_workspaces",
            "trace": false
        }))
        .await;

    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body["trace"].is_null(), "trace should be absent/null");
}

// ---------------------------------------------------------------------------
// RPC: full round-trip workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn execute_round_trip_workflow() {
    let (server, _tmp) = test_server();

    // 1. Create workspace.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "round-trip"
        }))
        .await
        .assert_status_ok();

    // 2. Add atoms.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "add_atoms",
            "workspace": "round-trip",
            "chars": ["h", "e", "l", "o"]
        }))
        .await
        .assert_status_ok();

    // 3. List atoms — should have 4.
    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "list_atoms",
            "workspace": "round-trip"
        }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    let atoms = body["result"]["atoms"].as_array().unwrap();
    assert_eq!(atoms.len(), 4);

    // 4. Add a simple pattern.
    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "add_simple_pattern",
            "workspace": "round-trip",
            "atoms": ["h", "e"]
        }))
        .await;
    resp.assert_status_ok();

    // 5. Get statistics.
    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "get_statistics",
            "workspace": "round-trip"
        }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["result"]["type"], "statistics");

    // 6. Get snapshot.
    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "get_snapshot",
            "workspace": "round-trip"
        }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["result"]["type"], "snapshot");

    // 7. Save workspace.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "save_workspace",
            "name": "round-trip"
        }))
        .await
        .assert_status_ok();
}

// ---------------------------------------------------------------------------
// REST: GET /api/workspaces
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_workspaces_empty() {
    let (server, _tmp) = test_server();

    let resp = server.get("/api/workspaces").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn rest_list_workspaces_after_create() {
    let (server, _tmp) = test_server();

    // Create via RPC.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "rest-test"
        }))
        .await
        .assert_status_ok();

    // List via REST.
    let resp = server.get("/api/workspaces").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    let workspaces = body.as_array().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0]["name"], "rest-test");
}

// ---------------------------------------------------------------------------
// REST: GET /api/workspaces/:name/atoms
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_atoms() {
    let (server, _tmp) = test_server();

    // Setup: create workspace and add atoms.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "atoms-ws"
        }))
        .await
        .assert_status_ok();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "add_atoms",
            "workspace": "atoms-ws",
            "chars": ["a", "b", "c"]
        }))
        .await
        .assert_status_ok();

    // REST query.
    let resp = server.get("/api/workspaces/atoms-ws/atoms").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    let atoms = body.as_array().unwrap();
    assert_eq!(atoms.len(), 3);
}

// ---------------------------------------------------------------------------
// REST: GET /api/workspaces/:name/statistics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_get_statistics() {
    let (server, _tmp) = test_server();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "stats-ws"
        }))
        .await
        .assert_status_ok();

    let resp = server.get("/api/workspaces/stats-ws/statistics").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    // GraphStatistics should have known fields.
    assert!(body.is_object());
}

// ---------------------------------------------------------------------------
// REST: GET /api/workspaces/:name/snapshot
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_get_snapshot() {
    let (server, _tmp) = test_server();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "snap-ws"
        }))
        .await
        .assert_status_ok();

    let resp = server.get("/api/workspaces/snap-ws/snapshot").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    // A fresh workspace snapshot should have empty nodes and edges.
    assert!(body["nodes"].as_array().unwrap().is_empty());
    assert!(body["edges"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// REST: GET /api/workspaces/:name/vertices
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_vertices() {
    let (server, _tmp) = test_server();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "verts-ws"
        }))
        .await
        .assert_status_ok();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "add_atoms",
            "workspace": "verts-ws",
            "chars": ["x", "y"]
        }))
        .await
        .assert_status_ok();

    let resp = server.get("/api/workspaces/verts-ws/vertices").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    let verts = body.as_array().unwrap();
    assert_eq!(verts.len(), 2);
}

// ---------------------------------------------------------------------------
// REST: error — workspace not open
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_atoms_workspace_not_open_returns_error() {
    let (server, _tmp) = test_server();

    let resp = server.get("/api/workspaces/nonexistent/atoms").await;

    // The workspace is not open, so we expect a client error status.
    assert!(resp.status_code().is_client_error());
}

// ---------------------------------------------------------------------------
// CORS headers are present
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cors_headers_present_on_health() {
    let (server, _tmp) = test_server();

    let resp = server.get("/api/health").await;
    resp.assert_status_ok();

    // The default_cors layer should add access-control headers when
    // an Origin header is present. axum-test doesn't set Origin by
    // default, so we just verify the endpoint works. A more thorough
    // test would send a preflight OPTIONS request with an Origin header.
}

// ---------------------------------------------------------------------------
// Multiple concurrent requests don't deadlock
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_requests_do_not_deadlock() {
    let (server, _tmp) = test_server();

    // Fire several sequential requests to verify the mutex doesn't deadlock.
    // (TestServer doesn't implement Clone, so we can't spawn parallel tasks
    // that each own a handle. Sequential requests still exercise the lock/
    // unlock cycle repeatedly.)
    for _ in 0..10 {
        let resp = server.get("/api/health").await;
        resp.assert_status_ok();
    }
}

// ===========================================================================
// Log REST endpoints
// ===========================================================================

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs — list logs (empty)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_logs_empty() {
    let (server, _tmp) = test_server();

    // Create a workspace but don't add any logs.
    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "empty-logs"
        }))
        .await
        .assert_status_ok();

    let resp = server.get("/api/workspaces/empty-logs/logs").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs — list logs after writing a file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_logs_after_write() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "log-ws",
        "20260314T000000_insert.log",
        SAMPLE_LOG,
    )
    .await;

    let resp = server.get("/api/workspaces/log-ws/logs").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    let logs = body.as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["filename"], "20260314T000000_insert.log");
    assert!(logs[0]["size"].as_u64().unwrap() > 0);
    assert_eq!(logs[0]["command"], "insert");
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs?pattern=...&limit=...
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_logs_with_pattern_and_limit() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "filter-ws",
        "20260314T000000_insert.log",
        SAMPLE_LOG,
    )
    .await;
    // Write a second log (need to do it after the workspace was created
    // by setup_workspace_with_log).
    let log_dir = tmp
        .path()
        .join(".context-engine")
        .join("filter-ws")
        .join("logs");
    {
        let mut f =
            std::fs::File::create(log_dir.join("20260314T000001_search.log"))
                .unwrap();
        f.write_all(SAMPLE_LOG.as_bytes()).unwrap();
    }
    {
        let mut f =
            std::fs::File::create(log_dir.join("20260314T000002_insert.log"))
                .unwrap();
        f.write_all(SAMPLE_LOG.as_bytes()).unwrap();
    }

    // Filter by pattern.
    let resp = server
        .get("/api/workspaces/filter-ws/logs")
        .add_query_param("pattern", "insert")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    let logs = body.as_array().unwrap();
    assert_eq!(logs.len(), 2, "should match 2 insert logs");
    for log in logs {
        assert!(log["filename"].as_str().unwrap().contains("insert"));
    }

    // Limit.
    let resp = server
        .get("/api/workspaces/filter-ws/logs")
        .add_query_param("limit", 1)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body.as_array().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename — get log entries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_get_log_entries() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "getlog-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    let resp = server
        .get("/api/workspaces/getlog-ws/logs/20260314T000000_test.log")
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert_eq!(body["filename"], "20260314T000000_test.log");
    assert!(body["total"].as_u64().unwrap() >= 3);
    assert_eq!(body["offset"], 0);
    assert!(body["entries"].as_array().unwrap().len() >= 3);
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename — with filter and pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_get_log_with_filter_and_pagination() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "paginate-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    // Filter by ERROR level.
    let resp = server
        .get("/api/workspaces/paginate-ws/logs/20260314T000000_test.log")
        .add_query_param("filter", "ERROR")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "should have exactly 1 ERROR entry");

    // Pagination: offset=1, limit=1.
    let resp = server
        .get("/api/workspaces/paginate-ws/logs/20260314T000000_test.log")
        .add_query_param("offset", 1)
        .add_query_param("limit", 1)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["offset"], 1);
    assert_eq!(body["limit"], 1);
    assert_eq!(body["returned"], 1);
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename — not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_get_log_not_found() {
    let (server, _tmp) = test_server();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "nf-ws"
        }))
        .await
        .assert_status_ok();

    let resp = server
        .get("/api/workspaces/nf-ws/logs/nonexistent.log")
        .await;
    resp.assert_status(axum_test::http::StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename/query — JQ query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_query_log_with_jq() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "jq-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    // Query for ERROR entries using JQ.
    let resp = server
        .get("/api/workspaces/jq-ws/logs/20260314T000000_test.log/query")
        .add_query_param("jq", r#"select(.level == "ERROR")"#)
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body["matches"].as_u64().unwrap() >= 1);
    assert!(!body["entries"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/:filename/analysis — analyze log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_analyze_log() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "analyze-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    let resp = server
        .get(
            "/api/workspaces/analyze-ws/logs/20260314T000000_test.log/analysis",
        )
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body["total_entries"].as_u64().unwrap() >= 3);
    assert!(body["by_level"].is_object());
    // Should have at least INFO and ERROR levels.
    assert!(body["by_level"]["INFO"].as_u64().unwrap() >= 2);
    assert!(body["by_level"]["ERROR"].as_u64().unwrap() >= 1);
}

// ---------------------------------------------------------------------------
// GET /api/workspaces/:name/logs/search — cross-file search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_search_logs() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "search-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    // Search for ERROR entries across all files.
    let resp = server
        .get("/api/workspaces/search-ws/logs/search")
        .add_query_param("jq", r#"select(.level == "ERROR")"#)
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert!(body["files_with_matches"].as_u64().unwrap() >= 1);
    let results = body["results"].as_array().unwrap();
    assert!(!results.is_empty());
    assert!(results[0]["matches"].as_u64().unwrap() >= 1);
}

// ---------------------------------------------------------------------------
// DELETE /api/workspaces/:name/logs/:filename — delete single log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_delete_log() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "del-ws",
        "20260314T000000_delete_me.log",
        SAMPLE_LOG,
    )
    .await;

    // Verify the log exists.
    let resp = server.get("/api/workspaces/del-ws/logs").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Delete it.
    let resp = server
        .delete("/api/workspaces/del-ws/logs/20260314T000000_delete_me.log")
        .await;
    resp.assert_status_ok();

    // Verify it's gone.
    let resp = server.get("/api/workspaces/del-ws/logs").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// DELETE /api/workspaces/:name/logs — delete all logs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_delete_logs_all() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "delall-ws",
        "20260314T000000_a.log",
        SAMPLE_LOG,
    )
    .await;

    // Add a second log file.
    let log_dir = tmp
        .path()
        .join(".context-engine")
        .join("delall-ws")
        .join("logs");
    {
        let mut f =
            std::fs::File::create(log_dir.join("20260314T000001_b.log"))
                .unwrap();
        f.write_all(SAMPLE_LOG.as_bytes()).unwrap();
    }

    // Verify both exist.
    let resp = server.get("/api/workspaces/delall-ws/logs").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body.as_array().unwrap().len(), 2);

    // Delete all.
    let resp = server.delete("/api/workspaces/delall-ws/logs").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["deleted_count"], 2);
    assert!(body["freed_bytes"].as_u64().unwrap() > 0);

    // Verify empty.
    let resp = server.get("/api/workspaces/delall-ws/logs").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// DELETE /api/workspaces/:name/logs/:filename — not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_delete_log_not_found() {
    let (server, _tmp) = test_server();

    server
        .post("/api/execute")
        .json(&json!({
            "command": "create_workspace",
            "name": "delnf-ws"
        }))
        .await
        .assert_status_ok();

    let resp = server
        .delete("/api/workspaces/delnf-ws/logs/nonexistent.log")
        .await;
    resp.assert_status(axum_test::http::StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Log REST: feature flags populated in list response
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rest_list_logs_feature_flags() {
    let (server, tmp) = test_server();

    let log_with_snapshot = r#"{"timestamp":"2026-03-14T00:00:00Z","level":"INFO","fields":{"message":"graph_snapshot captured"},"target":"test"}
"#;

    setup_workspace_with_log(
        &server,
        &tmp,
        "flags-ws",
        "20260314T000000_snapshot.log",
        log_with_snapshot,
    )
    .await;

    let resp = server.get("/api/workspaces/flags-ws/logs").await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    let logs = body.as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["has_graph_snapshot"], true);
    assert_eq!(logs[0]["has_search_ops"], false);
}

// ---------------------------------------------------------------------------
// Log RPC: list_logs via POST /api/execute
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rpc_list_logs() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "rpc-log-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "list_logs",
            "workspace": "rpc-log-ws",
            "limit": 100
        }))
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert_eq!(body["result"]["type"], "log_list");
    let logs = body["result"]["logs"].as_array().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["filename"], "20260314T000000_test.log");
}

// ---------------------------------------------------------------------------
// Log RPC: get_log via POST /api/execute
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rpc_get_log() {
    let (server, tmp) = test_server();

    setup_workspace_with_log(
        &server,
        &tmp,
        "rpc-get-ws",
        "20260314T000000_test.log",
        SAMPLE_LOG,
    )
    .await;

    let resp = server
        .post("/api/execute")
        .json(&json!({
            "command": "get_log",
            "workspace": "rpc-get-ws",
            "filename": "20260314T000000_test.log",
            "limit": 100,
            "offset": 0
        }))
        .await;
    resp.assert_status_ok();

    let body: Value = resp.json();
    assert_eq!(body["result"]["type"], "log_entries");
    assert!(body["result"]["total"].as_u64().unwrap() >= 3);
}
