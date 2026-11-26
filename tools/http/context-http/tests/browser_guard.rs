//! Browser protection (Origin and Host checks) and discovery of graphs
//! opened by path.

mod common;

use common::*;
use reqwest::StatusCode;
use serde_json::json;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn foreign_origins_and_hosts_are_refused() {
    let server = TestServer::start_with(|config| {
        config.cors_origins = vec!["http://viewer.example".into()];
    })
    .await;
    let post = |origin: Option<&str>, graph: &str| {
        let mut request = server
            .client
            .post(format!("{}/v1/execute", server.url))
            .header("content-type", "text/plain")
            .body(json!({ "command": "new", "graph": graph }).to_string());
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        request.send()
    };
    // A no-cors POST from a foreign page must not run.
    let foreign = post(Some("http://evil.example"), "a").await.unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert!(!server.store().join("a.ctx").exists());
    // Same origin, configured origin and non-browser clients work.
    assert_eq!(post(Some(&server.url), "b").await.unwrap().status(), StatusCode::OK);
    assert_eq!(post(Some("http://viewer.example"), "c").await.unwrap().status(), StatusCode::OK);
    assert_eq!(post(None, "d").await.unwrap().status(), StatusCode::OK);

    // WebSocket upgrades from foreign pages are refused.
    let ws_url = format!("{}/v1/ws", server.url.replacen("http", "ws", 1));
    let mut request = ws_url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("origin", "http://evil.example".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    let mut request = ws_url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("origin", "http://viewer.example".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_ok());

    // DNS rebinding: a foreign host name reaching a token-less server.
    let rebound = server
        .client
        .get(format!("{}/v1/graphs", server.url))
        .header("host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(rebound.status(), StatusCode::FORBIDDEN);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn graphs_with_the_same_name_are_discovered_separately() {
    let files = tempfile::tempdir().unwrap();
    let allowed = files.path().canonicalize().unwrap();
    let server = TestServer::start_with(|config| {
        config.allow_fs = Some(allowed.clone());
    })
    .await;
    let by_path = allowed.join("foo.ctx");
    for graph in ["foo".to_string(), by_path.display().to_string()] {
        let (status, body) = server
            .execute(json!({ "command": "new", "graph": graph }))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert!(server.store().join("foo.ctx.server.json").exists());
    assert!(allowed.join("foo.ctx.server.json").exists());
    let (status, _) = server
        .execute(json!({ "command": "close", "graph": "foo" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!server.store().join("foo.ctx.server.json").exists());
    assert!(allowed.join("foo.ctx.server.json").exists());
    server.stop().await;
    assert!(!allowed.join("foo.ctx.server.json").exists());
}
