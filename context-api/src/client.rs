//! Blocking client of a context server (feature `client`).
//!
//! - [`RemoteBackend`]: a [`Backend`] that sends every [`Request`] to
//!   `POST /v1/execute` (ureq, no async runtime), plus the streamed
//!   `POST /v1/graphs/{g}/read` route for large texts;
//! - [`EventStream`] / [`EventWatcher`]: event subscriptions over the
//!   WebSocket route `GET /v1/ws` (tungstenite);
//! - [`discover`]: the live server holding a graph, if any.
//!
//! Transport failures become [`Response`]s with [`ErrorKind::Io`] naming the
//! server URL, so callers handle remote and local errors alike.

use std::{
    collections::VecDeque,
    io::{
        self,
        Read,
    },
    net::{
        TcpStream,
        ToSocketAddrs,
    },
    sync::{
        Arc,
        atomic::{
            AtomicBool,
            Ordering,
        },
    },
    thread::JoinHandle,
    time::Duration,
};

use serde::Deserialize;
use serde_json::{
    Value,
    json,
};
use tungstenite::{
    Message,
    WebSocket,
    client::IntoClientRequest,
};

use crate::{
    Backend,
    ErrorKind,
    ErrorResponse,
    Event,
    Outcome,
    Request,
    Response,
    Topic,
    store::{
        GraphStore,
        ServerInfo,
    },
    types::Split,
};

/// Time a discovered server has to answer `/v1/health`.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(200);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Poll interval of [`EventWatcher`] threads (how fast they stop).
const WATCH_POLL: Duration = Duration::from_millis(50);

/// `GET /v1/health`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Health {
    pub status: String,
    pub version: String,
    #[serde(default)]
    pub graphs_open: Vec<String>,
}

fn error(
    kind: ErrorKind,
    message: impl Into<String>,
    hint: Option<String>,
) -> ErrorResponse {
    ErrorResponse {
        kind,
        message: message.into(),
        hint,
    }
}

fn error_reply(
    id: Option<u64>,
    error: ErrorResponse,
) -> Response {
    Response {
        id,
        outcome: Outcome::Error(error),
    }
}

