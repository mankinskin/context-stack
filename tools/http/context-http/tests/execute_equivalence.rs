//! The same request script gives the same `Response` JSON through
//! `POST /v1/execute` as through `Session::execute` in process.

mod common;

use common::*;
use context_http::status_for;
use serde_json::{
    json,
    Value,
};

fn script() -> Vec<Value> {
    let text = |t: &str| json!({ "kind": "text", "text": t });
    vec![
        json!({ "id": 1, "command": "new", "graph": "g" }),
        json!({ "id": 2, "command": "read", "graph": "g", "inputs": [text("abcabcab")] }),
        json!({ "id": 3, "command": "read", "graph": "g", "split": "lines",
                "inputs": [{ "kind": "text", "text": "bcab\nxyz\n\nabcxyz", "label": "notes" }] }),
        json!({ "id": 4, "command": "info", "graph": "g" }),
        json!({ "id": 5, "command": "inputs", "graph": "g" }),
        json!({ "id": 6, "command": "vertex", "graph": "g", "vertex": { "index": 0 } }),
        json!({ "id": 7, "command": "vertex", "graph": "g", "vertex": { "text": "abc" } }),
        json!({ "id": 8, "command": "vertex", "graph": "g", "vertex": { "text": "qqq" } }),
        json!({ "id": 9, "command": "stats", "graph": "g" }),
        json!({ "id": 10, "command": "validate", "graph": "g" }),
        json!({ "id": 11, "command": "read", "graph": "g", "dry_run": true, "inputs": [text("cabcabx")] }),
        json!({ "id": 12, "command": "search", "graph": "g", "query": "bca", "view": "contexts" }),
        json!({ "id": 13, "command": "render", "graph": "g", "format": "dot" }),
        json!({ "id": 14, "command": "list" }),
        json!({ "id": 15, "command": "new", "graph": "g" }),
        json!({ "id": 16, "command": "info", "graph": "missing" }),
        json!({ "id": 17, "command": "read", "graph": "g", "inputs": [{ "kind": "file", "path": "/etc/hostname" }] }),
        json!({ "id": 18, "command": "read", "graph": "h", "create": true, "save": "defer", "inputs": [text("hello")] }),
        json!({ "id": 19, "command": "save", "graph": "h" }),
        json!({ "id": 20, "command": "close", "graph": "g", "save": true }),
        json!({ "id": 21, "command": "info", "graph": "g" }),
        json!({ "id": 22, "command": "delete", "graph": "h" }),
        json!({ "id": 23, "command": "list" }),
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn execute_equivalence() {
    let server = TestServer::start().await;
    let local_dir = tempfile::tempdir().unwrap();
    let local = local_session(local_dir.path());
    for request in script() {
        let (status, mut remote) = server.execute(request.clone()).await;
        let mut expected = local_execute(&local, request.clone());
        normalize(&mut remote);
        normalize(&mut expected);
        assert_eq!(remote, expected, "request {request}");
        let expected_status = match expected["error"]["kind"].as_str() {
            Some(kind) =>
                status_for(serde_json::from_value(json!(kind)).unwrap()),
            None => reqwest::StatusCode::OK,
        };
        assert_eq!(status, expected_status, "request {request}");
    }
    local.shutdown().unwrap();
    server.stop().await;
}
