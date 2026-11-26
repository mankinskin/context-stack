//! context-http — HTTP and WebSocket server of the context API.
//!
//! The server keeps graphs open in one [`context_api::Session`] and executes
//! the same [`context_api::Request`]s as the CLI:
//!
//! - `POST /v1/execute` — any request, body and reply are the stage-1
//!   envelope (`{id, command, ...}` → `{id, ok}` / `{id, error}`);
//! - `GET /v1/ws` — WebSocket: pipelined requests and event subscriptions;
//! - REST convenience routes under `/v1/graphs/...`.
//!
//! [`serve`] is the entry point of the `context-http` binary and of
//! `context-cli serve` (via [`ServeArgs`]).
//!
//! Plan: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/03_SERVER.md`.

mod args;
mod routes;
mod state;
mod ws;

use std::{
    future::Future,
    io::Write,
    net::SocketAddr,
    path::PathBuf,
    time::Duration,
};

use context_api::{
    ErrorResponse,
    SaveMode,
    SessionConfig,
};
use tokio::net::TcpListener;

pub use self::{
    args::{
        SaveArg,
        ServeArgs,
    },
    routes::{
        router,
        status_for,
    },
    state::AppState,
};

/// Default listen address.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7878";
/// Default request body limit (64 MiB).
pub const DEFAULT_MAX_BODY: usize = 64 * 1024 * 1024;
/// Default hard vertex cap of render/export.
pub const DEFAULT_RENDER_CAP: usize = 2000;
/// Default upper bound of a search's `max_visited` work budget.
pub const DEFAULT_MAX_VISITED_CAP: usize = 1_000_000;
/// Capacity of the event broadcast channel.
pub const EVENT_CAPACITY: usize = 1024;

/// Configuration of a server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Directory of `<name>.ctx` graphs.
    pub store_dir: PathBuf,
    /// Listen address; port 0 binds an ephemeral port.
    pub addr: SocketAddr,
    /// Graphs opened at start.
    pub preload: Vec<String>,
    /// When mutating commands save (`Manual` with `save_interval` for
    /// periodic saving).
    pub save: SaveMode,
    /// Save dirty open graphs periodically.
    pub save_interval: Option<Duration>,
    /// Bearer token; required when `addr` is not loopback.
    pub token: Option<String>,
    /// `Input::File` paths are allowed below this directory.
    pub allow_fs: Option<PathBuf>,
    /// Request body limit in bytes (also the WebSocket message limit).
    pub max_body: usize,
    /// Hard vertex cap of render/export that `force` cannot lift.
    pub render_cap: Option<usize>,
    /// Upper bound of `Limits::max_visited`.
    pub max_visited_cap: Option<usize>,
    /// Allowed CORS origins (none: no CORS headers).
    pub cors_origins: Vec<String>,
    /// Frontend served for paths outside `/v1` (SPA fallback to
    /// `index.html`).
    pub static_dir: Option<PathBuf>,
}

impl ServerConfig {
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
            addr: DEFAULT_ADDR.parse().expect("valid default address"),
            preload: Vec::new(),
            save: SaveMode::OnWrite,
            save_interval: None,
            token: None,
            allow_fs: None,
            max_body: DEFAULT_MAX_BODY,
            render_cap: Some(DEFAULT_RENDER_CAP),
            max_visited_cap: Some(DEFAULT_MAX_VISITED_CAP),
            cors_origins: Vec::new(),
            static_dir: None,
        }
    }

    /// Refuse configurations that would expose graphs without a token.
    pub fn validate(&self) -> Result<(), Error> {
        if !self.addr.ip().is_loopback() && self.token.is_none() {
            return Err(Error::Config(format!(
                "refusing to listen on non-loopback address {} without a token; pass --token or set CONTEXT_TOKEN",
                self.addr
            )));
        }
        if self.token.as_deref() == Some("") {
            return Err(Error::Config("the token must not be empty".into()));
        }
        if self
            .save_interval
            .is_some_and(|interval| interval.is_zero())
        {
            return Err(Error::Config(
                "the save interval must be positive".into(),
            ));
        }
        for origin in &self.cors_origins {
            axum::http::HeaderValue::from_str(origin).map_err(|_| {
                Error::Config(format!("invalid CORS origin '{origin}'"))
            })?;
        }
        Ok(())
    }

    /// The session configuration of the server: graphs stay open.
    pub fn session_config(&self) -> SessionConfig {
        let mut config = SessionConfig::new(&self.store_dir);
        config.keep_open = true;
        config.save = self.save;
        config.allow_fs = self.allow_fs.clone();
        config.render_cap = self.render_cap;
        config.max_visited_cap = self.max_visited_cap;
        config
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new(".context-engine")
    }
}

/// Errors of starting or stopping a server.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot open graph '{graph}': {error}")]
    Preload { graph: String, error: ErrorResponse },
    #[error("saving graphs on shutdown failed: {0}")]
    Shutdown(ErrorResponse),
}

/// A bound server, not yet serving.
pub struct Server {
    listener: TcpListener,
    state: AppState,
}

impl Server {
    /// Validate the configuration, bind the address and preload graphs.
    pub async fn bind(config: ServerConfig) -> Result<Self, Error> {
        let state = AppState::new(config)?;
        let listener = TcpListener::bind(state.config().addr).await?;
        let addr = listener.local_addr()?;
        state.set_url(format!("http://{addr}"));
        if let Err(err) = state.preload().await {
            let _ = state.shutdown().await;
            return Err(err);
        }
        Ok(Self { listener, state })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .expect("bound listener has an address")
    }

    /// `http://ADDR` of the bound listener.
    pub fn url(&self) -> String {
        format!("http://{}", self.local_addr())
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Serve until `shutdown` completes, then save dirty graphs, release
    /// them and remove the discovery files.
    pub async fn run(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Error> {
        let Self { listener, state } = self;
        let autosave = state.spawn_autosave();
        let notify = state.clone();
        let served = axum::serve(listener, router(state.clone()))
            .with_graceful_shutdown(async move {
                shutdown.await;
                notify.notify_shutdown();
            })
            .await;
        if let Some(task) = autosave {
            task.abort();
        }
        let closed = state.shutdown().await;
        served?;
        closed
    }
}

/// Run a server: bind, print `{"listening":"http://ADDR","pid":PID}` on
/// stdout, serve until `shutdown`, save and release all graphs.
pub async fn serve(
    config: ServerConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Error> {
    let server = Server::bind(config).await?;
    let line = serde_json::json!({
        "listening": server.url(),
        "pid": std::process::id(),
    });
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{line}")?;
    stdout.flush()?;
    drop(stdout);
    tracing::info!(url = %server.url(), "listening");
    server.run(shutdown).await
}