/// Percent-encode a path segment.
fn encode_segment(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
        {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A [`Backend`] executing requests on a context server over HTTP.
#[derive(Clone)]
pub struct RemoteBackend {
    url: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl std::fmt::Debug for RemoteBackend {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("RemoteBackend")
            .field("url", &self.url)
            .field("token", &self.token.as_ref().map(|_| "***"))
            .finish()
    }
}

impl RemoteBackend {
    /// `url` is the server base URL (`http://127.0.0.1:7878`).
    pub fn new(
        url: impl Into<String>,
        token: Option<String>,
    ) -> Self {
        let url = url.into().trim_end_matches('/').to_string();
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .build();
        Self {
            url,
            token: token.filter(|token| !token.is_empty()),
            agent,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn request(
        &self,
        agent: &ureq::Agent,
        method: &str,
        path: &str,
    ) -> ureq::Request {
        let request = agent.request(method, &format!("{}{path}", self.url));
        match &self.token {
            Some(token) =>
                request.set("Authorization", &format!("Bearer {token}")),
            None => request,
        }
    }

    fn unreachable(
        &self,
        err: impl std::fmt::Display,
    ) -> ErrorResponse {
        error(
            ErrorKind::Io,
            format!("cannot reach server {}: {err}", self.url),
            Some(
                "is `context-cli serve` running? check --server / CONTEXT_SERVER, or pass --local"
                    .into(),
            ),
        )
    }

    /// Decode a reply: a `Response` body for any status; transport errors
    /// and foreign bodies become `Io` errors naming the URL.
    fn reply(
        &self,
        id: Option<u64>,
        result: Result<ureq::Response, ureq::Error>,
    ) -> Response {
        let (status, response) = match result {
            Ok(response) => (response.status(), response),
            Err(ureq::Error::Status(status, response)) => (status, response),
            Err(ureq::Error::Transport(err)) =>
                return error_reply(id, self.unreachable(err)),
        };
        let mut body = String::new();
        if let Err(err) = response.into_reader().read_to_string(&mut body) {
            return error_reply(id, self.unreachable(err));
        }
        match serde_json::from_str::<Response>(&body) {
            Ok(mut response) => {
                if status == 401
                    && let Outcome::Error(error) = &mut response.outcome
                    && error.hint.is_none()
                {
                    error.hint = Some(format!(
                        "the server {} requires a bearer token: pass --token or set CONTEXT_TOKEN",
                        self.url
                    ));
                }
                response.id = response.id.or(id);
                response
            },
            Err(err) => {
                let excerpt: String = body.chars().take(200).collect();
                error_reply(
                    id,
                    error(
                        ErrorKind::Io,
                        format!(
                            "unexpected reply from {} (HTTP {status}, {err}): {}",
                            self.url,
                            excerpt.trim()
                        ),
                        Some("is this URL a context server?".into()),
                    ),
                )
            },
        }
    }

    /// Read one text through the streamed `POST /v1/graphs/{g}/read`
    /// route (no JSON encoding of the text; for large inputs).
    pub fn read_text(
        &self,
        graph: &str,
        label: Option<&str>,
        text: &str,
        split: Split,
        create: bool,
    ) -> Response {
        let split = match split {
            Split::None => "none",
            Split::Lines => "lines",
            Split::Paragraphs => "paragraphs",
        };
        let mut request = self
            .request(
                &self.agent,
                "POST",
                &format!("/v1/graphs/{}/read", encode_segment(graph)),
            )
            .set("Content-Type", "text/plain; charset=utf-8")
            .query("split", split)
            .query("create", if create { "true" } else { "false" });
        if let Some(label) = label {
            request = request.query("label", label);
        }
        self.reply(None, request.send_string(text))
    }

    /// `GET /v1/health` with the default timeouts.
    pub fn health(&self) -> Result<Health, ErrorResponse> {
        self.health_within(CONNECT_TIMEOUT)
    }

    /// `GET /v1/health`, failing if the server does not answer in time.
    pub fn health_within(
        &self,
        timeout: Duration,
    ) -> Result<Health, ErrorResponse> {
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        let response = self
            .request(&agent, "GET", "/v1/health")
            .call()
            .map_err(|err| self.unreachable(err))?;
        let mut body = String::new();
        response
            .into_reader()
            .take(1 << 20)
            .read_to_string(&mut body)
            .map_err(|err| self.unreachable(err))?;
        serde_json::from_str::<Health>(&body).map_err(|err| {
            error(
                ErrorKind::Io,
                format!("unexpected health reply from {}: {err}", self.url),
                Some("is this URL a context server?".into()),
            )
        })
    }

    /// Open a WebSocket and subscribe to `topics` (all if empty) of `graph`.
    pub fn subscribe(
        &self,
        graph: &str,
        topics: &[Topic],
    ) -> Result<EventStream, ErrorResponse> {
        let mut stream =
            EventStream::connect(&self.url, self.token.as_deref())?;
        stream.subscribe(graph, topics)?;
        Ok(stream)
    }

    /// Subscribe and call `on_event` for every event on a background
    /// thread until the returned watcher is dropped. The subscription is
    /// active when this returns.
    pub fn watch(
        &self,
        graph: &str,
        topics: &[Topic],
        mut on_event: impl FnMut(Event) + Send + 'static,
    ) -> Result<EventWatcher, ErrorResponse> {
        let mut stream = self.subscribe(graph, topics)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                match stream.recv_timeout(Some(WATCH_POLL)) {
                    Ok(Some(event)) => on_event(event),
                    Ok(None) => {},
                    Err(_) => break,
                }
            }
            stream.close();
        });
        Ok(EventWatcher {
            stop,
            thread: Some(thread),
        })
    }
}

impl Backend for RemoteBackend {
    fn execute(
        &self,
        request: Request,
    ) -> Response {
        let id = request.id;
        let body = match serde_json::to_string(&request) {
            Ok(body) => body,
            Err(err) =>
                return error_reply(
                    id,
                    error(
                        ErrorKind::Internal,
                        format!("cannot encode request: {err}"),
                        None,
                    ),
                ),
        };
        let result = self
            .request(&self.agent, "POST", "/v1/execute")
            .set("Content-Type", "application/json")
            .send_string(&body);
        self.reply(id, result)
    }
}

/// The live server holding `graph` in `store`: its discovery file names a
/// running process and `/v1/health` answers within [`DISCOVERY_TIMEOUT`].
pub fn discover(
    store: &GraphStore,
    graph: &str,
) -> Option<ServerInfo> {
    let location = store.resolve(graph).ok()?;
    let info = store.server(&location)?;
    RemoteBackend::new(info.url.clone(), None)
        .health_within(DISCOVERY_TIMEOUT)
        .ok()?;
    Some(info)
}

/// A WebSocket connection receiving events (`GET /v1/ws`).
pub struct EventStream {
    socket: WebSocket<TcpStream>,
    url: String,
    next_id: u64,
    pending: VecDeque<Event>,
}

impl EventStream {
    /// Connect to the server at `url` (`http://HOST:PORT[/prefix]`).
    pub fn connect(
        url: &str,
        token: Option<&str>,
    ) -> Result<Self, ErrorResponse> {
        let base = url.trim_end_matches('/');
        let unreachable = |err: &dyn std::fmt::Display| {
            error(
                ErrorKind::Io,
                format!("cannot open a WebSocket to {base}: {err}"),
                None,
            )
        };
        let rest = base.strip_prefix("http://").ok_or_else(|| {
            error(
                ErrorKind::Unsupported,
                format!(
                    "unsupported server URL '{base}': only http:// is supported"
                ),
                None,
            )
        })?;
        let (authority, prefix) =
            rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let host_port = if authority
            .rsplit_once(':')
            .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
        {
            authority.to_string()
        } else {
            format!("{authority}:80")
        };
        let addr = host_port
            .to_socket_addrs()
            .map_err(|err| unreachable(&err))?
            .next()
            .ok_or_else(|| unreachable(&"address resolves to nothing"))?;
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
            .map_err(|err| unreachable(&err))?;
        stream
            .set_read_timeout(Some(CONNECT_TIMEOUT))
            .map_err(|err| unreachable(&err))?;
        let mut request = format!("ws://{authority}{prefix}/v1/ws")
            .into_client_request()
            .map_err(|err| unreachable(&err))?;
        if let Some(token) = token.filter(|token| !token.is_empty()) {
            let value = format!("Bearer {token}").parse().map_err(|_| {
                error(ErrorKind::InvalidInput, "invalid token", None)
            })?;
            request.headers_mut().insert("Authorization", value);
        }
        let (socket, _) =
            tungstenite::client(request, stream).map_err(|err| match err {
                tungstenite::HandshakeError::Failure(
                    tungstenite::Error::Http(response),
                ) if response.status() == 401 => error(
                    ErrorKind::Forbidden,
                    format!("{base}: missing or invalid bearer token"),
                    Some("pass --token or set CONTEXT_TOKEN".into()),
                ),
                err => unreachable(&err),
            })?;
        Ok(Self {
            socket,
            url: base.to_string(),
            next_id: 1,
            pending: VecDeque::new(),
        })
    }

    fn closed(
        &self,
        err: impl std::fmt::Display,
    ) -> ErrorResponse {
        error(
            ErrorKind::Io,
            format!("WebSocket to {} failed: {err}", self.url),
            None,
        )
    }

    /// Send a control message and wait for its reply; events received in
    /// between are queued.
    fn call(
        &mut self,
        mut message: Value,
    ) -> Result<Value, ErrorResponse> {
        let id = self.next_id;
        self.next_id += 1;
        message["id"] = json!(id);
        self.socket
            .send(Message::text(message.to_string()))
            .map_err(|err| self.closed(err))?;
        self.socket
            .get_ref()
            .set_read_timeout(Some(CONNECT_TIMEOUT))
            .map_err(|err| self.closed(err))?;
        loop {
            let value = match self.read_value()? {
                Some(value) => value,
                None => return Err(self.closed("no reply")),
            };
            if value.get("event").is_some() {
                if let Ok(event) = serde_json::from_value(value) {
                    self.pending.push_back(event);
                }
                continue;
            }
            if value.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(serde_json::from_value(error.clone())
                    .unwrap_or_else(|_| {
                        self.closed(format!("error reply {error}"))
                    }));
            }
            return Ok(value.get("ok").cloned().unwrap_or(Value::Null));
        }
    }

    /// Receive `topics` (all if empty) of `graph` from now on.
    pub fn subscribe(
        &mut self,
        graph: &str,
        topics: &[Topic],
    ) -> Result<(), ErrorResponse> {
        self.call(json!({ "subscribe": { "graph": graph, "topics": topics } }))
            .map(drop)
    }

    /// Stop receiving events of `graph`.
    pub fn unsubscribe(
        &mut self,
        graph: &str,
    ) -> Result<(), ErrorResponse> {
        self.call(json!({ "unsubscribe": { "graph": graph } }))
            .map(drop)
    }

    /// The next JSON message; `Ok(None)` on a read timeout.
    fn read_value(&mut self) -> Result<Option<Value>, ErrorResponse> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) =>
                    return serde_json::from_str(text.as_str())
                        .map(Some)
                        .map_err(|err| self.closed(err)),
                Ok(Message::Close(_)) =>
                    return Err(self.closed("closed by the server")),
                Ok(_) => {},
                Err(tungstenite::Error::Io(err))
                    if matches!(
                        err.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                    return Ok(None),
                Err(err) => return Err(self.closed(err)),
            }
        }
    }

    /// The next event, waiting at most `timeout` (`None`: forever).
    /// `Ok(None)` on timeout; an error once the connection is closed.
    pub fn recv_timeout(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<Event>, ErrorResponse> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }
        self.socket
            .get_ref()
            .set_read_timeout(timeout.map(|t| t.max(Duration::from_millis(1))))
            .map_err(|err| self.closed(err))?;
        loop {
            let Some(value) = self.read_value()? else {
                return Ok(None);
            };
            if value.get("event").is_some()
                && let Ok(event) = serde_json::from_value(value)
            {
                return Ok(Some(event));
            }
        }
    }

    /// Close the connection.
    pub fn close(mut self) {
        let _ = self.socket.close(None);
        let _ = self.socket.flush();
    }
}

impl Iterator for EventStream {
    type Item = Event;

    /// Blocks until the next event; ends when the connection closes.
    fn next(&mut self) -> Option<Event> {
        loop {
            match self.recv_timeout(None) {
                Ok(Some(event)) => return Some(event),
                Ok(None) => {},
                Err(_) => return None,
            }
        }
    }
}

/// A background subscription started by [`RemoteBackend::watch`]; stops
/// (and closes its WebSocket) when dropped.
pub struct EventWatcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for EventWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_segments() {
        assert_eq!(encode_segment("corpus-1.v2_x"), "corpus-1.v2_x");
        assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
    }

    #[test]
    fn unreachable_server_is_an_io_error_naming_the_url() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let url = format!("http://127.0.0.1:{port}");
        let backend = RemoteBackend::new(format!("{url}/"), None);
        assert_eq!(backend.url(), url);
        let response = backend.execute(Request::new(crate::Command::List));
        let Outcome::Error(error) = response.outcome else {
            panic!("expected an error")
        };
        assert_eq!(error.kind, ErrorKind::Io);
        assert!(error.message.contains(&url), "{}", error.message);
    }
}
