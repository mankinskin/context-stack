//! `context-mcp` — MCP server for context-engine graphs.
//!
//! Exposes the `context-api` command surface to agents as typed MCP tools
//! (`context_search`, `context_read`, …) plus a raw `context_execute`
//! escape hatch. Every tool returns the `context_api::Response` JSON
//! unchanged (compact), subject to an output budget.
//!
//! ```text
//! MCP client ──stdio──► ContextServer (rmcp tools)
//!                           │ spawn_blocking
//!                           ▼
//!                  Box<dyn Backend + Send>   ← local Session (stage 5)
//!                                            ← RemoteBackend (proxy, stage 4)
//! ```
//!
//! stdout is reserved for MCP JSON-RPC; log to stderr only.
//!
//! Plan: `context-api/agents/plans/20261126_PLAN_CONTEXT_TOOLS/05_MCP.md`.

mod budget;
mod params;
mod server;

use std::path::PathBuf;

use context_api::{
    SaveMode,
    Session,
    SessionConfig,
};
use rmcp::{
    ServiceExt,
    transport::stdio,
};

pub use self::{
    params::*,
    server::ContextServer,
};

/// Default output budget of one tool result in bytes.
pub const DEFAULT_MAX_OUTPUT: usize = 32 * 1024;
/// Default hard vertex cap of `context_render`.
pub const DEFAULT_RENDER_CAP: usize = 200;

/// Configuration of the MCP server.
#[derive(Debug, Clone)]
pub struct McpConfig {
    /// Directory of `<name>.ctx` graphs.
    pub store_dir: PathBuf,
    /// `files` inputs are allowed below this directory (relative paths are
    /// resolved against it); `None` refuses file inputs.
    pub allow_fs: Option<PathBuf>,
    /// Maximum size of one tool result in bytes; larger results are
    /// truncated item-wise.
    pub max_output: usize,
    /// Register `context_delete_graph` and allow destructive overwrites.
    pub allow_delete: bool,
    /// Hard vertex cap of rendering (`None`: the API default gate only).
    pub render_cap: Option<usize>,
}

impl McpConfig {
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
            allow_fs: None,
            max_output: DEFAULT_MAX_OUTPUT,
            allow_delete: false,
            render_cap: Some(DEFAULT_RENDER_CAP),
        }
    }

    /// The configuration of the local [`Session`]: graphs stay open (and
    /// locked) between tool calls and every write is saved immediately.
    pub fn session_config(&self) -> SessionConfig {
        let mut config = SessionConfig::new(self.store_dir.clone());
        config.keep_open = true;
        config.save = SaveMode::OnWrite;
        config.allow_fs = self.allow_fs.clone();
        config.render_cap = self.render_cap;
        config
    }

    /// A local session over the configured store.
    pub fn local_session(&self) -> Session {
        Session::new(self.session_config())
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self::new(".context-engine")
    }
}

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Serve the tools over stdio (local mode) until the client disconnects.
pub async fn run_stdio(config: McpConfig) -> Result<(), BoxError> {
    tracing::info!(store = %config.store_dir.display(), "context-mcp serving on stdio");
    serve_stdio(ContextServer::new(config)).await
}

/// Serve a configured server (e.g. [`ContextServer::with_backend`] over a
/// remote backend: proxy mode) over stdio until the client disconnects.
pub async fn serve_stdio(server: ContextServer) -> Result<(), BoxError> {
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// A byte size for `--max-output`: `32768`, `64KiB`, `1MiB` (K/KB/KiB and
/// M/MB/MiB are binary).
pub fn parse_size(text: &str) -> Result<usize, String> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: usize = number
        .parse()
        .map_err(|_| format!("invalid size '{text}'"))?;
    let factor = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        _ =>
            return Err(format!(
                "invalid size unit in '{text}' (use B, KiB or MiB)"
            )),
    };
    number
        .checked_mul(factor)
        .ok_or_else(|| format!("size '{text}' is too large"))
}
