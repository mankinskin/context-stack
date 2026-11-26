//! In-process tests: an rmcp client talks to the server over a duplex pipe.

use std::{
    borrow::Cow,
    fs,
};

use context_api::{
    Request,
    Session,
    SessionConfig,
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

async fn connect(config: McpConfig) -> Client {
    let (server_io, client_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let service = ContextServer::new(config)
            .serve(server_io)
            .await
            .expect("server starts");
        let _ = service.waiting().await;
    });
    ().serve(client_io).await.expect("client connects")
}

/// Call a tool; the parsed JSON text and the `is_error` flag.
async fn call(
    client: &Client,
    tool: &str,
    arguments: Value,
) -> (Value, bool) {
    let (text, is_error) = call_text(client, tool, arguments).await;
    let value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{tool}: invalid JSON ({e}): {text}"));
    (value, is_error)
}

async fn call_text(
    client: &Client,
    tool: &str,
    arguments: Value,
) -> (String, bool) {
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
    (text, result.is_error == Some(true))
}

fn store() -> (tempfile::TempDir, McpConfig) {
    let dir = tempfile::tempdir().unwrap();
    let config = McpConfig::new(dir.path().join("store"));
    (dir, config)
}

/// Overlapping char offsets of `query` in `text`.
fn brute_force(
    text: &str,
    query: &str,
) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let query: Vec<char> = query.chars().collect();
    (0..chars.len())
        .filter(|&i| chars[i..].starts_with(&query))
        .collect()
}

