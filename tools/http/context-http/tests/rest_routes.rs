//! REST convenience routes, status mapping, body limit, CORS and static
//! files.

mod common;

use common::*;
use context_api::ErrorKind;
use context_http::status_for;
use reqwest::StatusCode;
use serde_json::json;

async fn server_with_graph() -> TestServer {
    let server = TestServer::start().await;
    let (status, _) = server
        .post_text("/v1/graphs/g/read?create=true&label=first", "abcabcab")
        .await;
    assert_eq!(status, StatusCode::OK);
    server
}

#[test]
fn status_mapping() {
    let cases = [
        (ErrorKind::NotFound, 404),
        (ErrorKind::AlreadyExists, 409),
        (ErrorKind::Locked, 409),
        (ErrorKind::InvalidInput, 400),
        (ErrorKind::Forbidden, 403),
        (ErrorKind::TooLarge, 413),
        (ErrorKind::Corrupt, 500),
        (ErrorKind::Io, 500),
        (ErrorKind::Unsupported, 501),
        (ErrorKind::Internal, 500),
    ];
    for (kind, status) in cases {
        assert_eq!(status_for(kind).as_u16(), status, "{kind:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rest_routes() {
    let server = server_with_graph().await;

    let (status, health) = server.get("/v1/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");
    assert_eq!(health["graphs_open"], json!(["g"]));

    let (status, list) = server.get("/v1/graphs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ok(&list, "graphs")["graphs"][0]["name"], "g");

    let (status, info) = server.get("/v1/graphs/g").await;
    assert_eq!(status, StatusCode::OK);
    let summary = ok(&info, "graph");
    assert_eq!((summary["input_count"].as_u64(), summary["open"].as_bool()), (Some(1), Some(true)));

    // read with label and split, appended to the open graph
    let (status, read) = server
        .post_text("/v1/graphs/g/read?label=notes&split=lines", "bcab\n\nxyzab\n")
        .await;
    assert_eq!(status, StatusCode::OK);
    let report = ok(&read, "read");
    let labels: Vec<_> = report["inputs"].as_array().unwrap().iter().map(|i| i["label"].clone()).collect();
    assert_eq!(labels.len(), 2);
    assert!(report["saved"].is_object(), "on-read saves: {read}");

    // vertex by text and by index
    let (status, vertex) = server.get("/v1/graphs/g/vertex/abcabcab").await;
    assert_eq!(status, StatusCode::OK, "{vertex}");
    let index = ok(&vertex, "vertex")["token"]["index"].as_u64().unwrap();
    assert_eq!(ok(&vertex, "vertex")["token"]["label"], "abcabcab");
    let (status, by_index) = server.get(&format!("/v1/graphs/g/vertex/{index}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ok(&by_index, "vertex")["token"]["label"], "abcabcab");

    let (status, saved) = decode(server.client.post(server.url("/v1/graphs/g/save")).send().await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(ok(&saved, "saved")["bytes"].as_u64().unwrap() > 0);

    // status mapping
    let (status, missing) = server.get("/v1/graphs/missing").await;
    assert_eq!((status, error_kind(&missing)), (StatusCode::NOT_FOUND, "not_found"));
    let (status, missing) = server.get("/v1/graphs/g/vertex/9999999").await;
    assert_eq!((status, error_kind(&missing)), (StatusCode::NOT_FOUND, "not_found"));
    let (status, bad) = server.get("/v1/graphs/.bad").await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, exists) = server.execute(json!({ "id": 5, "command": "new", "graph": "g" })).await;
    assert_eq!((status, error_kind(&exists)), (StatusCode::CONFLICT, "already_exists"));
    assert_eq!(exists["id"], 5);
    let (status, refused) = server
        .execute(json!({ "command": "read", "graph": "g", "inputs": [{ "kind": "file", "path": "/etc/hostname" }] }))
        .await;
    assert_eq!((status, error_kind(&refused)), (StatusCode::FORBIDDEN, "forbidden"));
    let (status, invalid) = server.execute(json!({ "command": "frobnicate" })).await;
    assert_eq!((status, error_kind(&invalid)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let response = server
        .client
        .post(server.url("/v1/execute"))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    let (status, invalid) = decode(response).await;
    assert_eq!((status, error_kind(&invalid)), (StatusCode::BAD_REQUEST, "invalid_input"));

    // invalid query parameters and bodies
    let (status, bad) = server.get("/v1/graphs/g/search").await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, bad) = server.get("/v1/graphs/g/search?q=ab&view=everything").await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, bad) = server.get("/v1/graphs/g/render?format=png").await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, bad) = server.post_text("/v1/graphs/g/read?split=words", "ab").await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, bad) = server.post_text("/v1/graphs/g/read", vec![b'a', 0xff, 0xfe]).await;
    assert_eq!((status, error_kind(&bad)), (StatusCode::BAD_REQUEST, "invalid_input"));
    let (status, missing) = server.post_text("/v1/graphs/new-one/read", "ab").await;
    assert_eq!((status, error_kind(&missing)), (StatusCode::NOT_FOUND, "not_found"));

    server.stop().await;
}

/// Needs `search` and `render` in context-api.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn search_and_render_routes() {
    let server = server_with_graph().await;

    let (status, search) = server.get("/v1/graphs/g/search?q=bca&view=contexts&limit=5&width=3").await;
    assert_eq!(status, StatusCode::OK, "{search}");
    assert_eq!(ok(&search, "search")["query"], "bca");
    let (status, search) = server.get("/v1/graphs/g/search?q=bca").await;
    assert_eq!(status, StatusCode::OK, "{search}");

    for (format, content_type) in [
        ("dot", "text/vnd.graphviz"),
        ("ascii", "text/plain; charset=utf-8"),
        ("mermaid", "text/plain; charset=utf-8"),
    ] {
        let response = server
            .client
            .get(server.url(&format!("/v1/graphs/g/render?format={format}")))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{format}");
        let header = response.headers()["content-type"].to_str().unwrap().to_string();
        assert!(header.starts_with(content_type), "{format}: {header}");
        let body = response.text().await.unwrap();
        assert!(!body.is_empty() && !body.starts_with("{\"ok\""), "{format}: {body}");
        if format == "dot" {
            assert!(body.contains("digraph"), "{body}");
        }
    }
    let (status, json_render) = server.get("/v1/graphs/g/render?format=json&root=abcabcab&depth=1").await;
    assert_eq!(status, StatusCode::OK, "{json_render}");
    assert!(ok(&json_render, "render")["graph"]["vertices"].is_array());

    // the gate: a graph above the default limit needs force
    let (status, _) = server
        .post_text("/v1/graphs/big/read?create=true", corpus(3000))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, gated) = server.get("/v1/graphs/big/render?format=dot").await;
    assert_eq!((status, error_kind(&gated)), (StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    let (status, _) = server.get("/v1/graphs/big/render?format=dot&force=true").await;
    assert_eq!(status, StatusCode::OK);

    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn body_limit() {
    let server = TestServer::start_with(|config| config.max_body = 1024).await;
    let (status, too_large) = server.post_text("/v1/graphs/g/read?create=true", "a".repeat(4096)).await;
    assert_eq!((status, error_kind(&too_large)), (StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    let big_request = json!({ "command": "read", "graph": "g", "create": true,
        "inputs": [{ "kind": "text", "text": "a".repeat(4096) }] });
    let (status, too_large) = server.execute(big_request).await;
    assert_eq!((status, error_kind(&too_large)), (StatusCode::PAYLOAD_TOO_LARGE, "too_large"));
    let (status, _) = server.post_text("/v1/graphs/g/read?create=true", "abcab").await;
    assert_eq!(status, StatusCode::OK);
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cors_and_static_files() {
    // no CORS by default
    let server = TestServer::start().await;
    let response = server
        .client
        .get(server.url("/v1/health"))
        .header("origin", "http://example.com")
        .send()
        .await
        .unwrap();
    assert!(response.headers().get("access-control-allow-origin").is_none());
    server.stop().await;

    let frontend = tempfile::tempdir().unwrap();
    std::fs::write(frontend.path().join("index.html"), "<html>viewer</html>").unwrap();
    std::fs::write(frontend.path().join("app.js"), "console.log(1)").unwrap();
    let static_dir = frontend.path().to_path_buf();
    let server = TestServer::start_with(|config| {
        config.cors_origins = vec!["http://example.com".into()];
        config.static_dir = Some(static_dir);
    })
    .await;
    let allowed = server
        .client
        .get(server.url("/v1/health"))
        .header("origin", "http://example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(allowed.headers()["access-control-allow-origin"], "http://example.com");
    let other = server
        .client
        .get(server.url("/v1/health"))
        .header("origin", "http://evil.example")
        .send()
        .await
        .unwrap();
    assert!(other.headers().get("access-control-allow-origin").is_none());

    let app = server.client.get(server.url("/app.js")).send().await.unwrap();
    assert_eq!(app.text().await.unwrap(), "console.log(1)");
    let spa = server.client.get(server.url("/graphs/g/view")).send().await.unwrap();
    assert_eq!(spa.status(), StatusCode::OK);
    assert_eq!(spa.text().await.unwrap(), "<html>viewer</html>");
    server.stop().await;
}
