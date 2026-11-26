//! `--save on-read | manual | interval`, and saving on graceful shutdown.

mod common;

use std::time::{
    Duration,
    Instant,
};

use common::*;
use context_api::{
    store::GraphStore,
    SaveMode,
};
use reqwest::StatusCode;

/// Inputs in the saved file (read without locking).
fn saved_inputs(server: &TestServer) -> usize {
    GraphStore::new(server.store()).summary("g").unwrap().input_count
}

async fn new_graph(server: &TestServer) {
    let (status, _) = server.execute(serde_json::json!({ "command": "new", "graph": "g" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_inputs(server), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn on_read_saves_after_every_read() {
    let server = TestServer::start().await;
    new_graph(&server).await;
    let (status, read) = server.post_text("/v1/graphs/g/read", "abcabc").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ok(&read, "read")["saved"].is_object());
    assert_eq!(saved_inputs(&server), 1);
    let (_, info) = server.get("/v1/graphs/g").await;
    assert_eq!(ok(&info, "graph")["dirty"], false);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn manual_saves_only_on_save() {
    let server = TestServer::start_with(|config| config.save = SaveMode::Manual).await;
    new_graph(&server).await;
    let (status, read) = server.post_text("/v1/graphs/g/read", "abcabc").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ok(&read, "read")["saved"].is_null());
    assert_eq!(saved_inputs(&server), 0);
    let (_, info) = server.get("/v1/graphs/g").await;
    assert_eq!(ok(&info, "graph")["dirty"], true);

    let (status, _) = decode(server.client.post(server.url("/v1/graphs/g/save")).send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_inputs(&server), 1);
    let (_, info) = server.get("/v1/graphs/g").await;
    assert_eq!(ok(&info, "graph")["dirty"], false);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_saves_dirty_graphs() {
    let server = TestServer::start_with(|config| config.save = SaveMode::Manual).await;
    new_graph(&server).await;
    server.post_text("/v1/graphs/g/read", "abcabc").await;
    server.post_text("/v1/graphs/g/read", "bcabx").await;
    assert_eq!(saved_inputs(&server), 0);
    let dir = server.stop().await;
    let saved = GraphStore::new(dir.path().join("store")).summary("g").unwrap();
    assert_eq!(saved.input_count, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn interval_saves_dirty_graphs() {
    let server = TestServer::start_with(|config| {
        config.save = SaveMode::Manual;
        config.save_interval = Some(Duration::from_millis(200));
    })
    .await;
    new_graph(&server).await;
    server.post_text("/v1/graphs/g/read", "abcabc").await;
    let start = Instant::now();
    while saved_inputs(&server) == 0 {
        assert!(start.elapsed() < Duration::from_secs(20), "no periodic save");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (_, info) = server.get("/v1/graphs/g").await;
    assert_eq!(ok(&info, "graph")["dirty"], false);
    server.stop().await;
}