fn input_ids(read: &Value) -> Vec<u64> {
    read["ok"]["read"]["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|input| input["id"].as_u64().unwrap())
        .collect()
}

#[tokio::test]
async fn list_tools_schemas() {
    let (_dir, config) = store();
    let client = connect(config.clone()).await;
    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in [
        "context_list_graphs",
        "context_graph_info",
        "context_create_graph",
        "context_read",
        "context_search",
        "context_vertex",
        "context_render",
        "context_execute",
        "context_help",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    assert!(!names.contains(&"context_delete_graph"));

    // Example arguments of 06_EXAMPLES.md §3 name only declared properties.
    let examples = [
        (
            "context_create_graph",
            json!({"graph": "repo", "texts": []}),
        ),
        (
            "context_read",
            json!({"graph": "repo", "files": ["src/lib.rs"]}),
        ),
        (
            "context_search",
            json!({"graph": "repo", "query": "fn parse_", "view": "occurrences", "limit": 10}),
        ),
        (
            "context_render",
            json!({"graph": "repo", "root": "impl Display for", "depth": 1}),
        ),
    ];
    for tool in &tools {
        let schema = &tool.input_schema;
        assert_eq!(
            schema.get("type"),
            Some(&json!("object")),
            "{}: {schema:?}",
            tool.name
        );
        assert!(tool.description.as_ref().is_some_and(|d| !d.is_empty()));
        if let Some((_, args)) =
            examples.iter().find(|(name, _)| *name == tool.name)
        {
            let properties = schema["properties"].as_object().unwrap();
            for key in args.as_object().unwrap().keys() {
                assert!(
                    properties.contains_key(key),
                    "{}: no property {key}",
                    tool.name
                );
            }
            for required in schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                assert!(args.get(required.as_str().unwrap()).is_some());
            }
        }
    }
    client.cancel().await.unwrap();

    let client = connect(McpConfig {
        allow_delete: true,
        ..config
    })
    .await;
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|t| t.name == "context_delete_graph"));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn create_read_search() {
    let (_dir, config) = store();
    let client = connect(config).await;
    let first = ["abcabca", "bcab"];
    let more = ["cabcab\nbca", "xbcay"];

    let (created, err) = call(
        &client,
        "context_create_graph",
        json!({"graph": "demo", "texts": first}),
    )
    .await;
    assert!(!err, "{created}");
    let (read, err) = call(
        &client,
        "context_read",
        json!({"graph": "demo", "texts": more}),
    )
    .await;
    assert!(!err, "{read}");
    let ids: Vec<u64> = input_ids(&created)
        .into_iter()
        .chain(input_ids(&read))
        .collect();
    let texts: Vec<&str> = first.iter().chain(&more).copied().collect();
    assert_eq!(ids.len(), texts.len());

    for query in ["bca", "ab", "c", "abcab", "zz"] {
        let (found, err) = call(
            &client,
            "context_search",
            json!({"graph": "demo", "query": query, "view": "occurrences", "limit": 1000}),
        )
        .await;
        assert!(!err, "{found}");
        let mut expected = Vec::new();
        for (id, text) in ids.iter().zip(&texts) {
            expected.extend(
                brute_force(text, query)
                    .into_iter()
                    .map(|offset| (*id, offset as u64)),
            );
        }
        let actual: Vec<(u64, u64)> =
            found["ok"]["search"]["occurrences"]["items"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            (
                                item["input_id"].as_u64().unwrap(),
                                item["offset"].as_u64().unwrap(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
        assert_eq!(actual, expected, "query {query:?}: {found}");
    }

    let (summary, err) = call(
        &client,
        "context_search",
        json!({"graph": "demo", "query": "bca"}),
    )
    .await;
    assert!(!err);
    assert!(summary["ok"]["search"]["outcome"]["kind"].is_string());
    let (vertex, err) = call(
        &client,
        "context_vertex",
        json!({"graph": "demo", "vertex": "#0"}),
    )
    .await;
    assert!(!err, "{vertex}");
    assert_eq!(vertex["ok"]["vertex"]["token"]["index"], 0);
    let (missing, err) = call(
        &client,
        "context_search",
        json!({"graph": "nope", "query": "a"}),
    )
    .await;
    assert!(err);
    assert_eq!(missing["error"]["kind"], "not_found");
    assert!(
        missing["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("context_create_graph")
    );
    client.cancel().await.unwrap();
}

/// Deterministic pseudo-random text.
fn noise(
    seed: u64,
    len: usize,
) -> String {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (b'a' + ((state >> 33) % 6) as u8) as char
        })
        .collect()
}

#[tokio::test]
async fn render_cap() {
    let (_dir, config) = store();
    let client = connect(config).await;
    let texts: Vec<String> = (0..6).map(|seed| noise(seed, 200)).collect();
    let (_, err) = call(
        &client,
        "context_create_graph",
        json!({"graph": "big", "texts": texts}),
    )
    .await;
    assert!(!err);
    let (info, _) =
        call(&client, "context_graph_info", json!({"graph": "big"})).await;
    let vertices = info["ok"]["stats"]["summary"]["vertex_count"]
        .as_u64()
        .unwrap();
    assert!(vertices > 200, "graph too small for the test: {vertices}");

    let (render, err) =
        call(&client, "context_render", json!({"graph": "big"})).await;
    assert!(err, "{render}");
    assert_eq!(render["error"]["kind"], "too_large");
    assert!(render["error"]["hint"].as_str().unwrap().contains("root"));

    // A small subgraph renders.
    let (render, err) = call(
        &client,
        "context_render",
        json!({"graph": "big", "root": "#0", "depth": 1}),
    )
    .await;
    assert!(!err, "{render}");
    assert!(render["ok"]["render"]["text"].as_str().is_some());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn files_require_allow_fs() {
    let (dir, config) = store();
    let docs = dir.path().join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("a.txt"), "hello world").unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("b.txt"), "secret").unwrap();

    let client = connect(config.clone()).await;
    let (refused, err) = call(
        &client,
        "context_read",
        json!({"graph": "g", "files": [docs.join("a.txt")], "create": true}),
    )
    .await;
    assert!(err);
    assert_eq!(refused["error"]["kind"], "forbidden");
    client.cancel().await.unwrap();

    let client = connect(McpConfig {
        allow_fs: Some(docs.clone()),
        ..config
    })
    .await;
    let (read, err) = call(
        &client,
        "context_read",
        json!({"graph": "g", "files": ["a.txt"], "create": true}),
    )
    .await;
    assert!(!err, "{read}");
    assert_eq!(read["ok"]["read"]["inputs"][0]["chars"], 11);
    let (refused, err) = call(
        &client,
        "context_read",
        json!({"graph": "g", "files": [outside.path().join("b.txt")]}),
    )
    .await;
    assert!(err);
    assert_eq!(refused["error"]["kind"], "forbidden");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn output_budget() {
    let (_dir, config) = store();
    let client = connect(McpConfig {
        max_output: 2048,
        ..config
    })
    .await;
    let text = "ab".repeat(400);
    let (_, err) = call(
        &client,
        "context_create_graph",
        json!({"graph": "rep", "texts": [text]}),
    )
    .await;
    assert!(!err);
    let (text, err) = call_text(
        &client,
        "context_search",
        json!({"graph": "rep", "query": "ab", "view": "occurrences", "limit": 500}),
    )
    .await;
    assert!(!err, "{text}");
    assert!(text.len() <= 2048, "{} bytes", text.len());
    let value: Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(value["truncated_output"], true);
    assert!(value["truncated_hint"].is_string());
    let occurrences = &value["ok"]["search"]["occurrences"];
    assert_eq!(occurrences["truncated"], true);
    let kept = occurrences["items"].as_array().unwrap().len();
    assert!(kept > 0 && kept < 400, "kept {kept}");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn execute_escape_hatch() {
    let (_dir, config) = store();
    let client = connect(config).await;
    let reference_dir = tempfile::tempdir().unwrap();
    let reference = Session::new(SessionConfig::new(reference_dir.path()));

    let script = [
        json!({"id": 1, "command": "create", "graph": "g", "inputs": [
            {"kind": "text", "text": "abcabca"}, {"kind": "text", "text": "bcab"}]}),
        json!({"id": 2, "command": "search", "graph": "g", "query": "bca", "view": "occurrences"}),
        json!({"id": 3, "command": "search", "graph": "g", "query": "ca", "view": "contexts"}),
        json!({"id": 4, "command": "vertex", "graph": "g", "vertex": {"text": "bc"}}),
        json!({"id": 5, "command": "render", "graph": "g", "format": "dot"}),
        json!({"id": 6, "command": "info", "graph": "missing"}),
    ];
    for request in script {
        let (text, is_error) =
            call_text(&client, "context_execute", request.clone()).await;
        let expected = reference.execute(
            serde_json::from_value::<Request>(request.clone()).unwrap(),
        );
        assert_eq!(is_error, !expected.is_ok());
        if request["command"] == "create" {
            // Paths and timestamps differ between the stores.
            assert!(!is_error, "{text}");
            continue;
        }
        assert_eq!(
            text,
            serde_json::to_string(&expected).unwrap(),
            "request {request}"
        );
    }

    // MCP policies apply to raw requests too.
    let (denied, err) = call(
        &client,
        "context_execute",
        json!({"id": 7, "command": "delete", "graph": "g"}),
    )
    .await;
    assert!(err);
    assert_eq!(denied["error"]["kind"], "forbidden");
    assert_eq!(denied["id"], 7);
    let (denied, err) = call(
        &client,
        "context_execute",
        json!({"command": "info", "graph": "../elsewhere/x.ctx"}),
    )
    .await;
    assert!(err);
    assert_eq!(denied["error"]["kind"], "forbidden");
    client.cancel().await.unwrap();
}
