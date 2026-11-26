//! MCP proxy mode (plan 05 §5.3): the MCP tools over a `RemoteBackend`
//! give the same results as a local MCP server, and their reads reach the
//! server's other clients as events.

#![cfg(all(feature = "server", feature = "mcp"))]

mod common;

use std::{
    borrow::Cow,
    sync::Arc,
    time::Duration,
};

use common::ServerProcess;
use context_api::{
    Event,
    Topic,
    client::RemoteBackend,
};
use context_mcp::{
    ContextServer,
    McpConfig,
};
use rmcp::{
    RoleClient,
    ServiceExt,
    model::CallToolRequestParams,
    service::RunningService,
};
use serde_json::{
    Value,
    json,
};

type Client = RunningService<RoleClient, ()>;

async fn connect(server: ContextServer) -> Client {
    let (server_io, client_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let service = server.serve(server_io).await.expect("server starts");
        let _ = service.waiting().await;
    });
    ().serve(client_io).await.expect("client connects")
}

/// The text result of a tool call.
async fn call(
    client: &Client,
    tool: &str,
    arguments: Value,
) -> String {
    let Value::Object(arguments) = arguments else {
        panic!("arguments must be an object")
    };
    let result = client
        .call_tool(CallToolRequestParams {
            meta: None,
            name: Cow::Owned(tool.to_string()),
            arguments: Some(arguments),
            task: None,
        })
        .await
        .unwrap_or_else(|e| panic!("{tool}: protocol error {e}"));
    let text = result.content[0]
        .as_text()
        .expect("text content")
        .text
        .clone();
    assert_ne!(result.is_error, Some(true), "{tool}: {text}");
    text
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_proxy() {
    let local_dir = tempfile::tempdir().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    let server = ServerProcess::start(remote_dir.path(), &[]);

    let local =
        connect(ContextServer::new(McpConfig::new(local_dir.path()))).await;
    // The proxy's own store is unused: every call goes to the server.
    let proxy_config = McpConfig::new(local_dir.path().join("unused"));
    let remote = RemoteBackend::new(server.url.clone(), None);
    let proxy = connect(ContextServer::with_backend(
        Arc::new(remote.clone()),
        proxy_config,
    ))
    .await;

    let create =
        json!({"graph": "g", "texts": ["the brown fox", "the brown dog"]});
    let search = json!({"graph": "g", "query": "brown", "view": "occurrences"});
    for client in [&local, &proxy] {
        call(client, "context_create_graph", create.clone()).await;
    }
    let local_search = call(&local, "context_search", search.clone()).await;
    let proxy_search = call(&proxy, "context_search", search.clone()).await;
    assert_eq!(local_search, proxy_search);
    assert!(local_search.contains(r#""occurrences""#), "{local_search}");

    // A read through the proxy is a graph change for other clients.
    let mut events = tokio::task::spawn_blocking(move || {
        remote.subscribe("g", &[Topic::GraphChanged]).unwrap()
    })
    .await
    .unwrap();
    call(
        &proxy,
        "context_read",
        json!({"graph": "g", "texts": ["a brown cow"]}),
    )
    .await;
    let event = tokio::task::spawn_blocking(move || {
        events.recv_timeout(Some(Duration::from_secs(10))).unwrap()
    })
    .await
    .unwrap();
    match event {
        Some(Event::GraphChanged {
            graph, new_inputs, ..
        }) => {
            assert_eq!(graph, "g");
            assert_eq!(new_inputs, [2]);
        },
        other => panic!("{other:?}"),
    }

    // MCP-side policies still apply in proxy mode.
    let Value::Object(arguments) = json!({"graph": "g", "overwrite": true})
    else {
        unreachable!()
    };
    let refused = proxy
        .call_tool(CallToolRequestParams {
            meta: None,
            name: Cow::Borrowed("context_create_graph"),
            arguments: Some(arguments),
            task: None,
        })
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    let _ = local.cancel().await;
    let _ = proxy.cancel().await;
}
