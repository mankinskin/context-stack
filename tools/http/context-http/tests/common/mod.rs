#![allow(dead_code)]
//! Test harness: a server on an ephemeral port with a temp store.

use std::{
    path::PathBuf,
    time::Duration,
};

use context_api::{
    Request,
    Response,
    Session,
    SessionConfig,
};
use context_http::{
    Server,
    ServerConfig,
};
use futures_util::{
    SinkExt,
    StreamExt,
};
use reqwest::StatusCode;
use serde_json::{
    json,
    Value,
};
use tempfile::TempDir;
use tokio::{
    net::TcpStream,
    sync::oneshot,
    task::JoinHandle,
};
use tokio_tungstenite::{
    tungstenite::Message,
    MaybeTlsStream,
    WebSocketStream,
};

pub struct TestServer {
    pub url: String,
    pub dir: TempDir,
    pub client: reqwest::Client,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), context_http::Error>>>,
}

impl TestServer {
    pub async fn start() -> Self {
        Self::start_with(|_| {}).await
    }

    pub async fn start_with(configure: impl FnOnce(&mut ServerConfig)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut config = ServerConfig::new(dir.path().join("store"));
        config.addr = "127.0.0.1:0".parse().unwrap();
        configure(&mut config);
        Self::start_in(dir, config).await
    }

    pub async fn start_in(
        dir: TempDir,
        config: ServerConfig,
    ) -> Self {
        let server = Server::bind(config).await.expect("server binds");
        let url = server.url();
        let (tx, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = rx.await;
        }));
        Self {
            url,
            dir,
            client: reqwest::Client::new(),
            shutdown: Some(tx),
            task: Some(task),
        }
    }

    pub fn store(&self) -> PathBuf {
        self.dir.path().join("store")
    }

    /// Graceful shutdown; returns the temp dir for inspection.
    pub async fn stop(mut self) -> TempDir {
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(
            Duration::from_secs(60),
            self.task.take().unwrap(),
        )
        .await
        .expect("server stops")
        .unwrap()
        .expect("clean shutdown");
        self.dir
    }

    pub fn url(
        &self,
        path: &str,
    ) -> String {
        format!("{}{}", self.url, path)
    }

    pub async fn execute(
        &self,
        request: Value,
    ) -> (StatusCode, Value) {
        let response = self
            .client
            .post(self.url("/v1/execute"))
            .json(&request)
            .send()
            .await
            .unwrap();
        decode(response).await
    }

    pub async fn get(
        &self,
        path: &str,
    ) -> (StatusCode, Value) {
        decode(self.client.get(self.url(path)).send().await.unwrap()).await
    }

    pub async fn post_text(
        &self,
        path: &str,
        text: impl Into<reqwest::Body>,
    ) -> (StatusCode, Value) {
        let response = self
            .client
            .post(self.url(path))
            .header("content-type", "text/plain; charset=utf-8")
            .body(text)
            .send()
            .await
            .unwrap();
        decode(response).await
    }

    pub async fn ws(&self) -> Ws {
        Ws::connect(&format!("{}/v1/ws", self.url.replacen("http", "ws", 1)))
            .await
    }
}

pub async fn decode(response: reqwest::Response) -> (StatusCode, Value) {
    let status = response.status();
    let text = response.text().await.unwrap();
    let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (status, value)
}

pub struct Ws {
    pub stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Ws {
    pub async fn connect(url: &str) -> Self {
        let (stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("ws connects");
        Self { stream }
    }

    pub async fn send(
        &mut self,
        value: Value,
    ) {
        self.send_text(&value.to_string()).await;
    }

    pub async fn send_text(
        &mut self,
        text: &str,
    ) {
        self.stream
            .send(Message::Text(text.to_string().into()))
            .await
            .unwrap();
    }

    /// The next text message (panics after 300 s).
    pub async fn recv(&mut self) -> Value {
        loop {
            let message = tokio::time::timeout(
                Duration::from_secs(300),
                self.stream.next(),
            )
            .await
            .expect("ws message in time")
            .expect("ws open")
            .expect("ws message");
            if let Message::Text(text) = message {
                return serde_json::from_str(text.as_str()).unwrap();
            }
        }
    }

    /// Messages until one with `"id": id`; returns (earlier messages, reply).
    pub async fn until_id(
        &mut self,
        id: u64,
    ) -> (Vec<Value>, Value) {
        let mut before = Vec::new();
        loop {
            let message = self.recv().await;
            if message.get("id") == Some(&json!(id)) {
                return (before, message);
            }
            before.push(message);
        }
    }
}

/// An in-process session configured like the server's, on its own store.
pub fn local_session(dir: &std::path::Path) -> Session {
    let mut config = ServerConfig::new(dir).session_config();
    config.keep_open = true;
    Session::new(config)
}

/// A one-shot session (like the CLI) on `dir`.
pub fn one_shot_session(dir: &std::path::Path) -> Session {
    Session::new(SessionConfig::new(dir))
}

pub fn local_execute(
    session: &Session,
    request: Value,
) -> Value {
    let request: Request = serde_json::from_value(request).unwrap();
    let response: Response = session.execute(request);
    serde_json::to_value(response).unwrap()
}

/// Drop fields that differ between stores or runs (paths, timestamps,
/// file sizes).
pub fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in
                ["path", "created_at", "modified_at", "read_at", "file_size"]
            {
                map.remove(key);
            }
            map.values_mut().for_each(normalize);
        },
        Value::Array(items) => items.iter_mut().for_each(normalize),
        _ => {},
    }
}

/// Deterministic prose-like text of about `len` bytes.
pub fn corpus(len: usize) -> String {
    const WORDS: [&str; 16] = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "and",
        "a", "cat", "sleeps", "in", "warm", "sun", "today",
    ];
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut text = String::with_capacity(len + 16);
    while text.len() < len {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        text.push_str(WORDS[(seed % WORDS.len() as u64) as usize]);
        text.push(if seed.is_multiple_of(11) { '\n' } else { ' ' });
    }
    text
}

pub fn ok<'a>(
    value: &'a Value,
    variant: &str,
) -> &'a Value {
    value
        .get("ok")
        .and_then(|ok| ok.get(variant))
        .unwrap_or_else(|| panic!("expected ok.{variant}: {value}"))
}

pub fn error_kind(value: &Value) -> &str {
    value["error"]["kind"]
        .as_str()
        .unwrap_or_else(|| panic!("expected an error: {value}"))
}
