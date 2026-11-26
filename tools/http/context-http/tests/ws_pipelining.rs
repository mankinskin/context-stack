//! WebSocket protocol: pipelined requests answered by id (out of order),
//! subscriptions, protocol errors.

mod common;

use std::collections::HashMap;

use common::*;
use reqwest::StatusCode;
use serde_json::{
    json,
    Value,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ws_pipelining() {
    let server = TestServer::start().await;
    let (status, _) = server.post_text("/v1/graphs/g/read?create=true", "abcabcab").await;
    assert_eq!(status, StatusCode::OK);

    let mut ws = server.ws().await;
    // a long read first, then quick queries: the queries do not wait for it
    ws.send(json!({ "id": 1, "command": "read", "graph": "g",
                    "inputs": [{ "kind": "text", "text": corpus(6000) }] }))
        .await;
    for id in 2..=6u64 {
        ws.send(json!({ "id": id, "command": "info", "graph": "g" })).await;
    }
    ws.send(json!({ "id": 7, "command": "vertex", "graph": "g", "vertex": { "text": "abcabcab" } }))
        .await;
    ws.send(json!({ "id": 8, "command": "info", "graph": "missing" })).await;
    ws.send(json!({ "id": 9, "command": "info" })).await;
    ws.send(json!({ "id": 10, "subscribe": { "graph": "g" } })).await;
    ws.send_text("{oops").await;

    let mut replies: HashMap<Value, Value> = HashMap::new();
    let mut order = Vec::new();
    while replies.len() < 11 {
        let message = ws.recv().await;
        if message.get("event").is_some() {
            continue;
        }
        let id = message["id"].clone();
        order.push(id.clone());
        assert!(replies.insert(id.clone(), message).is_none(), "duplicate reply {id}");
    }
    assert_eq!(ok(&replies[&json!(1)], "read")["inputs"].as_array().unwrap().len(), 1);
    for id in 2..=6 {
        assert_eq!(ok(&replies[&json!(id)], "graph")["name"], "g");
    }
    assert_eq!(ok(&replies[&json!(7)], "vertex")["token"]["label"], "abcabcab");
    assert_eq!(error_kind(&replies[&json!(8)]), "not_found");
    assert_eq!(error_kind(&replies[&json!(9)]), "invalid_input");
    assert_eq!(
        replies[&json!(10)]["ok"]["subscribed"],
        json!(["graph_changed", "read_progress", "saved"])
    );
    assert_eq!(error_kind(&replies[&Value::Null]), "invalid_input");
    let read_position = order.iter().position(|id| id == &json!(1)).unwrap();
    let first_info = order.iter().position(|id| id == &json!(2)).unwrap();
    assert!(first_info < read_position, "queries answered before the long read: {order:?}");

    // events of the subscribed graph; none after unsubscribe
    ws.send(json!({ "id": 11, "command": "read", "graph": "g", "inputs": [{ "kind": "text", "text": "xyzxy" }] }))
        .await;
    let mut replied = false;
    loop {
        let message = ws.recv().await;
        replied |= message["id"] == 11;
        if message["event"] == "graph_changed" && message["new_inputs"] == json!([2]) {
            break;
        }
    }
    if !replied {
        ws.until_id(11).await;
    }
    ws.send(json!({ "id": 12, "unsubscribe": { "graph": "g" } })).await;
    let (_, reply) = ws.until_id(12).await;
    assert_eq!(reply["ok"]["unsubscribed"], "g");
    ws.send(json!({ "id": 13, "command": "read", "graph": "g", "inputs": [{ "kind": "text", "text": "qrsqr" }] }))
        .await;
    let (mut messages, _) = ws.until_id(13).await;
    ws.send(json!({ "id": 14, "command": "list" })).await;
    messages.extend(ws.until_id(14).await.0);
    let late: Vec<_> = messages.iter().filter(|m| m["event"] == "graph_changed" && m["new_inputs"] == json!([3])).collect();
    assert!(late.is_empty(), "events after unsubscribe: {late:?}");
    server.stop().await;
}
