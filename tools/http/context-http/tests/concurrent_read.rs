//! Queries run while a long read holds the graph's writer slot.

mod common;

use std::time::Duration;

use common::*;
use reqwest::StatusCode;
use serde_json::{
    json,
    Value,
};

/// Start a long read of `g` and run `query` repeatedly until it ends;
/// returns the query replies completed while the read was running.
async fn queries_during_read(
    query: impl Fn(&TestServer) -> Vec<(String, Value)>
) -> Vec<(StatusCode, Value)> {
    let server = TestServer::start().await;
    let (status, _) = server
        .post_text("/v1/graphs/g/read?create=true", "the quick brown fox")
        .await;
    assert_eq!(status, StatusCode::OK);

    let client = server.client.clone();
    let url = server.url("/v1/graphs/g/read?label=long");
    let read = tokio::spawn(async move {
        decode(client.post(url).body(corpus(6000)).send().await.unwrap()).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut during = Vec::new();
    while !read.is_finished() {
        for (path, request) in query(&server) {
            let reply = if request.is_null() {
                server.get(&path).await
            } else {
                server.execute(request).await
            };
            if !read.is_finished() {
                during.push(reply);
            }
        }
    }
    let (status, read) = read.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{read}");
    server.stop().await;
    assert!(!during.is_empty(), "no query completed during the read");
    during
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_queries_during_read() {
    let replies = queries_during_read(|_| {
        vec![
            ("/v1/graphs/g".into(), Value::Null),
            ("/v1/graphs/g/vertex/0".into(), Value::Null),
            (String::new(), json!({ "command": "inputs", "graph": "g" })),
            (String::new(), json!({ "command": "vertex", "graph": "g", "vertex": { "text": "the quick brown fox" } })),
        ]
    })
    .await;
    let mut generations = Vec::new();
    for (status, reply) in &replies {
        assert_eq!(*status, StatusCode::OK, "{reply}");
        if let Some(graph) = reply["ok"].get("graph") {
            generations.push(graph["generation"].as_u64().unwrap());
        }
    }
    assert!(
        generations.windows(2).all(|w| w[0] <= w[1]),
        "generations go forward: {generations:?}"
    );
}

/// Needs `search` in context-api.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_search_during_read() {
    let replies = queries_during_read(|_| {
        vec![
            (
                "/v1/graphs/g/search?q=quick%20brown&view=contexts".into(),
                Value::Null,
            ),
            (
                "/v1/graphs/g/search?q=fox&view=occurrences".into(),
                Value::Null,
            ),
        ]
    })
    .await;
    for (status, reply) in &replies {
        assert_eq!(*status, StatusCode::OK, "{reply}");
        // "the quick brown fox" was read before: always found
        assert_ne!(
            ok(reply, "search")["outcome"]["kind"],
            "not_found",
            "{reply}"
        );
    }
}
