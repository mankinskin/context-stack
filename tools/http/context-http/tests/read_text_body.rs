//! `POST /v1/graphs/{g}/read` with a text body: events on a subscribed
//! WebSocket, the resulting graph equals a local `create`, large bodies.

mod common;

use common::*;
use context_api::store::GraphStore;
use reqwest::StatusCode;
use serde_json::{
    json,
    Value,
};

/// Body size of the read test. The positioned reader is superlinear and
/// slow without optimizations, so the default test reads a moderate body;
/// `read_text_body_5mb` (ignored) reads 5 MB.
const BODY_BYTES: usize = 24 * 1024;

async fn read_body_and_compare(bytes: usize) {
    let server = TestServer::start().await;
    let text = corpus(bytes);

    let mut ws = server.ws().await;
    ws.send(json!({ "id": 1, "subscribe": { "graph": "g", "topics": ["graph_changed", "read_progress"] } }))
        .await;
    let (_, subscribed) = ws.until_id(1).await;
    assert_eq!(subscribed["ok"]["subscribed"], json!(["graph_changed", "read_progress"]));

    let (status, read) = server
        .post_text("/v1/graphs/g/read?create=true&label=corpus.txt&split=lines", text.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{read}");
    let report = ok(&read, "read");

    let mut progress = 0;
    let changed = loop {
        let event = ws.recv().await;
        match event["event"].as_str() {
            Some("read_progress") => {
                assert_eq!(event["graph"], "g");
                progress += 1;
            },
            Some("graph_changed") => break event,
            // many short inputs emit more progress events than the channel
            // holds while this test does not read the socket
            Some("lagged") => assert!(event["skipped"].as_u64().unwrap() > 0),
            other => panic!("unexpected message {other:?}: {event}"),
        }
    };
    assert!(progress > 0, "no read_progress events");
    assert_eq!(changed["graph"], "g");
    assert_eq!(changed["vertex_count"], report["vertex_count_after"]);
    assert_eq!(
        changed["new_inputs"].as_array().unwrap().len(),
        report["inputs"].as_array().unwrap().len()
    );

    // the same text read locally
    let local_dir = tempfile::tempdir().unwrap();
    let local = one_shot_session(local_dir.path());
    let created = local_execute(
        &local,
        json!({ "command": "create", "graph": "g", "split": "lines",
                "inputs": [{ "kind": "text", "text": text, "label": "corpus.txt" }] }),
    );
    assert!(created.get("ok").is_some(), "{created}");

    let compare = |remote: Value, local: Value, keys: &[&str]| {
        for key in keys {
            assert_eq!(remote[key], local[key], "{key}");
        }
    };
    let (_, remote_info) = server.get("/v1/graphs/g").await;
    let local_info = local_execute(&local, json!({ "command": "info", "graph": "g" }));
    compare(
        ok(&remote_info, "graph").clone(),
        ok(&local_info, "graph").clone(),
        &["vertex_count", "atom_count", "composite_count", "input_count", "total_input_chars", "generation"],
    );
    let inputs = json!({ "command": "inputs", "graph": "g", "limits": { "limit": 100000 } });
    let (_, mut remote_inputs) = server.execute(inputs.clone()).await;
    let mut local_inputs = local_execute(&local, inputs);
    normalize(&mut remote_inputs);
    normalize(&mut local_inputs);
    assert_eq!(remote_inputs, local_inputs);

    // the on-read save wrote the same graph
    let saved = GraphStore::new(server.store()).summary("g").unwrap();
    assert_eq!(saved.vertex_count, ok(&local_info, "graph")["vertex_count"].as_u64().unwrap() as usize);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn read_text_body() {
    read_body_and_compare(BODY_BYTES).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "5 MB read takes very long with the current reader; run with --release --ignored"]
async fn read_text_body_5mb() {
    read_body_and_compare(5 * 1024 * 1024).await;
}

/// A 5 MB body passes the transport: refused above the limit, UTF-8
/// validated before the reader runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_bodies() {
    let mut body = corpus(5 * 1024 * 1024).into_bytes();
    let server = TestServer::start_with(|config| config.max_body = 4 * 1024 * 1024).await;
    let (status, refused) = server.post_text("/v1/graphs/g/read?create=true", body.clone()).await;
    assert_eq!((status, error_kind(&refused)), (StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    server.stop().await;

    let server = TestServer::start().await;
    body.push(0xff);
    let (status, invalid) = server.post_text("/v1/graphs/g/read?create=true", body).await;
    assert_eq!((status, error_kind(&invalid)), (StatusCode::BAD_REQUEST, "invalid_input"));
    assert!(invalid["error"]["message"].as_str().unwrap().contains("UTF-8"));
    let (status, _) = server.get("/v1/graphs/g").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing was read");
    server.stop().await;
}
