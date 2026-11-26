//! Bearer token auth and file input access.

mod common;

use common::*;
use context_http::{
    AppState,
    Error,
    Server,
    ServerConfig,
};
use reqwest::StatusCode;
use serde_json::json;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test]
async fn non_loopback_requires_token() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = ServerConfig::new(dir.path());
    config.addr = "0.0.0.0:0".parse().unwrap();
    let err = AppState::new(config.clone()).err().expect("refused");
    assert!(matches!(err, Error::Config(_)));
    assert!(err.to_string().contains("token"), "{err}");
    assert!(matches!(Server::bind(config).await, Err(Error::Config(_))));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn token_required() {
    let server = TestServer::start_with(|config| {
        config.addr = "0.0.0.0:0".parse().unwrap();
        config.token = Some("s3cret".into());
    })
    .await;
    let url = server.url.replace("0.0.0.0", "127.0.0.1");

    let get = |token: Option<&str>| {
        let mut request = server.client.get(format!("{url}/v1/graphs"));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send()
    };
    let missing = get(None).await.unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(missing.headers()["www-authenticate"], "Bearer");
    let (_, body) = decode(missing).await;
    assert!(body["error"]["message"].as_str().unwrap().contains("token"));
    assert_eq!(
        get(Some("wrong")).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(get(Some("s3cret")).await.unwrap().status(), StatusCode::OK);
    let execute = server
        .client
        .post(format!("{url}/v1/execute"))
        .json(&json!({ "command": "new", "graph": "g" }))
        .send()
        .await
        .unwrap();
    assert_eq!(execute.status(), StatusCode::UNAUTHORIZED);
    let health = server
        .client
        .get(format!("{url}/v1/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    // WebSocket: header or ?token=
    let ws_url = format!("{}/v1/ws", url.replacen("http", "ws", 1));
    assert!(tokio_tungstenite::connect_async(ws_url.as_str())
        .await
        .is_err());
    assert!(
        tokio_tungstenite::connect_async(format!("{ws_url}?token=wrong"))
            .await
            .is_err()
    );
    let mut ws = Ws::connect(&format!("{ws_url}?token=s3cret")).await;
    ws.send(json!({ "id": 1, "command": "new", "graph": "g" }))
        .await;
    let (_, reply) = ws.until_id(1).await;
    assert!(reply.get("ok").is_some(), "{reply}");
    let mut request = ws_url.as_str().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("authorization", "Bearer s3cret".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_ok());

    // the discovery file announces the token requirement, never the token
    let discovery =
        std::fs::read_to_string(server.store().join("g.ctx.server.json"))
            .unwrap();
    let info: serde_json::Value = serde_json::from_str(&discovery).unwrap();
    assert_eq!(info["token_required"], true);
    assert!(!discovery.contains("s3cret"));
    server.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn file_inputs_need_allow_fs() {
    let files = tempfile::tempdir().unwrap();
    let allowed = files.path().join("allowed");
    std::fs::create_dir_all(&allowed).unwrap();
    let inside = allowed.join("a.txt");
    let outside = files.path().join("b.txt");
    std::fs::write(&inside, "abcabc").unwrap();
    std::fs::write(&outside, "xyzxyz").unwrap();
    let read = |path: &std::path::Path| {
        json!({ "command": "read", "graph": "g", "create": true,
                "inputs": [{ "kind": "file", "path": path }] })
    };

    let server = TestServer::start().await;
    let (status, refused) = server.execute(read(&inside)).await;
    assert_eq!(
        (status, error_kind(&refused)),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    server.stop().await;

    let allow = allowed.clone();
    let server =
        TestServer::start_with(|config| config.allow_fs = Some(allow)).await;
    let (status, read_inside) = server.execute(read(&inside)).await;
    assert_eq!(status, StatusCode::OK, "{read_inside}");
    assert_eq!(ok(&read_inside, "read")["inputs"][0]["chars"], 6);
    let (status, refused) = server.execute(read(&outside)).await;
    assert_eq!(
        (status, error_kind(&refused)),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    let (status, refused) =
        server.execute(read(&allowed.join("../b.txt"))).await;
    assert_eq!(
        (status, error_kind(&refused)),
        (StatusCode::FORBIDDEN, "forbidden")
    );
    #[cfg(unix)]
    {
        let link = allowed.join("link.txt");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let (status, refused) = server.execute(read(&link)).await;
        assert_eq!(
            (status, error_kind(&refused)),
            (StatusCode::FORBIDDEN, "forbidden")
        );
    }
    server.stop().await;
}
