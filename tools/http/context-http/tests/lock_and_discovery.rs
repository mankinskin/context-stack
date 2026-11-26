//! Discovery files (`<graph>.ctx.server.json`), graph locks and preload.

mod common;

use common::*;
use context_api::{
    store::{
        GraphStore,
        OpenMode,
    },
    ErrorKind,
};
use context_http::{
    Error,
    Server,
    ServerConfig,
};
use reqwest::StatusCode;
use serde_json::{
    json,
    Value,
};

fn discovery(
    server: &TestServer,
    graph: &str,
) -> Option<Value> {
    let path = server.store().join(format!("{graph}.ctx.server.json"));
    std::fs::read(path).ok().map(|bytes| serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lock_and_discovery() {
    let server = TestServer::start().await;
    assert!(discovery(&server, "g").is_none());
    let (status, _) = server.execute(json!({ "command": "new", "graph": "g" })).await;
    assert_eq!(status, StatusCode::OK);
    let info = discovery(&server, "g").expect("discovery file written");
    assert_eq!(info["url"], server.url.as_str());
    assert_eq!(info["pid"], std::process::id());
    assert_eq!(info["token_required"], false);

    // a one-shot client (the CLI) cannot write the served graph
    let cli = one_shot_session(&server.store());
    let reply = local_execute(&cli, json!({ "command": "read", "graph": "g",
        "inputs": [{ "kind": "text", "text": "abc" }] }));
    assert_eq!(error_kind(&reply), "locked");
    let message = reply["error"]["message"].as_str().unwrap();
    assert!(message.contains(&server.url), "{message}");
    assert!(reply["error"]["hint"].as_str().unwrap().contains(&server.url));

    // closing unregisters, the next use registers again
    let (status, _) = server.execute(json!({ "command": "close", "graph": "g" })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(discovery(&server, "g").is_none());
    let (status, _) = server.post_text("/v1/graphs/g/read", "abcab").await;
    assert_eq!(status, StatusCode::OK);
    assert!(discovery(&server, "g").is_some());
    let (status, _) = server.execute(json!({ "command": "delete", "graph": "g" })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(discovery(&server, "g").is_none());

    // a graph held by another process: 409
    let store = GraphStore::new(server.store());
    let held = store.create("held", false).unwrap();
    let (status, locked) = server.post_text("/v1/graphs/held/read", "abc").await;
    assert_eq!((status, error_kind(&locked)), (StatusCode::CONFLICT, "locked"));
    assert!(discovery(&server, "held").is_none());
    drop(held);

    // shutdown removes the discovery files and keeps the graphs
    server.execute(json!({ "command": "new", "graph": "h" })).await;
    assert!(discovery(&server, "h").is_some());
    let dir = server.stop().await;
    let store_dir = dir.path().join("store");
    assert!(store_dir.join("h.ctx").is_file());
    assert!(!store_dir.join("h.ctx.server.json").exists());
    // the lock is released
    store.open("h", OpenMode::Exclusive).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn preload_opens_and_registers() {
    let dir = tempfile::tempdir().unwrap();
    let store_dir = dir.path().join("store");
    let cli = one_shot_session(&store_dir);
    let created = local_execute(&cli, json!({ "command": "create", "graph": "p",
        "inputs": [{ "kind": "text", "text": "abcabc" }] }));
    assert!(created.get("ok").is_some());

    let mut config = ServerConfig::new(&store_dir);
    config.addr = "127.0.0.1:0".parse().unwrap();
    config.preload = vec!["missing".into()];
    match Server::bind(config.clone()).await {
        Err(Error::Preload { graph, error }) => {
            assert_eq!(graph, "missing");
            assert_eq!(error.kind, ErrorKind::NotFound);
        },
        Err(other) => panic!("{other}"),
        Ok(_) => panic!("preload of a missing graph must fail"),
    }

    config.preload = vec!["p".into()];
    let server = TestServer::start_in(dir, config).await;
    assert!(discovery(&server, "p").is_some(), "registered before any request");
    let (_, health) = server.get("/v1/health").await;
    assert_eq!(health["graphs_open"], json!(["p"]));
    let reply = local_execute(&cli, json!({ "command": "read", "graph": "p",
        "inputs": [{ "kind": "text", "text": "x" }] }));
    assert_eq!(error_kind(&reply), "locked");
    server.stop().await;
}
