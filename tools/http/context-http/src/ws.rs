//! WebSocket protocol (`GET /v1/ws`).
//!
//! Text frames, one JSON message each:
//!
//! - `{"id": 1, "command": "info", "graph": "g"}` → `{"id": 1, "ok": ...}`
//!   or `{"id": 1, "error": ...}`; requests run concurrently, responses
//!   may arrive out of order;
//! - `{"id": 2, "subscribe": {"graph": "g", "topics": ["graph_changed"]}}`
//!   → `{"id": 2, "ok": {"subscribed": [...]}}` (no topics: all);
//! - `{"id": 3, "unsubscribe": {"graph": "g"}}` → `{"id": 3, "ok":
//!   {"unsubscribed": "g"}}` (no graph: all);
//! - events of subscribed graphs and topics: `{"event": "graph_changed",
//!   ...}`; a subscriber that falls behind gets `{"event": "lagged",
//!   "skipped": N}`.

use std::{
    collections::{
        HashMap,
        HashSet,
    },
    sync::{
        Arc,
        Mutex,
    },
};

use axum::{
    extract::{
        ws::{
            Message,
            WebSocket,
            WebSocketUpgrade,
        },
        State,
    },
    response::Response as HttpResponse,
};
use context_api::{
    ErrorKind,
    Event,
    Request,
    Topic,
};
use futures_util::{
    SinkExt,
    StreamExt,
};
use serde::Deserialize;
use serde_json::{
    json,
    Value,
};
use tokio::sync::{
    broadcast,
    mpsc,
};

use crate::{
    routes::error,
    state::AppState,
};

/// Outgoing frames buffered per socket; a full buffer makes the event
/// forwarder wait, so the broadcast receiver lags instead of the server.
const OUTGOING_CAPACITY: usize = 256;

const ALL_TOPICS: [Topic; 3] =
    [Topic::GraphChanged, Topic::ReadProgress, Topic::Saved];

type Subscriptions = Arc<Mutex<HashMap<String, HashSet<Topic>>>>;

#[derive(Deserialize)]
struct Subscribe {
    graph: String,
    #[serde(default)]
    topics: Vec<Topic>,
}

#[derive(Deserialize)]
struct Unsubscribe {
    #[serde(default)]
    graph: Option<String>,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    upgrade: WebSocketUpgrade,
) -> HttpResponse {
    let max = state.config().max_body;
    upgrade
        .max_message_size(max)
        .max_frame_size(max)
        .on_upgrade(move |socket| run(state, socket))
}

async fn run(
    state: AppState,
    socket: WebSocket,
) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::channel::<String>(OUTGOING_CAPACITY);
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                return;
            }
        }
        let _ = sink.close().await;
    });
    let subscriptions = Subscriptions::default();
    let forwarder = tokio::spawn(forward_events(
        state.subscribe(),
        subscriptions.clone(),
        tx.clone(),
    ));
    let mut shutdown = state.shutdown_receiver();
    loop {
        let message = tokio::select! {
            message = stream.next() => message,
            _ = shutdown.wait_for(|stop| *stop) => break,
        };
        match message {
            Some(Ok(Message::Text(text))) =>
                handle(&state, &subscriptions, &tx, text.as_str()).await,
            Some(Ok(Message::Binary(_))) =>
                send_error(
                    &tx,
                    Value::Null,
                    "binary frames are not supported; send JSON text frames",
                )
                .await,
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {},
            Some(Ok(Message::Close(_)) | Err(_)) | None => break,
        }
    }
    forwarder.abort();
    // The writer ends (and closes the socket) once running requests have
    // sent their responses.
    drop(tx);
    drop(writer);
}

async fn send_error(
    tx: &mpsc::Sender<String>,
    id: Value,
    message: impl Into<String>,
) {
    let reply =
        json!({ "id": id, "error": error(ErrorKind::InvalidInput, message) });
    let _ = tx.send(reply.to_string()).await;
}

async fn handle(
    state: &AppState,
    subscriptions: &Subscriptions,
    tx: &mpsc::Sender<String>,
    text: &str,
) {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) =>
            return send_error(tx, Value::Null, format!("invalid JSON: {err}"))
                .await,
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    if let Some(subscribe) = value.get("subscribe") {
        let Subscribe { graph, topics } =
            match Subscribe::deserialize(subscribe) {
                Ok(subscribe) => subscribe,
                Err(err) =>
                    return send_error(
                        tx,
                        id,
                        format!("invalid subscribe: {err}"),
                    )
                    .await,
            };
        let topics: Vec<Topic> = if topics.is_empty() {
            ALL_TOPICS.to_vec()
        } else {
            topics
        };
        subscriptions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(graph)
            .or_default()
            .extend(topics.iter().copied());
        let _ = tx
            .send(
                json!({ "id": id, "ok": { "subscribed": topics } }).to_string(),
            )
            .await;
    } else if let Some(unsubscribe) = value.get("unsubscribe") {
        let Unsubscribe { graph } = match Unsubscribe::deserialize(unsubscribe)
        {
            Ok(unsubscribe) => unsubscribe,
            Err(err) =>
                return send_error(
                    tx,
                    id,
                    format!("invalid unsubscribe: {err}"),
                )
                .await,
        };
        {
            let mut subscriptions =
                subscriptions.lock().unwrap_or_else(|e| e.into_inner());
            match &graph {
                Some(graph) => {
                    subscriptions.remove(graph);
                },
                None => subscriptions.clear(),
            }
        }
        let _ = tx
            .send(
                json!({ "id": id, "ok": { "unsubscribed": graph } })
                    .to_string(),
            )
            .await;
    } else {
        let request = match Request::deserialize(&value) {
            Ok(request) => request,
            Err(err) =>
                return send_error(tx, id, format!("invalid request: {err}"))
                    .await,
        };
        let state = state.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let response = state.execute(request).await;
            if let Ok(text) = serde_json::to_string(&response) {
                let _ = tx.send(text).await;
            }
        });
    }
}

async fn forward_events(
    mut events: broadcast::Receiver<Event>,
    subscriptions: Subscriptions,
    tx: mpsc::Sender<String>,
) {
    loop {
        let event = match events.recv().await {
            Ok(event) => {
                let wanted = {
                    let subscriptions =
                        subscriptions.lock().unwrap_or_else(|e| e.into_inner());
                    event.graph().zip(event.topic()).is_some_and(
                        |(graph, topic)| {
                            subscriptions
                                .get(graph)
                                .is_some_and(|topics| topics.contains(&topic))
                        },
                    )
                };
                if !wanted {
                    continue;
                }
                event
            },
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                let subscribed = !subscriptions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_empty();
                if !subscribed {
                    continue;
                }
                Event::Lagged { skipped }
            },
            Err(broadcast::error::RecvError::Closed) => return,
        };
        let Ok(text) = serde_json::to_string(&event) else {
            continue;
        };
        if tx.send(text).await.is_err() {
            return;
        }
    }
}
